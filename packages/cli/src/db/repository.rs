//! Bookmark repository — CRUD, list, search, and collection queries.
//!
//! All SQL values are parameter-bound. FTS synchronization is handled by
//! database triggers (see `schema.rs`), so repository methods only touch the
//! main `bookmarks` table.

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::models::Bookmark;
use crate::models::{BookmarkState, CaptureSource, ContentStatus, SummaryStatus};

use super::DbError;

/// Thin wrapper around a `Connection` that provides bookmark-specific queries.
pub struct BookmarkRepository<'a> {
    conn: &'a Connection,
}

// ── Column helpers ──────────────────────────────────────────────────

/// Columns for INSERT (excludes rowid; summary is DB-internal).
const INSERT_COLS: &str = "id, url, canonical_url, title, description, author, site_name, \
    published_at, saved_at, capture_source, user_tags, suggested_tags, collections, \
    note, action_prompt, state, content_status, summary_status, content_hash, schema_version";

/// Shared SELECT column list for bookmark reads.
const SELECT_COLS: &str = "id, url, canonical_url, title, description, author, site_name, \
    published_at, saved_at, capture_source, user_tags, suggested_tags, collections, \
    note, action_prompt, state, content_status, summary_status, content_hash, schema_version";

/// Table-qualified SELECT columns for JOINs (avoids ambiguity with FTS columns).
const QUALIFIED_SELECT_COLS: &str = "bookmarks.id, bookmarks.url, bookmarks.canonical_url, \
    bookmarks.title, bookmarks.description, bookmarks.author, bookmarks.site_name, \
    bookmarks.published_at, bookmarks.saved_at, bookmarks.capture_source, \
    bookmarks.user_tags, bookmarks.suggested_tags, bookmarks.collections, \
    bookmarks.note, bookmarks.action_prompt, bookmarks.state, bookmarks.content_status, \
    bookmarks.summary_status, bookmarks.content_hash, bookmarks.schema_version";

// ── JSON array helpers ──────────────────────────────────────────────

fn vec_to_json(v: &[String]) -> String {
    serde_json::to_string(v).expect("Vec<String> serialization cannot fail")
}

fn json_to_vec(s: &str) -> Result<Vec<String>, DbError> {
    serde_json::from_str(s).map_err(|e| DbError::Decode {
        field: "json_array".to_string(),
        detail: format!("invalid JSON array: {e}"),
    })
}

// ── Enum text helpers ───────────────────────────────────────────────

fn capture_source_to_text(cs: &CaptureSource) -> &'static str {
    match cs {
        CaptureSource::Cli => "cli",
        CaptureSource::ChromeExtension => "chrome_extension",
    }
}

fn text_to_capture_source(s: &str) -> Result<CaptureSource, DbError> {
    match s {
        "cli" => Ok(CaptureSource::Cli),
        "chrome_extension" => Ok(CaptureSource::ChromeExtension),
        _ => Err(DbError::Decode {
            field: "capture_source".to_string(),
            detail: format!("unknown value: {s}"),
        }),
    }
}

fn state_to_text(s: &BookmarkState) -> &'static str {
    match s {
        BookmarkState::Inbox => "inbox",
        BookmarkState::Processed => "processed",
        BookmarkState::Archived => "archived",
    }
}

fn text_to_state(s: &str) -> Result<BookmarkState, DbError> {
    match s {
        "inbox" => Ok(BookmarkState::Inbox),
        "processed" => Ok(BookmarkState::Processed),
        "archived" => Ok(BookmarkState::Archived),
        _ => Err(DbError::Decode {
            field: "state".to_string(),
            detail: format!("unknown value: {s}"),
        }),
    }
}

fn content_status_to_text(cs: &ContentStatus) -> &'static str {
    match cs {
        ContentStatus::Pending => "pending",
        ContentStatus::Extracted => "extracted",
        ContentStatus::Failed => "failed",
    }
}

fn text_to_content_status(s: &str) -> Result<ContentStatus, DbError> {
    match s {
        "pending" => Ok(ContentStatus::Pending),
        "extracted" => Ok(ContentStatus::Extracted),
        "failed" => Ok(ContentStatus::Failed),
        _ => Err(DbError::Decode {
            field: "content_status".to_string(),
            detail: format!("unknown value: {s}"),
        }),
    }
}

fn summary_status_to_text(ss: &SummaryStatus) -> &'static str {
    match ss {
        SummaryStatus::Pending => "pending",
        SummaryStatus::Done => "done",
        SummaryStatus::Failed => "failed",
    }
}

fn text_to_summary_status(s: &str) -> Result<SummaryStatus, DbError> {
    match s {
        "pending" => Ok(SummaryStatus::Pending),
        "done" => Ok(SummaryStatus::Done),
        "failed" => Ok(SummaryStatus::Failed),
        _ => Err(DbError::Decode {
            field: "summary_status".to_string(),
            detail: format!("unknown value: {s}"),
        }),
    }
}

// ── Timestamp helpers ───────────────────────────────────────────────

fn datetime_to_text(dt: &chrono::DateTime<chrono::Utc>) -> String {
    dt.to_rfc3339()
}

fn text_to_datetime(s: &str) -> Result<chrono::DateTime<chrono::Utc>, DbError> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| DbError::Decode {
            field: "saved_at".to_string(),
            detail: format!("invalid RFC3339 timestamp: {e}"),
        })
}

// ── Row mapping ─────────────────────────────────────────────────────

/// Decode a `Bookmark` from a row that was selected using `SELECT_COLS`.
fn row_to_bookmark(row: &Row<'_>) -> Result<Bookmark, DbError> {
    let id: String = row.get(0)?;
    let url: String = row.get(1)?;
    let canonical_url: String = row.get(2)?;
    let title: String = row.get(3)?;
    let description: Option<String> = row.get(4)?;
    let author: Option<String> = row.get(5)?;
    let site_name: Option<String> = row.get(6)?;
    let published_at: Option<String> = row.get(7)?;
    let saved_at_text: String = row.get(8)?;
    let capture_source_text: String = row.get(9)?;
    let user_tags_text: String = row.get(10)?;
    let suggested_tags_text: String = row.get(11)?;
    let collections_text: String = row.get(12)?;
    let note: Option<String> = row.get(13)?;
    let action_prompt: Option<String> = row.get(14)?;
    let state_text: String = row.get(15)?;
    let content_status_text: String = row.get(16)?;
    let summary_status_text: String = row.get(17)?;
    let content_hash: Option<String> = row.get(18)?;
    let schema_version: u32 = row.get(19)?;

    Ok(Bookmark {
        id,
        url,
        canonical_url,
        title,
        description,
        author,
        site_name,
        published_at,
        saved_at: text_to_datetime(&saved_at_text)?,
        capture_source: text_to_capture_source(&capture_source_text)?,
        user_tags: json_to_vec(&user_tags_text)?,
        suggested_tags: json_to_vec(&suggested_tags_text)?,
        collections: json_to_vec(&collections_text)?,
        note,
        action_prompt,
        state: text_to_state(&state_text)?,
        content_status: text_to_content_status(&content_status_text)?,
        summary_status: text_to_summary_status(&summary_status_text)?,
        content_hash,
        schema_version,
    })
}

// ── Repository ──────────────────────────────────────────────────────

impl<'a> BookmarkRepository<'a> {
    /// Create a new repository wrapping the given connection.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Insert a bookmark. FTS is updated automatically by the trigger.
    pub fn insert(&self, bookmark: &Bookmark) -> Result<(), DbError> {
        let sql = format!(
            "INSERT INTO bookmarks ({INSERT_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)"
        );
        self.conn.execute(
            &sql,
            params![
                bookmark.id,
                bookmark.url,
                bookmark.canonical_url,
                bookmark.title,
                bookmark.description,
                bookmark.author,
                bookmark.site_name,
                bookmark.published_at,
                datetime_to_text(&bookmark.saved_at),
                capture_source_to_text(&bookmark.capture_source),
                vec_to_json(&bookmark.user_tags),
                vec_to_json(&bookmark.suggested_tags),
                vec_to_json(&bookmark.collections),
                bookmark.note,
                bookmark.action_prompt,
                state_to_text(&bookmark.state),
                content_status_to_text(&bookmark.content_status),
                summary_status_to_text(&bookmark.summary_status),
                bookmark.content_hash,
                bookmark.schema_version,
            ],
        )?;
        Ok(())
    }

    /// Update a bookmark by ID. FTS is re-indexed automatically by the trigger.
    ///
    /// Returns `Ok(true)` if a row was updated, `Ok(false)` if the ID was not found.
    pub fn update(&self, bookmark: &Bookmark) -> Result<bool, DbError> {
        let sql = "UPDATE bookmarks SET \
            url=?2, canonical_url=?3, title=?4, description=?5, author=?6, \
            site_name=?7, published_at=?8, saved_at=?9, capture_source=?10, \
            user_tags=?11, suggested_tags=?12, collections=?13, note=?14, \
            action_prompt=?15, state=?16, content_status=?17, summary_status=?18, \
            content_hash=?19, schema_version=?20 \
            WHERE id=?1";
        let updated = self.conn.execute(
            sql,
            params![
                bookmark.id,
                bookmark.url,
                bookmark.canonical_url,
                bookmark.title,
                bookmark.description,
                bookmark.author,
                bookmark.site_name,
                bookmark.published_at,
                datetime_to_text(&bookmark.saved_at),
                capture_source_to_text(&bookmark.capture_source),
                vec_to_json(&bookmark.user_tags),
                vec_to_json(&bookmark.suggested_tags),
                vec_to_json(&bookmark.collections),
                bookmark.note,
                bookmark.action_prompt,
                state_to_text(&bookmark.state),
                content_status_to_text(&bookmark.content_status),
                summary_status_to_text(&bookmark.summary_status),
                bookmark.content_hash,
                bookmark.schema_version,
            ],
        )?;
        Ok(updated > 0)
    }

    /// Get a bookmark by its ID.
    pub fn get_by_id(&self, id: &str) -> Result<Option<Bookmark>, DbError> {
        let sql = format!("SELECT {SELECT_COLS} FROM bookmarks WHERE id = ?1");
        let result = self
            .conn
            .query_row(&sql, params![id], |row| {
                row_to_bookmark(row).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            })
            .optional()?;
        Ok(result)
    }

    /// Get a bookmark by its canonical URL.
    pub fn get_by_canonical_url(&self, url: &str) -> Result<Option<Bookmark>, DbError> {
        let sql = format!("SELECT {SELECT_COLS} FROM bookmarks WHERE canonical_url = ?1");
        let result = self
            .conn
            .query_row(&sql, params![url], |row| {
                row_to_bookmark(row).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            })
            .optional()?;
        Ok(result)
    }

    /// List bookmarks ordered by `saved_at` DESC with pagination and optional filters.
    ///
    /// - `collection`: exact match against the `collections` JSON array
    /// - `tag`: exact match against either `user_tags` or `suggested_tags` JSON arrays
    pub fn list(
        &self,
        limit: usize,
        offset: usize,
        collection: Option<&str>,
        tag: Option<&str>,
    ) -> Result<Vec<Bookmark>, DbError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let mut sql = format!("SELECT {SELECT_COLS} FROM bookmarks WHERE 1=1");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if let Some(c) = collection {
            // Match exact JSON string element: the array contains `"value"`
            let pattern = format!("%\"{}\",%", escape_like(c));
            let pattern2 = format!("%\"{}\"%%", escape_like(c));
            // Use a robust pattern: the value appears as a quoted string in the JSON array
            let json_pattern = format!("%\"{}\"%%", escape_like(c));
            // Simpler approach: check if the JSON array contains the exact quoted string
            sql.push_str(&format!(" AND collections LIKE ?{param_idx}"));
            param_values.push(Box::new(format!("%\"{}\"%%", escape_like(c))));
            let _ = (pattern, pattern2, json_pattern); // suppress unused
            param_idx += 1;
        }

        if let Some(t) = tag {
            let like_pat = format!("%\"{}\"%%", escape_like(t));
            sql.push_str(&format!(
                " AND (user_tags LIKE ?{pi} OR suggested_tags LIKE ?{pi})",
                pi = param_idx
            ));
            param_values.push(Box::new(like_pat));
            param_idx += 1;
        }

        sql.push_str(&format!(
            " ORDER BY saved_at DESC LIMIT ?{} OFFSET ?{}",
            param_idx,
            param_idx + 1
        ));
        param_values.push(Box::new(limit as i64));
        param_values.push(Box::new(offset as i64));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            row_to_bookmark(row).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Full-text search using FTS5. Results are ranked by relevance.
    ///
    /// - Empty or whitespace-only queries return an empty vector.
    /// - Optional `collection` filter scopes results.
    pub fn search(
        &self,
        query: &str,
        limit: usize,
        collection: Option<&str>,
    ) -> Result<Vec<Bookmark>, DbError> {
        let trimmed = query.trim();
        if trimmed.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let mut sql = format!(
            "SELECT {QUALIFIED_SELECT_COLS} FROM bookmarks \
             JOIN bookmarks_fts ON bookmarks.rowid = bookmarks_fts.rowid \
             WHERE bookmarks_fts MATCH ?1"
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        param_values.push(Box::new(trimmed.to_string()));
        let mut param_idx = 2;

        if let Some(c) = collection {
            sql.push_str(&format!(" AND bookmarks.collections LIKE ?{param_idx}"));
            param_values.push(Box::new(format!("%\"{}\"%%", escape_like(c))));
            param_idx += 1;
        }

        sql.push_str(&format!(" ORDER BY bm25(bookmarks_fts) LIMIT ?{param_idx}"));
        param_values.push(Box::new(limit as i64));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql).map_err(|e| {
            // Surface FTS parse errors clearly
            DbError::Sqlite(e)
        })?;

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            row_to_bookmark(row).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// List all collections with their bookmark counts.
    ///
    /// A bookmark with `collections: ["a", "b"]` contributes 1 to each count.
    pub fn list_collections(&self) -> Result<Vec<(String, usize)>, DbError> {
        let mut stmt = self.conn.prepare("SELECT collections FROM bookmarks")?;
        let rows = stmt.query_map([], |row| {
            let text: String = row.get(0)?;
            Ok(text)
        })?;

        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for row in rows {
            let text = row?;
            let cols: Vec<String> = json_to_vec(&text)?;
            for c in cols {
                *counts.entry(c).or_insert(0) += 1;
            }
        }

        let mut result: Vec<(String, usize)> = counts.into_iter().collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }

    /// Delete a bookmark by ID. Returns `true` if a row was deleted.
    pub fn delete(&self, id: &str) -> Result<bool, DbError> {
        let deleted = self
            .conn
            .execute("DELETE FROM bookmarks WHERE id = ?1", params![id])?;
        Ok(deleted > 0)
    }

    /// Set the DB-internal summary for a bookmark. Used by enrichment pipeline.
    pub fn set_summary(&self, id: &str, summary: &str) -> Result<bool, DbError> {
        let updated = self.conn.execute(
            "UPDATE bookmarks SET summary = ?2 WHERE id = ?1",
            params![id, summary],
        )?;
        Ok(updated > 0)
    }
}

/// Escape `%` and `_` in a string for use in SQL `LIKE` patterns.
fn escape_like(s: &str) -> String {
    s.replace('%', "%%").replace('_', "__")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::Bookmark;
    use chrono::Utc;

    fn setup() -> Connection {
        db::open_memory().unwrap()
    }

    fn sample_bookmark(url: &str, title: &str) -> Bookmark {
        Bookmark::new(url, title)
    }

    // ── Insert + get roundtrip ──────────────────────────────────────

    #[test]
    fn insert_and_get_by_id() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);
        let mut bm = sample_bookmark("https://example.com", "Example");
        bm.description = Some("A great site".to_string());
        bm.author = Some("Author".to_string());
        bm.site_name = Some("Example.com".to_string());
        bm.published_at = Some("2026-01-01".to_string());
        bm.user_tags = vec!["rust".to_string(), "web".to_string()];
        bm.suggested_tags = vec!["dev".to_string()];
        bm.collections = vec!["reading".to_string()];
        bm.note = Some("Important".to_string());
        bm.action_prompt = Some("Read later".to_string());
        bm.content_hash = Some("abc123".to_string());
        bm.state = BookmarkState::Processed;
        bm.content_status = ContentStatus::Extracted;
        bm.summary_status = SummaryStatus::Done;

        repo.insert(&bm).unwrap();
        let loaded = repo.get_by_id(&bm.id).unwrap().unwrap();

        assert_eq!(loaded.id, bm.id);
        assert_eq!(loaded.url, bm.url);
        assert_eq!(loaded.canonical_url, bm.canonical_url);
        assert_eq!(loaded.title, bm.title);
        assert_eq!(loaded.description, bm.description);
        assert_eq!(loaded.author, bm.author);
        assert_eq!(loaded.site_name, bm.site_name);
        assert_eq!(loaded.published_at, bm.published_at);
        assert_eq!(loaded.capture_source, bm.capture_source);
        assert_eq!(loaded.user_tags, bm.user_tags);
        assert_eq!(loaded.suggested_tags, bm.suggested_tags);
        assert_eq!(loaded.collections, bm.collections);
        assert_eq!(loaded.note, bm.note);
        assert_eq!(loaded.action_prompt, bm.action_prompt);
        assert_eq!(loaded.state, bm.state);
        assert_eq!(loaded.content_status, bm.content_status);
        assert_eq!(loaded.summary_status, bm.summary_status);
        assert_eq!(loaded.content_hash, bm.content_hash);
        assert_eq!(loaded.schema_version, bm.schema_version);
        // Timestamp roundtrip (RFC3339 may lose sub-nanosecond precision)
        assert_eq!(
            loaded.saved_at.timestamp_millis(),
            bm.saved_at.timestamp_millis()
        );
    }

    #[test]
    fn get_by_id_returns_none_for_missing() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);
        assert!(repo.get_by_id("am_nonexistent").unwrap().is_none());
    }

    // ── Canonical URL lookup ────────────────────────────────────────

    #[test]
    fn get_by_canonical_url_finds_correct_row() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);
        let bm = sample_bookmark("https://example.com/page", "Page");
        repo.insert(&bm).unwrap();

        let found = repo
            .get_by_canonical_url("https://example.com/page")
            .unwrap()
            .unwrap();
        assert_eq!(found.id, bm.id);
    }

    #[test]
    fn get_by_canonical_url_returns_none_when_missing() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);
        assert!(repo
            .get_by_canonical_url("https://nope.com")
            .unwrap()
            .is_none());
    }

    // ── Update ──────────────────────────────────────────────────────

    #[test]
    fn update_modifies_row() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);
        let mut bm = sample_bookmark("https://example.com", "Original");
        repo.insert(&bm).unwrap();

        bm.title = "Updated Title".to_string();
        bm.user_tags = vec!["new-tag".to_string()];
        bm.state = BookmarkState::Archived;
        let updated = repo.update(&bm).unwrap();
        assert!(updated);

        let loaded = repo.get_by_id(&bm.id).unwrap().unwrap();
        assert_eq!(loaded.title, "Updated Title");
        assert_eq!(loaded.user_tags, vec!["new-tag".to_string()]);
        assert_eq!(loaded.state, BookmarkState::Archived);
    }

    #[test]
    fn update_missing_id_returns_false() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);
        let bm = sample_bookmark("https://example.com", "Ghost");
        assert!(!repo.update(&bm).unwrap());
    }

    // ── Delete ──────────────────────────────────────────────────────

    #[test]
    fn delete_removes_row() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);
        let bm = sample_bookmark("https://example.com", "Delete Me");
        repo.insert(&bm).unwrap();

        assert!(repo.delete(&bm.id).unwrap());
        assert!(repo.get_by_id(&bm.id).unwrap().is_none());
    }

    #[test]
    fn delete_missing_id_returns_false() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);
        assert!(!repo.delete("am_nope").unwrap());
    }

    // ── List ────────────────────────────────────────────────────────

    #[test]
    fn list_returns_reverse_chronological() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        for i in 0..3 {
            let mut bm =
                sample_bookmark(&format!("https://example.com/{i}"), &format!("Title {i}"));
            // Ensure distinct saved_at by shifting milliseconds
            bm.saved_at = Utc::now() + chrono::Duration::milliseconds(i * 100);
            repo.insert(&bm).unwrap();
        }

        let results = repo.list(10, 0, None, None).unwrap();
        assert_eq!(results.len(), 3);
        // Most recent first
        assert!(results[0].saved_at >= results[1].saved_at);
        assert!(results[1].saved_at >= results[2].saved_at);
    }

    #[test]
    fn list_pagination() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        for i in 0..5 {
            let mut bm =
                sample_bookmark(&format!("https://example.com/{i}"), &format!("Title {i}"));
            bm.saved_at = Utc::now() + chrono::Duration::milliseconds(i * 100);
            repo.insert(&bm).unwrap();
        }

        let page1 = repo.list(2, 0, None, None).unwrap();
        let page2 = repo.list(2, 2, None, None).unwrap();
        let page3 = repo.list(2, 4, None, None).unwrap();

        assert_eq!(page1.len(), 2);
        assert_eq!(page2.len(), 2);
        assert_eq!(page3.len(), 1);
        // No overlap
        assert_ne!(page1[0].id, page2[0].id);
    }

    #[test]
    fn list_zero_limit_returns_empty() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);
        let bm = sample_bookmark("https://example.com", "Test");
        repo.insert(&bm).unwrap();
        assert!(repo.list(0, 0, None, None).unwrap().is_empty());
    }

    #[test]
    fn list_filters_by_collection() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let mut bm1 = sample_bookmark("https://a.com", "A");
        bm1.collections = vec!["reading".to_string()];
        repo.insert(&bm1).unwrap();

        let mut bm2 = sample_bookmark("https://b.com", "B");
        bm2.collections = vec!["work".to_string()];
        repo.insert(&bm2).unwrap();

        let results = repo.list(10, 0, Some("reading"), None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, bm1.id);
    }

    #[test]
    fn list_filters_by_tag_in_user_tags() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let mut bm1 = sample_bookmark("https://a.com", "A");
        bm1.user_tags = vec!["rust".to_string()];
        repo.insert(&bm1).unwrap();

        let mut bm2 = sample_bookmark("https://b.com", "B");
        bm2.user_tags = vec!["python".to_string()];
        repo.insert(&bm2).unwrap();

        let results = repo.list(10, 0, None, Some("rust")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, bm1.id);
    }

    #[test]
    fn list_filters_by_tag_in_suggested_tags() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let mut bm = sample_bookmark("https://a.com", "A");
        bm.suggested_tags = vec!["ai".to_string()];
        repo.insert(&bm).unwrap();

        let results = repo.list(10, 0, None, Some("ai")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, bm.id);
    }

    // ── Collections ─────────────────────────────────────────────────

    #[test]
    fn list_collections_counts() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let mut bm1 = sample_bookmark("https://a.com", "A");
        bm1.collections = vec!["reading".to_string(), "work".to_string()];
        repo.insert(&bm1).unwrap();

        let mut bm2 = sample_bookmark("https://b.com", "B");
        bm2.collections = vec!["reading".to_string()];
        repo.insert(&bm2).unwrap();

        let mut bm3 = sample_bookmark("https://c.com", "C");
        bm3.collections = vec!["fun".to_string()];
        repo.insert(&bm3).unwrap();

        let collections = repo.list_collections().unwrap();
        assert_eq!(collections.len(), 3);

        let map: std::collections::HashMap<&str, usize> =
            collections.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        assert_eq!(map["reading"], 2);
        assert_eq!(map["work"], 1);
        assert_eq!(map["fun"], 1);
    }

    #[test]
    fn list_collections_empty_when_no_bookmarks() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);
        assert!(repo.list_collections().unwrap().is_empty());
    }

    #[test]
    fn list_collections_ignores_empty_arrays() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let bm = sample_bookmark("https://a.com", "A");
        // collections is empty by default
        repo.insert(&bm).unwrap();

        assert!(repo.list_collections().unwrap().is_empty());
    }

    // ── FTS Search ──────────────────────────────────────────────────

    #[test]
    fn search_finds_by_title() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let bm = sample_bookmark("https://example.com", "Rust Programming Guide");
        repo.insert(&bm).unwrap();

        let results = repo.search("Rust", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, bm.id);
    }

    #[test]
    fn search_finds_by_description() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let mut bm = sample_bookmark("https://example.com", "Some Page");
        bm.description = Some("Learn about quantum computing".to_string());
        repo.insert(&bm).unwrap();

        let results = repo.search("quantum", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, bm.id);
    }

    #[test]
    fn search_finds_by_note() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let mut bm = sample_bookmark("https://example.com", "Page");
        bm.note = Some("Remember to check the benchmarks section".to_string());
        repo.insert(&bm).unwrap();

        let results = repo.search("benchmarks", 10, None).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_empty_query_returns_empty() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let bm = sample_bookmark("https://example.com", "Test");
        repo.insert(&bm).unwrap();

        assert!(repo.search("", 10, None).unwrap().is_empty());
        assert!(repo.search("   ", 10, None).unwrap().is_empty());
    }

    #[test]
    fn search_zero_limit_returns_empty() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let bm = sample_bookmark("https://example.com", "Test");
        repo.insert(&bm).unwrap();

        assert!(repo.search("Test", 0, None).unwrap().is_empty());
    }

    #[test]
    fn search_with_collection_filter() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let mut bm1 = sample_bookmark("https://a.com", "Rust Tutorial");
        bm1.collections = vec!["dev".to_string()];
        repo.insert(&bm1).unwrap();

        let mut bm2 = sample_bookmark("https://b.com", "Rust Cookbook");
        bm2.collections = vec!["recipes".to_string()];
        repo.insert(&bm2).unwrap();

        let results = repo.search("Rust", 10, Some("dev")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, bm1.id);
    }

    // ── FTS sync after update/delete ────────────────────────────────

    #[test]
    fn search_reflects_update() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let mut bm = sample_bookmark("https://example.com", "Old Title");
        repo.insert(&bm).unwrap();
        assert_eq!(repo.search("Old", 10, None).unwrap().len(), 1);

        bm.title = "New Title".to_string();
        repo.update(&bm).unwrap();

        assert!(repo.search("Old", 10, None).unwrap().is_empty());
        assert_eq!(repo.search("New", 10, None).unwrap().len(), 1);
    }

    #[test]
    fn search_reflects_delete() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let bm = sample_bookmark("https://example.com", "Deletable Content");
        repo.insert(&bm).unwrap();
        assert_eq!(repo.search("Deletable", 10, None).unwrap().len(), 1);

        repo.delete(&bm.id).unwrap();
        assert!(repo.search("Deletable", 10, None).unwrap().is_empty());
    }

    // ── Summary (DB-internal) ───────────────────────────────────────

    #[test]
    fn set_summary_and_search() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let bm = sample_bookmark("https://example.com", "Page Title");
        repo.insert(&bm).unwrap();

        repo.set_summary(&bm.id, "A comprehensive overview of microservices")
            .unwrap();

        let results = repo.search("microservices", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, bm.id);
    }

    // ── Search relevance ordering ───────────────────────────────────

    #[test]
    fn search_ranks_better_match_higher() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        // bm1 has "rust" only in tags (JSON text)
        let mut bm1 = sample_bookmark("https://other.com", "Some Page");
        bm1.user_tags = vec!["rust".to_string()];
        bm1.saved_at = Utc::now() - chrono::Duration::seconds(10);
        repo.insert(&bm1).unwrap();

        // bm2 has "rust" in title and description — stronger match
        let mut bm2 = sample_bookmark("https://rust.com", "Rust Programming");
        bm2.description = Some("The Rust programming language".to_string());
        bm2.saved_at = Utc::now();
        repo.insert(&bm2).unwrap();

        let results = repo.search("Rust", 10, None).unwrap();
        assert_eq!(results.len(), 2);
        // The result with title+description match should rank first
        assert_eq!(results[0].id, bm2.id);
    }

    // ── Corrupt data handling ───────────────────────────────────────

    #[test]
    fn corrupt_json_in_tags_surfaces_decode_error() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let bm = sample_bookmark("https://example.com", "Test");
        repo.insert(&bm).unwrap();

        // Inject corrupt JSON directly
        conn.execute(
            "UPDATE bookmarks SET user_tags = 'not-json' WHERE id = ?1",
            params![bm.id],
        )
        .unwrap();

        let result = repo.get_by_id(&bm.id);
        assert!(
            result.is_err(),
            "should surface decode error for corrupt JSON"
        );
    }

    #[test]
    fn corrupt_enum_in_state_surfaces_decode_error() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let bm = sample_bookmark("https://example.com", "Test");
        repo.insert(&bm).unwrap();

        conn.execute(
            "UPDATE bookmarks SET state = 'invalid_state' WHERE id = ?1",
            params![bm.id],
        )
        .unwrap();

        let result = repo.get_by_id(&bm.id);
        assert!(
            result.is_err(),
            "should surface decode error for invalid enum"
        );
    }

    #[test]
    fn corrupt_timestamp_surfaces_decode_error() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);

        let bm = sample_bookmark("https://example.com", "Test");
        repo.insert(&bm).unwrap();

        conn.execute(
            "UPDATE bookmarks SET saved_at = 'not-a-date' WHERE id = ?1",
            params![bm.id],
        )
        .unwrap();

        let result = repo.get_by_id(&bm.id);
        assert!(
            result.is_err(),
            "should surface decode error for invalid timestamp"
        );
    }

    // ── File-based DB ───────────────────────────────────────────────

    #[test]
    fn open_file_based_db() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let conn = db::open_and_migrate(&db_path).unwrap();
        let repo = BookmarkRepository::new(&conn);

        let bm = sample_bookmark("https://example.com", "File DB Test");
        repo.insert(&bm).unwrap();

        let loaded = repo.get_by_id(&bm.id).unwrap().unwrap();
        assert_eq!(loaded.title, "File DB Test");
    }

    #[test]
    fn open_existing_zero_byte_db() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("empty.db");
        std::fs::write(&db_path, b"").unwrap();

        let conn = db::open_and_migrate(&db_path).unwrap();
        let repo = BookmarkRepository::new(&conn);

        let bm = sample_bookmark("https://example.com", "Zero Byte");
        repo.insert(&bm).unwrap();
        assert!(repo.get_by_id(&bm.id).unwrap().is_some());
    }

    #[test]
    fn repeated_open_preserves_data() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("persistent.db");

        let bm_id;
        {
            let conn = db::open_and_migrate(&db_path).unwrap();
            let repo = BookmarkRepository::new(&conn);
            let bm = sample_bookmark("https://example.com", "Persist");
            bm_id = bm.id.clone();
            repo.insert(&bm).unwrap();
        }

        // Reopen
        let conn = db::open_and_migrate(&db_path).unwrap();
        let repo = BookmarkRepository::new(&conn);
        let loaded = repo.get_by_id(&bm_id).unwrap().unwrap();
        assert_eq!(loaded.title, "Persist");
    }

    // ── Optional fields as NULL ─────────────────────────────────────

    #[test]
    fn optional_fields_roundtrip_as_none() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);
        let bm = sample_bookmark("https://example.com", "Minimal");
        repo.insert(&bm).unwrap();

        let loaded = repo.get_by_id(&bm.id).unwrap().unwrap();
        assert_eq!(loaded.description, None);
        assert_eq!(loaded.author, None);
        assert_eq!(loaded.site_name, None);
        assert_eq!(loaded.published_at, None);
        assert_eq!(loaded.note, None);
        assert_eq!(loaded.action_prompt, None);
        assert_eq!(loaded.content_hash, None);
    }

    // ── Empty arrays ────────────────────────────────────────────────

    #[test]
    fn empty_arrays_roundtrip() {
        let conn = setup();
        let repo = BookmarkRepository::new(&conn);
        let bm = sample_bookmark("https://example.com", "Empty Arrays");
        repo.insert(&bm).unwrap();

        let loaded = repo.get_by_id(&bm.id).unwrap().unwrap();
        assert!(loaded.user_tags.is_empty());
        assert!(loaded.suggested_tags.is_empty());
        assert!(loaded.collections.is_empty());
    }
}
