//! Delete command: remove a bookmark's bundle directory and its index row.
//!
//! The bundle goes first: if the files cannot be removed the row stays, so the
//! bookmark is still listed and the user can retry. FTS and URL alias rows are
//! cleaned up by database triggers when the bookmark row is deleted.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;
use tracing::{debug, instrument};

use crate::bundle::{Bundle, BundleError};
use crate::cli::DeleteArgs;
use crate::commands::reprocess::read_confirmation;
use crate::config::{self, Config, ConfigError};
use crate::db::{self, BookmarkRepository, DbError};

// ── Error type ──────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum DeleteError {
    #[error("{0}")]
    Config(#[from] ConfigError),

    #[error("database error: {0}")]
    Db(#[from] DbError),

    #[error("bookmark not found: {id}")]
    NotFound { id: String },

    #[error("failed to locate files for {id}: {source}")]
    Bundle { id: String, source: BundleError },

    #[error("refusing to delete {path}: {reason}")]
    UnsafePath { path: PathBuf, reason: String },

    #[error("failed to delete files at {path}: {source}; bookmark kept in the index")]
    RemoveFailed {
        path: PathBuf,
        source: std::io::Error,
    },
}

// ── Entry point ─────────────────────────────────────────────────────

/// Entry point for `agentmark delete <id>`.
pub fn run_delete(args: DeleteArgs) -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut writer = std::io::stdout();
    execute_delete_with_io(&home, &args, &mut reader, &mut writer)?;
    Ok(())
}

/// Testable delete pipeline over an injectable confirmation reader and output.
#[instrument(skip(home, args, reader, writer), fields(id = %args.id))]
pub(crate) fn execute_delete_with_io(
    home: &Path,
    args: &DeleteArgs,
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> Result<(), DeleteError> {
    let config = Config::load(home)?;
    let conn = db::open_and_migrate(&config::index_db_path(home))?;
    let repo = BookmarkRepository::new(&conn);

    let bookmark = repo
        .get_by_id(&args.id)?
        .ok_or_else(|| DeleteError::NotFound {
            id: args.id.clone(),
        })?;

    let bundle_dir = match Bundle::find(&config.storage_path, &bookmark.saved_at, &bookmark.id) {
        Ok(bundle) => Some(bundle.path().to_path_buf()),
        Err(BundleError::BundleNotFound { .. }) => None,
        Err(source) => {
            return Err(DeleteError::Bundle {
                id: bookmark.id.clone(),
                source,
            })
        }
    };
    // Check before asking, so the user is never asked to confirm a delete
    // that would then be refused.
    if let Some(dir) = &bundle_dir {
        check_bundle_dir(&config.storage_path, dir, &bookmark.id)?;
    }

    writeln!(writer, "{}", bookmark.id).ok();
    writeln!(writer, "  title: {}", bookmark.title).ok();
    writeln!(writer, "  url:   {}", bookmark.url).ok();
    match &bundle_dir {
        Some(dir) => writeln!(writer, "  files: {}", dir.display()).ok(),
        None => writeln!(writer, "  files: no files found").ok(),
    };

    if !args.yes {
        write!(writer, "Delete this bookmark? (y/n) ").ok();
        writer.flush().ok();
        if !read_confirmation(reader) {
            writeln!(writer, "Aborted.").ok();
            return Ok(());
        }
    }

    if let Some(dir) = &bundle_dir {
        debug!(path = %dir.display(), "removing bundle directory");
        std::fs::remove_dir_all(dir).map_err(|source| DeleteError::RemoveFailed {
            path: dir.clone(),
            source,
        })?;
    }
    // The row can vanish while the user is at the prompt (another delete, a
    // second terminal); only claim the delete when this call removed it.
    if repo.delete(&bookmark.id)? {
        writeln!(writer, "Deleted {}", bookmark.id).ok();
    } else {
        writeln!(
            writer,
            "{} was already removed from the index by something else.",
            bookmark.id
        )
        .ok();
    }
    Ok(())
}

/// Refuse to remove anything that is not a bundle directory of this bookmark
/// inside the storage root. Guards against a bad config or a crafted
/// directory name turning `remove_dir_all` loose on unrelated files.
fn check_bundle_dir(storage_root: &Path, dir: &Path, id: &str) -> Result<(), DeleteError> {
    let unsafe_path = |reason: &str| DeleteError::UnsafePath {
        path: dir.to_path_buf(),
        reason: reason.to_string(),
    };
    // Resolve symlinks so `..` or a linked directory cannot escape the root.
    let root = std::fs::canonicalize(storage_root)
        .map_err(|e| unsafe_path(&format!("cannot resolve storage root: {e}")))?;
    let resolved = std::fs::canonicalize(dir)
        .map_err(|e| unsafe_path(&format!("cannot resolve path: {e}")))?;

    if resolved == root || !resolved.starts_with(&root) {
        return Err(unsafe_path("not inside the storage root"));
    }
    let name_matches = resolved
        .file_name()
        .map(|name| name.to_string_lossy().ends_with(&format!("-{id}")))
        .unwrap_or(false);
    if !name_matches {
        return Err(unsafe_path("directory name does not match the bookmark id"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::PageMetadata;
    use crate::models::Bookmark;
    use std::io::Cursor;
    use tempfile::TempDir;

    // ── check_bundle_dir ─────────────────────────────────────────────

    #[test]
    fn safety_accepts_bundle_inside_root() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("2026/03/01/title-am_1");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(check_bundle_dir(tmp.path(), &dir, "am_1").is_ok());
    }

    #[test]
    fn safety_rejects_directory_outside_root() {
        let root = TempDir::new().unwrap();
        let other = TempDir::new().unwrap();
        let dir = other.path().join("title-am_1");
        std::fs::create_dir_all(&dir).unwrap();
        let err = check_bundle_dir(root.path(), &dir, "am_1").unwrap_err();
        assert!(matches!(err, DeleteError::UnsafePath { .. }), "{err}");
    }

    #[test]
    fn safety_rejects_escape_through_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("storage");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(tmp.path().join("title-am_1")).unwrap();
        let dir = root.join("../title-am_1");
        assert!(check_bundle_dir(&root, &dir, "am_1").is_err());
    }

    #[test]
    fn safety_rejects_name_without_id_suffix() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("2026/03/01/title-am_2");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(check_bundle_dir(tmp.path(), &dir, "am_1").is_err());
    }

    #[test]
    fn safety_rejects_storage_root_itself() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("x-am_1");
        std::fs::create_dir_all(&root).unwrap();
        assert!(check_bundle_dir(&root, &root, "am_1").is_err());
    }

    // ── execute_delete_with_io ───────────────────────────────────────

    struct Env {
        tmp: TempDir,
        bookmark: Bookmark,
        bundle_dir: PathBuf,
    }

    fn env_with_bookmark() -> Env {
        let tmp = TempDir::new().unwrap();
        let storage = tmp.path().join("storage");
        std::fs::create_dir_all(tmp.path().join(".agentmark")).unwrap();
        std::fs::create_dir_all(&storage).unwrap();
        std::fs::write(
            tmp.path().join(".agentmark/config.toml"),
            format!(
                "default_agent = \"claude\"\nstorage_path = \"{}\"\n\n[enrichment]\nenabled = false\n",
                storage.display()
            ),
        )
        .unwrap();

        let bookmark = Bookmark::new("https://example.com/a", "Example A");
        let conn = db::open_and_migrate(&config::index_db_path(tmp.path())).unwrap();
        BookmarkRepository::new(&conn).insert(&bookmark).unwrap();
        let bundle = Bundle::create(
            &storage,
            &bookmark,
            &PageMetadata::default(),
            "article",
            "<html></html>",
            "cli",
        )
        .unwrap();
        let bundle_dir = bundle.path().to_path_buf();
        Env {
            tmp,
            bookmark,
            bundle_dir,
        }
    }

    fn run(env: &Env, yes: bool, input: &str) -> (Result<(), DeleteError>, String) {
        let args = DeleteArgs {
            id: env.bookmark.id.clone(),
            yes,
        };
        let mut reader = Cursor::new(input.as_bytes().to_vec());
        let mut out = Vec::new();
        let result = execute_delete_with_io(env.tmp.path(), &args, &mut reader, &mut out);
        (result, String::from_utf8(out).unwrap())
    }

    fn row_exists(env: &Env) -> bool {
        let conn = db::open_and_migrate(&config::index_db_path(env.tmp.path())).unwrap();
        BookmarkRepository::new(&conn)
            .get_by_id(&env.bookmark.id)
            .unwrap()
            .is_some()
    }

    #[test]
    fn confirmed_delete_removes_files_and_row() {
        let env = env_with_bookmark();
        let (result, out) = run(&env, false, "y\n");
        result.unwrap();
        assert!(!env.bundle_dir.exists());
        assert!(!row_exists(&env));
        assert!(out.contains("Example A"), "{out}");
        assert!(
            out.contains(&format!("Deleted {}", env.bookmark.id)),
            "{out}"
        );
    }

    #[test]
    fn declined_delete_keeps_everything() {
        for input in ["n\n", "\n", "", "maybe\n"] {
            let env = env_with_bookmark();
            let (result, out) = run(&env, false, input);
            result.unwrap();
            assert!(out.contains("Aborted."), "{input:?}: {out}");
            assert!(env.bundle_dir.exists());
            assert!(row_exists(&env));
        }
    }

    #[test]
    fn yes_flag_skips_prompt() {
        let env = env_with_bookmark();
        let (result, out) = run(&env, true, "");
        result.unwrap();
        assert!(!out.contains("(y/n)"), "{out}");
        assert!(!row_exists(&env));
    }

    #[test]
    fn missing_bundle_still_deletes_row() {
        let env = env_with_bookmark();
        std::fs::remove_dir_all(&env.bundle_dir).unwrap();
        let (result, out) = run(&env, true, "");
        result.unwrap();
        assert!(out.contains("no files found"), "{out}");
        assert!(!row_exists(&env));
    }

    /// Answers "y" but deletes the row first, like a second process would
    /// while the user is at the prompt.
    struct RacingReader<'a> {
        env: &'a Env,
        inner: Cursor<Vec<u8>>,
        raced: bool,
    }

    impl std::io::Read for RacingReader<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.inner.read(buf)
        }
    }

    impl BufRead for RacingReader<'_> {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            if !self.raced {
                self.raced = true;
                let conn =
                    db::open_and_migrate(&config::index_db_path(self.env.tmp.path())).unwrap();
                BookmarkRepository::new(&conn)
                    .delete(&self.env.bookmark.id)
                    .unwrap();
            }
            self.inner.fill_buf()
        }

        fn consume(&mut self, amt: usize) {
            self.inner.consume(amt)
        }
    }

    #[test]
    fn row_removed_during_prompt_is_not_reported_as_deleted() {
        let env = env_with_bookmark();
        let args = DeleteArgs {
            id: env.bookmark.id.clone(),
            yes: false,
        };
        let mut reader = RacingReader {
            env: &env,
            inner: Cursor::new(b"y\n".to_vec()),
            raced: false,
        };
        let mut out = Vec::new();
        execute_delete_with_io(env.tmp.path(), &args, &mut reader, &mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(!out.contains("Deleted"), "{out}");
        assert!(out.contains("already removed"), "{out}");
    }

    #[test]
    fn unknown_id_is_not_found() {
        let env = env_with_bookmark();
        let args = DeleteArgs {
            id: "am_missing".to_string(),
            yes: true,
        };
        let err = execute_delete_with_io(
            env.tmp.path(),
            &args,
            &mut Cursor::new(Vec::new()),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(matches!(err, DeleteError::NotFound { .. }), "{err}");
        assert!(row_exists(&env));
    }
}
