//! Deferred model work for a saved bookmark.
//!
//! Interactive saves (the browser extension) return as soon as the bookmark is
//! durable. The native host then starts `agentmark post-process <id>` as a
//! detached process, which:
//!
//! 1. optionally has the model render `article.md` from the captured page HTML
//!    (browser snapshots of profiles, feeds and app-like pages, where generic
//!    article extraction picks the wrong part of the page), and
//! 2. runs enrichment (summary, suggested tags, suggested collection).
//!
//! Every step is best-effort: a failure leaves the bookmark as the save left it.

use std::path::Path;
use std::process::{Command, Stdio};

use tracing::{debug, info, instrument, warn};

use crate::agent::ArticleRequest;
use crate::bundle::Bundle;
use crate::cli::PostProcessArgs;
use crate::commands::save::{default_provider_factory, PostProcess};
use crate::config::{self, Config};
use crate::db::{self, BookmarkRepository};
use crate::enrich::{self, EnrichOutcome, ProviderFactory};
use crate::extract;
use crate::models::{Bookmark, BookmarkEvent, ContentStatus, EventType};

/// Upper bound on the simplified HTML sent to the model (bytes). Larger pages
/// keep the generically extracted article instead.
const MAX_MODEL_HTML_BYTES: usize = 400_000;

/// What happened to the article during post-processing.
#[derive(Debug, PartialEq)]
pub(crate) enum ArticleOutcome {
    /// Rendering was not requested.
    NotRequested,
    /// The model rendered a new `article.md`.
    Rendered,
    /// Rendering was requested but skipped or failed; the existing article is kept.
    Kept { reason: String },
}

/// Entry point for `agentmark post-process`.
pub fn run_post_process(args: PostProcessArgs) -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    execute_post_process(
        &home,
        &args.id,
        args.render_article,
        &default_provider_factory,
    )?;
    Ok(())
}

/// Start `agentmark post-process` as a detached background process.
///
/// The child gets its own process group and no stdio, so it survives the
/// native host exiting when the browser closes the messaging port.
pub(crate) fn spawn_detached(id: &str, work: PostProcess) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let mut command = Command::new(exe);
    command.arg("post-process").arg(id);
    if work.render_article {
        command.arg("--render-article");
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn().map(|_child| ())
}

/// Run post-processing with an explicit home directory and provider factory.
#[instrument(skip(home, provider_factory))]
pub(crate) fn execute_post_process(
    home: &Path,
    id: &str,
    render_article: bool,
    provider_factory: &ProviderFactory,
) -> Result<ArticleOutcome, Box<dyn std::error::Error>> {
    let config = Config::load(home)?;
    if !config.enrichment.enabled {
        debug!("enrichment disabled in config — nothing to do");
        return Ok(ArticleOutcome::NotRequested);
    }

    let conn = db::open_and_migrate(&config::index_db_path(home))?;
    let repo = BookmarkRepository::new(&conn);
    let bookmark = repo
        .get_by_id(id)?
        .ok_or_else(|| format!("bookmark not found: {id}"))?;
    let bundle = Bundle::find(&config.storage_path, &bookmark.saved_at, &bookmark.id)?;

    let article_outcome = if render_article {
        match render_article_with_model(&bookmark, &bundle, &repo, &config, provider_factory) {
            Ok(()) => {
                info!("article rendered by model");
                ArticleOutcome::Rendered
            }
            Err(reason) => {
                warn!(%reason, "article rendering skipped — keeping extracted article");
                let event = BookmarkEvent::new(
                    EventType::Reprocessed,
                    serde_json::json!({ "article_source": "agent", "status": "failed", "error": reason }),
                );
                let _ = bundle.append_event(&event);
                ArticleOutcome::Kept { reason }
            }
        }
    } else {
        ArticleOutcome::NotRequested
    };

    // Enrich from whatever article the bundle now holds, on the current record.
    let article = bundle.read_article_md().unwrap_or_default();
    let mut current = repo
        .get_by_id(id)?
        .ok_or_else(|| format!("bookmark not found: {id}"))?;
    match enrich::enrich_bookmark(
        &mut current,
        &article,
        &bundle,
        &repo,
        &config,
        provider_factory,
    ) {
        EnrichOutcome::Success => info!("enrichment succeeded"),
        EnrichOutcome::Skipped { reason } => debug!(%reason, "enrichment skipped"),
        EnrichOutcome::Failed { warning } => warn!(%warning, "enrichment failed"),
    }

    Ok(article_outcome)
}

/// Have the model turn the captured page into `article.md`.
///
/// `content_hash` is deliberately left alone: it identifies the captured page
/// for dedup, so re-saving the same page does not trigger another render.
fn render_article_with_model(
    bookmark: &Bookmark,
    bundle: &Bundle,
    repo: &BookmarkRepository<'_>,
    config: &Config,
    provider_factory: &ProviderFactory,
) -> Result<(), String> {
    let source_html = bundle
        .read_source_html()
        .map_err(|e| format!("cannot read source.html: {e}"))?;
    let page_html = extract::simplify_html(&source_html);
    if page_html.is_empty() {
        return Err("captured page has no visible text".to_string());
    }
    if page_html.len() > MAX_MODEL_HTML_BYTES {
        return Err(format!(
            "captured page is too large for the model ({} bytes after simplification, max {MAX_MODEL_HTML_BYTES})",
            page_html.len()
        ));
    }
    debug!(
        source_bytes = source_html.len(),
        simplified_bytes = page_html.len(),
        "page simplified for model"
    );

    // The publicly fetched copy is optional: drop it rather than exceed the budget.
    let fetched_html = bundle
        .read_fetched_html()
        .map(|html| extract::simplify_html(&html))
        .filter(|html| !html.is_empty() && page_html.len() + html.len() <= MAX_MODEL_HTML_BYTES);
    debug!(
        fetched_bytes = fetched_html.as_ref().map(String::len),
        "public page supplement"
    );

    let provider = provider_factory(&config.default_agent, config.system_prompt.as_deref())
        .map_err(|e| format!("failed to create agent provider: {e}"))?;
    let request = ArticleRequest {
        url: bookmark.url.clone(),
        title: bookmark.title.clone(),
        page_html,
        fetched_html,
    };
    let markdown = provider
        .render_article(&request)
        .map_err(|e| format!("agent failed to render article: {e}"))?;

    bundle
        .update_article_md(&markdown)
        .map_err(|e| format!("failed to write article.md: {e}"))?;

    // The render can take over a minute; apply the result to the current record.
    // The model's title replaces the page's own, which on app-like pages is
    // generic ("Post | LinkedIn") or a dump of hashtags.
    let title = article_title(&markdown);
    if let Ok(Some(mut current)) = repo.get_by_id(&bookmark.id) {
        let mut changed = false;
        if current.content_status != ContentStatus::Extracted {
            current.content_status = ContentStatus::Extracted;
            changed = true;
        }
        if let Some(ref title) = title {
            if &current.title != title {
                current.title = title.clone();
                changed = true;
            }
        }
        if changed {
            let _ = bundle.update_bookmark_md_preserving_body(&current);
            let _ = repo.update(&current);
        }
    }

    let event = BookmarkEvent::new(
        EventType::Reprocessed,
        serde_json::json!({
            "article_source": "agent",
            "status": "success",
            "article_length": markdown.len(),
            "title": title,
        }),
    );
    let _ = bundle.append_event(&event);
    Ok(())
}

/// Longest title accepted from the model, in characters.
const MAX_TITLE_CHARS: usize = 150;

/// The title the model wrote: the article's leading level-1 heading.
///
/// Returns `None` when the document does not start with one, or when the
/// heading is empty or implausibly long, so the existing title is kept.
fn article_title(markdown: &str) -> Option<String> {
    let first_line = markdown.lines().find(|line| !line.trim().is_empty())?;
    let heading = first_line.trim().strip_prefix("# ")?;
    let title = heading
        .trim()
        .trim_end_matches('#')
        .trim()
        .trim_matches(|c| c == '*' || c == '_' || c == '`')
        .trim();
    let length = title.chars().count();
    (length > 0 && length <= MAX_TITLE_CHARS).then(|| title.to_string())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentError, AgentProvider, EnrichmentRequest, EnrichmentResponse};
    use crate::models::SummaryStatus;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const SNAPSHOT: &str = r#"<html><head><title>Ada | LinkedIn</title></head><body>
        <nav><p>Home</p></nav><main><h2 class="x">Ada Lovelace</h2><p>Analyst</p></main></body></html>"#;
    const READABILITY_ARTICLE: &str = "One featured post only.\n";
    const RENDERED: &str = "# Ada Lovelace\n\nAnalyst\n";

    /// Provider whose article rendering echoes a marker proving it saw the
    /// simplified page, and whose enrichment echoes the article it was given.
    struct FakeProvider {
        render_ok: bool,
    }

    impl AgentProvider for FakeProvider {
        fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse, AgentError> {
            Ok(EnrichmentResponse {
                summary: format!("Summary of: {}", request.article_content.trim()),
                suggested_tags: vec!["profile".to_string()],
                suggested_collection: None,
            })
        }

        fn render_article(&self, request: &ArticleRequest) -> Result<String, AgentError> {
            assert!(request.page_html.contains("<h2>Ada Lovelace</h2>"));
            assert!(
                !request.page_html.contains("class="),
                "page must be simplified"
            );
            if let Some(ref fetched) = request.fetched_html {
                // Echo the supplement so tests can see it reached the model.
                return Ok(format!("{RENDERED}\nPublic: {fetched}\n"));
            }
            if self.render_ok {
                Ok(RENDERED.to_string())
            } else {
                Err(AgentError::ProcessFailed {
                    provider: "pi",
                    status: 1,
                    stderr: "model unavailable".to_string(),
                })
            }
        }
    }

    fn ok_factory(_: &str, _: Option<&str>) -> Result<Box<dyn AgentProvider>, AgentError> {
        Ok(Box::new(FakeProvider { render_ok: true }))
    }

    fn render_fails_factory(
        _: &str,
        _: Option<&str>,
    ) -> Result<Box<dyn AgentProvider>, AgentError> {
        Ok(Box::new(FakeProvider { render_ok: false }))
    }

    struct Fixture {
        _tmp: TempDir,
        home: PathBuf,
        bundle: Bundle,
        id: String,
    }

    impl Fixture {
        fn bookmark(&self) -> Bookmark {
            let conn = db::open_and_migrate(&config::index_db_path(&self.home)).unwrap();
            BookmarkRepository::new(&conn)
                .get_by_id(&self.id)
                .unwrap()
                .unwrap()
        }
    }

    fn fixture(enrichment_enabled: bool, source_html: &str) -> Fixture {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let storage = tmp.path().join("bookmarks");
        std::fs::create_dir_all(home.join(".agentmark")).unwrap();
        std::fs::create_dir_all(&storage).unwrap();
        std::fs::write(
            home.join(".agentmark/config.toml"),
            format!(
                "default_agent = \"pi\"\nstorage_path = \"{}\"\n\n[enrichment]\nenabled = {enrichment_enabled}\n",
                storage.display()
            ),
        )
        .unwrap();

        let mut bookmark = Bookmark::new("https://www.linkedin.com/in/ada/", "Ada | LinkedIn");
        bookmark.content_status = ContentStatus::Extracted;
        bookmark.content_hash = Some("sha256:snapshot".to_string());
        let bundle = Bundle::create(
            &storage,
            &bookmark,
            &crate::fetch::PageMetadata::default(),
            READABILITY_ARTICLE,
            source_html,
            "chrome_extension",
        )
        .unwrap();

        let conn = db::open_and_migrate(&config::index_db_path(&home)).unwrap();
        BookmarkRepository::new(&conn).insert(&bookmark).unwrap();

        Fixture {
            _tmp: tmp,
            home,
            bundle,
            id: bookmark.id,
        }
    }

    #[test]
    fn renders_article_then_enriches_from_it() {
        let fx = fixture(true, SNAPSHOT);

        let outcome = execute_post_process(&fx.home, &fx.id, true, &ok_factory).unwrap();

        assert_eq!(outcome, ArticleOutcome::Rendered);
        assert_eq!(fx.bundle.read_article_md().unwrap(), RENDERED);
        let bookmark = fx.bookmark();
        assert_eq!(bookmark.summary_status, SummaryStatus::Done);
        let summary = fx
            .bundle
            .read_body_sections()
            .unwrap()
            .summary
            .unwrap_or_default();
        assert!(
            summary.contains("Summary of: # Ada Lovelace"),
            "summary should come from the rendered article: {summary}"
        );
    }

    #[test]
    fn sends_simplified_public_page_as_supplement_when_present() {
        let fx = fixture(true, SNAPSHOT);
        fx.bundle
            .update_fetched_html(Some(
                r#"<html><body><script>x()</script><p class="c">A public comment</p></body></html>"#,
            ))
            .unwrap();

        execute_post_process(&fx.home, &fx.id, true, &ok_factory).unwrap();

        let article = fx.bundle.read_article_md().unwrap();
        assert!(
            article.contains("Public: <p>A public comment</p>"),
            "{article}"
        );
    }

    #[test]
    fn uses_the_models_title_for_the_bookmark() {
        let fx = fixture(true, SNAPSHOT);
        assert_eq!(fx.bookmark().title, "Ada | LinkedIn");

        execute_post_process(&fx.home, &fx.id, true, &ok_factory).unwrap();

        assert_eq!(fx.bookmark().title, "Ada Lovelace");
        let bookmark_md = std::fs::read_to_string(fx.bundle.path().join("bookmark.md")).unwrap();
        assert!(bookmark_md.contains("title: Ada Lovelace"), "{bookmark_md}");
    }

    #[test]
    fn keeps_the_page_title_when_rendering_fails() {
        let fx = fixture(true, SNAPSHOT);
        execute_post_process(&fx.home, &fx.id, true, &render_fails_factory).unwrap();
        assert_eq!(fx.bookmark().title, "Ada | LinkedIn");
    }

    #[test]
    fn article_title_reads_the_leading_h1() {
        assert_eq!(
            article_title("# Jane Doe on seed rounds\n\nBody").as_deref(),
            Some("Jane Doe on seed rounds")
        );
        assert_eq!(
            article_title("\n\n#  **Ada Lovelace: LinkedIn profile**  #\n").as_deref(),
            Some("Ada Lovelace: LinkedIn profile")
        );
    }

    #[test]
    fn article_title_rejects_missing_empty_or_overlong_headings() {
        assert_eq!(article_title("Body first\n\n# Later heading"), None);
        assert_eq!(article_title("## Only a subheading"), None);
        assert_eq!(article_title("#NoSpace"), None);
        assert_eq!(article_title("# \n\nBody"), None);
        assert_eq!(article_title(""), None);
        assert_eq!(article_title(&format!("# {}", "x".repeat(151))), None);
    }

    #[test]
    fn keeps_content_hash_so_resaving_the_same_page_is_unchanged() {
        let fx = fixture(true, SNAPSHOT);
        execute_post_process(&fx.home, &fx.id, true, &ok_factory).unwrap();
        assert_eq!(
            fx.bookmark().content_hash.as_deref(),
            Some("sha256:snapshot")
        );
    }

    #[test]
    fn without_render_flag_only_enriches() {
        let fx = fixture(true, SNAPSHOT);

        let outcome = execute_post_process(&fx.home, &fx.id, false, &ok_factory).unwrap();

        assert_eq!(outcome, ArticleOutcome::NotRequested);
        assert_eq!(fx.bundle.read_article_md().unwrap(), READABILITY_ARTICLE);
        assert_eq!(fx.bookmark().summary_status, SummaryStatus::Done);
    }

    #[test]
    fn render_failure_keeps_extracted_article_and_still_enriches() {
        let fx = fixture(true, SNAPSHOT);

        let outcome = execute_post_process(&fx.home, &fx.id, true, &render_fails_factory).unwrap();

        assert!(
            matches!(outcome, ArticleOutcome::Kept { ref reason } if reason.contains("model unavailable"))
        );
        assert_eq!(fx.bundle.read_article_md().unwrap(), READABILITY_ARTICLE);
        assert_eq!(fx.bookmark().summary_status, SummaryStatus::Done);
    }

    #[test]
    fn page_without_text_is_not_sent_to_the_model() {
        let fx = fixture(true, "<html><body><script>x()</script></body></html>");

        let outcome = execute_post_process(&fx.home, &fx.id, true, &ok_factory).unwrap();

        assert!(
            matches!(outcome, ArticleOutcome::Kept { ref reason } if reason.contains("no visible text"))
        );
        assert_eq!(fx.bundle.read_article_md().unwrap(), READABILITY_ARTICLE);
    }

    #[test]
    fn marks_content_extracted_when_generic_extraction_had_failed() {
        let fx = fixture(true, SNAPSHOT);
        {
            let conn = db::open_and_migrate(&config::index_db_path(&fx.home)).unwrap();
            let repo = BookmarkRepository::new(&conn);
            let mut bookmark = repo.get_by_id(&fx.id).unwrap().unwrap();
            bookmark.content_status = ContentStatus::Failed;
            repo.update(&bookmark).unwrap();
        }

        execute_post_process(&fx.home, &fx.id, true, &ok_factory).unwrap();

        assert_eq!(fx.bookmark().content_status, ContentStatus::Extracted);
    }

    #[test]
    fn does_nothing_when_enrichment_is_disabled() {
        let fx = fixture(false, SNAPSHOT);

        let outcome = execute_post_process(&fx.home, &fx.id, true, &ok_factory).unwrap();

        assert_eq!(outcome, ArticleOutcome::NotRequested);
        assert_eq!(fx.bundle.read_article_md().unwrap(), READABILITY_ARTICLE);
        assert_eq!(fx.bookmark().summary_status, SummaryStatus::Pending);
    }

    #[test]
    fn unknown_bookmark_is_an_error() {
        let fx = fixture(true, SNAPSHOT);
        let err = execute_post_process(&fx.home, "am_missing", true, &ok_factory).unwrap_err();
        assert!(err.to_string().contains("bookmark not found"));
    }

    #[test]
    fn user_edits_made_during_processing_are_preserved() {
        let fx = fixture(true, SNAPSHOT);
        // Simulate the user adding a tag after the save but before the model
        // finishes: the provider factory runs after the bookmark was first loaded.
        let home = fx.home.clone();
        let tagging_factory = move |_: &str, _: Option<&str>| {
            let conn = db::open_and_migrate(&config::index_db_path(&home)).unwrap();
            let repo = BookmarkRepository::new(&conn);
            let mut bookmark = repo.list(1, 0, None, None, None).unwrap().remove(0);
            if !bookmark.user_tags.contains(&"added-later".to_string()) {
                bookmark.user_tags.push("added-later".to_string());
                repo.update(&bookmark).unwrap();
            }
            Ok(Box::new(FakeProvider { render_ok: true }) as Box<dyn AgentProvider>)
        };

        execute_post_process(&fx.home, &fx.id, true, &tagging_factory).unwrap();

        let bookmark = fx.bookmark();
        assert_eq!(bookmark.user_tags, vec!["added-later"]);
        assert_eq!(bookmark.summary_status, SummaryStatus::Done);
    }
}
