//! Schema creation and migration for the bookmark index database.
//!
//! Uses `PRAGMA user_version` for versioning. Each migration bumps the version
//! and runs inside a transaction where SQLite allows it.

use rusqlite::Connection;
use tracing::{debug, info, warn};

use super::DbError;
use crate::canonical;

/// Current schema version. Bump when adding migrations.
const CURRENT_VERSION: u32 = 2;

/// Ensure the database schema is at the current version.
///
/// - Version 0 (fresh DB): creates the full schema.
/// - Version == CURRENT_VERSION: no-op.
/// - Version > CURRENT_VERSION: error (downgrade not supported).
pub fn ensure_schema(conn: &Connection) -> Result<(), DbError> {
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| DbError::Migration(format!("failed to read user_version: {e}")))?;

    debug!(version, target = CURRENT_VERSION, "checking schema version");
    if version == CURRENT_VERSION {
        return Ok(());
    }
    if version > CURRENT_VERSION {
        warn!(
            version,
            supported = CURRENT_VERSION,
            "schema version too new"
        );
        return Err(DbError::Migration(format!(
            "database version {version} is newer than supported version {CURRENT_VERSION}"
        )));
    }

    info!(
        from = version,
        to = CURRENT_VERSION,
        "migrating database schema"
    );
    if version < 1 {
        migrate_v1(conn)?;
    }
    if version < 2 {
        migrate_v2(conn)?;
    }

    Ok(())
}

/// Migration 0 → 1: create bookmarks table, indexes, FTS5 table, and triggers.
fn migrate_v1(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "BEGIN;

        CREATE TABLE IF NOT EXISTS bookmarks (
            id              TEXT PRIMARY KEY NOT NULL,
            url             TEXT NOT NULL,
            canonical_url   TEXT NOT NULL,
            title           TEXT NOT NULL,
            description     TEXT,
            author          TEXT,
            site_name       TEXT,
            published_at    TEXT,
            saved_at        TEXT NOT NULL,
            capture_source  TEXT NOT NULL,
            user_tags       TEXT NOT NULL DEFAULT '[]',
            suggested_tags  TEXT NOT NULL DEFAULT '[]',
            collections     TEXT NOT NULL DEFAULT '[]',
            note            TEXT,
            action_prompt   TEXT,
            state           TEXT NOT NULL DEFAULT 'inbox',
            content_status  TEXT NOT NULL DEFAULT 'pending',
            summary_status  TEXT NOT NULL DEFAULT 'pending',
            content_hash    TEXT,
            schema_version  INTEGER NOT NULL DEFAULT 1,
            summary         TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_bookmarks_canonical_url
            ON bookmarks(canonical_url);

        CREATE INDEX IF NOT EXISTS idx_bookmarks_saved_at
            ON bookmarks(saved_at);

        CREATE VIRTUAL TABLE IF NOT EXISTS bookmarks_fts USING fts5(
            title,
            description,
            url,
            note,
            user_tags,
            suggested_tags,
            summary,
            content=bookmarks,
            content_rowid=rowid
        );

        -- Triggers to keep FTS in sync with the main table.
        CREATE TRIGGER IF NOT EXISTS bookmarks_ai AFTER INSERT ON bookmarks BEGIN
            INSERT INTO bookmarks_fts(rowid, title, description, url, note, user_tags, suggested_tags, summary)
            VALUES (new.rowid, new.title, new.description, new.url, new.note, new.user_tags, new.suggested_tags, new.summary);
        END;

        CREATE TRIGGER IF NOT EXISTS bookmarks_ad AFTER DELETE ON bookmarks BEGIN
            INSERT INTO bookmarks_fts(bookmarks_fts, rowid, title, description, url, note, user_tags, suggested_tags, summary)
            VALUES ('delete', old.rowid, old.title, old.description, old.url, old.note, old.user_tags, old.suggested_tags, old.summary);
        END;

        CREATE TRIGGER IF NOT EXISTS bookmarks_au AFTER UPDATE ON bookmarks BEGIN
            INSERT INTO bookmarks_fts(bookmarks_fts, rowid, title, description, url, note, user_tags, suggested_tags, summary)
            VALUES ('delete', old.rowid, old.title, old.description, old.url, old.note, old.user_tags, old.suggested_tags, old.summary);
            INSERT INTO bookmarks_fts(rowid, title, description, url, note, user_tags, suggested_tags, summary)
            VALUES (new.rowid, new.title, new.description, new.url, new.note, new.user_tags, new.suggested_tags, new.summary);
        END;

        PRAGMA user_version = 1;

        COMMIT;",
    )
    .map_err(|e| DbError::Migration(format!("v1 migration failed: {e}")))?;

    Ok(())
}

/// Migration 1 → 2: `bookmark_urls`, every canonical URL a bookmark has been
/// seen under, backfilled from the existing rows.
///
/// The backfill canonicalizes the raw `url` column in Rust, so the DDL, the
/// backfill and the version bump share one explicit transaction: a failure
/// rolls everything back and leaves the database at v1.
fn migrate_v2(conn: &Connection) -> Result<(), DbError> {
    let fail = |e: rusqlite::Error| DbError::Migration(format!("v2 migration failed: {e}"));

    conn.execute_batch("BEGIN;").map_err(fail)?;
    let result = migrate_v2_body(conn);
    match result {
        Ok(()) => conn.execute_batch("COMMIT;").map_err(fail),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(fail(e))
        }
    }
}

fn migrate_v2_body(conn: &Connection) -> Result<(), rusqlite::Error> {
    // No foreign key: FK enforcement is off in this project, so the trigger
    // does the cleanup when a bookmark is deleted.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS bookmark_urls (
            canonical_url TEXT NOT NULL,
            bookmark_id   TEXT NOT NULL,
            PRIMARY KEY (canonical_url, bookmark_id)
        );

        CREATE INDEX IF NOT EXISTS idx_bookmark_urls_bookmark_id
            ON bookmark_urls(bookmark_id);

        CREATE TRIGGER IF NOT EXISTS bookmark_urls_ad AFTER DELETE ON bookmarks BEGIN
            DELETE FROM bookmark_urls WHERE bookmark_id = old.id;
        END;",
    )?;

    let rows: Vec<(String, String, String)> = {
        let mut stmt = conn.prepare("SELECT id, url, canonical_url FROM bookmarks")?;
        let mapped = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        mapped.collect::<Result<_, _>>()?
    };

    let mut insert = conn.prepare(
        "INSERT OR IGNORE INTO bookmark_urls (canonical_url, bookmark_id) VALUES (?1, ?2)",
    )?;
    for (id, url, canonical_url) in &rows {
        insert.execute(rusqlite::params![canonical_url, id])?;
        // Older saves may have stored a canonical that differs from the
        // requested URL (page-declared canonical); remember both.
        if let Ok(from_url) = canonical::canonicalize(url) {
            insert.execute(rusqlite::params![from_url, id])?;
        }
    }
    drop(insert);

    conn.pragma_update(None, "user_version", 2)?;
    debug!(backfilled = rows.len(), "bookmark_urls backfilled");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    fn user_version(conn: &Connection) -> u32 {
        conn.pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }

    fn url_rows(conn: &Connection) -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare("SELECT canonical_url, bookmark_id FROM bookmark_urls ORDER BY 1, 2")
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    /// Build a v1 database with two bookmarks, as an older binary left it.
    fn v1_conn_with_rows() -> Connection {
        let conn = fresh_conn();
        migrate_v1(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO bookmarks (id, url, canonical_url, title, saved_at, capture_source)
             VALUES ('am_a', 'https://www.linkedin.com/feed/update/urn:li:activity:1/?utm_source=x',
                     'https://linkedin.com/posts/someone_activity-1', 'A',
                     '2026-01-01T00:00:00+00:00', 'cli');
             INSERT INTO bookmarks (id, url, canonical_url, title, saved_at, capture_source)
             VALUES ('am_b', 'not a url', 'https://example.com/b', 'B',
                     '2026-01-02T00:00:00+00:00', 'cli');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn fresh_db_migrates_to_current_version() {
        let conn = fresh_conn();
        ensure_schema(&conn).unwrap();
        assert_eq!(user_version(&conn), 2);
    }

    #[test]
    fn bookmark_urls_table_exists() {
        let conn = fresh_conn();
        ensure_schema(&conn).unwrap();

        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='bookmark_urls'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn v1_db_migrates_and_backfills_bookmark_urls() {
        let conn = v1_conn_with_rows();
        assert_eq!(user_version(&conn), 1);

        ensure_schema(&conn).unwrap();
        assert_eq!(user_version(&conn), 2);

        assert_eq!(
            url_rows(&conn),
            vec![
                // Unparseable raw url: only the stored canonical is kept.
                ("https://example.com/b".to_string(), "am_b".to_string()),
                // Raw url canonicalized; tracking param and www stripped.
                (
                    "https://linkedin.com/feed/update/urn:li:activity:1".to_string(),
                    "am_a".to_string()
                ),
                (
                    "https://linkedin.com/posts/someone_activity-1".to_string(),
                    "am_a".to_string()
                ),
            ]
        );
    }

    #[test]
    fn v2_migration_is_idempotent() {
        let conn = v1_conn_with_rows();
        ensure_schema(&conn).unwrap();
        let first = url_rows(&conn);

        // Re-running the migration body must not duplicate rows or fail.
        migrate_v2(&conn).unwrap();
        ensure_schema(&conn).unwrap();
        assert_eq!(url_rows(&conn), first);
        assert_eq!(user_version(&conn), 2);
    }

    #[test]
    fn deleting_bookmark_removes_its_urls() {
        let conn = v1_conn_with_rows();
        ensure_schema(&conn).unwrap();
        conn.execute("DELETE FROM bookmarks WHERE id = 'am_a'", [])
            .unwrap();
        assert_eq!(
            url_rows(&conn),
            vec![("https://example.com/b".to_string(), "am_b".to_string())]
        );
    }

    #[test]
    fn idempotent_schema_bootstrap() {
        let conn = fresh_conn();
        ensure_schema(&conn).unwrap();
        ensure_schema(&conn).unwrap(); // second call is a no-op
        assert_eq!(user_version(&conn), 2);
    }

    #[test]
    fn bookmarks_table_exists() {
        let conn = fresh_conn();
        ensure_schema(&conn).unwrap();

        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='bookmarks'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn fts_table_exists() {
        let conn = fresh_conn();
        ensure_schema(&conn).unwrap();

        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='bookmarks_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn triggers_exist() {
        let conn = fresh_conn();
        ensure_schema(&conn).unwrap();

        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name LIKE 'bookmarks_a%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 3, "expected 3 triggers (ai, ad, au)");
    }

    #[test]
    fn future_version_errors() {
        let conn = fresh_conn();
        conn.pragma_update(None, "user_version", 999).unwrap();
        let err = ensure_schema(&conn).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("newer"),
            "error should mention newer version: {msg}"
        );
    }
}
