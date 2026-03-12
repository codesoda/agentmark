//! List command: browse saved bookmarks with optional filters.

use crate::cli::{ListArgs, StateFilter};
use crate::config::{self, Config};
use crate::db::{self, BookmarkRepository};
use crate::display;
use crate::models::BookmarkState;

/// Entry point for `agentmark list`.
pub fn run_list(args: ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    let config = Config::load(&home)?;
    let db_path = config::index_db_path(&home);
    let conn = db::open_and_migrate(&db_path)?;
    let repo = BookmarkRepository::new(&conn);

    let state = args.state.as_ref().map(state_filter_to_model);

    let bookmarks = repo.list(
        args.limit as usize,
        0,
        args.collection.as_deref(),
        args.tag.as_deref(),
        state.as_ref(),
    )?;

    let _ = config; // config loaded to verify init; not used otherwise

    let width = display::terminal_width();
    let output = display::format_list(&bookmarks, width);
    println!("{output}");

    Ok(())
}

/// Map CLI-local state filter to shared model enum.
fn state_filter_to_model(filter: &StateFilter) -> BookmarkState {
    match filter {
        StateFilter::Inbox => BookmarkState::Inbox,
        StateFilter::Processed => BookmarkState::Processed,
        StateFilter::Archived => BookmarkState::Archived,
    }
}
