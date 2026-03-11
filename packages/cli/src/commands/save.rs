//! Save command: wire fetch, extract, bundle, and DB into `agentmark save <url>`.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::bundle::Bundle;
use crate::cli::SaveArgs;
use crate::config::{self, Config, ConfigError};
use crate::db::{self, BookmarkRepository, DbError};
use crate::extract::{self, ExtractionResult};
use crate::fetch::{self, FetchError, PageMetadata};
use crate::models::{Bookmark, CaptureSource, ContentStatus};

// ── Public entry point ──────────────────────────────────────────────

/// Entry point for `agentmark save` using real environment.
pub fn run_save(args: SaveArgs) -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    let outcome = execute_save(&home, &args)?;

    for warning in &outcome.warnings {
        eprintln!("warning: {warning}");
    }
    println!("Saved bookmark {}", outcome.id);
    println!("  path: {}", outcome.bundle_path.display());

    Ok(())
}

// ── Typed outcome and errors ────────────────────────────────────────

/// Successful save result returned from the testable helper.
#[derive(Debug)]
pub struct SaveOutcome {
    pub id: String,
    pub bundle_path: PathBuf,
    pub warnings: Vec<String>,
}

/// Save-specific errors with pipeline stage context.
#[derive(Debug)]
pub enum SaveError {
    Config(ConfigError),
    Fetch(FetchError),
    Bundle(crate::bundle::BundleError),
    Db(DbError),
    /// Bundle was created but DB insertion failed. The bundle is preserved.
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
            SaveError::PartialSave {
                id,
                bundle_path,
                db_error,
            } => write!(
                f,
                "bundle saved ({id} at {}) but index insertion failed: {db_error}",
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

// ── Bookmark construction ───────────────────────────────────────────

/// Build a Bookmark from CLI args, fetched metadata, and extraction output.
fn build_bookmark(
    url: &str,
    metadata: &PageMetadata,
    extraction: &ExtractionResult,
    tags: Vec<String>,
    collection: Option<String>,
    note: Option<String>,
    action: Option<String>,
) -> Bookmark {
    let title = metadata.title.clone().unwrap_or_else(|| url.to_string());

    let mut bm = Bookmark::new(url, &title);

    // Canonical URL: prefer metadata, fall back to input URL.
    if let Some(ref canonical) = metadata.canonical_url {
        bm.canonical_url = canonical.clone();
    }

    // Metadata fields
    bm.description = metadata.description.clone();
    bm.author = metadata.author.clone();
    bm.site_name = metadata.site_name.clone();
    bm.published_at = metadata.published_at.clone();

    // CLI inputs
    bm.user_tags = tags;
    if let Some(c) = collection {
        bm.collections = vec![c];
    }
    bm.note = note;
    bm.action_prompt = action;

    // Extraction
    let (content_status, _) = classify_extraction(extraction);
    bm.content_status = content_status;
    bm.content_hash = Some(extraction.content_hash.clone());

    // CLI capture source
    bm.capture_source = CaptureSource::Cli;

    bm
}

// ── Testable save pipeline ──────────────────────────────────────────

/// Execute the save pipeline with an explicit home directory.
/// This is the main testable seam.
pub fn execute_save(home: &Path, args: &SaveArgs) -> Result<SaveOutcome, SaveError> {
    // 1. Load config
    let config = Config::load(home)?;

    // 2. Open/migrate the SQLite index
    let db_path = config::index_db_path(home);
    let conn = db::open_and_migrate(&db_path).map_err(SaveError::Db)?;

    // 3. Fetch page
    let (raw_html, metadata) = fetch::fetch_page(&args.url)?;

    // 4. Extract content
    let extraction = extract::extract_content(&raw_html);

    // 5. Classify extraction and collect warnings
    let mut warnings = Vec::new();
    let (_, extraction_warning) = classify_extraction(&extraction);
    if let Some(w) = extraction_warning {
        warnings.push(w);
    }

    // 6. Normalize CLI inputs
    let tags = parse_tags(args.tags.as_deref());
    let collection = normalize_optional_text(args.collection.as_deref());
    let note = normalize_optional_text(args.note.as_deref());
    let action = normalize_optional_text(args.action.as_deref());

    // 7. Build bookmark
    let bookmark = build_bookmark(
        &args.url,
        &metadata,
        &extraction,
        tags,
        collection,
        note,
        action,
    );

    // 8. Create bundle
    let capture_source_str = match bookmark.capture_source {
        CaptureSource::Cli => "cli",
        CaptureSource::ChromeExtension => "chrome_extension",
    };
    let bundle = Bundle::create(
        &config.storage_path,
        &bookmark,
        &metadata,
        &extraction.article_markdown,
        &raw_html,
        capture_source_str,
    )?;

    let bundle_path = bundle.path().to_path_buf();
    let id = bookmark.id.clone();

    // 9. Insert into SQLite — if this fails, the bundle is preserved
    let repo = BookmarkRepository::new(&conn);
    if let Err(db_err) = repo.insert(&bookmark) {
        return Err(SaveError::PartialSave {
            id,
            bundle_path,
            db_error: Box::new(db_err),
        });
    }

    Ok(SaveOutcome {
        id,
        bundle_path,
        warnings,
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

        let bm = build_bookmark(
            "https://example.com/page",
            &metadata,
            &extraction,
            vec!["rust".to_string()],
            Some("dev".to_string()),
            Some("good read".to_string()),
            Some("review".to_string()),
        );

        assert!(bm.id.starts_with("am_"));
        assert_eq!(bm.url, "https://example.com/page");
        assert_eq!(bm.canonical_url, "https://example.com/canonical");
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
        let metadata = PageMetadata::default();
        let extraction = ExtractionResult {
            article_html: String::new(),
            article_markdown: String::new(),
            content_hash: "sha256:empty".to_string(),
        };

        let bm = build_bookmark(
            "https://example.com/page",
            &metadata,
            &extraction,
            Vec::new(),
            None,
            None,
            None,
        );

        assert_eq!(bm.title, "https://example.com/page");
    }

    #[test]
    fn build_bookmark_missing_canonical_falls_back_to_url() {
        let metadata = PageMetadata::default();
        let extraction = ExtractionResult {
            article_html: String::new(),
            article_markdown: String::new(),
            content_hash: "sha256:empty".to_string(),
        };

        let bm = build_bookmark(
            "https://example.com/page",
            &metadata,
            &extraction,
            Vec::new(),
            None,
            None,
            None,
        );

        assert_eq!(bm.canonical_url, "https://example.com/page");
    }

    #[test]
    fn build_bookmark_empty_extraction_sets_failed() {
        let metadata = PageMetadata::default();
        let extraction = ExtractionResult {
            article_html: String::new(),
            article_markdown: String::new(),
            content_hash: "sha256:empty".to_string(),
        };

        let bm = build_bookmark(
            "https://example.com",
            &metadata,
            &extraction,
            Vec::new(),
            None,
            None,
            None,
        );

        assert_eq!(bm.content_status, ContentStatus::Failed);
    }

    #[test]
    fn build_bookmark_no_collection_leaves_empty_vec() {
        let metadata = PageMetadata::default();
        let extraction = ExtractionResult {
            article_html: String::new(),
            article_markdown: String::new(),
            content_hash: "sha256:x".to_string(),
        };

        let bm = build_bookmark(
            "https://example.com",
            &metadata,
            &extraction,
            Vec::new(),
            None,
            None,
            None,
        );

        assert!(bm.collections.is_empty());
    }

    #[test]
    fn build_bookmark_summary_status_is_pending() {
        let metadata = PageMetadata::default();
        let extraction = ExtractionResult {
            article_html: String::new(),
            article_markdown: String::new(),
            content_hash: "sha256:x".to_string(),
        };

        let bm = build_bookmark(
            "https://example.com",
            &metadata,
            &extraction,
            Vec::new(),
            None,
            None,
            None,
        );

        assert_eq!(bm.summary_status, crate::models::SummaryStatus::Pending);
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
        assert!(msg.contains("index insertion failed"));
    }
}
