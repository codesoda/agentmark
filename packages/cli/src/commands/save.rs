//! Save command: wire fetch, extract, bundle, and DB into `agentmark save <url>`.
//!
//! Handles three paths:
//! - New save: create bundle + insert DB row
//! - Duplicate with unchanged content: merge user fields + update bundle/DB + append `resaved`
//! - Duplicate with changed content: update bundle files + update DB + append `content_updated`
//!
//! After durable save, runs best-effort enrichment via the configured agent provider.

use std::fmt;
use std::path::{Path, PathBuf};

use tracing::{debug, info, instrument, warn};

use crate::agent;
use crate::bundle::{BodySections, Bundle};
use crate::canonical;
use crate::cli::SaveArgs;
use crate::config::{self, Config, ConfigError};
use crate::db::{self, BookmarkRepository, DbError};
use crate::enrich::{self, EnrichOutcome, ProviderFactory};
use crate::extract::{self, ExtractionResult};
use crate::fetch::{self, FetchError, PageMetadata};
use crate::models::{
    Bookmark, BookmarkEvent, CaptureSource, ContentStatus, EventType, SummaryStatus,
};

// ── Public entry point ──────────────────────────────────────────────

/// Entry point for `agentmark save` using real environment.
#[instrument(skip(args), fields(url = %args.url))]
pub fn run_save(args: SaveArgs) -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    let outcome = execute_save(&home, &args)?;

    for warning in &outcome.warnings {
        eprintln!("warning: {warning}");
    }

    match outcome.dedup {
        DedupResult::New => {
            println!("Saved bookmark {}", outcome.id);
            println!("  path: {}", outcome.bundle_path.display());
        }
        DedupResult::Unchanged => {
            println!("already saved — updated existing bookmark {}", outcome.id);
            println!("  path: {}", outcome.bundle_path.display());
        }
        DedupResult::ContentChanged => {
            println!(
                "already saved — content updated, marked for re-enrichment {}",
                outcome.id
            );
            println!("  path: {}", outcome.bundle_path.display());
        }
    }

    Ok(())
}

// ── Typed outcome and errors ────────────────────────────────────────

/// Which dedup path was taken.
#[derive(Debug, PartialEq)]
pub enum DedupResult {
    New,
    Unchanged,
    ContentChanged,
}

/// Successful save result returned from the testable helper.
#[derive(Debug)]
pub struct SaveOutcome {
    pub id: String,
    pub bundle_path: PathBuf,
    pub warnings: Vec<String>,
    pub dedup: DedupResult,
}

/// Save-specific errors with pipeline stage context.
#[derive(Debug)]
pub enum SaveError {
    Config(ConfigError),
    Fetch(FetchError),
    Bundle(crate::bundle::BundleError),
    Db(DbError),
    Canonical(canonical::CanonicalError),
    /// Bundle was created/updated but DB operation failed. The bundle is preserved.
    PartialSave {
        id: String,
        bundle_path: PathBuf,
        db_error: Box<DbError>,
    },
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Config(e) => write!(f, "{e}"),
            SaveError::Fetch(e) => write!(f, "fetch failed: {e}"),
            SaveError::Bundle(e) => write!(f, "bundle creation failed: {e}"),
            SaveError::Db(e) => write!(f, "database error: {e}"),
            SaveError::Canonical(e) => write!(f, "URL error: {e}"),
            SaveError::PartialSave {
                id,
                bundle_path,
                db_error,
            } => write!(
                f,
                "bundle saved ({id} at {}) but index update failed: {db_error}",
                bundle_path.display()
            ),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<ConfigError> for SaveError {
    fn from(e: ConfigError) -> Self {
        SaveError::Config(e)
    }
}

impl From<FetchError> for SaveError {
    fn from(e: FetchError) -> Self {
        SaveError::Fetch(e)
    }
}

impl From<crate::bundle::BundleError> for SaveError {
    fn from(e: crate::bundle::BundleError) -> Self {
        SaveError::Bundle(e)
    }
}

impl From<canonical::CanonicalError> for SaveError {
    fn from(e: canonical::CanonicalError) -> Self {
        SaveError::Canonical(e)
    }
}

// ── Input normalization ─────────────────────────────────────────────

/// Parse comma-separated tags, trim each, drop empty segments.
fn parse_tags(raw: Option<&str>) -> Vec<String> {
    match raw {
        None => Vec::new(),
        Some(s) => s
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
    }
}

/// Normalize optional text: blank/whitespace-only → None.
fn normalize_optional_text(raw: Option<&str>) -> Option<String> {
    raw.map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Normalized CLI inputs parsed once and reused across all branches.
struct NormalizedInputs {
    tags: Vec<String>,
    collection: Option<String>,
    note: Option<String>,
    action: Option<String>,
}

// ── Transport-neutral save input ────────────────────────────────────

/// Transport-neutral save request shared by CLI and native-host callers.
/// Keeps the save pipeline independent of CLI arg shapes or native message schemas.
pub(crate) struct SaveRequest {
    pub url: String,
    pub tags: Vec<String>,
    pub collection: Option<String>,
    pub note: Option<String>,
    pub action: Option<String>,
    pub capture_source: CaptureSource,
    pub provided_title: Option<String>,
    pub no_enrich: bool,
}

impl SaveRequest {
    /// Adapt CLI args into a transport-neutral save request.
    pub(crate) fn from_cli(args: &SaveArgs) -> Self {
        Self {
            url: args.url.clone(),
            tags: parse_tags(args.tags.as_deref()),
            collection: normalize_optional_text(args.collection.as_deref()),
            note: normalize_optional_text(args.note.as_deref()),
            action: normalize_optional_text(args.action.as_deref()),
            capture_source: CaptureSource::Cli,
            provided_title: None,
            no_enrich: args.no_enrich,
        }
    }
}

// ── Extraction classification ───────────────────────────────────────

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

// ── Fetched page data ───────────────────────────────────────────────

/// Result of fetching + extracting a page, packaged for reuse.
struct FetchedPage {
    raw_html: String,
    metadata: PageMetadata,
    extraction: ExtractionResult,
    content_status: ContentStatus,
    extraction_warning: Option<String>,
}

#[instrument(skip(url), fields(%url))]
fn fetch_and_extract(url: &str) -> Result<FetchedPage, SaveError> {
    let (raw_html, metadata) = fetch::fetch_page(url)?;
    let extraction = extract::extract_content(&raw_html);
    let (content_status, extraction_warning) = classify_extraction(&extraction);
    debug!(content_status = ?content_status, hash = %extraction.content_hash, "fetch and extract complete");
    Ok(FetchedPage {
        raw_html,
        metadata,
        extraction,
        content_status,
        extraction_warning,
    })
}

// ── Bookmark construction ───────────────────────────────────────────

/// Build a new Bookmark from fetched metadata, extraction output, and normalized inputs.
fn build_bookmark(
    url: &str,
    canonical_url: &str,
    page: &FetchedPage,
    inputs: &NormalizedInputs,
    capture_source: CaptureSource,
    provided_title: Option<&str>,
) -> Bookmark {
    // Title priority: page metadata > caller-provided title > URL fallback
    let title = page
        .metadata
        .title
        .clone()
        .or_else(|| provided_title.map(|s| s.to_string()))
        .unwrap_or_else(|| url.to_string());

    let mut bm = Bookmark::new(url, &title);
    bm.canonical_url = canonical_url.to_string();

    // Metadata fields
    bm.description = page.metadata.description.clone();
    bm.author = page.metadata.author.clone();
    bm.site_name = page.metadata.site_name.clone();
    bm.published_at = page.metadata.published_at.clone();

    // User inputs
    bm.user_tags = inputs.tags.clone();
    if let Some(ref c) = inputs.collection {
        bm.collections = vec![c.clone()];
    }
    bm.note = inputs.note.clone();
    bm.action_prompt = inputs.action.clone();

    // Extraction
    bm.content_status = page.content_status.clone();
    bm.content_hash = Some(page.extraction.content_hash.clone());

    // Capture source
    bm.capture_source = capture_source;

    bm
}

// ── Merge helpers ───────────────────────────────────────────────────

/// Merge new tags into existing, preserving order and appending unique new values.
fn merge_tags(existing: &[String], new: &[String]) -> Vec<String> {
    let mut result = existing.to_vec();
    for tag in new {
        if !result.contains(tag) {
            result.push(tag.clone());
        }
    }
    result
}

/// Merge new collections into existing, preserving order and appending unique new values.
fn merge_collections(existing: &[String], new: &[String]) -> Vec<String> {
    merge_tags(existing, new) // same logic
}

/// Merge note: keep existing if incoming is blank; replace if different and non-empty.
fn merge_note(existing: &Option<String>, incoming: &Option<String>) -> Option<String> {
    match incoming {
        Some(new_note) if !new_note.is_empty() => Some(new_note.clone()),
        _ => existing.clone(),
    }
}

/// Merge action_prompt: prefer newest non-empty value.
fn merge_action(existing: &Option<String>, incoming: &Option<String>) -> Option<String> {
    match incoming {
        Some(new_action) if !new_action.is_empty() => Some(new_action.clone()),
        _ => existing.clone(),
    }
}

// ── Canonical URL resolution ────────────────────────────────────────

/// Determine the best canonical URL after fetch.
/// Prefers page-declared canonical if it canonicalizes successfully.
fn best_canonical_url(pre_fetch_canonical: &str, page_metadata: &PageMetadata) -> String {
    if let Some(ref page_canonical) = page_metadata.canonical_url {
        if let Ok(canonicalized) = canonical::canonicalize(page_canonical) {
            return canonicalized;
        }
    }
    pre_fetch_canonical.to_string()
}

// ── Post-save context (internal) ────────────────────────────────────

/// Internal richer context returned by save branch handlers for enrichment.
struct PostSaveContext {
    bookmark: Bookmark,
    bundle: Bundle,
    article_markdown: String,
    warnings: Vec<String>,
    dedup: DedupResult,
}

impl PostSaveContext {
    fn into_outcome(self) -> SaveOutcome {
        SaveOutcome {
            id: self.bookmark.id.clone(),
            bundle_path: self.bundle.path().to_path_buf(),
            warnings: self.warnings,
            dedup: self.dedup,
        }
    }
}

// ── Default provider factory ─────────────────────────────────────────

pub(crate) fn default_provider_factory(
    default_agent: &str,
    system_prompt: Option<&str>,
) -> Result<Box<dyn crate::agent::AgentProvider>, crate::agent::AgentError> {
    agent::create_provider(default_agent, system_prompt)
}

// ── Testable save pipeline ──────────────────────────────────────────

/// Execute the save pipeline with an explicit home directory.
/// This is the main testable seam.
pub fn execute_save(home: &Path, args: &SaveArgs) -> Result<SaveOutcome, SaveError> {
    execute_save_with_deps(home, args, &default_provider_factory)
}

/// Internal save pipeline with injectable provider factory for testing.
pub(crate) fn execute_save_with_deps(
    home: &Path,
    args: &SaveArgs,
    provider_factory: &ProviderFactory,
) -> Result<SaveOutcome, SaveError> {
    let req = SaveRequest::from_cli(args);
    execute_save_request(home, &req, provider_factory)
}

/// Transport-neutral save pipeline used by both CLI and native-host callers.
#[instrument(skip(home, req, provider_factory), fields(url = %req.url))]
pub(crate) fn execute_save_request(
    home: &Path,
    req: &SaveRequest,
    provider_factory: &ProviderFactory,
) -> Result<SaveOutcome, SaveError> {
    // 1. Load config
    let config = Config::load(home)?;
    debug!(agent = %config.default_agent, enrichment = config.enrichment.enabled, "config loaded");

    // 2. Open/migrate the SQLite index
    let db_path = config::index_db_path(home);
    let conn = db::open_and_migrate(&db_path).map_err(SaveError::Db)?;
    let repo = BookmarkRepository::new(&conn);

    // 3. Build normalized inputs from the request
    let inputs = NormalizedInputs {
        tags: req.tags.clone(),
        collection: req.collection.clone(),
        note: req.note.clone(),
        action: req.action.clone(),
    };

    // 4. Canonicalize the requested URL
    let pre_fetch_canonical = canonical::canonicalize(&req.url)?;
    debug!(canonical = %pre_fetch_canonical, "pre-fetch canonical URL");

    // 5. Initial duplicate check by canonical URL
    let initial_duplicate = repo
        .get_by_canonical_url(&pre_fetch_canonical)
        .map_err(SaveError::Db)?;
    debug!(
        found = initial_duplicate.is_some(),
        "initial duplicate check"
    );

    // 6. Fetch page and extract content
    let page = fetch_and_extract(&req.url)?;
    let mut warnings = Vec::new();
    if let Some(w) = &page.extraction_warning {
        warnings.push(w.clone());
    }

    // 7. Resolve best canonical URL after fetch (may differ from pre-fetch)
    let final_canonical = best_canonical_url(&pre_fetch_canonical, &page.metadata);
    if final_canonical != pre_fetch_canonical {
        debug!(pre = %pre_fetch_canonical, post = %final_canonical, "canonical URL changed after fetch");
    }

    // 8. If initial lookup missed, try again with post-fetch canonical
    let existing = if initial_duplicate.is_some() {
        initial_duplicate
    } else if final_canonical != pre_fetch_canonical {
        repo.get_by_canonical_url(&final_canonical)
            .map_err(SaveError::Db)?
    } else {
        None
    };

    // 9. Branch: new save vs duplicate → get post-save context
    let mut ctx = match existing {
        Some(existing_bm) => handle_duplicate(
            &config,
            &repo,
            existing_bm,
            &page,
            &inputs,
            &final_canonical,
            warnings,
        )?,
        None => handle_new_save(
            &config,
            &repo,
            &req.url,
            &final_canonical,
            &page,
            &inputs,
            req.capture_source.clone(),
            req.provided_title.as_deref(),
            warnings,
        )?,
    };

    // 10. Run enrichment (best-effort, after durable save)
    let enrich_eligible = !req.no_enrich && ctx.dedup != DedupResult::Unchanged;
    debug!(eligible = enrich_eligible, dedup = ?ctx.dedup, "enrichment decision");

    if enrich_eligible {
        let outcome = enrich::enrich_bookmark(
            &mut ctx.bookmark,
            &ctx.article_markdown,
            &ctx.bundle,
            &repo,
            &config,
            provider_factory,
        );
        match outcome {
            EnrichOutcome::Success => {
                info!(id = %ctx.bookmark.id, "enrichment succeeded");
            }
            EnrichOutcome::Skipped { ref reason } => {
                debug!(id = %ctx.bookmark.id, reason = %reason, "enrichment skipped");
            }
            EnrichOutcome::Failed { warning } => {
                warn!(id = %ctx.bookmark.id, warning = %warning, "enrichment failed");
                ctx.warnings.push(warning);
            }
        }
    }

    info!(id = %ctx.bookmark.id, dedup = ?ctx.dedup, "save complete");
    Ok(ctx.into_outcome())
}

// ── New save path ───────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
#[instrument(skip_all, fields(%url, %canonical_url))]
fn handle_new_save(
    config: &Config,
    repo: &BookmarkRepository<'_>,
    url: &str,
    canonical_url: &str,
    page: &FetchedPage,
    inputs: &NormalizedInputs,
    capture_source: CaptureSource,
    provided_title: Option<&str>,
    warnings: Vec<String>,
) -> Result<PostSaveContext, SaveError> {
    let bookmark = build_bookmark(
        url,
        canonical_url,
        page,
        inputs,
        capture_source,
        provided_title,
    );

    let capture_source_str = match bookmark.capture_source {
        CaptureSource::Cli => "cli",
        CaptureSource::ChromeExtension => "chrome_extension",
    };
    let bundle = Bundle::create(
        &config.storage_path,
        &bookmark,
        &page.metadata,
        &page.extraction.article_markdown,
        &page.raw_html,
        capture_source_str,
    )?;

    let bundle_path = bundle.path().to_path_buf();
    let id = bookmark.id.clone();

    if let Err(db_err) = repo.insert(&bookmark) {
        return Err(SaveError::PartialSave {
            id,
            bundle_path,
            db_error: Box::new(db_err),
        });
    }

    Ok(PostSaveContext {
        bookmark,
        bundle,
        article_markdown: page.extraction.article_markdown.clone(),
        warnings,
        dedup: DedupResult::New,
    })
}

// ── Duplicate save path ─────────────────────────────────────────────

#[instrument(skip_all, fields(id = %existing.id, %canonical_url))]
fn handle_duplicate(
    config: &Config,
    repo: &BookmarkRepository<'_>,
    mut existing: Bookmark,
    page: &FetchedPage,
    inputs: &NormalizedInputs,
    canonical_url: &str,
    warnings: Vec<String>,
) -> Result<PostSaveContext, SaveError> {
    let old_hash = existing.content_hash.clone();
    let new_hash = &page.extraction.content_hash;
    let content_changed = old_hash.as_deref() != Some(new_hash);
    debug!(content_changed, "duplicate detected");

    // Find existing bundle on disk
    let bundle = Bundle::find(&config.storage_path, &existing.saved_at, &existing.id)?;

    if content_changed {
        // Content changed: update bundle files, metadata, and reset enrichment
        handle_content_changed(
            repo,
            &mut existing,
            page,
            inputs,
            canonical_url,
            &bundle,
            &old_hash,
            warnings,
        )
        .map(|mut ctx| {
            ctx.bundle = bundle;
            ctx
        })
    } else {
        // Content unchanged: merge user fields only
        handle_unchanged(
            repo,
            &mut existing,
            inputs,
            canonical_url,
            &bundle,
            warnings,
        )
        .map(|mut ctx| {
            ctx.bundle = bundle;
            ctx
        })
    }
}

fn handle_unchanged(
    repo: &BookmarkRepository<'_>,
    existing: &mut Bookmark,
    inputs: &NormalizedInputs,
    canonical_url: &str,
    bundle: &Bundle,
    warnings: Vec<String>,
) -> Result<PostSaveContext, SaveError> {
    // Merge user-owned fields
    existing.user_tags = merge_tags(&existing.user_tags, &inputs.tags);
    let new_collections: Vec<String> = inputs.collection.iter().cloned().collect();
    existing.collections = merge_collections(&existing.collections, &new_collections);
    existing.note = merge_note(&existing.note, &inputs.note);
    existing.action_prompt = merge_action(&existing.action_prompt, &inputs.action);
    existing.canonical_url = canonical_url.to_string();

    // Update bundle bookmark.md preserving body sections
    bundle.update_bookmark_md_preserving_body(existing)?;

    // Append resaved event
    let event = BookmarkEvent::new(
        EventType::Resaved,
        serde_json::json!({
            "url": existing.url,
            "merged_tags": existing.user_tags,
        }),
    );
    bundle.append_event(&event)?;

    // Update DB
    let id = existing.id.clone();
    let bundle_path = bundle.path().to_path_buf();
    match repo.update(existing) {
        Err(db_err) => {
            return Err(SaveError::PartialSave {
                id,
                bundle_path,
                db_error: Box::new(db_err),
            });
        }
        Ok(false) => {
            return Err(SaveError::PartialSave {
                id: id.clone(),
                bundle_path,
                db_error: Box::new(DbError::NotFound { id }),
            });
        }
        Ok(true) => {}
    }

    Ok(PostSaveContext {
        bookmark: existing.clone(),
        bundle: Bundle::open(bundle.path().to_path_buf())?,
        article_markdown: String::new(), // not needed for unchanged
        warnings,
        dedup: DedupResult::Unchanged,
    })
}

#[allow(clippy::too_many_arguments)]
fn handle_content_changed(
    repo: &BookmarkRepository<'_>,
    existing: &mut Bookmark,
    page: &FetchedPage,
    inputs: &NormalizedInputs,
    canonical_url: &str,
    bundle: &Bundle,
    old_hash: &Option<String>,
    warnings: Vec<String>,
) -> Result<PostSaveContext, SaveError> {
    // Update metadata from fresh fetch
    if let Some(ref title) = page.metadata.title {
        existing.title = title.clone();
    }
    existing.description = page.metadata.description.clone();
    existing.author = page.metadata.author.clone();
    existing.site_name = page.metadata.site_name.clone();
    existing.published_at = page.metadata.published_at.clone();
    existing.canonical_url = canonical_url.to_string();

    // Update content fields
    existing.content_hash = Some(page.extraction.content_hash.clone());
    existing.content_status = page.content_status.clone();

    // Reset enrichment state
    existing.summary_status = SummaryStatus::Pending;
    existing.suggested_tags = Vec::new();

    // Merge user-owned fields
    existing.user_tags = merge_tags(&existing.user_tags, &inputs.tags);
    let new_collections: Vec<String> = inputs.collection.iter().cloned().collect();
    existing.collections = merge_collections(&existing.collections, &new_collections);
    existing.note = merge_note(&existing.note, &inputs.note);
    existing.action_prompt = merge_action(&existing.action_prompt, &inputs.action);

    // Update bundle capture files
    bundle.update_article_md(&page.extraction.article_markdown)?;
    bundle.update_metadata_json(&page.metadata)?;
    bundle.update_source_html(&page.raw_html)?;

    // Clear stale enrichment body sections when content changes.
    // Use fresh BodySections::default() instead of preserving old body,
    // so stale summaries don't survive if re-enrichment fails or is skipped.
    let sections = BodySections::default();
    bundle.update_bookmark_md(existing, &sections)?;

    // Append content_updated event with old/new hashes
    let event = BookmarkEvent::new(
        EventType::ContentUpdated,
        serde_json::json!({
            "old_hash": old_hash,
            "new_hash": page.extraction.content_hash,
            "url": existing.url,
        }),
    );
    bundle.append_event(&event)?;

    // Update DB
    let id = existing.id.clone();
    let bundle_path = bundle.path().to_path_buf();
    match repo.update(existing) {
        Err(db_err) => {
            return Err(SaveError::PartialSave {
                id,
                bundle_path,
                db_error: Box::new(db_err),
            });
        }
        Ok(false) => {
            return Err(SaveError::PartialSave {
                id: id.clone(),
                bundle_path,
                db_error: Box::new(DbError::NotFound { id }),
            });
        }
        Ok(true) => {}
    }

    // Clear DB summary when content changes (stale summary must not survive)
    let _ = repo.set_summary(&existing.id, "");

    Ok(PostSaveContext {
        bookmark: existing.clone(),
        bundle: Bundle::open(bundle.path().to_path_buf())?,
        article_markdown: page.extraction.article_markdown.clone(),
        warnings,
        dedup: DedupResult::ContentChanged,
    })
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_tags ──────────────────────────────────────────────────

    #[test]
    fn parse_tags_none_returns_empty() {
        assert!(parse_tags(None).is_empty());
    }

    #[test]
    fn parse_tags_simple() {
        assert_eq!(parse_tags(Some("rust,cli")), vec!["rust", "cli"]);
    }

    #[test]
    fn parse_tags_trims_whitespace() {
        assert_eq!(
            parse_tags(Some(" rust , cli , web ")),
            vec!["rust", "cli", "web"]
        );
    }

    #[test]
    fn parse_tags_drops_empty_segments() {
        assert_eq!(parse_tags(Some(",,rust, cli ,,")), vec!["rust", "cli"]);
    }

    #[test]
    fn parse_tags_all_empty() {
        assert!(parse_tags(Some(",,,")).is_empty());
    }

    #[test]
    fn parse_tags_preserves_order() {
        assert_eq!(
            parse_tags(Some("beta,alpha,gamma")),
            vec!["beta", "alpha", "gamma"]
        );
    }

    // ── normalize_optional_text ─────────────────────────────────────

    #[test]
    fn normalize_none_returns_none() {
        assert_eq!(normalize_optional_text(None), None);
    }

    #[test]
    fn normalize_blank_returns_none() {
        assert_eq!(normalize_optional_text(Some("")), None);
    }

    #[test]
    fn normalize_whitespace_only_returns_none() {
        assert_eq!(normalize_optional_text(Some("   ")), None);
    }

    #[test]
    fn normalize_trims_and_returns_some() {
        assert_eq!(
            normalize_optional_text(Some("  hello  ")),
            Some("hello".to_string())
        );
    }

    // ── classify_extraction ─────────────────────────────────────────

    #[test]
    fn classify_nonempty_extraction_is_extracted() {
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
    fn classify_empty_extraction_is_failed_with_warning() {
        let result = ExtractionResult {
            article_html: String::new(),
            article_markdown: String::new(),
            content_hash: "sha256:abc".to_string(),
        };
        let (status, warning) = classify_extraction(&result);
        assert_eq!(status, ContentStatus::Failed);
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("no readable text"));
    }

    #[test]
    fn classify_whitespace_only_extraction_is_failed() {
        let result = ExtractionResult {
            article_html: "  ".to_string(),
            article_markdown: "  \n  ".to_string(),
            content_hash: "sha256:abc".to_string(),
        };
        let (status, _) = classify_extraction(&result);
        assert_eq!(status, ContentStatus::Failed);
    }

    // ── merge helpers ───────────────────────────────────────────────

    #[test]
    fn merge_tags_appends_unique() {
        let existing = vec!["rust".to_string(), "web".to_string()];
        let new = vec!["web".to_string(), "cli".to_string()];
        assert_eq!(merge_tags(&existing, &new), vec!["rust", "web", "cli"]);
    }

    #[test]
    fn merge_tags_preserves_order() {
        let existing = vec!["b".to_string(), "a".to_string()];
        let new = vec!["c".to_string()];
        assert_eq!(merge_tags(&existing, &new), vec!["b", "a", "c"]);
    }

    #[test]
    fn merge_tags_no_duplicates_from_same_input() {
        let existing = vec!["rust".to_string()];
        let new = vec!["rust".to_string()];
        assert_eq!(merge_tags(&existing, &new), vec!["rust"]);
    }

    #[test]
    fn merge_tags_empty_inputs() {
        assert!(merge_tags(&[], &[]).is_empty());
    }

    #[test]
    fn merge_note_keeps_existing_when_incoming_blank() {
        let existing = Some("old note".to_string());
        assert_eq!(merge_note(&existing, &None), Some("old note".to_string()));
    }

    #[test]
    fn merge_note_replaces_with_incoming() {
        let existing = Some("old note".to_string());
        let incoming = Some("new note".to_string());
        assert_eq!(
            merge_note(&existing, &incoming),
            Some("new note".to_string())
        );
    }

    #[test]
    fn merge_note_both_none() {
        assert_eq!(merge_note(&None, &None), None);
    }

    #[test]
    fn merge_action_prefers_newest() {
        let existing = Some("old action".to_string());
        let incoming = Some("new action".to_string());
        assert_eq!(
            merge_action(&existing, &incoming),
            Some("new action".to_string())
        );
    }

    #[test]
    fn merge_action_keeps_existing_when_incoming_none() {
        let existing = Some("action".to_string());
        assert_eq!(merge_action(&existing, &None), Some("action".to_string()));
    }

    // ── build_bookmark ──────────────────────────────────────────────

    #[test]
    fn build_bookmark_basic_fields() {
        let metadata = PageMetadata {
            title: Some("Test Title".to_string()),
            canonical_url: Some("https://example.com/canonical".to_string()),
            description: Some("A description".to_string()),
            author: Some("Author".to_string()),
            site_name: Some("Example".to_string()),
            published_at: Some("2026-01-01".to_string()),
            ..Default::default()
        };
        let extraction = ExtractionResult {
            article_html: "<p>content</p>".to_string(),
            article_markdown: "content".to_string(),
            content_hash: "sha256:abc123".to_string(),
        };
        let page = FetchedPage {
            raw_html: String::new(),
            metadata,
            extraction,
            content_status: ContentStatus::Extracted,
            extraction_warning: None,
        };
        let inputs = NormalizedInputs {
            tags: vec!["rust".to_string()],
            collection: Some("dev".to_string()),
            note: Some("good read".to_string()),
            action: Some("review".to_string()),
        };

        let bm = build_bookmark(
            "https://example.com/page",
            "https://example.com/page",
            &page,
            &inputs,
            CaptureSource::Cli,
            None,
        );

        assert!(bm.id.starts_with("am_"));
        assert_eq!(bm.url, "https://example.com/page");
        assert_eq!(bm.title, "Test Title");
        assert_eq!(bm.description.as_deref(), Some("A description"));
        assert_eq!(bm.author.as_deref(), Some("Author"));
        assert_eq!(bm.site_name.as_deref(), Some("Example"));
        assert_eq!(bm.published_at.as_deref(), Some("2026-01-01"));
        assert_eq!(bm.user_tags, vec!["rust"]);
        assert_eq!(bm.collections, vec!["dev"]);
        assert_eq!(bm.note.as_deref(), Some("good read"));
        assert_eq!(bm.action_prompt.as_deref(), Some("review"));
        assert_eq!(bm.capture_source, CaptureSource::Cli);
        assert_eq!(bm.content_status, ContentStatus::Extracted);
        assert_eq!(bm.content_hash.as_deref(), Some("sha256:abc123"));
    }

    #[test]
    fn build_bookmark_missing_title_falls_back_to_url() {
        let page = FetchedPage {
            raw_html: String::new(),
            metadata: PageMetadata::default(),
            extraction: ExtractionResult {
                article_html: String::new(),
                article_markdown: String::new(),
                content_hash: "sha256:empty".to_string(),
            },
            content_status: ContentStatus::Failed,
            extraction_warning: None,
        };
        let inputs = NormalizedInputs {
            tags: Vec::new(),
            collection: None,
            note: None,
            action: None,
        };

        let bm = build_bookmark(
            "https://example.com/page",
            "https://example.com/page",
            &page,
            &inputs,
            CaptureSource::Cli,
            None,
        );
        assert_eq!(bm.title, "https://example.com/page");
    }

    #[test]
    fn build_bookmark_summary_status_is_pending() {
        let page = FetchedPage {
            raw_html: String::new(),
            metadata: PageMetadata::default(),
            extraction: ExtractionResult {
                article_html: String::new(),
                article_markdown: String::new(),
                content_hash: "sha256:x".to_string(),
            },
            content_status: ContentStatus::Failed,
            extraction_warning: None,
        };
        let inputs = NormalizedInputs {
            tags: Vec::new(),
            collection: None,
            note: None,
            action: None,
        };

        let bm = build_bookmark(
            "https://example.com",
            "https://example.com/",
            &page,
            &inputs,
            CaptureSource::Cli,
            None,
        );
        assert_eq!(bm.summary_status, SummaryStatus::Pending);
    }

    // ── SaveError display ───────────────────────────────────────────

    #[test]
    fn save_error_config_display() {
        let err = SaveError::Config(ConfigError::HomeMissing);
        assert!(err.to_string().contains("HOME"));
    }

    #[test]
    fn save_error_fetch_display() {
        let err = SaveError::Fetch(FetchError::InvalidUrl {
            url: "bad".to_string(),
            reason: "nope".to_string(),
        });
        assert!(err.to_string().contains("fetch failed"));
    }

    #[test]
    fn save_error_partial_save_display() {
        let err = SaveError::PartialSave {
            id: "am_123".to_string(),
            bundle_path: PathBuf::from("/tmp/bundle"),
            db_error: Box::new(DbError::Migration("test".to_string())),
        };
        let msg = err.to_string();
        assert!(msg.contains("am_123"));
        assert!(msg.contains("/tmp/bundle"));
        assert!(msg.contains("index update failed"));
    }

    // ── best_canonical_url ──────────────────────────────────────────

    #[test]
    fn best_canonical_prefers_page_declared() {
        let meta = PageMetadata {
            canonical_url: Some("https://example.com/real-page".to_string()),
            ..Default::default()
        };
        let result = best_canonical_url("https://example.com/redirect", &meta);
        assert_eq!(result, "https://example.com/real-page");
    }

    #[test]
    fn best_canonical_falls_back_to_pre_fetch() {
        let meta = PageMetadata::default();
        let result = best_canonical_url("https://example.com/page", &meta);
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn best_canonical_ignores_malformed_page_canonical() {
        let meta = PageMetadata {
            canonical_url: Some("not a valid url".to_string()),
            ..Default::default()
        };
        let result = best_canonical_url("https://example.com/page", &meta);
        assert_eq!(result, "https://example.com/page");
    }

    // ── PartialSave on update returning false ─────────────────────────

    #[test]
    fn partial_save_error_includes_not_found_id() {
        let err = SaveError::PartialSave {
            id: "am_test123".to_string(),
            bundle_path: PathBuf::from("/tmp/bundle"),
            db_error: Box::new(DbError::NotFound {
                id: "am_test123".to_string(),
            }),
        };
        let msg = err.to_string();
        assert!(msg.contains("am_test123"), "should include bookmark ID");
        assert!(
            msg.contains("index update failed"),
            "should mention index update failure"
        );
        assert!(msg.contains("not found"), "should mention row not found");
    }

    // ── build_bookmark capture source and provided title ─────────────

    #[test]
    fn build_bookmark_chrome_extension_capture_source() {
        let page = FetchedPage {
            raw_html: String::new(),
            metadata: PageMetadata {
                title: Some("Page Title".to_string()),
                ..Default::default()
            },
            extraction: ExtractionResult {
                article_html: String::new(),
                article_markdown: String::new(),
                content_hash: "sha256:x".to_string(),
            },
            content_status: ContentStatus::Failed,
            extraction_warning: None,
        };
        let inputs = NormalizedInputs {
            tags: Vec::new(),
            collection: None,
            note: None,
            action: None,
        };

        let bm = build_bookmark(
            "https://example.com",
            "https://example.com/",
            &page,
            &inputs,
            CaptureSource::ChromeExtension,
            None,
        );
        assert_eq!(bm.capture_source, CaptureSource::ChromeExtension);
    }

    #[test]
    fn build_bookmark_provided_title_used_when_no_metadata_title() {
        let page = FetchedPage {
            raw_html: String::new(),
            metadata: PageMetadata::default(),
            extraction: ExtractionResult {
                article_html: String::new(),
                article_markdown: String::new(),
                content_hash: "sha256:x".to_string(),
            },
            content_status: ContentStatus::Failed,
            extraction_warning: None,
        };
        let inputs = NormalizedInputs {
            tags: Vec::new(),
            collection: None,
            note: None,
            action: None,
        };

        let bm = build_bookmark(
            "https://example.com",
            "https://example.com/",
            &page,
            &inputs,
            CaptureSource::ChromeExtension,
            Some("Extension Title"),
        );
        assert_eq!(bm.title, "Extension Title");
    }

    #[test]
    fn build_bookmark_metadata_title_takes_priority_over_provided() {
        let page = FetchedPage {
            raw_html: String::new(),
            metadata: PageMetadata {
                title: Some("Metadata Title".to_string()),
                ..Default::default()
            },
            extraction: ExtractionResult {
                article_html: String::new(),
                article_markdown: String::new(),
                content_hash: "sha256:x".to_string(),
            },
            content_status: ContentStatus::Failed,
            extraction_warning: None,
        };
        let inputs = NormalizedInputs {
            tags: Vec::new(),
            collection: None,
            note: None,
            action: None,
        };

        let bm = build_bookmark(
            "https://example.com",
            "https://example.com/",
            &page,
            &inputs,
            CaptureSource::ChromeExtension,
            Some("Extension Title"),
        );
        assert_eq!(bm.title, "Metadata Title");
    }
}
