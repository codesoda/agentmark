//! Open command: open a bookmark URL in the default browser.

use std::fmt;
use std::process::Command as ProcessCommand;

use crate::cli::OpenArgs;
use crate::config::{self, Config};
use crate::db::{self, BookmarkRepository, DbError};

// ── Error type ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum OpenError {
    Config(crate::config::ConfigError),
    Db(DbError),
    NotFound { id: String },
    LaunchFailed { url: String, detail: String },
    UnsupportedPlatform,
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::Config(e) => write!(f, "{e}"),
            OpenError::Db(e) => write!(f, "database error: {e}"),
            OpenError::NotFound { id } => write!(f, "bookmark not found: {id}"),
            OpenError::LaunchFailed { url, detail } => {
                write!(f, "failed to open {url}: {detail}")
            }
            OpenError::UnsupportedPlatform => {
                write!(f, "unsupported platform: no known browser opener command")
            }
        }
    }
}

impl std::error::Error for OpenError {}

impl From<crate::config::ConfigError> for OpenError {
    fn from(e: crate::config::ConfigError) -> Self {
        OpenError::Config(e)
    }
}

impl From<DbError> for OpenError {
    fn from(e: DbError) -> Self {
        OpenError::Db(e)
    }
}

// ── Browser launcher seam ───────────────────────────────────────────

/// Resolve the browser opener command.
///
/// Checks `AGENTMARK_OPENER` env var first (for tests), then falls back
/// to the platform default.
fn resolve_opener() -> Result<String, OpenError> {
    if let Ok(override_cmd) = std::env::var("AGENTMARK_OPENER") {
        return Ok(override_cmd);
    }

    if cfg!(target_os = "macos") {
        Ok("open".to_string())
    } else if cfg!(target_os = "linux") {
        Ok("xdg-open".to_string())
    } else {
        Err(OpenError::UnsupportedPlatform)
    }
}

/// Launch a URL in the browser using the resolved opener command.
fn launch_url(opener: &str, url: &str) -> Result<(), OpenError> {
    let result = ProcessCommand::new(opener).arg(url).status();

    match result {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(OpenError::LaunchFailed {
            url: url.to_string(),
            detail: format!("opener exited with {status}"),
        }),
        Err(e) => Err(OpenError::LaunchFailed {
            url: url.to_string(),
            detail: format!("failed to spawn opener: {e}"),
        }),
    }
}

// ── Entry point ─────────────────────────────────────────────────────

/// Entry point for `agentmark open <id>`.
pub fn run_open(args: OpenArgs) -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    let _config = Config::load(&home)?;
    let db_path = config::index_db_path(&home);
    let conn = db::open_and_migrate(&db_path)?;
    let repo = BookmarkRepository::new(&conn);

    let bookmark = repo
        .get_by_id(&args.id)?
        .ok_or_else(|| OpenError::NotFound {
            id: args.id.clone(),
        })?;

    let opener = resolve_opener()?;
    launch_url(&opener, &bookmark.url)?;

    println!("Opened {} in browser", bookmark.url);

    Ok(())
}
