//! Show command: display full details of a specific bookmark.
//!
//! Uses the shared `bookmark_detail` helper for DB+bundle loading,
//! then adds article content and CLI formatting on top.

use crate::bundle::Bundle;
use crate::cli::ShowArgs;
use crate::config;
use crate::display::{self, ShowDetail};

use super::bookmark_detail::{self, DetailError};

// ── Entry point ─────────────────────────────────────────────────────

/// Entry point for `agentmark show <id>`.
pub fn run_show(args: ShowArgs) -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;

    // Load DB+bundle detail via the shared helper (summary only, no article)
    let loaded = bookmark_detail::load_bookmark_detail(&home, &args.id)?;

    // Read article content from the bundle (show-specific; detail helper skips this)
    let config = config::Config::load(&home)?;
    let bundle = Bundle::find(
        &config.storage_path,
        &loaded.bookmark.saved_at,
        &loaded.bookmark.id,
    )
    .map_err(|e| DetailError::BundleDrift {
        id: loaded.bookmark.id.clone(),
        detail: e.to_string(),
    })?;

    let article = match bundle.read_article_md() {
        Ok(content) => Some(content),
        Err(e) => {
            return Err(Box::new(DetailError::BundleDrift {
                id: loaded.bookmark.id.clone(),
                detail: format!("article.md: {e}"),
            }));
        }
    };

    let detail = ShowDetail {
        bookmark: &loaded.bookmark,
        summary: loaded.summary,
        article,
        full: args.full,
    };

    let use_color = display::color_enabled();
    let output = display::format_show(&detail, use_color);
    print!("{output}");

    Ok(())
}
