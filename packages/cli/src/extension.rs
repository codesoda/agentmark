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
    !EXTENSION_DIR.entries().is_empty()
}

/// Extract all embedded extension files to the target directory.
///
/// Creates the target directory if it doesn't exist.
/// Uses `include_dir`'s built-in `Dir::extract()` which handles nested
/// paths correctly (file paths are relative to the embedded root).
pub fn extract_to(target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    EXTENSION_DIR.extract(target)
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

    /// The development copy keeps the manifest `key` (only the store zip
    /// drops it), so "Load unpacked" gives the store ID that
    /// `register_native_hosts` already allows.
    #[test]
    fn embedded_manifest_keeps_the_key() {
        if !is_embedded() {
            return; // a build without the extension has nothing to check
        }
        let tmp = TempDir::new().unwrap();
        extract_to(tmp.path()).unwrap();
        let text = std::fs::read_to_string(tmp.path().join("manifest.json")).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&text).unwrap();
        let source: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../extension/manifest.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(manifest["key"].is_string(), "embedded manifest has no key");
        assert_eq!(manifest["key"], source["key"]);
    }
}
