//! Show command: display full details of a specific bookmark.

use std::fmt;

use crate::bundle::Bundle;
use crate::cli::ShowArgs;
use crate::config::{self, Config};
use crate::db::{self, BookmarkRepository, DbError};
use crate::display::{self, ShowDetail};

// ── Error type ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ShowError {
    Config(crate::config::ConfigError),
    Db(DbError),
    NotFound { id: String },
    BundleDrift { id: String, detail: String },
}

impl fmt::Display for ShowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShowError::Config(e) => write!(f, "{e}"),
            ShowError::Db(e) => write!(f, "database error: {e}"),
            ShowError::NotFound { id } => write!(f, "bookmark not found: {id}"),
            ShowError::BundleDrift { id, detail } => {
                write!(f, "bundle/index drift for {id}: {detail}")
            }
        }
    }
}

impl std::error::Error for ShowError {}

impl From<crate::config::ConfigError> for ShowError {
    fn from(e: crate::config::ConfigError) -> Self {
        ShowError::Config(e)
    }
}

impl From<DbError> for ShowError {
    fn from(e: DbError) -> Self {
        ShowError::Db(e)
    }
}

// ── Entry point ─────────────────────────────────────────────────────

/// Entry point for `agentmark show <id>`.
pub fn run_show(args: ShowArgs) -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    let config = Config::load(&home)?;
    let db_path = config::index_db_path(&home);
    let conn = db::open_and_migrate(&db_path)?;
    let repo = BookmarkRepository::new(&conn);

    let bookmark = repo
        .get_by_id(&args.id)?
        .ok_or_else(|| ShowError::NotFound {
            id: args.id.clone(),
        })?;

    // Locate bundle via saved_at + id
    let bundle =
        Bundle::find(&config.storage_path, &bookmark.saved_at, &bookmark.id).map_err(|e| {
            ShowError::BundleDrift {
                id: bookmark.id.clone(),
                detail: e.to_string(),
            }
        })?;

    // Read summary from bookmark.md body sections
    let summary = match bundle.read_body_sections() {
        Ok(sections) => sections.summary,
        Err(e) => {
            return Err(Box::new(ShowError::BundleDrift {
                id: bookmark.id.clone(),
                detail: format!("bookmark.md: {e}"),
            }));
        }
    };

    // Read article content
    let article = match bundle.read_article_md() {
        Ok(content) => Some(content),
        Err(e) => {
            return Err(Box::new(ShowError::BundleDrift {
                id: bookmark.id.clone(),
                detail: format!("article.md: {e}"),
            }));
        }
    };

    let detail = ShowDetail {
        bookmark: &bookmark,
        summary,
        article,
        full: args.full,
    };

    let use_color = display::color_enabled();
    let output = display::format_show(&detail, use_color);
    print!("{output}");

    Ok(())
}
