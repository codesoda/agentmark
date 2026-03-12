//! Collections command: list all collections with bookmark counts.

use crate::config::{self, Config};
use crate::db::{self, BookmarkRepository};

/// Entry point for `agentmark collections`.
pub fn run_collections() -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    let _config = Config::load(&home)?;
    let db_path = config::index_db_path(&home);
    let conn = db::open_and_migrate(&db_path)?;
    let repo = BookmarkRepository::new(&conn);

    let collections = repo.list_collections()?;

    if collections.is_empty() {
        println!("No collections found.");
    } else {
        for (name, count) in &collections {
            println!("{name}  ({count} bookmarks)");
        }
    }

    Ok(())
}
