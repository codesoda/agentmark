use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, Utc};

use crate::fetch::PageMetadata;
use crate::models::{Bookmark, BookmarkEvent, EventType};

use super::bookmark_md::{self, BodySections};
use super::BundleError;

// Canonical artifact filenames within a bundle directory.
const BOOKMARK_MD: &str = "bookmark.md";
const ARTICLE_MD: &str = "article.md";
const METADATA_JSON: &str = "metadata.json";
const SOURCE_HTML: &str = "source.html";
const EVENTS_JSONL: &str = "events.jsonl";

/// Build the canonical bundle directory path:
/// `<storage>/<YYYY>/<MM>/<DD>/<slug>-<id>/`
pub fn bundle_dir_path(
    storage_root: &Path,
    saved_at: &DateTime<Utc>,
    slug: &str,
    id: &str,
) -> PathBuf {
    let dir_name = format!("{}-{}", slug, id);
    storage_root
        .join(format!("{:04}", saved_at.year()))
        .join(format!("{:02}", saved_at.month()))
        .join(format!("{:02}", saved_at.day()))
        .join(dir_name)
}

/// Input data for writing a complete bundle.
pub struct BundleInput<'a> {
    pub bookmark: &'a Bookmark,
    pub metadata: &'a PageMetadata,
    pub article_markdown: &'a str,
    pub raw_html: &'a str,
    pub capture_source: &'a str,
}

/// Create a new bundle directory with all five canonical artifacts.
///
/// Writes into a temporary staging directory first, then renames to the
/// final path so that partially written bundles are never visible at the
/// canonical location.
pub fn create_bundle(storage_root: &Path, input: &BundleInput<'_>) -> Result<PathBuf, BundleError> {
    let slug = input.bookmark.slug();
    let final_dir = bundle_dir_path(
        storage_root,
        &input.bookmark.saved_at,
        &slug,
        &input.bookmark.id,
    );

    if final_dir.exists() {
        return Err(BundleError::DirectoryExists { path: final_dir });
    }

    // Create date parent directories
    let parent = final_dir.parent().ok_or_else(|| BundleError::PathError {
        path: final_dir.clone(),
        message: "bundle path has no parent directory".to_string(),
    })?;
    fs::create_dir_all(parent).map_err(|source| BundleError::Io {
        path: parent.to_path_buf(),
        source,
    })?;

    // Stage in a temp sibling directory
    let staging_name = format!(".tmp-{}", input.bookmark.id);
    let staging_dir = parent.join(&staging_name);
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir).map_err(|source| BundleError::Io {
            path: staging_dir.clone(),
            source,
        })?;
    }
    fs::create_dir(&staging_dir).map_err(|source| BundleError::Io {
        path: staging_dir.clone(),
        source,
    })?;

    // Write artifacts into staging directory
    let write_result = write_artifacts(&staging_dir, input);
    if let Err(e) = write_result {
        // Clean up staging on failure
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(e);
    }

    // Rename staging to final
    fs::rename(&staging_dir, &final_dir).map_err(|source| BundleError::Io {
        path: final_dir.clone(),
        source,
    })?;

    Ok(final_dir)
}

/// Write all five artifacts into the given directory.
fn write_artifacts(dir: &Path, input: &BundleInput<'_>) -> Result<(), BundleError> {
    // bookmark.md
    let bookmark_md_content = bookmark_md::render(input.bookmark, &BodySections::default())
        .map_err(|source| BundleError::Yaml {
            path: dir.join(BOOKMARK_MD),
            source,
        })?;
    write_file(&dir.join(BOOKMARK_MD), bookmark_md_content.as_bytes())?;

    // article.md
    write_file(&dir.join(ARTICLE_MD), input.article_markdown.as_bytes())?;

    // metadata.json
    let metadata_json =
        serde_json::to_string_pretty(input.metadata).map_err(|source| BundleError::Json {
            path: dir.join(METADATA_JSON),
            source,
        })?;
    write_file(&dir.join(METADATA_JSON), metadata_json.as_bytes())?;

    // source.html
    write_file(&dir.join(SOURCE_HTML), input.raw_html.as_bytes())?;

    // events.jsonl — initial saved event
    let event = BookmarkEvent::new(
        EventType::Saved,
        serde_json::json!({
            "capture_source": input.capture_source,
            "url": input.bookmark.url,
        }),
    );
    let event_line = event.to_jsonl().map_err(|source| BundleError::Json {
        path: dir.join(EVENTS_JSONL),
        source,
    })?;
    let mut events_content = event_line;
    events_content.push('\n');
    write_file(&dir.join(EVENTS_JSONL), events_content.as_bytes())?;

    Ok(())
}

/// Rewrite `bookmark.md` in an existing bundle directory from structured inputs.
pub fn rewrite_bookmark_md(
    bundle_dir: &Path,
    bookmark: &Bookmark,
    sections: &BodySections,
) -> Result<(), BundleError> {
    if !bundle_dir.is_dir() {
        return Err(BundleError::BundleNotFound {
            path: bundle_dir.to_path_buf(),
        });
    }

    let content = bookmark_md::render(bookmark, sections).map_err(|source| BundleError::Yaml {
        path: bundle_dir.join(BOOKMARK_MD),
        source,
    })?;

    write_file(&bundle_dir.join(BOOKMARK_MD), content.as_bytes())
}

/// Append a single event to `events.jsonl` in an existing bundle directory.
pub fn append_event(bundle_dir: &Path, event: &BookmarkEvent) -> Result<(), BundleError> {
    let events_path = bundle_dir.join(EVENTS_JSONL);
    if !events_path.exists() {
        return Err(BundleError::EventsLogMissing { path: events_path });
    }

    let line = event.to_jsonl().map_err(|source| BundleError::Json {
        path: events_path.clone(),
        source,
    })?;

    let mut file = OpenOptions::new()
        .append(true)
        .open(&events_path)
        .map_err(|source| BundleError::Io {
            path: events_path.clone(),
            source,
        })?;

    writeln!(file, "{}", line).map_err(|source| BundleError::Io {
        path: events_path,
        source,
    })?;

    Ok(())
}

/// Write bytes to a file, creating it if it doesn't exist.
fn write_file(path: &Path, contents: &[u8]) -> Result<(), BundleError> {
    fs::write(path, contents).map_err(|source| BundleError::Io {
        path: path.to_path_buf(),
        source,
    })
}
