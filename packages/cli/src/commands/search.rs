//! Search command: full-text search across bookmarks using FTS5.

use crate::cli::SearchArgs;
use crate::config::{self, Config};
use crate::db::{self, BookmarkRepository};
use crate::display;

/// Entry point for `agentmark search`.
pub fn run_search(args: SearchArgs) -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    let config = Config::load(&home)?;
    let db_path = config::index_db_path(&home);
    let conn = db::open_and_migrate(&db_path)?;
    let repo = BookmarkRepository::new(&conn);

    let bookmarks = repo.search(&args.query, args.limit as usize, args.collection.as_deref())?;

    let _ = config; // config loaded to verify init; not used otherwise

    if bookmarks.is_empty() {
        println!("No results found.");
    } else {
        let width = display::terminal_width();
        let output = display::format_list(&bookmarks, width);
        println!("{output}");
    }

    Ok(())
}
