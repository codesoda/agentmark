//! Reprocess command: re-fetch, re-extract, and re-enrich an existing bookmark.
//!
//! Handles two paths:
//! - Single bookmark: `agentmark reprocess <id>`
//! - Batch: `agentmark reprocess --all` (with confirmation prompt)
//!
//! Uses current `config.toml` settings so reprocessing after config changes
//! (new agent, updated system_prompt) uses the new settings.

use std::fmt;
use std::io::{BufRead, Write};
use std::path::Path;

use crate::agent;
use crate::bundle::{BodySections, Bundle};
use crate::cli::ReprocessArgs;
use crate::config::{self, Config, ConfigError};
use crate::db::{self, BookmarkRepository, DbError};
use crate::enrich::{self, EnrichOutcome, ProviderFactory};
use crate::extract::{self, ExtractionResult};
use crate::fetch::{self, PageMetadata};
use crate::models::{Bookmark, BookmarkEvent, ContentStatus, EventType, SummaryStatus};

// ── Public entry point ──────────────────────────────────────────────

/// Entry point for `agentmark reprocess` using real environment.
pub fn run_reprocess(args: ReprocessArgs) -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut writer = std::io::stderr();
    execute_reprocess_with_io(
        &home,
        &args,
        &mut reader,
        &mut writer,
        &default_provider_factory,
    )?;
    Ok(())
}

// ── Typed errors ────────────────────────────────────────────────────

#[derive(Debug)]
pub(crate) enum ReprocessError {
    Config(ConfigError),
    Db(DbError),
    NotFound { id: String },
    BundleDrift { id: String, detail: String },
    FetchFailed { id: String, detail: String },
    BundleWrite { id: String, detail: String },
    PartialUpdate { id: String, detail: String },
    EventAppend { id: String, detail: String },
    Cancelled,
}

impl fmt::Display for ReprocessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReprocessError::Config(e) => write!(f, "{e}"),
            ReprocessError::Db(e) => write!(f, "database error: {e}"),
            ReprocessError::NotFound { id } => write!(f, "bookmark not found: {id}"),
            ReprocessError::BundleDrift { id, detail } => {
                write!(f, "bundle not found for {id}: {detail}")
            }
            ReprocessError::FetchFailed { id, detail } => {
                write!(f, "fetch failed for {id}: {detail}")
            }
            ReprocessError::BundleWrite { id, detail } => {
                write!(f, "bundle write failed for {id}: {detail}")
            }
            ReprocessError::PartialUpdate { id, detail } => {
                write!(f, "bundle updated but DB write failed for {id}: {detail}")
            }
            ReprocessError::EventAppend { id, detail } => {
                write!(f, "reprocessed event append failed for {id}: {detail}")
            }
            ReprocessError::Cancelled => write!(f, "reprocess cancelled"),
        }
    }
}

impl std::error::Error for ReprocessError {}

impl From<ConfigError> for ReprocessError {
    fn from(e: ConfigError) -> Self {
        ReprocessError::Config(e)
    }
}

// ── Single reprocess result ─────────────────────────────────────────

/// What changed during a single reprocess.
#[derive(Debug)]
struct ReprocessResult {
    id: String,
    content_changed: bool,
    metadata_changed: bool,
    enrichment_outcome: String,
}

impl ReprocessResult {
    fn summary_line(&self) -> String {
        let content = if self.content_changed {
            "content updated"
        } else {
            "content unchanged"
        };
        let meta = if self.metadata_changed {
            ", metadata updated"
        } else {
            ""
        };
        format!(
            "Reprocessed {} — {}{}, enrichment: {}",
            self.id, content, meta, self.enrichment_outcome
        )
    }
}

// ── Batch summary ───────────────────────────────────────────────────

#[derive(Debug, Default)]
struct BatchSummary {
    attempted: usize,
    succeeded: usize,
    failed: usize,
    content_changed: usize,
}

impl BatchSummary {
    fn format(&self) -> String {
        format!(
            "Reprocessed {}/{} bookmarks ({} content changed, {} failed)",
            self.succeeded, self.attempted, self.content_changed, self.failed
        )
    }

    fn has_failures(&self) -> bool {
        self.failed > 0
    }
}

// ── Default provider factory ─────────────────────────────────────────

fn default_provider_factory(
    default_agent: &str,
    system_prompt: Option<&str>,
) -> Result<Box<dyn crate::agent::AgentProvider>, crate::agent::AgentError> {
    agent::create_provider(default_agent, system_prompt)
}

// ── Testable reprocess pipeline ─────────────────────────────────────

/// Testable reprocess implementation with injected I/O and provider factory.
pub(crate) fn execute_reprocess_with_io(
    home: &Path,
    args: &ReprocessArgs,
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
    provider_factory: &ProviderFactory,
) -> Result<(), ReprocessError> {
    let config = Config::load(home)?;
    let db_path = config::index_db_path(home);
    let conn = db::open_and_migrate(&db_path).map_err(ReprocessError::Db)?;
    let repo = BookmarkRepository::new(&conn);

    if args.all {
        reprocess_all(&config, &repo, reader, writer, provider_factory)
    } else {
        let id = args
            .id
            .as_deref()
            .expect("clap ensures id is present when --all is not set");
        let result = reprocess_single(id, &config, &repo, provider_factory)?;
        writeln!(writer, "{}", result.summary_line()).ok();
        Ok(())
    }
}

// ── Single bookmark reprocess ───────────────────────────────────────

fn reprocess_single(
    id: &str,
    config: &Config,
    repo: &BookmarkRepository<'_>,
    provider_factory: &ProviderFactory,
) -> Result<ReprocessResult, ReprocessError> {
    // 1. Look up bookmark
    let mut bookmark = repo
        .get_by_id(id)
        .map_err(ReprocessError::Db)?
        .ok_or_else(|| ReprocessError::NotFound { id: id.to_string() })?;

    // 2. Find existing bundle
    let bundle =
        Bundle::find(&config.storage_path, &bookmark.saved_at, &bookmark.id).map_err(|e| {
            ReprocessError::BundleDrift {
                id: id.to_string(),
                detail: e.to_string(),
            }
        })?;

    // 3. Fetch + extract current page
    let (raw_html, page_metadata) =
        fetch::fetch_page(&bookmark.url).map_err(|e| ReprocessError::FetchFailed {
            id: id.to_string(),
            detail: e.to_string(),
        })?;
    let extraction = extract::extract_content(&raw_html);

    // 4. Classify changes
    let old_hash = bookmark.content_hash.clone();
    let new_hash = &extraction.content_hash;
    let content_changed = old_hash.as_deref() != Some(new_hash);

    let metadata_changed = has_metadata_changed(&bookmark, &page_metadata);

    // 5. Update bookmark metadata from fresh fetch
    apply_metadata(&mut bookmark, &page_metadata);
    bookmark.content_hash = Some(extraction.content_hash.clone());

    let (content_status, _extraction_warning) = classify_extraction(&extraction);
    bookmark.content_status = content_status;

    // 6. Update bundle and DB based on content change status
    let article_markdown = if content_changed {
        // Content changed: update all bundle files, clear stale enrichment
        bookmark.summary_status = SummaryStatus::Pending;
        bookmark.suggested_tags = Vec::new();

        bundle
            .update_article_md(&extraction.article_markdown)
            .map_err(|e| ReprocessError::BundleWrite {
                id: id.to_string(),
                detail: e.to_string(),
            })?;
        bundle
            .update_metadata_json(&page_metadata)
            .map_err(|e| ReprocessError::BundleWrite {
                id: id.to_string(),
                detail: e.to_string(),
            })?;
        bundle
            .update_source_html(&raw_html)
            .map_err(|e| ReprocessError::BundleWrite {
                id: id.to_string(),
                detail: e.to_string(),
            })?;

        // Clear stale enrichment body sections
        let sections = BodySections::default();
        bundle
            .update_bookmark_md(&bookmark, &sections)
            .map_err(|e| ReprocessError::BundleWrite {
                id: id.to_string(),
                detail: e.to_string(),
            })?;

        // Clear DB summary
        repo.set_summary(&bookmark.id, "")
            .map_err(ReprocessError::Db)?;

        extraction.article_markdown.clone()
    } else {
        // Content unchanged: refresh metadata/source but preserve enriched body
        bundle
            .update_metadata_json(&page_metadata)
            .map_err(|e| ReprocessError::BundleWrite {
                id: id.to_string(),
                detail: e.to_string(),
            })?;
        bundle
            .update_source_html(&raw_html)
            .map_err(|e| ReprocessError::BundleWrite {
                id: id.to_string(),
                detail: e.to_string(),
            })?;
        bundle
            .update_bookmark_md_preserving_body(&bookmark)
            .map_err(|e| ReprocessError::BundleWrite {
                id: id.to_string(),
                detail: e.to_string(),
            })?;

        extraction.article_markdown.clone()
    };

    // 7. Persist bookmark to DB
    match repo.update(&bookmark) {
        Err(e) => {
            return Err(ReprocessError::PartialUpdate {
                id: id.to_string(),
                detail: e.to_string(),
            });
        }
        Ok(false) => {
            return Err(ReprocessError::PartialUpdate {
                id: id.to_string(),
                detail: "bookmark row disappeared during reprocess".to_string(),
            });
        }
        Ok(true) => {}
    }

    // 8. Re-run enrichment with current config
    let enrichment_outcome = run_enrichment(
        &mut bookmark,
        &article_markdown,
        &bundle,
        repo,
        config,
        provider_factory,
    );

    // 9. Append reprocessed event
    let event = BookmarkEvent::new(
        EventType::Reprocessed,
        serde_json::json!({
            "content_changed": content_changed,
            "metadata_changed": metadata_changed,
            "old_hash": old_hash,
            "new_hash": extraction.content_hash,
            "enrichment": &enrichment_outcome,
            "agent": &config.default_agent,
        }),
    );
    bundle
        .append_event(&event)
        .map_err(|e| ReprocessError::EventAppend {
            id: id.to_string(),
            detail: e.to_string(),
        })?;

    Ok(ReprocessResult {
        id: id.to_string(),
        content_changed,
        metadata_changed,
        enrichment_outcome,
    })
}

// ── Batch reprocess ─────────────────────────────────────────────────

fn reprocess_all(
    config: &Config,
    repo: &BookmarkRepository<'_>,
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
    provider_factory: &ProviderFactory,
) -> Result<(), ReprocessError> {
    let count = repo.count_bookmarks().map_err(ReprocessError::Db)?;

    if count == 0 {
        writeln!(writer, "No bookmarks to reprocess.").ok();
        return Ok(());
    }

    // Prompt for confirmation
    write!(writer, "Reprocess all {count} bookmarks? (y/n) ").ok();
    writer.flush().ok();

    let confirmed = read_confirmation(reader);
    if !confirmed {
        writeln!(writer, "Reprocess cancelled.").ok();
        return Err(ReprocessError::Cancelled);
    }

    // Process in pages
    let page_size = 50;
    let mut offset = 0;
    let mut summary = BatchSummary::default();

    loop {
        let page = repo
            .list(page_size, offset, None, None, None)
            .map_err(ReprocessError::Db)?;

        if page.is_empty() {
            break;
        }

        let page_len = page.len();

        for bookmark in &page {
            summary.attempted += 1;
            let progress = format!("[{}/{}]", summary.attempted, count);

            match reprocess_single(&bookmark.id, config, repo, provider_factory) {
                Ok(result) => {
                    summary.succeeded += 1;
                    if result.content_changed {
                        summary.content_changed += 1;
                    }
                    writeln!(writer, "{progress} {}", result.summary_line()).ok();
                }
                Err(e) => {
                    summary.failed += 1;
                    writeln!(writer, "{progress} error: {e}").ok();
                }
            }
        }

        if page_len < page_size {
            break;
        }
        offset += page_len;
    }

    writeln!(writer, "\n{}", summary.format()).ok();

    if summary.has_failures() {
        Err(ReprocessError::Db(DbError::NotFound {
            id: format!("{} bookmark(s) failed", summary.failed),
        }))
    } else {
        Ok(())
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Read a yes/no confirmation from stdin. Default is No.
fn read_confirmation(reader: &mut dyn BufRead) -> bool {
    let mut buf = String::new();
    match reader.read_line(&mut buf) {
        Ok(0) => false, // EOF
        Ok(_) => {
            let trimmed = buf.trim().to_lowercase();
            matches!(trimmed.as_str(), "y" | "yes")
        }
        Err(_) => false,
    }
}

/// Check if page metadata differs from current bookmark metadata.
fn has_metadata_changed(bookmark: &Bookmark, metadata: &PageMetadata) -> bool {
    let title_changed = metadata.title.as_deref() != Some(&bookmark.title);
    let desc_changed = metadata.description != bookmark.description;
    let author_changed = metadata.author != bookmark.author;
    let site_changed = metadata.site_name != bookmark.site_name;
    let pub_changed = metadata.published_at != bookmark.published_at;

    title_changed || desc_changed || author_changed || site_changed || pub_changed
}

/// Apply fresh metadata from fetch to a bookmark, preserving identity fields.
fn apply_metadata(bookmark: &mut Bookmark, metadata: &PageMetadata) {
    if let Some(ref title) = metadata.title {
        bookmark.title = title.clone();
    }
    bookmark.description = metadata.description.clone();
    bookmark.author = metadata.author.clone();
    bookmark.site_name = metadata.site_name.clone();
    bookmark.published_at = metadata.published_at.clone();
}

/// Classify extraction result into content status and optional warning.
fn classify_extraction(result: &ExtractionResult) -> (ContentStatus, Option<String>) {
    if result.article_markdown.trim().is_empty() {
        (
            ContentStatus::Failed,
            Some("content extraction produced no readable text".to_string()),
        )
    } else {
        (ContentStatus::Extracted, None)
    }
}

/// Run enrichment and return a string describing the outcome.
fn run_enrichment(
    bookmark: &mut Bookmark,
    article_markdown: &str,
    bundle: &Bundle,
    repo: &BookmarkRepository<'_>,
    config: &Config,
    provider_factory: &ProviderFactory,
) -> String {
    let outcome = enrich::enrich_bookmark(
        bookmark,
        article_markdown,
        bundle,
        repo,
        config,
        provider_factory,
    );
    match outcome {
        EnrichOutcome::Success => "success".to_string(),
        EnrichOutcome::Skipped { reason } => format!("skipped ({reason})"),
        EnrichOutcome::Failed { warning } => format!("failed ({warning})"),
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── read_confirmation ────────────────────────────────────────────

    #[test]
    fn confirmation_yes_returns_true() {
        let mut reader = std::io::Cursor::new(b"y\n".to_vec());
        assert!(read_confirmation(&mut reader));
    }

    #[test]
    fn confirmation_yes_full_returns_true() {
        let mut reader = std::io::Cursor::new(b"yes\n".to_vec());
        assert!(read_confirmation(&mut reader));
    }

    #[test]
    fn confirmation_yes_uppercase_returns_true() {
        let mut reader = std::io::Cursor::new(b"Y\n".to_vec());
        assert!(read_confirmation(&mut reader));
    }

    #[test]
    fn confirmation_no_returns_false() {
        let mut reader = std::io::Cursor::new(b"n\n".to_vec());
        assert!(!read_confirmation(&mut reader));
    }

    #[test]
    fn confirmation_blank_returns_false() {
        let mut reader = std::io::Cursor::new(b"\n".to_vec());
        assert!(!read_confirmation(&mut reader));
    }

    #[test]
    fn confirmation_eof_returns_false() {
        let mut reader = std::io::Cursor::new(b"".to_vec());
        assert!(!read_confirmation(&mut reader));
    }

    #[test]
    fn confirmation_invalid_returns_false() {
        let mut reader = std::io::Cursor::new(b"maybe\n".to_vec());
        assert!(!read_confirmation(&mut reader));
    }

    // ── has_metadata_changed ─────────────────────────────────────────

    #[test]
    fn metadata_unchanged_returns_false() {
        let bm = Bookmark::new("https://example.com", "Title");
        let meta = PageMetadata {
            title: Some("Title".to_string()),
            ..Default::default()
        };
        assert!(!has_metadata_changed(&bm, &meta));
    }

    #[test]
    fn metadata_title_changed_returns_true() {
        let bm = Bookmark::new("https://example.com", "Old Title");
        let meta = PageMetadata {
            title: Some("New Title".to_string()),
            ..Default::default()
        };
        assert!(has_metadata_changed(&bm, &meta));
    }

    #[test]
    fn metadata_description_changed_returns_true() {
        let bm = Bookmark::new("https://example.com", "Title");
        let meta = PageMetadata {
            title: Some("Title".to_string()),
            description: Some("New desc".to_string()),
            ..Default::default()
        };
        assert!(has_metadata_changed(&bm, &meta));
    }

    // ── classify_extraction ──────────────────────────────────────────

    #[test]
    fn classify_nonempty_is_extracted() {
        let result = ExtractionResult {
            article_html: "<p>text</p>".to_string(),
            article_markdown: "text".to_string(),
            content_hash: "sha256:abc".to_string(),
        };
        let (status, warning) = classify_extraction(&result);
        assert_eq!(status, ContentStatus::Extracted);
        assert!(warning.is_none());
    }

    #[test]
    fn classify_empty_is_failed() {
        let result = ExtractionResult {
            article_html: String::new(),
            article_markdown: String::new(),
            content_hash: "sha256:abc".to_string(),
        };
        let (status, warning) = classify_extraction(&result);
        assert_eq!(status, ContentStatus::Failed);
        assert!(warning.is_some());
    }

    // ── apply_metadata ───────────────────────────────────────────────

    #[test]
    fn apply_metadata_updates_fields() {
        let mut bm = Bookmark::new("https://example.com", "Old Title");
        let meta = PageMetadata {
            title: Some("New Title".to_string()),
            description: Some("Desc".to_string()),
            author: Some("Author".to_string()),
            site_name: Some("Site".to_string()),
            published_at: Some("2026-01-01".to_string()),
            ..Default::default()
        };
        apply_metadata(&mut bm, &meta);

        assert_eq!(bm.title, "New Title");
        assert_eq!(bm.description.as_deref(), Some("Desc"));
        assert_eq!(bm.author.as_deref(), Some("Author"));
        assert_eq!(bm.site_name.as_deref(), Some("Site"));
        assert_eq!(bm.published_at.as_deref(), Some("2026-01-01"));
    }

    #[test]
    fn apply_metadata_preserves_title_when_none() {
        let mut bm = Bookmark::new("https://example.com", "Original Title");
        let meta = PageMetadata::default();
        apply_metadata(&mut bm, &meta);

        assert_eq!(bm.title, "Original Title");
    }

    // ── ReprocessResult ──────────────────────────────────────────────

    #[test]
    fn reprocess_result_summary_content_changed() {
        let result = ReprocessResult {
            id: "am_123".to_string(),
            content_changed: true,
            metadata_changed: false,
            enrichment_outcome: "success".to_string(),
        };
        let line = result.summary_line();
        assert!(line.contains("content updated"));
        assert!(line.contains("success"));
        assert!(!line.contains("metadata updated"));
    }

    #[test]
    fn reprocess_result_summary_metadata_changed() {
        let result = ReprocessResult {
            id: "am_123".to_string(),
            content_changed: false,
            metadata_changed: true,
            enrichment_outcome: "skipped (disabled)".to_string(),
        };
        let line = result.summary_line();
        assert!(line.contains("content unchanged"));
        assert!(line.contains("metadata updated"));
        assert!(line.contains("skipped"));
    }

    // ── BatchSummary ─────────────────────────────────────────────────

    #[test]
    fn batch_summary_format() {
        let summary = BatchSummary {
            attempted: 5,
            succeeded: 4,
            failed: 1,
            content_changed: 2,
        };
        let formatted = summary.format();
        assert!(formatted.contains("4/5"));
        assert!(formatted.contains("2 content changed"));
        assert!(formatted.contains("1 failed"));
    }

    #[test]
    fn batch_summary_no_failures() {
        let summary = BatchSummary {
            attempted: 3,
            succeeded: 3,
            failed: 0,
            content_changed: 0,
        };
        assert!(!summary.has_failures());
    }

    #[test]
    fn batch_summary_with_failures() {
        let summary = BatchSummary {
            attempted: 3,
            succeeded: 2,
            failed: 1,
            content_changed: 1,
        };
        assert!(summary.has_failures());
    }

    // ── ReprocessError display ───────────────────────────────────────

    #[test]
    fn error_not_found_display() {
        let err = ReprocessError::NotFound {
            id: "am_123".to_string(),
        };
        assert!(err.to_string().contains("not found"));
        assert!(err.to_string().contains("am_123"));
    }

    #[test]
    fn error_bundle_drift_display() {
        let err = ReprocessError::BundleDrift {
            id: "am_123".to_string(),
            detail: "missing dir".to_string(),
        };
        assert!(err.to_string().contains("bundle not found"));
    }

    #[test]
    fn error_cancelled_display() {
        let err = ReprocessError::Cancelled;
        assert!(err.to_string().contains("cancelled"));
    }

    #[test]
    fn error_partial_update_display() {
        let err = ReprocessError::PartialUpdate {
            id: "am_test".to_string(),
            detail: "row gone".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("DB write failed"));
        assert!(msg.contains("am_test"));
    }
}
