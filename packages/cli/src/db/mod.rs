//! SQLite index database for bookmark storage and full-text search.
//!
//! The database is a derived index over filesystem bundles — it is not the
//! canonical data store. The DB layer accepts explicit paths or connections
//! so that callers (commands, tests) control where the database lives.
//!
//! State routing is index-only for now: `Bookmark.state` and the DB `state`
//! column are the sole state source. Hidden filesystem directories (`.inbox/`,
//! `.archive/`) are deferred until bundle-writing specs make that evaluation
//! concrete (see Specs 08/09).

pub mod repository;
pub mod schema;

use std::path::Path;

use rusqlite::Connection;
use thiserror::Error;
use tracing::{debug, instrument};

pub use repository::BookmarkRepository;

/// Errors originating from the database layer.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("failed to open database at {path}: {source}")]
    Open {
        path: String,
        source: rusqlite::Error,
    },

    #[error("schema migration failed: {0}")]
    Migration(String),

    #[error("row decode error: {field}: {detail}")]
    Decode { field: String, detail: String },

    #[error("bookmark not found: {id}")]
    NotFound { id: String },
}

/// Open (or create) a SQLite database at `path` and ensure the schema is current.
#[instrument(fields(path = %path.display()))]
pub fn open_and_migrate(path: &Path) -> Result<Connection, DbError> {
    let conn = Connection::open(path).map_err(|e| DbError::Open {
        path: path.display().to_string(),
        source: e,
    })?;
    schema::ensure_schema(&conn)?;
    debug!("database opened and schema verified");
    Ok(conn)
}

/// Open an in-memory database with the schema applied. Useful for tests.
#[cfg(test)]
pub fn open_memory() -> Result<Connection, DbError> {
    let conn = Connection::open_in_memory().map_err(|e| DbError::Open {
        path: ":memory:".to_string(),
        source: e,
    })?;
    schema::ensure_schema(&conn)?;
    Ok(conn)
}
