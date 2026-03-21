//! Tag command: add or remove user tags on a bookmark.

use std::fmt;

use tracing::{debug, instrument};

use crate::bundle::Bundle;
use crate::cli::TagArgs;
use crate::config::{self, Config};
use crate::db::{self, BookmarkRepository, DbError};

// ── Error type ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum TagError {
    Config(crate::config::ConfigError),
    Db(DbError),
    NotFound { id: String },
    BundleDrift { id: String, detail: String },
    NoValidTags,
    PartialUpdate { id: String, detail: String },
}

impl fmt::Display for TagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TagError::Config(e) => write!(f, "{e}"),
            TagError::Db(e) => write!(f, "database error: {e}"),
            TagError::NotFound { id } => write!(f, "bookmark not found: {id}"),
            TagError::BundleDrift { id, detail } => {
                write!(f, "bundle/index drift for {id}: {detail}")
            }
            TagError::NoValidTags => {
                write!(f, "no valid tags provided (all were empty or whitespace)")
            }
            TagError::PartialUpdate { id, detail } => {
                write!(
                    f,
                    "warning: bundle updated for {id} but index update failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for TagError {}

impl From<crate::config::ConfigError> for TagError {
    fn from(e: crate::config::ConfigError) -> Self {
        TagError::Config(e)
    }
}

impl From<DbError> for TagError {
    fn from(e: DbError) -> Self {
        TagError::Db(e)
    }
}

// ── Pure tag mutation helpers ───────────────────────────────────────

/// Normalize a list of tag strings: trim whitespace, drop empties, deduplicate.
fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for tag in tags {
        let trimmed = tag.trim().to_string();
        if !trimmed.is_empty() && !result.contains(&trimmed) {
            result.push(trimmed);
        }
    }
    result
}

/// Add tags to existing list, preserving order, appending only unique new values.
fn add_tags(existing: &[String], new: &[String]) -> Vec<String> {
    let mut result = existing.to_vec();
    for tag in new {
        if !result.contains(tag) {
            result.push(tag.clone());
        }
    }
    result
}

/// Remove tags from existing list by exact match.
fn remove_tags(existing: &[String], to_remove: &[String]) -> Vec<String> {
    existing
        .iter()
        .filter(|t| !to_remove.contains(t))
        .cloned()
        .collect()
}

// ── Entry point ─────────────────────────────────────────────────────

/// Entry point for `agentmark tag <id> <tags...>` / `agentmark tag <id> --remove <tags...>`.
#[instrument(skip(args), fields(id = %args.id))]
pub fn run_tag(args: TagArgs) -> Result<(), Box<dyn std::error::Error>> {
    let is_remove = !args.remove.is_empty();
    let raw_tags = if is_remove { &args.remove } else { &args.tags };
    let normalized = normalize_tags(raw_tags);

    if normalized.is_empty() {
        return Err(Box::new(TagError::NoValidTags));
    }

    let home = config::home_dir()?;
    let config = Config::load(&home)?;
    let db_path = config::index_db_path(&home);
    let conn = db::open_and_migrate(&db_path)?;
    let repo = BookmarkRepository::new(&conn);

    // Fetch bookmark
    let mut bookmark = repo
        .get_by_id(&args.id)?
        .ok_or_else(|| TagError::NotFound {
            id: args.id.clone(),
        })?;

    debug!(remove = is_remove, tags = ?normalized, "updating tags");

    // Mutate tags
    bookmark.user_tags = if is_remove {
        remove_tags(&bookmark.user_tags, &normalized)
    } else {
        add_tags(&bookmark.user_tags, &normalized)
    };

    // Bundle-first update: locate and rewrite bookmark.md
    let bundle =
        Bundle::find(&config.storage_path, &bookmark.saved_at, &bookmark.id).map_err(|e| {
            TagError::BundleDrift {
                id: bookmark.id.clone(),
                detail: e.to_string(),
            }
        })?;

    bundle
        .update_bookmark_md_preserving_body(&bookmark)
        .map_err(|e| TagError::BundleDrift {
            id: bookmark.id.clone(),
            detail: format!("bookmark.md rewrite: {e}"),
        })?;

    // DB update: treat Ok(false) as concurrent-delete failure
    match repo.update(&bookmark) {
        Ok(true) => {}
        Ok(false) => {
            return Err(Box::new(TagError::PartialUpdate {
                id: bookmark.id.clone(),
                detail: "bookmark was deleted from index after bundle update".to_string(),
            }));
        }
        Err(e) => {
            return Err(Box::new(TagError::PartialUpdate {
                id: bookmark.id.clone(),
                detail: format!("index update failed: {e}"),
            }));
        }
    }

    // Print confirmation
    if bookmark.user_tags.is_empty() {
        println!("Tags updated for {}: (none)", bookmark.id);
    } else {
        println!(
            "Tags updated for {}: {}",
            bookmark.id,
            bookmark.user_tags.join(", ")
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── normalize_tags ──────────────────────────────────────────────

    #[test]
    fn normalize_trims_and_drops_empty() {
        let input = vec![
            " rust ".to_string(),
            "".to_string(),
            "  ".to_string(),
            "cli".to_string(),
        ];
        assert_eq!(normalize_tags(&input), vec!["rust", "cli"]);
    }

    #[test]
    fn normalize_deduplicates() {
        let input = vec!["rust".to_string(), "cli".to_string(), "rust".to_string()];
        assert_eq!(normalize_tags(&input), vec!["rust", "cli"]);
    }

    #[test]
    fn normalize_all_empty() {
        let input = vec!["  ".to_string(), "".to_string()];
        assert!(normalize_tags(&input).is_empty());
    }

    // ── add_tags ────────────────────────────────────────────────────

    #[test]
    fn add_appends_unique_only() {
        let existing = vec!["rust".to_string(), "cli".to_string()];
        let new = vec!["cli".to_string(), "tools".to_string()];
        assert_eq!(add_tags(&existing, &new), vec!["rust", "cli", "tools"]);
    }

    #[test]
    fn add_preserves_order() {
        let existing = vec!["b".to_string()];
        let new = vec!["a".to_string(), "c".to_string()];
        assert_eq!(add_tags(&existing, &new), vec!["b", "a", "c"]);
    }

    #[test]
    fn add_all_duplicates_returns_existing() {
        let existing = vec!["rust".to_string()];
        let new = vec!["rust".to_string()];
        assert_eq!(add_tags(&existing, &new), vec!["rust"]);
    }

    #[test]
    fn add_to_empty() {
        let new = vec!["rust".to_string(), "cli".to_string()];
        assert_eq!(add_tags(&[], &new), vec!["rust", "cli"]);
    }

    // ── remove_tags ─────────────────────────────────────────────────

    #[test]
    fn remove_filters_exact_match() {
        let existing = vec!["rust".to_string(), "cli".to_string(), "tools".to_string()];
        let to_remove = vec!["cli".to_string()];
        assert_eq!(remove_tags(&existing, &to_remove), vec!["rust", "tools"]);
    }

    #[test]
    fn remove_absent_is_noop() {
        let existing = vec!["rust".to_string()];
        let to_remove = vec!["nonexistent".to_string()];
        assert_eq!(remove_tags(&existing, &to_remove), vec!["rust"]);
    }

    #[test]
    fn remove_all_tags() {
        let existing = vec!["rust".to_string(), "cli".to_string()];
        let to_remove = vec!["rust".to_string(), "cli".to_string()];
        assert!(remove_tags(&existing, &to_remove).is_empty());
    }

    #[test]
    fn remove_from_empty() {
        let to_remove = vec!["rust".to_string()];
        assert!(remove_tags(&[], &to_remove).is_empty());
    }
}
