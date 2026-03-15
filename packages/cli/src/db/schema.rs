//! Schema creation and migration for the bookmark index database.
//!
//! Uses `PRAGMA user_version` for versioning. Each migration bumps the version
//! and runs inside a transaction where SQLite allows it.

use rusqlite::Connection;

use super::DbError;

/// Current schema version. Bump when adding migrations.
const CURRENT_VERSION: u32 = 1;

/// Ensure the database schema is at the current version.
///
/// - Version 0 (fresh DB): creates the full schema.
/// - Version == CURRENT_VERSION: no-op.
/// - Version > CURRENT_VERSION: error (downgrade not supported).
pub fn ensure_schema(conn: &Connection) -> Result<(), DbError> {
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| DbError::Migration(format!("failed to read user_version: {e}")))?;

    if version == CURRENT_VERSION {
        return Ok(());
    }
    if version > CURRENT_VERSION {
        return Err(DbError::Migration(format!(
            "database version {version} is newer than supported version {CURRENT_VERSION}"
        )));
    }

    // Version 0 → 1: initial schema
    if version < 1 {
        migrate_v1(conn)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn fresh_db_migrates_to_v1() {
        let conn = fresh_conn();
        ensure_schema(&conn).unwrap();

        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn idempotent_schema_bootstrap() {
        let conn = fresh_conn();
        ensure_schema(&conn).unwrap();
        ensure_schema(&conn).unwrap(); // second call is a no-op

        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
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
