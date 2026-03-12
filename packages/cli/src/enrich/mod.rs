//! Enrichment pipeline: auto-enrich bookmarks via configured agent provider.
//!
//! This module handles request construction, provider invocation, and
//! persistence of enrichment results (summary, suggested_tags, collection)
//! back to the bundle, database, and event log.
//!
//! Designed for reuse by both `commands/save.rs` and future `commands/reprocess.rs`.

use crate::agent::{AgentError, AgentProvider, EnrichmentRequest, EnrichmentResponse};
use crate::bundle::{BodySections, Bundle};
use crate::config::Config;
use crate::db::BookmarkRepository;
use crate::models::{Bookmark, BookmarkEvent, EventType, SummaryStatus};

// ── Provider factory type ────────────────────────────────────────────

/// Factory function signature for creating agent providers.
/// Production uses `agent::create_provider`; tests inject fakes.
pub(crate) type ProviderFactory =
    dyn Fn(&str, Option<&str>) -> Result<Box<dyn AgentProvider>, AgentError>;

// ── Enrichment outcome ───────────────────────────────────────────────

/// Result of an enrichment attempt.
#[derive(Debug)]
pub(crate) enum EnrichOutcome {
    /// Provider returned a valid response; fields have been persisted.
    Success,
    /// Enrichment was skipped (disabled, no-enrich flag, blank article, etc.).
    #[allow(dead_code)]
    Skipped { reason: String },
    /// Provider or persistence failed; save is still successful.
    Failed { warning: String },
}

// ── Public enrichment entry point ────────────────────────────────────

/// Run enrichment on a saved bookmark.
///
/// This is the single entry point for both new saves and content-changed resaves.
/// The caller is responsible for gating on `--no-enrich` and unchanged duplicates
/// before calling this function.
///
/// On success, mutates `bookmark` in place and persists to bundle, DB, and events.
/// On failure, marks `summary_status = Failed` and returns a warning string.
/// Never returns an error — enrichment failures degrade to warnings.
pub(crate) fn enrich_bookmark(
    bookmark: &mut Bookmark,
    article_markdown: &str,
    bundle: &Bundle,
    repo: &BookmarkRepository<'_>,
    config: &Config,
    provider_factory: &ProviderFactory,
) -> EnrichOutcome {
    // Gate: enrichment disabled in config
    if !config.enrichment.enabled {
        return EnrichOutcome::Skipped {
            reason: "enrichment disabled in config".to_string(),
        };
    }

    // Gate: blank article content — no point sending to provider
    if article_markdown.trim().is_empty() {
        return handle_failure(bookmark, bundle, repo, "no article content to enrich");
    }

    // Create provider
    let provider = match provider_factory(&config.default_agent, config.system_prompt.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            return handle_failure(
                bookmark,
                bundle,
                repo,
                &format!("failed to create agent provider: {e}"),
            );
        }
    };

    // Build request
    let request = build_request(bookmark, article_markdown);

    // Call provider
    let response = match provider.enrich(&request) {
        Ok(r) => r,
        Err(e) => {
            let detail = sanitize_error(&e);
            return handle_failure(bookmark, bundle, repo, &detail);
        }
    };

    // Apply success
    apply_success(bookmark, response, article_markdown, bundle, repo)
}

// ── Request construction ─────────────────────────────────────────────

fn build_request(bookmark: &Bookmark, article_markdown: &str) -> EnrichmentRequest {
    EnrichmentRequest {
        url: bookmark.url.clone(),
        title: bookmark.title.clone(),
        article_content: article_markdown.to_string(),
        user_note: bookmark.note.clone(),
        existing_tags: bookmark.user_tags.clone(),
    }
}

// ── Success path ─────────────────────────────────────────────────────

fn apply_success(
    bookmark: &mut Bookmark,
    response: EnrichmentResponse,
    article_markdown: &str,
    bundle: &Bundle,
    repo: &BookmarkRepository<'_>,
) -> EnrichOutcome {
    // [R-003] Reject blank provider summary as a failure
    if response.summary.trim().is_empty() {
        return handle_failure(bookmark, bundle, repo, "provider returned blank summary");
    }

    // Update bookmark fields
    bookmark.suggested_tags = response.suggested_tags.clone();
    bookmark.summary_status = SummaryStatus::Done;

    // Apply suggested collection only if bookmark has none
    if bookmark.collections.is_empty() {
        if let Some(ref collection) = response.suggested_collection {
            bookmark.collections = vec![collection.clone()];
        }
    }

    // Build body sections with summary
    let sections = BodySections {
        summary: Some(response.summary.clone()),
        ..Default::default()
    };

    // Persist to bundle (bookmark.md with enriched summary)
    if let Err(e) = bundle.update_bookmark_md(bookmark, &sections) {
        let detail = format!("enrichment succeeded but failed to update bookmark.md: {e}");
        // [R-001] Log failure event when post-success persistence fails
        log_failure_event(bundle, &detail);
        bookmark.summary_status = SummaryStatus::Failed;
        return EnrichOutcome::Failed { warning: detail };
    }

    // Persist to DB: bookmark fields + summary in one update
    match repo.update_enrichment(&bookmark.id, bookmark, &response.summary) {
        Ok(true) => {}
        Ok(false) => {
            let detail = "enrichment succeeded but bookmark row not found in DB".to_string();
            log_failure_event(bundle, &detail);
            bookmark.summary_status = SummaryStatus::Failed;
            return EnrichOutcome::Failed { warning: detail };
        }
        Err(e) => {
            let detail = format!("enrichment succeeded but DB update failed: {e}");
            log_failure_event(bundle, &detail);
            bookmark.summary_status = SummaryStatus::Failed;
            return EnrichOutcome::Failed { warning: detail };
        }
    }

    // Append enriched event
    let event = BookmarkEvent::new(
        EventType::Enriched,
        serde_json::json!({
            "status": "success",
            "provider": "agent",
            "suggested_tags": response.suggested_tags,
            "suggested_collection": response.suggested_collection,
            "summary_length": response.summary.len(),
            "article_length": article_markdown.len(),
        }),
    );
    if let Err(e) = bundle.append_event(&event) {
        // Event logging failure is not fatal — enrichment data is already persisted
        return EnrichOutcome::Failed {
            warning: format!("enrichment succeeded but failed to log event: {e}"),
        };
    }

    EnrichOutcome::Success
}

/// Best-effort append of a failure event to the bundle's events.jsonl.
fn log_failure_event(bundle: &Bundle, detail: &str) {
    let truncated = truncate_utf8(detail, 500);
    let event = BookmarkEvent::new(
        EventType::Enriched,
        serde_json::json!({
            "status": "failed",
            "error": truncated,
        }),
    );
    let _ = bundle.append_event(&event);
}

// ── Failure path ─────────────────────────────────────────────────────

fn handle_failure(
    bookmark: &mut Bookmark,
    bundle: &Bundle,
    repo: &BookmarkRepository<'_>,
    detail: &str,
) -> EnrichOutcome {
    bookmark.summary_status = SummaryStatus::Failed;

    // Best-effort: update bookmark.md with failed status (no summary content)
    let sections = BodySections::default();
    let _ = bundle.update_bookmark_md(bookmark, &sections);

    // Best-effort: update DB with failed status
    let _ = repo.update(bookmark);

    // Best-effort: log failure event
    let truncated_detail = truncate_utf8(detail, 500);
    let event = BookmarkEvent::new(
        EventType::Enriched,
        serde_json::json!({
            "status": "failed",
            "error": truncated_detail,
        }),
    );
    let _ = bundle.append_event(&event);

    EnrichOutcome::Failed {
        warning: format!("enrichment failed: {detail}"),
    }
}

// ── UTF-8-safe truncation ────────────────────────────────────────────

/// Truncate a string to at most `max_chars` characters, appending "..." if truncated.
/// Unlike byte slicing, this never panics on multibyte UTF-8.
fn truncate_utf8(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{truncated}...")
    }
}

// ── Error sanitization ───────────────────────────────────────────────

/// Summarize an agent error without leaking sensitive content.
fn sanitize_error(err: &AgentError) -> String {
    match err {
        AgentError::InvalidAgent { value } => {
            format!("invalid agent: {value}")
        }
        AgentError::Spawn { provider, .. } => {
            format!("failed to spawn {provider} CLI")
        }
        AgentError::ProcessFailed {
            provider, status, ..
        } => {
            format!("{provider} CLI exited with status {status}")
        }
        AgentError::InvalidResponse { provider, reason } => {
            format!("{provider} CLI produced invalid output: {reason}")
        }
        AgentError::TempFileWrite { provider, .. } => {
            format!("failed to write temp file for {provider}")
        }
        AgentError::TempFileRead { provider, .. } => {
            format!("failed to read output file for {provider}")
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::ContentStatus;
    use tempfile::TempDir;

    // ── Test helpers ─────────────────────────────────────────────────

    fn test_config(storage_path: &std::path::Path) -> Config {
        Config {
            default_agent: "claude".to_string(),
            storage_path: storage_path.to_path_buf(),
            system_prompt: None,
            enrichment: crate::config::EnrichmentConfig { enabled: true },
        }
    }

    fn test_bookmark() -> Bookmark {
        use chrono::{TimeZone, Utc};
        let mut bm = Bookmark::new("https://example.com/article", "Test Article");
        bm.id = "am_01TEST123456".to_string();
        bm.saved_at = Utc.with_ymd_and_hms(2026, 3, 5, 14, 30, 0).unwrap();
        bm.content_status = ContentStatus::Extracted;
        bm.content_hash = Some("sha256:abc".to_string());
        bm
    }

    fn success_factory(
        _agent: &str,
        _system_prompt: Option<&str>,
    ) -> Result<Box<dyn AgentProvider>, AgentError> {
        Ok(Box::new(FakeProvider {
            response: Ok(EnrichmentResponse {
                summary: "A great article about testing.".to_string(),
                suggested_tags: vec!["testing".to_string(), "rust".to_string()],
                suggested_collection: Some("dev".to_string()),
            }),
        }))
    }

    fn failure_factory(
        _agent: &str,
        _system_prompt: Option<&str>,
    ) -> Result<Box<dyn AgentProvider>, AgentError> {
        Ok(Box::new(FakeProvider {
            response: Err(AgentError::ProcessFailed {
                provider: "claude",
                status: 1,
                stderr: "something went wrong".to_string(),
            }),
        }))
    }

    fn invalid_agent_factory(
        agent: &str,
        _system_prompt: Option<&str>,
    ) -> Result<Box<dyn AgentProvider>, AgentError> {
        Err(AgentError::InvalidAgent {
            value: agent.to_string(),
        })
    }

    struct FakeProvider {
        response: Result<EnrichmentResponse, AgentError>,
    }

    impl AgentProvider for FakeProvider {
        fn enrich(&self, _request: &EnrichmentRequest) -> Result<EnrichmentResponse, AgentError> {
            match &self.response {
                Ok(r) => Ok(r.clone()),
                Err(e) => Err(match e {
                    AgentError::ProcessFailed {
                        provider,
                        status,
                        stderr,
                    } => AgentError::ProcessFailed {
                        provider,
                        status: *status,
                        stderr: stderr.clone(),
                    },
                    AgentError::InvalidResponse { provider, reason } => {
                        AgentError::InvalidResponse {
                            provider,
                            reason: reason.clone(),
                        }
                    }
                    _ => AgentError::InvalidAgent {
                        value: "test".to_string(),
                    },
                }),
            }
        }
    }

    /// Create a real bundle + DB setup for integration testing.
    fn setup_bundle_and_db(
        tmp: &TempDir,
    ) -> (
        Bookmark,
        Bundle,
        rusqlite::Connection,
        std::path::PathBuf,
        String,
    ) {
        let storage = tmp.path().join("bookmarks");
        std::fs::create_dir_all(&storage).unwrap();

        let bm = test_bookmark();
        let metadata = crate::fetch::PageMetadata {
            title: Some("Test Article".to_string()),
            ..Default::default()
        };

        let bundle = Bundle::create(
            &storage,
            &bm,
            &metadata,
            "# Article\n\nSome content here.",
            "<html><body>test</body></html>",
            "cli",
        )
        .unwrap();

        let conn = db::open_memory().unwrap();
        let repo = BookmarkRepository::new(&conn);
        repo.insert(&bm).unwrap();

        let article = "# Article\n\nSome content here.".to_string();
        (bm, bundle, conn, storage, article)
    }

    // ── Request construction tests ───────────────────────────────────

    #[test]
    fn build_request_populates_all_fields() {
        let mut bm = test_bookmark();
        bm.note = Some("check this".to_string());
        bm.user_tags = vec!["rust".to_string()];

        let req = build_request(&bm, "article content");
        assert_eq!(req.url, "https://example.com/article");
        assert_eq!(req.title, "Test Article");
        assert_eq!(req.article_content, "article content");
        assert_eq!(req.user_note, Some("check this".to_string()));
        assert_eq!(req.existing_tags, vec!["rust"]);
    }

    #[test]
    fn build_request_handles_no_note_no_tags() {
        let bm = test_bookmark();
        let req = build_request(&bm, "content");
        assert_eq!(req.user_note, None);
        assert!(req.existing_tags.is_empty());
    }

    // ── Skip path tests ──────────────────────────────────────────────

    #[test]
    fn enrichment_disabled_in_config_skips() {
        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, article) = setup_bundle_and_db(&tmp);
        let repo = BookmarkRepository::new(&conn);
        let mut config = test_config(&storage);
        config.enrichment.enabled = false;

        let outcome = enrich_bookmark(&mut bm, &article, &bundle, &repo, &config, &success_factory);

        assert!(matches!(outcome, EnrichOutcome::Skipped { .. }));
        assert_eq!(bm.summary_status, SummaryStatus::Pending);
    }

    #[test]
    fn blank_article_fails_enrichment() {
        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, _article) = setup_bundle_and_db(&tmp);
        let repo = BookmarkRepository::new(&conn);
        let config = test_config(&storage);

        let outcome = enrich_bookmark(&mut bm, "   ", &bundle, &repo, &config, &success_factory);

        assert!(matches!(outcome, EnrichOutcome::Failed { .. }));
        assert_eq!(bm.summary_status, SummaryStatus::Failed);
    }

    // ── Success path tests ───────────────────────────────────────────

    #[test]
    fn successful_enrichment_updates_bookmark_fields() {
        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, article) = setup_bundle_and_db(&tmp);
        let repo = BookmarkRepository::new(&conn);
        let config = test_config(&storage);

        let outcome = enrich_bookmark(&mut bm, &article, &bundle, &repo, &config, &success_factory);

        assert!(matches!(outcome, EnrichOutcome::Success));
        assert_eq!(bm.summary_status, SummaryStatus::Done);
        assert_eq!(bm.suggested_tags, vec!["testing", "rust"]);
        assert_eq!(bm.collections, vec!["dev"]);
    }

    #[test]
    fn successful_enrichment_writes_summary_to_bookmark_md() {
        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, article) = setup_bundle_and_db(&tmp);
        let repo = BookmarkRepository::new(&conn);
        let config = test_config(&storage);

        enrich_bookmark(&mut bm, &article, &bundle, &repo, &config, &success_factory);

        let content = std::fs::read_to_string(bundle.path().join("bookmark.md")).unwrap();
        assert!(content.contains("A great article about testing."));
        // Summary section should have real content, not placeholder
        let summary_start = content.find("# Summary").unwrap();
        let next_heading = content[summary_start + 1..].find("\n# ").unwrap() + summary_start + 1;
        let summary_section = &content[summary_start..next_heading];
        assert!(!summary_section.contains("[pending enrichment]"));
    }

    #[test]
    fn successful_enrichment_updates_db_summary() {
        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, article) = setup_bundle_and_db(&tmp);
        let repo = BookmarkRepository::new(&conn);
        let config = test_config(&storage);

        enrich_bookmark(&mut bm, &article, &bundle, &repo, &config, &success_factory);

        // Verify DB fields
        let loaded = repo.get_by_id("am_01TEST123456").unwrap().unwrap();
        assert_eq!(loaded.summary_status, SummaryStatus::Done);
        assert_eq!(loaded.suggested_tags, vec!["testing", "rust"]);

        // Verify summary is searchable via FTS
        let results = repo
            .search("great article about testing", 10, None)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "am_01TEST123456");
    }

    #[test]
    fn successful_enrichment_logs_event() {
        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, article) = setup_bundle_and_db(&tmp);
        let repo = BookmarkRepository::new(&conn);
        let config = test_config(&storage);

        enrich_bookmark(&mut bm, &article, &bundle, &repo, &config, &success_factory);

        let events_content = std::fs::read_to_string(bundle.path().join("events.jsonl")).unwrap();
        let lines: Vec<&str> = events_content.lines().collect();
        assert_eq!(lines.len(), 2); // saved + enriched
        let last_event = BookmarkEvent::from_json_line(lines[1]).unwrap();
        assert_eq!(last_event.event_type, EventType::Enriched);
        assert_eq!(last_event.details["status"], "success");
    }

    #[test]
    fn successful_enrichment_preserves_existing_collection() {
        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, article) = setup_bundle_and_db(&tmp);
        bm.collections = vec!["existing".to_string()];
        // Update DB with the collection
        let repo = BookmarkRepository::new(&conn);
        repo.update(&bm).unwrap();

        let config = test_config(&storage);

        enrich_bookmark(&mut bm, &article, &bundle, &repo, &config, &success_factory);

        // Provider suggests "dev" but existing "existing" should be preserved
        assert_eq!(bm.collections, vec!["existing"]);
    }

    // ── Failure path tests ───────────────────────────────────────────

    #[test]
    fn provider_failure_marks_status_failed() {
        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, article) = setup_bundle_and_db(&tmp);
        let repo = BookmarkRepository::new(&conn);
        let config = test_config(&storage);

        let outcome = enrich_bookmark(&mut bm, &article, &bundle, &repo, &config, &failure_factory);

        assert!(matches!(outcome, EnrichOutcome::Failed { .. }));
        assert_eq!(bm.summary_status, SummaryStatus::Failed);

        // DB should reflect failed status
        let loaded = repo.get_by_id("am_01TEST123456").unwrap().unwrap();
        assert_eq!(loaded.summary_status, SummaryStatus::Failed);
    }

    #[test]
    fn provider_failure_logs_failure_event() {
        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, article) = setup_bundle_and_db(&tmp);
        let repo = BookmarkRepository::new(&conn);
        let config = test_config(&storage);

        enrich_bookmark(&mut bm, &article, &bundle, &repo, &config, &failure_factory);

        let events_content = std::fs::read_to_string(bundle.path().join("events.jsonl")).unwrap();
        let lines: Vec<&str> = events_content.lines().collect();
        assert!(lines.len() >= 2); // saved + enriched(failed)
        let last_event = BookmarkEvent::from_json_line(lines.last().unwrap()).unwrap();
        assert_eq!(last_event.event_type, EventType::Enriched);
        assert_eq!(last_event.details["status"], "failed");
    }

    #[test]
    fn invalid_agent_factory_fails_enrichment() {
        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, article) = setup_bundle_and_db(&tmp);
        let repo = BookmarkRepository::new(&conn);
        let config = test_config(&storage);

        let outcome = enrich_bookmark(
            &mut bm,
            &article,
            &bundle,
            &repo,
            &config,
            &invalid_agent_factory,
        );

        assert!(matches!(outcome, EnrichOutcome::Failed { .. }));
        if let EnrichOutcome::Failed { warning } = outcome {
            assert!(warning.contains("failed to create agent provider"));
        }
    }

    // ── System prompt forwarding test ────────────────────────────────

    #[test]
    fn system_prompt_passed_to_provider_factory() {
        use std::sync::{Arc, Mutex};

        let captured_prompt: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let captured_clone = captured_prompt.clone();

        let factory = move |_agent: &str,
                            system_prompt: Option<&str>|
              -> Result<Box<dyn AgentProvider>, AgentError> {
            *captured_clone.lock().unwrap() = Some(system_prompt.map(|s| s.to_string()));
            Ok(Box::new(FakeProvider {
                response: Ok(EnrichmentResponse {
                    summary: "test summary".to_string(),
                    suggested_tags: vec![],
                    suggested_collection: None,
                }),
            }))
        };

        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, article) = setup_bundle_and_db(&tmp);
        let repo = BookmarkRepository::new(&conn);
        let mut config = test_config(&storage);
        config.system_prompt = Some("Be helpful and concise.".to_string());

        enrich_bookmark(&mut bm, &article, &bundle, &repo, &config, &factory);

        let captured = captured_prompt.lock().unwrap();
        assert_eq!(*captured, Some(Some("Be helpful and concise.".to_string())));
    }

    // ── Error sanitization tests ─────────────────────────────────────

    #[test]
    fn sanitize_error_hides_stderr() {
        let err = AgentError::ProcessFailed {
            provider: "claude",
            status: 1,
            stderr: "secret stuff in stderr".to_string(),
        };
        let msg = sanitize_error(&err);
        assert!(!msg.contains("secret"));
        assert!(msg.contains("claude CLI exited with status 1"));
    }

    #[test]
    fn sanitize_error_hides_io_details() {
        let err = AgentError::Spawn {
            provider: "codex",
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "binary not found"),
        };
        let msg = sanitize_error(&err);
        assert!(!msg.contains("binary not found"));
        assert!(msg.contains("failed to spawn codex CLI"));
    }

    // ── UTF-8 truncation tests ─────────────────────────────────────

    #[test]
    fn truncate_utf8_short_string_unchanged() {
        assert_eq!(truncate_utf8("hello", 10), "hello");
    }

    #[test]
    fn truncate_utf8_exact_length_unchanged() {
        assert_eq!(truncate_utf8("hello", 5), "hello");
    }

    #[test]
    fn truncate_utf8_truncates_with_ellipsis() {
        assert_eq!(truncate_utf8("hello world", 5), "hello...");
    }

    #[test]
    fn truncate_utf8_handles_multibyte_characters() {
        // Each emoji is a multibyte character; byte slicing would panic here
        let s = "🎉🎊🎈🎆🎇";
        let result = truncate_utf8(s, 3);
        assert_eq!(result, "🎉🎊🎈...");
    }

    // ── Blank provider summary test ─────────────────────────────────

    #[test]
    fn blank_provider_summary_treated_as_failure() {
        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, article) = setup_bundle_and_db(&tmp);
        let repo = BookmarkRepository::new(&conn);
        let config = test_config(&storage);

        fn blank_summary_factory(
            _agent: &str,
            _system_prompt: Option<&str>,
        ) -> Result<Box<dyn AgentProvider>, AgentError> {
            Ok(Box::new(FakeProvider {
                response: Ok(EnrichmentResponse {
                    summary: "   ".to_string(), // blank/whitespace only
                    suggested_tags: vec!["tag".to_string()],
                    suggested_collection: Some("col".to_string()),
                }),
            }))
        }

        let outcome = enrich_bookmark(
            &mut bm,
            &article,
            &bundle,
            &repo,
            &config,
            &blank_summary_factory,
        );

        assert!(matches!(outcome, EnrichOutcome::Failed { .. }));
        assert_eq!(bm.summary_status, SummaryStatus::Failed);

        // Tags/collection should NOT be applied on blank summary
        assert!(bm.suggested_tags.is_empty());

        // DB should reflect failed status
        let loaded = repo.get_by_id("am_01TEST123456").unwrap().unwrap();
        assert_eq!(loaded.summary_status, SummaryStatus::Failed);

        // Failure event should be logged
        let events_content = std::fs::read_to_string(bundle.path().join("events.jsonl")).unwrap();
        let lines: Vec<&str> = events_content.lines().collect();
        assert!(lines.len() >= 2);
        let last_event = BookmarkEvent::from_json_line(lines.last().unwrap()).unwrap();
        assert_eq!(last_event.details["status"], "failed");
        assert!(last_event.details["error"]
            .as_str()
            .unwrap()
            .contains("blank summary"));
    }

    // ── Persistence failure event logging tests ─────────────────────

    #[test]
    fn db_update_enrichment_false_logs_failure_event() {
        let tmp = TempDir::new().unwrap();
        let (mut bm, bundle, conn, storage, article) = setup_bundle_and_db(&tmp);
        let repo = BookmarkRepository::new(&conn);
        let config = test_config(&storage);

        // Delete the DB row so update_enrichment returns Ok(false)
        conn.execute("DELETE FROM bookmarks WHERE id = ?1", [&bm.id])
            .unwrap();

        let outcome = enrich_bookmark(&mut bm, &article, &bundle, &repo, &config, &success_factory);

        assert!(matches!(outcome, EnrichOutcome::Failed { .. }));
        if let EnrichOutcome::Failed { ref warning } = outcome {
            assert!(
                warning.contains("bookmark row not found"),
                "warning: {warning}"
            );
        }
        assert_eq!(bm.summary_status, SummaryStatus::Failed);

        // Should have logged a failure event
        let events_content = std::fs::read_to_string(bundle.path().join("events.jsonl")).unwrap();
        let lines: Vec<&str> = events_content.lines().collect();
        // saved + enriched(failed)
        assert!(lines.len() >= 2);
        let last_event = BookmarkEvent::from_json_line(lines.last().unwrap()).unwrap();
        assert_eq!(last_event.event_type, EventType::Enriched);
        assert_eq!(last_event.details["status"], "failed");
        assert!(last_event.details["error"]
            .as_str()
            .unwrap()
            .contains("bookmark row not found"));
    }

    // ── Save-pipeline integration tests ──────────────────────────────
    //
    // These exercise the full save → enrich flow using execute_save_with_deps
    // with injected fake providers.

    mod save_integration {
        use crate::cli::SaveArgs;
        use crate::commands::save::{execute_save_with_deps, DedupResult};

        use std::path::Path;
        use tempfile::TempDir;

        fn setup_home_with_enrichment(
            tmp: &TempDir,
            storage_path: &Path,
            enabled: bool,
        ) -> std::path::PathBuf {
            let home = tmp.path().join("home");
            let config_dir = home.join(".agentmark");
            std::fs::create_dir_all(&config_dir).unwrap();
            std::fs::create_dir_all(storage_path).unwrap();

            let config_content = format!(
                r#"default_agent = "claude"
storage_path = "{}"
system_prompt = "Be helpful."

[enrichment]
enabled = {}
"#,
                storage_path.display(),
                enabled
            );
            std::fs::write(config_dir.join("config.toml"), config_content).unwrap();
            std::fs::write(config_dir.join("index.db"), b"").unwrap();
            home
        }

        fn save_args(url: &str) -> SaveArgs {
            SaveArgs {
                url: url.to_string(),
                tags: None,
                collection: None,
                note: None,
                action: None,
                no_enrich: false,
            }
        }

        #[test]
        fn save_with_enrichment_enabled_success() {
            use super::*;

            let tmp = TempDir::new().unwrap();
            let storage = tmp.path().join("bookmarks");
            let home = setup_home_with_enrichment(&tmp, &storage, true);

            let mut server = mockito::Server::new();
            let _mock = server
                .mock("GET", "/article")
                .with_status(200)
                .with_body(
                    r#"<!DOCTYPE html><html><head><title>Test</title></head>
                    <body><article><h1>Test</h1>
                    <p>Content here with enough text for extraction to succeed properly.</p>
                    <p>More text in the second paragraph for good measure.</p>
                    </article></body></html>"#,
                )
                .create();

            let url = format!("{}/article", server.url());
            let args = save_args(&url);

            let outcome = execute_save_with_deps(&home, &args, &success_factory).unwrap();
            assert_eq!(outcome.dedup, DedupResult::New);
            assert!(
                outcome.warnings.is_empty(),
                "warnings: {:?}",
                outcome.warnings
            );

            // Verify DB: summary_status = done, suggested_tags populated
            let db_path = home.join(".agentmark/index.db");
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            let (status, tags): (String, String) = conn
                .query_row(
                    "SELECT summary_status, suggested_tags FROM bookmarks LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(status, "done");
            let parsed_tags: Vec<String> = serde_json::from_str(&tags).unwrap();
            assert!(parsed_tags.contains(&"testing".to_string()));

            // Verify FTS can find the summary
            let found: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM bookmarks_fts WHERE bookmarks_fts MATCH 'great article about testing'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(found, 1);

            // Verify events.jsonl has saved + enriched
            let bundle_dirs = walk_for_bundles_helper(&storage);
            assert_eq!(bundle_dirs.len(), 1);
            let events = std::fs::read_to_string(bundle_dirs[0].join("events.jsonl")).unwrap();
            let lines: Vec<&str> = events.lines().collect();
            assert_eq!(lines.len(), 2);
            assert!(lines[0].contains("\"saved\""));
            assert!(lines[1].contains("\"enriched\""));
            assert!(lines[1].contains("\"success\""));
        }

        #[test]
        fn save_with_enrichment_failure_still_succeeds() {
            use super::*;

            let tmp = TempDir::new().unwrap();
            let storage = tmp.path().join("bookmarks");
            let home = setup_home_with_enrichment(&tmp, &storage, true);

            let mut server = mockito::Server::new();
            let _mock = server
                .mock("GET", "/article")
                .with_status(200)
                .with_body(
                    r#"<!DOCTYPE html><html><head><title>Test</title></head>
                    <body><article><h1>Test</h1>
                    <p>Content here with enough text for extraction to succeed properly.</p>
                    <p>More text in the second paragraph for good measure.</p>
                    </article></body></html>"#,
                )
                .create();

            let url = format!("{}/article", server.url());
            let args = save_args(&url);

            let outcome = execute_save_with_deps(&home, &args, &failure_factory).unwrap();
            assert_eq!(outcome.dedup, DedupResult::New);
            // Should have a warning from enrichment failure
            assert!(!outcome.warnings.is_empty());
            assert!(outcome.warnings[0].contains("enrichment failed"));

            // DB: summary_status = failed, save still succeeded
            let db_path = home.join(".agentmark/index.db");
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            let status: String = conn
                .query_row("SELECT summary_status FROM bookmarks LIMIT 1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(status, "failed");
        }

        #[test]
        fn save_with_no_enrich_flag_skips() {
            use super::*;

            let tmp = TempDir::new().unwrap();
            let storage = tmp.path().join("bookmarks");
            let home = setup_home_with_enrichment(&tmp, &storage, true);

            let mut server = mockito::Server::new();
            let _mock = server
                .mock("GET", "/article")
                .with_status(200)
                .with_body(
                    r#"<!DOCTYPE html><html><head><title>Test</title></head>
                    <body><article><h1>Test</h1>
                    <p>Content for extraction.</p><p>More content.</p>
                    </article></body></html>"#,
                )
                .create();

            let url = format!("{}/article", server.url());
            let mut args = save_args(&url);
            args.no_enrich = true;

            let outcome = execute_save_with_deps(&home, &args, &success_factory).unwrap();
            assert_eq!(outcome.dedup, DedupResult::New);

            // summary_status should be pending (enrichment skipped)
            let db_path = home.join(".agentmark/index.db");
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            let status: String = conn
                .query_row("SELECT summary_status FROM bookmarks LIMIT 1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(status, "pending");
        }

        #[test]
        fn save_content_changed_triggers_re_enrichment() {
            use super::*;

            let tmp = TempDir::new().unwrap();
            let storage = tmp.path().join("bookmarks");
            let home = setup_home_with_enrichment(&tmp, &storage, true);

            let mut server = mockito::Server::new();

            // First save: original content
            let _mock1 = server
                .mock("GET", "/article")
                .with_status(200)
                .with_body(
                    r#"<!DOCTYPE html><html><head><title>Test</title></head>
                    <body><article><h1>Original</h1>
                    <p>Original content for extraction here with enough text.</p>
                    <p>More original text in the second paragraph.</p>
                    </article></body></html>"#,
                )
                .create();

            let url = format!("{}/article", server.url());
            let args = save_args(&url);
            let outcome1 = execute_save_with_deps(&home, &args, &success_factory).unwrap();
            assert_eq!(outcome1.dedup, DedupResult::New);

            // Verify summary is "done" after first save
            let db_path = home.join(".agentmark/index.db");
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            let status1: String = conn
                .query_row("SELECT summary_status FROM bookmarks LIMIT 1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(status1, "done");

            // Second save: changed content
            let _mock2 = server
                .mock("GET", "/article")
                .with_status(200)
                .with_body(
                    r#"<!DOCTYPE html><html><head><title>Test</title></head>
                    <body><article><h1>Updated</h1>
                    <p>Completely different content that changes the hash significantly.</p>
                    <p>New paragraphs with entirely different words and meaning.</p>
                    </article></body></html>"#,
                )
                .create();

            let outcome2 = execute_save_with_deps(&home, &args, &success_factory).unwrap();
            assert_eq!(outcome2.dedup, DedupResult::ContentChanged);

            // summary_status should be "done" again (re-enrichment succeeded)
            let status2: String = conn
                .query_row("SELECT summary_status FROM bookmarks LIMIT 1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(status2, "done");

            // Verify events: saved + enriched + content_updated + enriched
            let bundle_dirs = walk_for_bundles_helper(&storage);
            let events = std::fs::read_to_string(bundle_dirs[0].join("events.jsonl")).unwrap();
            let lines: Vec<&str> = events.lines().collect();
            assert_eq!(lines.len(), 4);
            assert!(lines[0].contains("\"saved\""));
            assert!(lines[1].contains("\"enriched\""));
            assert!(lines[2].contains("\"content_updated\""));
            assert!(lines[3].contains("\"enriched\""));
        }

        #[test]
        fn save_content_changed_clears_stale_summary_on_failure() {
            use super::*;

            let tmp = TempDir::new().unwrap();
            let storage = tmp.path().join("bookmarks");
            let home = setup_home_with_enrichment(&tmp, &storage, true);

            let mut server = mockito::Server::new();

            // First save: enrichment succeeds
            let _mock1 = server
                .mock("GET", "/article")
                .with_status(200)
                .with_body(
                    r#"<!DOCTYPE html><html><head><title>Test</title></head>
                    <body><article><h1>Original</h1>
                    <p>Original content for extraction here with enough text.</p>
                    <p>More original text in the second paragraph.</p>
                    </article></body></html>"#,
                )
                .create();

            let url = format!("{}/article", server.url());
            let args = save_args(&url);
            execute_save_with_deps(&home, &args, &success_factory).unwrap();

            // Verify summary is in bookmark.md
            let bundle_dirs = walk_for_bundles_helper(&storage);
            let bm_content = std::fs::read_to_string(bundle_dirs[0].join("bookmark.md")).unwrap();
            assert!(bm_content.contains("A great article about testing."));

            // Second save: changed content, enrichment fails
            let _mock2 = server
                .mock("GET", "/article")
                .with_status(200)
                .with_body(
                    r#"<!DOCTYPE html><html><head><title>Test</title></head>
                    <body><article><h1>Updated</h1>
                    <p>Completely different content that changes the hash significantly.</p>
                    <p>New paragraphs with entirely different words and meaning.</p>
                    </article></body></html>"#,
                )
                .create();

            let outcome = execute_save_with_deps(&home, &args, &failure_factory).unwrap();
            assert_eq!(outcome.dedup, DedupResult::ContentChanged);

            // Old summary should be cleared (not preserved from first enrichment)
            let bm_content = std::fs::read_to_string(bundle_dirs[0].join("bookmark.md")).unwrap();
            assert!(
                !bm_content.contains("A great article about testing."),
                "stale summary should be cleared on content change"
            );

            // summary_status should be failed
            let db_path = home.join(".agentmark/index.db");
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            let status: String = conn
                .query_row("SELECT summary_status FROM bookmarks LIMIT 1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(status, "failed");
        }

        #[test]
        fn save_system_prompt_forwarded_through_pipeline() {
            use super::*;
            use std::sync::{Arc, Mutex};

            let captured_prompt: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
            let captured_clone = captured_prompt.clone();

            let factory = move |_agent: &str,
                                system_prompt: Option<&str>|
                  -> Result<Box<dyn AgentProvider>, AgentError> {
                *captured_clone.lock().unwrap() = Some(system_prompt.map(|s| s.to_string()));
                Ok(Box::new(FakeProvider {
                    response: Ok(EnrichmentResponse {
                        summary: "test".to_string(),
                        suggested_tags: vec![],
                        suggested_collection: None,
                    }),
                }))
            };

            let tmp = TempDir::new().unwrap();
            let storage = tmp.path().join("bookmarks");
            let home = setup_home_with_enrichment(&tmp, &storage, true);

            let mut server = mockito::Server::new();
            let _mock = server
                .mock("GET", "/article")
                .with_status(200)
                .with_body(
                    r#"<!DOCTYPE html><html><head><title>Test</title></head>
                    <body><article><h1>Test</h1>
                    <p>Content for extraction.</p><p>More content.</p>
                    </article></body></html>"#,
                )
                .create();

            let url = format!("{}/article", server.url());
            let args = save_args(&url);
            execute_save_with_deps(&home, &args, &factory).unwrap();

            let captured = captured_prompt.lock().unwrap();
            assert_eq!(*captured, Some(Some("Be helpful.".to_string())));
        }

        /// Helper to find bundle dirs in storage.
        fn walk_for_bundles_helper(dir: &Path) -> Vec<std::path::PathBuf> {
            let mut results = Vec::new();
            walk_recursive(dir, &mut results);
            results
        }

        fn walk_recursive(dir: &Path, results: &mut Vec<std::path::PathBuf>) {
            if !dir.is_dir() {
                return;
            }
            if dir.join("bookmark.md").exists() {
                results.push(dir.to_path_buf());
                return;
            }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    walk_recursive(&entry.path(), results);
                }
            }
        }
    }
}
