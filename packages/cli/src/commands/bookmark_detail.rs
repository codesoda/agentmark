//! Shared detail loading and update helpers for bookmark detail views.
//!
//! Used by both `show.rs` (CLI formatting) and `native_host.rs` (structured native responses).
//! Keeps DB+bundle assembly logic in one place instead of duplicating it.

use std::fmt;
use std::path::Path;

use crate::bundle::Bundle;
use crate::config::{self, Config};
use crate::db::{self, BookmarkRepository, DbError};
use crate::models::Bookmark;
use crate::native::messages::{BookmarkChanges, BookmarkDetail};

// ── Error type ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum DetailError {
    Config(crate::config::ConfigError),
    Db(DbError),
    NotFound { id: String },
    BundleDrift { id: String, detail: String },
    PartialUpdate { id: String, detail: String },
}

impl fmt::Display for DetailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DetailError::Config(e) => write!(f, "{e}"),
            DetailError::Db(e) => write!(f, "database error: {e}"),
            DetailError::NotFound { id } => write!(f, "bookmark not found: {id}"),
            DetailError::BundleDrift { id, detail } => {
                write!(f, "bundle/index drift for {id}: {detail}")
            }
            DetailError::PartialUpdate { id, detail } => {
                write!(
                    f,
                    "warning: bundle updated for {id} but index update failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for DetailError {}

impl From<crate::config::ConfigError> for DetailError {
    fn from(e: crate::config::ConfigError) -> Self {
        DetailError::Config(e)
    }
}

impl From<DbError> for DetailError {
    fn from(e: DbError) -> Self {
        DetailError::Db(e)
    }
}

// ── Loaded detail ──────────────────────────────────────────────────

/// Intermediate result from loading a bookmark with its bundle summary.
/// Used by `show.rs` for full CLI formatting, and converted to `BookmarkDetail` for native.
pub struct LoadedBookmarkDetail {
    pub bookmark: Bookmark,
    pub summary: Option<String>,
}

impl LoadedBookmarkDetail {
    /// Convert to the native wire DTO.
    pub fn to_detail_dto(&self) -> BookmarkDetail {
        bookmark_to_detail(&self.bookmark, self.summary.clone())
    }
}

// ── Detail loading ─────────────────────────────────────────────────

/// Load a bookmark's full detail from DB + bundle.
/// Reads summary from `bookmark.md` but skips `article.md`.
pub fn load_bookmark_detail(home: &Path, id: &str) -> Result<LoadedBookmarkDetail, DetailError> {
    let config = Config::load(home)?;
    let db_path = config::index_db_path(home);
    let conn = db::open_and_migrate(&db_path)?;
    let repo = BookmarkRepository::new(&conn);

    load_bookmark_detail_with_repo(&repo, &config.storage_path, id)
}

/// Load detail using an existing repo and storage path — useful when the caller
/// already has an open DB connection (e.g., the native-host loop).
pub fn load_bookmark_detail_with_repo(
    repo: &BookmarkRepository,
    storage_path: &Path,
    id: &str,
) -> Result<LoadedBookmarkDetail, DetailError> {
    let bookmark = repo
        .get_by_id(id)?
        .ok_or_else(|| DetailError::NotFound { id: id.to_string() })?;

    let bundle = Bundle::find(storage_path, &bookmark.saved_at, &bookmark.id).map_err(|e| {
        DetailError::BundleDrift {
            id: bookmark.id.clone(),
            detail: e.to_string(),
        }
    })?;

    let summary = match bundle.read_body_sections() {
        Ok(sections) => sections.summary,
        Err(e) => {
            return Err(DetailError::BundleDrift {
                id: bookmark.id.clone(),
                detail: format!("bookmark.md: {e}"),
            });
        }
    };

    Ok(LoadedBookmarkDetail { bookmark, summary })
}

// ── Update application ─────────────────────────────────────────────

/// Apply changes to a bookmark, rewrite the bundle, and update the DB.
/// Returns the updated detail snapshot.
///
/// Follows bundle-first semantics: writes `bookmark.md` first, then DB.
/// Surfaces partial-update errors if the DB write fails after the bundle write.
pub fn apply_bookmark_update(
    home: &Path,
    id: &str,
    changes: &BookmarkChanges,
) -> Result<LoadedBookmarkDetail, DetailError> {
    let config = Config::load(home)?;
    let db_path = config::index_db_path(home);
    let conn = db::open_and_migrate(&db_path)?;
    let repo = BookmarkRepository::new(&conn);

    let mut bookmark = repo
        .get_by_id(id)?
        .ok_or_else(|| DetailError::NotFound { id: id.to_string() })?;

    // Apply changes in memory
    apply_changes(&mut bookmark, changes);

    // Bundle-first update
    let bundle =
        Bundle::find(&config.storage_path, &bookmark.saved_at, &bookmark.id).map_err(|e| {
            DetailError::BundleDrift {
                id: bookmark.id.clone(),
                detail: e.to_string(),
            }
        })?;

    bundle
        .update_bookmark_md_preserving_body(&bookmark)
        .map_err(|e| DetailError::BundleDrift {
            id: bookmark.id.clone(),
            detail: format!("bookmark.md rewrite: {e}"),
        })?;

    // DB update
    match repo.update(&bookmark) {
        Ok(true) => {}
        Ok(false) => {
            return Err(DetailError::PartialUpdate {
                id: bookmark.id.clone(),
                detail: "bookmark was deleted from index after bundle update".to_string(),
            });
        }
        Err(e) => {
            return Err(DetailError::PartialUpdate {
                id: bookmark.id.clone(),
                detail: format!("index update failed: {e}"),
            });
        }
    }

    // Read summary from the preserved bundle for the response
    let summary = match bundle.read_body_sections() {
        Ok(sections) => sections.summary,
        Err(_) => None,
    };

    Ok(LoadedBookmarkDetail { bookmark, summary })
}

// ── Pure mutation helpers ──────────────────────────────────────────

/// Apply typed changes to a bookmark in memory.
fn apply_changes(bookmark: &mut Bookmark, changes: &BookmarkChanges) {
    if let Some(tags) = &changes.user_tags {
        bookmark.user_tags = tags.clone();
    }
    if let Some(tags) = &changes.suggested_tags {
        bookmark.suggested_tags = tags.clone();
    }
    if let Some(collections) = &changes.collections {
        bookmark.collections = collections.clone();
    }
    if let Some(note_opt) = &changes.note {
        bookmark.note = note_opt.as_ref().and_then(|n| {
            let trimmed = n.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
    }
    if let Some(state) = &changes.state {
        bookmark.state = state.clone();
    }
}

/// Convert a Bookmark + optional summary into the native wire DTO.
pub fn bookmark_to_detail(bookmark: &Bookmark, summary: Option<String>) -> BookmarkDetail {
    BookmarkDetail {
        id: bookmark.id.clone(),
        url: bookmark.url.clone(),
        title: bookmark.title.clone(),
        summary,
        saved_at: bookmark.saved_at.to_rfc3339(),
        capture_source: serde_json::to_value(&bookmark.capture_source)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "unknown".to_string()),
        state: bookmark.state.clone(),
        user_tags: bookmark.user_tags.clone(),
        suggested_tags: bookmark.suggested_tags.clone(),
        collections: bookmark.collections.clone(),
        note: bookmark.note.clone(),
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BookmarkState, CaptureSource};

    fn test_bookmark() -> Bookmark {
        let mut bm = Bookmark::new("https://example.com", "Test Title");
        bm.id = "am_test123".to_string();
        bm.user_tags = vec!["existing".to_string()];
        bm.suggested_tags = vec!["suggested1".to_string(), "suggested2".to_string()];
        bm.collections = vec!["reading".to_string()];
        bm.note = Some("original note".to_string());
        bm.state = BookmarkState::Inbox;
        bm
    }

    #[test]
    fn apply_changes_updates_user_tags() {
        let mut bm = test_bookmark();
        let changes = BookmarkChanges {
            user_tags: Some(vec!["new1".into(), "new2".into()]),
            ..default_changes()
        };
        apply_changes(&mut bm, &changes);
        assert_eq!(bm.user_tags, vec!["new1", "new2"]);
    }

    #[test]
    fn apply_changes_updates_suggested_tags() {
        let mut bm = test_bookmark();
        let changes = BookmarkChanges {
            suggested_tags: Some(vec!["only_one".into()]),
            ..default_changes()
        };
        apply_changes(&mut bm, &changes);
        assert_eq!(bm.suggested_tags, vec!["only_one"]);
    }

    #[test]
    fn apply_changes_clears_collections() {
        let mut bm = test_bookmark();
        let changes = BookmarkChanges {
            collections: Some(vec![]),
            ..default_changes()
        };
        apply_changes(&mut bm, &changes);
        assert!(bm.collections.is_empty());
    }

    #[test]
    fn apply_changes_sets_note() {
        let mut bm = test_bookmark();
        let changes = BookmarkChanges {
            note: Some(Some("new note".into())),
            ..default_changes()
        };
        apply_changes(&mut bm, &changes);
        assert_eq!(bm.note, Some("new note".to_string()));
    }

    #[test]
    fn apply_changes_clears_note_with_null() {
        let mut bm = test_bookmark();
        let changes = BookmarkChanges {
            note: Some(None),
            ..default_changes()
        };
        apply_changes(&mut bm, &changes);
        assert!(bm.note.is_none());
    }

    #[test]
    fn apply_changes_clears_note_with_whitespace() {
        let mut bm = test_bookmark();
        let changes = BookmarkChanges {
            note: Some(Some("   ".into())),
            ..default_changes()
        };
        apply_changes(&mut bm, &changes);
        assert!(bm.note.is_none());
    }

    #[test]
    fn apply_changes_trims_note() {
        let mut bm = test_bookmark();
        let changes = BookmarkChanges {
            note: Some(Some("  trimmed  ".into())),
            ..default_changes()
        };
        apply_changes(&mut bm, &changes);
        assert_eq!(bm.note, Some("trimmed".to_string()));
    }

    #[test]
    fn apply_changes_updates_state() {
        let mut bm = test_bookmark();
        let changes = BookmarkChanges {
            state: Some(BookmarkState::Processed),
            ..default_changes()
        };
        apply_changes(&mut bm, &changes);
        assert_eq!(bm.state, BookmarkState::Processed);
    }

    #[test]
    fn apply_changes_noop_preserves_all() {
        let original = test_bookmark();
        let mut bm = original.clone();
        let changes = default_changes();
        apply_changes(&mut bm, &changes);
        assert_eq!(bm.user_tags, original.user_tags);
        assert_eq!(bm.suggested_tags, original.suggested_tags);
        assert_eq!(bm.collections, original.collections);
        assert_eq!(bm.note, original.note);
        assert_eq!(bm.state, original.state);
    }

    #[test]
    fn bookmark_to_detail_maps_all_fields() {
        let bm = test_bookmark();
        let detail = bookmark_to_detail(&bm, Some("summary text".to_string()));
        assert_eq!(detail.id, "am_test123");
        assert_eq!(detail.url, "https://example.com");
        assert_eq!(detail.title, "Test Title");
        assert_eq!(detail.summary, Some("summary text".to_string()));
        assert_eq!(detail.state, BookmarkState::Inbox);
        assert_eq!(detail.user_tags, vec!["existing"]);
        assert_eq!(detail.suggested_tags, vec!["suggested1", "suggested2"]);
        assert_eq!(detail.collections, vec!["reading"]);
        assert_eq!(detail.note, Some("original note".to_string()));
        assert_eq!(detail.capture_source, "cli");
    }

    #[test]
    fn bookmark_to_detail_with_chrome_extension_source() {
        let mut bm = test_bookmark();
        bm.capture_source = CaptureSource::ChromeExtension;
        let detail = bookmark_to_detail(&bm, None);
        assert_eq!(detail.capture_source, "chrome_extension");
        assert!(detail.summary.is_none());
    }

    fn default_changes() -> BookmarkChanges {
        BookmarkChanges {
            user_tags: None,
            suggested_tags: None,
            collections: None,
            note: None,
            state: None,
        }
    }
}
