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
use crate::agent::provider::RealRunner;
use crate::bundle::{BodySections, Bundle, BundleError};
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
use crate::snapshot_rules;
use crate::youtube;

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
    /// Set when model work was deferred instead of run during the save.
    /// The caller is responsible for starting it (see `commands::post_process`).
    pub post_process: Option<PostProcess>,
}

/// Model work left to do after a save that deferred it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PostProcess {
    /// The content came from a browser snapshot, where generic article
    /// extraction is unreliable: have the model render `article.md` from it.
    pub render_article: bool,
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
    /// HTML snapshot of the page as rendered in the caller's browser.
    /// Used only when the server-side fetch fails (login walls, bot blocking).
    pub page_html: Option<String>,
    pub no_enrich: bool,
    /// Return as soon as the bookmark is durable and report the remaining model
    /// work in `SaveOutcome::post_process` instead of running it inline.
    /// Interactive callers use this so a slow model never blocks the save.
    pub defer_enrichment: bool,
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
            page_html: None,
            no_enrich: args.no_enrich,
            defer_enrichment: false,
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

/// Where the page content of a save came from.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PageSource {
    /// Fetched over HTTP by the CLI.
    Fetched,
    /// HTML snapshot supplied by the caller's browser after the fetch failed.
    Snapshot,
    /// Fetch failed and no snapshot was supplied — there is no page content.
    Unavailable,
}

/// Result of fetching + extracting a page, packaged for reuse.
struct FetchedPage {
    source: PageSource,
    /// The page as the CLI fetched it, when `raw_html` is a browser snapshot
    /// and the fetch also succeeded. Supplementary content for the agent.
    fetched_html: Option<String>,
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
        source: PageSource::Fetched,
        fetched_html: None,
        raw_html,
        metadata,
        extraction,
        content_status,
        extraction_warning,
    })
}

/// HTTP statuses that mean "the page exists but refuses automated clients"
/// (e.g. LinkedIn answers 999 to anything that is not a logged-in browser).
fn is_access_blocked_status(status: u16) -> bool {
    matches!(status, 401 | 403 | 429 | 999)
}

/// Decide whether a failed fetch may still produce a bookmark.
///
/// - Malformed/unsupported URLs never fall back.
/// - Browser captures always fall back: the user is looking at the page, so
///   the URL is known to be real even if the CLI cannot reach it.
/// - CLI saves fall back only when the server actively blocks automated
///   clients; other failures (404, 5xx, network) stay hard errors so typos
///   and transient outages are not silently bookmarked.
fn fetch_fallback_allowed(err: &FetchError, req: &SaveRequest) -> bool {
    match err {
        FetchError::InvalidUrl { .. } | FetchError::UnsupportedScheme { .. } => false,
        _ if req.page_html.is_some() || req.capture_source == CaptureSource::ChromeExtension => {
            true
        }
        FetchError::HttpStatus { status, .. } => is_access_blocked_status(*status),
        _ => false,
    }
}

/// Build a page from a browser-supplied HTML snapshot.
fn page_from_snapshot(url: &str, html: &str) -> Option<FetchedPage> {
    if html.trim().is_empty() {
        return None;
    }
    let base_url = url::Url::parse(url).ok()?;
    let metadata = fetch::metadata::extract_metadata(html, &base_url);
    let mut extraction = extract::extract_content(html);
    let (content_status, extraction_warning) = classify_extraction(&extraction);
    // The extracted article is provisional (the agent renders the real one) and
    // covers only part of the page, so it must not decide whether the page changed.
    extraction.content_hash = extract::snapshot_hash(html);
    Some(FetchedPage {
        source: PageSource::Snapshot,
        fetched_html: None,
        raw_html: html.to_string(),
        metadata,
        extraction,
        content_status,
        extraction_warning,
    })
}

/// Longest video description kept as the bookmark's description, in characters.
const VIDEO_DESCRIPTION_CHARS: usize = 300;

/// Build a page from a captured video: the rendered transcript is the article.
fn page_from_video(video: &youtube::Video) -> FetchedPage {
    let article_markdown = youtube::render_markdown(video);
    let description = video.description.as_deref().map(|full| {
        let first_paragraph = full.split("\n\n").next().unwrap_or(full).trim();
        first_paragraph
            .chars()
            .take(VIDEO_DESCRIPTION_CHARS)
            .collect::<String>()
    });
    let extraction = ExtractionResult {
        article_html: String::new(),
        content_hash: extract::hash_text(&article_markdown),
        article_markdown,
    };
    FetchedPage {
        source: PageSource::Fetched,
        fetched_html: None,
        raw_html: String::new(),
        metadata: PageMetadata {
            title: Some(video.title.clone()),
            description,
            author: video.channel.clone(),
            site_name: Some("YouTube".to_string()),
            published_at: video.published.clone(),
            canonical_url: Some(video.url.clone()),
            favicon_url: None,
            hero_image_url: video.thumbnail_url.clone(),
        },
        extraction,
        content_status: ContentStatus::Extracted,
        extraction_warning: None,
    }
}

/// Build a content-less page so the bookmark itself can still be saved.
fn unavailable_page() -> FetchedPage {
    FetchedPage {
        source: PageSource::Unavailable,
        fetched_html: None,
        raw_html: String::new(),
        metadata: PageMetadata::default(),
        extraction: extract::extract_content(""),
        content_status: ContentStatus::Failed,
        extraction_warning: None,
    }
}

/// Fetch and extract the page, degrading to a browser snapshot or a
/// content-less bookmark when the fetch fails in a recoverable way.
/// Returns the page plus an optional warning describing the fallback.
fn acquire_page(
    req: &SaveRequest,
    config: &Config,
) -> Result<(FetchedPage, Option<String>), SaveError> {
    // A video has no article to extract: capture its description and transcript.
    if let Some(video_id) = youtube::video_id(&req.url) {
        match youtube::fetch_video(&video_id, &RealRunner) {
            Ok(video) => return Ok((page_from_video(&video), None)),
            // yt-dlp missing or failing: save the page the ordinary way.
            Err(e) => warn!(error = %e, "video capture unavailable — saving as a normal page"),
        }
    }

    // Sites that show a logged-in browser more than an anonymous client: the
    // snapshot is the content, and the fetch only adds what it can.
    if snapshot_rules::prefers_browser_snapshot(&req.url, &config.snapshot_sites) {
        let snapshot = req
            .page_html
            .as_deref()
            .and_then(|html| page_from_snapshot(&req.url, html));
        if let Some(mut page) = snapshot {
            match fetch::fetch_page_with_final_url(&req.url) {
                Ok((fetched_html, fetched_metadata, final_url))
                    if landed_on_requested_page(&req.url, &final_url)
                        && !looks_like_login_wall(&req.url, &fetched_metadata)
                        && declares_its_own_identity(&fetched_metadata) =>
                {
                    debug!("snapshot preferred — fetched page kept as supplement");
                    page.metadata = prefer_metadata(fetched_metadata, page.metadata);
                    page.fetched_html = Some(fetched_html);
                }
                Ok((_, _, final_url)) => {
                    // Typically a login wall or an app shell (HTTP 200, a bare
                    // site-name title, no canonical URL). It does not describe
                    // this page, so none of it may be used.
                    debug!(%final_url, "snapshot preferred — fetch did not return this page, ignoring it");
                }
                Err(e) => debug!(error = %e, "snapshot preferred — fetch unavailable"),
            }
            return Ok((page, None));
        }
    }

    let err = match fetch_and_extract(&req.url) {
        Ok(page) => return Ok((page, None)),
        Err(SaveError::Fetch(e)) if fetch_fallback_allowed(&e, req) => e,
        Err(e) => return Err(e),
    };

    let snapshot = req
        .page_html
        .as_deref()
        .and_then(|html| page_from_snapshot(&req.url, html));
    match snapshot {
        Some(page) => {
            warn!(error = %err, "fetch failed — using browser snapshot");
            Ok((
                page,
                Some(format!("fetch failed ({err}); saved from browser snapshot")),
            ))
        }
        None => {
            warn!(error = %err, "fetch failed — saving without page content");
            Ok((
                unavailable_page(),
                Some(format!("fetch failed ({err}); saved without page content")),
            ))
        }
    }
}

/// Whether a fetch ended on the page that was asked for.
///
/// Redirects that keep the path (a regional subdomain, `http` → `https`, a
/// trailing slash) are the same page. A different path is a different page.
fn landed_on_requested_page(requested: &str, final_url: &url::Url) -> bool {
    let Ok(requested) = url::Url::parse(requested) else {
        return false;
    };
    requested.path().trim_end_matches('/') == final_url.path().trim_end_matches('/')
}

/// Whether a fetched page is a login or sign-up wall served in place of the
/// requested page (some sites answer HTTP 200 on the original URL).
///
/// Signals: the page declares the site root as its canonical URL although a
/// deeper page was requested, or its title is a login prompt. A false positive
/// only costs the supplementary copy, so the check errs on the side of dropping.
fn looks_like_login_wall(requested: &str, metadata: &PageMetadata) -> bool {
    let requested_is_root = url::Url::parse(requested)
        .map(|u| u.path().trim_matches('/').is_empty())
        .unwrap_or(false);
    let canonical_is_root = metadata
        .canonical_url
        .as_deref()
        .and_then(|c| url::Url::parse(c).ok())
        .is_some_and(|u| u.path().trim_matches('/').is_empty());
    if canonical_is_root && !requested_is_root {
        return true;
    }

    let title = metadata.title.as_deref().unwrap_or("").to_lowercase();
    ["log in", "login", "sign in", "sign up", "signin", "signup"]
        .iter()
        .any(|needle| title.starts_with(needle) || title.contains(&format!(" {needle}")))
}

/// Whether a fetched page shows it is a real document rather than an app shell.
///
/// Single-page apps answer an anonymous client with HTTP 200 and a generic
/// shell titled with the bare site name. A page that declares a canonical URL
/// (`<link rel="canonical">` or `og:url`) names the document it is, so its
/// metadata can be trusted over the snapshot's. Without one, the snapshot the
/// user is looking at is the better source.
fn declares_its_own_identity(metadata: &PageMetadata) -> bool {
    metadata
        .canonical_url
        .as_deref()
        .is_some_and(|c| !c.trim().is_empty())
}

/// Merge page metadata field by field, taking `primary` where it has a value.
///
/// The publicly fetched page usually carries better metadata (canonical URL,
/// Open Graph tags) than a logged-in app page does.
fn prefer_metadata(primary: PageMetadata, fallback: PageMetadata) -> PageMetadata {
    PageMetadata {
        title: primary.title.or(fallback.title),
        description: primary.description.or(fallback.description),
        author: primary.author.or(fallback.author),
        site_name: primary.site_name.or(fallback.site_name),
        published_at: primary.published_at.or(fallback.published_at),
        canonical_url: primary.canonical_url.or(fallback.canonical_url),
        favicon_url: primary.favicon_url.or(fallback.favicon_url),
        hero_image_url: primary.hero_image_url.or(fallback.hero_image_url),
    }
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
    fn into_outcome(self, post_process: Option<PostProcess>) -> SaveOutcome {
        SaveOutcome {
            id: self.bookmark.id.clone(),
            bundle_path: self.bundle.path().to_path_buf(),
            warnings: self.warnings,
            dedup: self.dedup,
            post_process,
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
    let (page, fetch_warning) = acquire_page(req, &config)?;
    let mut warnings = Vec::new();
    warnings.extend(fetch_warning);
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

    // 10. Remember the requested URL too, so lookups by the URL the user
    // actually visited (e.g. a feed link whose page declares another canonical)
    // find this bookmark. insert/update already recorded the final canonical.
    // The bookmark is durably saved by now, so a failure here is only a warning.
    if let Err(e) = repo.add_url_alias(&ctx.bookmark.id, &pre_fetch_canonical) {
        warn!(id = %ctx.bookmark.id, error = %e, "failed to record requested URL alias");
        ctx.warnings
            .push(format!("failed to record requested URL alias: {e}"));
    }

    // 11. Run enrichment (best-effort, after durable save)
    let enrich_eligible = !req.no_enrich && ctx.dedup != DedupResult::Unchanged;
    debug!(eligible = enrich_eligible, dedup = ?ctx.dedup, "enrichment decision");

    if enrich_eligible && req.defer_enrichment {
        let post_process = config.enrichment.enabled.then_some(PostProcess {
            render_article: page.source == PageSource::Snapshot,
        });
        info!(id = %ctx.bookmark.id, dedup = ?ctx.dedup, ?post_process, "save complete, model work deferred");
        return Ok(ctx.into_outcome(post_process));
    }

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
    Ok(ctx.into_outcome(None))
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

    let bundle = Bundle::create(
        &config.storage_path,
        &bookmark,
        &page.metadata,
        &page.extraction.article_markdown,
        &page.raw_html,
        capture_source_name(&bookmark.capture_source),
    )?;

    bundle.update_fetched_html(page.fetched_html.as_deref())?;

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

/// The capture source as written in `bookmark.md` front matter.
fn capture_source_name(source: &CaptureSource) -> &'static str {
    match source {
        CaptureSource::Cli => "cli",
        CaptureSource::ChromeExtension => "chrome_extension",
    }
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
    // A save with no page content must never overwrite a previous capture.
    let mut content_changed =
        page.source != PageSource::Unavailable && old_hash.as_deref() != Some(new_hash);
    debug!(content_changed, "duplicate detected");

    // Find existing bundle on disk. After the storage folder changes, an older
    // bookmark's files stay in the previous folder: write a fresh bundle here,
    // keeping the bookmark's id, date and user-owned fields from the index.
    let bundle = match Bundle::find(&config.storage_path, &existing.saved_at, &existing.id) {
        Ok(bundle) => bundle,
        Err(BundleError::BundleNotFound { path }) => {
            warn!(missing = %path.display(), "bundle not in the storage folder — writing a new one");
            content_changed = true;
            Bundle::create(
                &config.storage_path,
                &existing,
                &page.metadata,
                &page.extraction.article_markdown,
                &page.raw_html,
                capture_source_name(&existing.capture_source),
            )?
        }
        Err(e) => return Err(e.into()),
    };

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
    bundle.update_fetched_html(page.fetched_html.as_deref())?;

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
            source: PageSource::Fetched,
            fetched_html: None,
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
            source: PageSource::Fetched,
            fetched_html: None,
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
            source: PageSource::Fetched,
            fetched_html: None,
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
            source: PageSource::Fetched,
            fetched_html: None,
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
            source: PageSource::Fetched,
            fetched_html: None,
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
            source: PageSource::Fetched,
            fetched_html: None,
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

    // ── fetch fallback policy ───────────────────────────────────────

    fn fallback_req(capture_source: CaptureSource, page_html: Option<&str>) -> SaveRequest {
        SaveRequest {
            url: "https://example.com".to_string(),
            tags: Vec::new(),
            collection: None,
            note: None,
            action: None,
            capture_source,
            provided_title: None,
            page_html: page_html.map(str::to_string),
            no_enrich: true,
            defer_enrichment: false,
        }
    }

    fn http_status(status: u16) -> FetchError {
        FetchError::HttpStatus {
            status,
            url: "https://example.com".to_string(),
        }
    }

    #[test]
    fn cli_falls_back_only_on_access_blocked_statuses() {
        let req = fallback_req(CaptureSource::Cli, None);
        assert!(fetch_fallback_allowed(&http_status(999), &req));
        assert!(fetch_fallback_allowed(&http_status(403), &req));
        assert!(!fetch_fallback_allowed(&http_status(404), &req));
        assert!(!fetch_fallback_allowed(&http_status(500), &req));
        assert!(!fetch_fallback_allowed(
            &FetchError::Timeout {
                url: "https://example.com".to_string()
            },
            &req
        ));
    }

    #[test]
    fn extension_falls_back_on_any_network_failure() {
        let req = fallback_req(CaptureSource::ChromeExtension, None);
        assert!(fetch_fallback_allowed(&http_status(404), &req));
        assert!(fetch_fallback_allowed(
            &FetchError::Timeout {
                url: "https://example.com".to_string()
            },
            &req
        ));
    }

    #[test]
    fn bad_urls_never_fall_back() {
        let req = fallback_req(CaptureSource::ChromeExtension, Some("<html></html>"));
        assert!(!fetch_fallback_allowed(
            &FetchError::InvalidUrl {
                url: "bad".to_string(),
                reason: "nope".to_string()
            },
            &req
        ));
    }

    #[test]
    fn same_path_counts_as_the_requested_page() {
        let landed = |requested: &str, final_url: &str| {
            landed_on_requested_page(requested, &url::Url::parse(final_url).unwrap())
        };
        assert!(landed(
            "https://www.linkedin.com/company/acme/",
            "https://au.linkedin.com/company/acme"
        ));
        assert!(landed(
            "http://example.com/posts/a?utm_source=x",
            "https://example.com/posts/a/"
        ));
        assert!(!landed(
            "https://www.linkedin.com/company/acme/about/",
            "https://www.linkedin.com/login"
        ));
        assert!(!landed(
            "https://www.linkedin.com/feed/",
            "https://www.linkedin.com/"
        ));
        assert!(!landed("not a url", "https://example.com/"));
    }

    #[test]
    fn login_walls_are_recognised_by_root_canonical_or_title() {
        let meta = |title: &str, canonical: Option<&str>| PageMetadata {
            title: Some(title.to_string()),
            canonical_url: canonical.map(str::to_string),
            ..Default::default()
        };
        let feed = "https://www.linkedin.com/feed/foryou/";
        assert!(looks_like_login_wall(
            feed,
            &meta("Log In or Sign Up", Some("https://www.linkedin.com/"))
        ));
        assert!(looks_like_login_wall(
            feed,
            &meta("Welcome", Some("https://www.linkedin.com/"))
        ));
        assert!(looks_like_login_wall(
            feed,
            &meta("LinkedIn Login, Sign in | LinkedIn", None)
        ));
    }

    #[test]
    fn real_pages_are_not_login_walls() {
        let post = "https://www.linkedin.com/feed/update/urn:li:activity:1/";
        let meta = PageMetadata {
            title: Some("#agenticai #canva | Nathan Challen".to_string()),
            canonical_url: Some(
                "https://www.linkedin.com/posts/nathan_activity-1-TUUQ".to_string(),
            ),
            ..Default::default()
        };
        assert!(!looks_like_login_wall(post, &meta));
        // Requesting the site root itself: a root canonical is expected.
        let home = PageMetadata {
            title: Some("Example".to_string()),
            canonical_url: Some("https://example.com/".to_string()),
            ..Default::default()
        };
        assert!(!looks_like_login_wall("https://example.com/", &home));
        assert!(!looks_like_login_wall(post, &PageMetadata::default()));
    }

    #[test]
    fn video_page_uses_transcript_as_article_and_video_metadata() {
        let video = youtube::Video {
            id: "zjkBMFhNj_g".to_string(),
            url: youtube::watch_url("zjkBMFhNj_g"),
            title: "Intro to LLMs".to_string(),
            channel: Some("Andrej Karpathy".to_string()),
            published: Some("2023-11-23".to_string()),
            duration_seconds: Some(60),
            description: Some(format!("First paragraph.\n\n{}", "x".repeat(1000))),
            thumbnail_url: Some("https://i.ytimg.com/vi/x/hq.jpg".to_string()),
            chapters: vec![],
            transcript: None,
        };

        let page = page_from_video(&video);

        assert_eq!(page.source, PageSource::Fetched);
        assert_eq!(page.content_status, ContentStatus::Extracted);
        assert!(page
            .extraction
            .article_markdown
            .starts_with("# Intro to LLMs\n"));
        assert_eq!(
            page.extraction.content_hash,
            extract::hash_text(&page.extraction.article_markdown)
        );
        assert_eq!(page.metadata.title.as_deref(), Some("Intro to LLMs"));
        assert_eq!(page.metadata.author.as_deref(), Some("Andrej Karpathy"));
        assert_eq!(page.metadata.site_name.as_deref(), Some("YouTube"));
        assert_eq!(
            page.metadata.description.as_deref(),
            Some("First paragraph.")
        );
        assert_eq!(
            page.metadata.canonical_url.as_deref(),
            Some("https://www.youtube.com/watch?v=zjkBMFhNj_g")
        );
    }

    #[test]
    fn blank_snapshot_is_ignored() {
        assert!(page_from_snapshot("https://example.com", "  \n ").is_none());
    }
}
