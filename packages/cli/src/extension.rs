//! Embedded Chrome extension assets and extraction helpers.
//!
//! The extension `dist/` directory is included at compile time via `include_dir!`.
//! If the extension was not built before `cargo build`, the embedded directory
//! will be empty and `is_embedded()` returns false.

use std::path::Path;

use include_dir::{include_dir, Dir};

static EXTENSION_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../extension/dist");

/// Returns true if the build embedded a non-empty extension directory.
pub fn is_embedded() -> bool {
    // Check for at least one file (not just empty dirs)
    EXTENSION_DIR.files().next().is_some()
        || EXTENSION_DIR.dirs().any(|d| d.files().next().is_some())
}

/// Extract all embedded extension files to the target directory.
///
/// Creates the target directory if it doesn't exist. Overwrites existing files.
pub fn extract_to(target: &Path) -> std::io::Result<()> {
    extract_dir(&EXTENSION_DIR, target)
}

fn extract_dir(dir: &Dir, target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;

    for file in dir.files() {
        let dest = target.join(file.path());
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, file.contents())?;
    }

    for subdir in dir.dirs() {
        extract_dir(subdir, target)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extract_creates_target_dir() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("ext");
        // Even if empty, extraction should not error
        let _ = extract_to(&target);
        assert!(target.exists());
    }
}
