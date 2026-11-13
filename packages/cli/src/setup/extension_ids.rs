//! The extension IDs allowed to start the native host.
//!
//! The Web Store ID is a constant. The manifest's `key` gives an unpacked
//! copy the same ID, so it needs no development ID either.
//! Development IDs, from unpacked extensions, persist in
//! `~/.agentmark/native-host.json` so re-registering never drops one.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::install::{manifest_targets, InstallError};
use super::write_file_atomic;
use crate::config;

/// The Chrome Web Store ID. Must equal the extension's
/// `src/shared/constants.ts` copy and the ID derived from the manifest `key`
/// (both checked by tests). Code paths keep working with an empty value (a
/// fork without a store item) through the `*_with_store_id` functions.
pub const WEB_STORE_EXTENSION_ID: &str = "oepdfndmpcpimbkaaocgolenncdkljeo";

#[derive(Debug, Default, Serialize, Deserialize)]
struct NativeHostFile {
    #[serde(default)]
    development_extension_ids: Vec<String>,
}

fn ids_file(home: &Path) -> PathBuf {
    config::config_dir(home).join("native-host.json")
}

/// 32 letters from `a` to `p`, as shown at chrome://extensions (Chrome
/// derives IDs from a hash written in base 16 with the letters a to p).
pub fn is_valid_extension_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b))
}

/// `chrome-extension://<id>/` → `<id>`, for the origin Chrome passes the host.
pub fn extension_id_from_origin(origin: &str) -> Option<String> {
    let id = origin
        .strip_prefix("chrome-extension://")?
        .trim_end_matches('/');
    is_valid_extension_id(id).then(|| id.to_string())
}

/// Store ID (when set) followed by development IDs, deduplicated. Before
/// `native-host.json` exists, the development IDs are the ones found in
/// manifests already on disk (nothing is written by this read).
pub fn allowed_extension_ids(home: &Path) -> Vec<String> {
    allowed_extension_ids_with_store_id(home, WEB_STORE_EXTENSION_ID)
}

/// [`allowed_extension_ids`] for a given store ID (`""` = no store item).
pub fn allowed_extension_ids_with_store_id(home: &Path, store_id: &str) -> Vec<String> {
    merge_ids(store_id, development_extension_ids(home))
}

/// `store_id` (when non-empty) followed by `dev_ids`, deduplicated.
pub(super) fn merge_ids(store_id: &str, dev_ids: Vec<String>) -> Vec<String> {
    let mut ids = Vec::new();
    if !store_id.is_empty() {
        ids.push(store_id.to_string());
    }
    for id in dev_ids {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

/// The persisted development IDs, or the IDs in existing manifests when the
/// file has never been written.
pub fn development_extension_ids(home: &Path) -> Vec<String> {
    match read_ids_file(home) {
        Some(file) => file.development_extension_ids,
        None => ids_from_existing_manifests(home),
    }
}

/// Validate `id` and add it to the development IDs. Returns `true` if it was new.
pub fn add_development_extension_id(home: &Path, id: &str) -> Result<bool, InstallError> {
    if !is_valid_extension_id(id) {
        return Err(InstallError::InvalidExtensionId { id: id.to_string() });
    }
    let mut ids = ensure_ids_file(home)?;
    if ids.contains(&id.to_string()) || id == WEB_STORE_EXTENSION_ID {
        return Ok(false);
    }
    ids.push(id.to_string());
    save_ids(home, &ids)?;
    info!(%id, "development extension ID added");
    Ok(true)
}

/// Load the persisted IDs, migrating them from existing manifests the first
/// time so unpacked installs registered before this file existed keep working.
pub(super) fn ensure_ids_file(home: &Path) -> Result<Vec<String>, InstallError> {
    if let Some(file) = read_ids_file(home) {
        return Ok(file.development_extension_ids);
    }
    let migrated = ids_from_existing_manifests(home);
    debug!(
        count = migrated.len(),
        "migrating development IDs from manifests"
    );
    save_ids(home, &migrated)?;
    Ok(migrated)
}

fn read_ids_file(home: &Path) -> Option<NativeHostFile> {
    let text = std::fs::read_to_string(ids_file(home)).ok()?;
    match serde_json::from_str::<NativeHostFile>(&text) {
        Ok(mut file) => {
            // A hand-edited entry must not reach a manifest's allowed origins.
            file.development_extension_ids.retain(|id| {
                let valid = is_valid_extension_id(id);
                if !valid {
                    warn!(%id, "ignoring an invalid extension ID in native-host.json");
                }
                valid
            });
            Some(file)
        }
        Err(e) => {
            // A damaged file is treated as empty rather than blocking install.
            debug!("ignoring unreadable native-host.json: {e}");
            Some(NativeHostFile::default())
        }
    }
}

fn save_ids(home: &Path, ids: &[String]) -> Result<(), InstallError> {
    let dir = config::config_dir(home);
    let path = ids_file(home);
    std::fs::create_dir_all(&dir).map_err(|source| InstallError::Io {
        path: dir.clone(),
        source,
    })?;
    let file = NativeHostFile {
        development_extension_ids: ids.to_vec(),
    };
    let mut text = serde_json::to_string_pretty(&file).expect("plain struct serializes");
    text.push('\n');
    write_file_atomic(&path, text.as_bytes()).map_err(|source| InstallError::Io { path, source })
}

/// Non-store IDs from every manifest this code would write, in target order.
fn ids_from_existing_manifests(home: &Path) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for target in manifest_targets(home) {
        let Ok(text) = std::fs::read_to_string(target.manifest_path()) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let origins = manifest["allowed_origins"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for origin in origins.iter().filter_map(|o| o.as_str()) {
            if let Some(id) = extension_id_from_origin(origin) {
                if id != WEB_STORE_EXTENSION_ID && !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const DEV_A: &str = "abcdefghijklmnopabcdefghijklmnop";
    const DEV_B: &str = "ponmlkjihgfedcbaponmlkjihgfedcba";

    #[test]
    fn store_id_is_a_valid_id() {
        // Chrome IDs use only the letters a to p.
        assert!(is_valid_extension_id(WEB_STORE_EXTENSION_ID));
    }

    #[test]
    fn store_id_matches_the_extension_constant() {
        let ts = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../extension/src/shared/constants.ts"),
        )
        .unwrap();
        let line = ts
            .lines()
            .find(|l| l.starts_with("export const WEB_STORE_EXTENSION_ID"))
            .expect("WEB_STORE_EXTENSION_ID in constants.ts");
        assert_eq!(
            line,
            format!("export const WEB_STORE_EXTENSION_ID = \"{WEB_STORE_EXTENSION_ID}\";")
        );
    }

    /// Chrome's rule: SHA-256 of the public key (DER), first 16 bytes, each
    /// hex digit 0-f written as a letter a-p.
    fn id_from_public_key(der: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(der)[..16]
            .iter()
            .flat_map(|b| [b >> 4, b & 0xf])
            .map(|n| char::from(b'a' + n))
            .collect()
    }

    /// Standard base64 (with padding), enough for the manifest key.
    fn decode_base64(text: &str) -> Vec<u8> {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let (mut acc, mut bits) = (0u32, 0u32);
        for c in text.bytes().filter(|&c| c != b'=') {
            let v = ALPHABET.iter().position(|&a| a == c).expect("base64 char") as u32;
            acc = (acc << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
                acc &= (1 << bits) - 1;
            }
        }
        out
    }

    #[test]
    fn manifest_key_gives_the_store_id() {
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../extension/manifest.json"),
        )
        .unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&text).unwrap();
        let key = manifest["key"].as_str().expect("manifest.json has a key");
        let der = decode_base64(key);
        // An RSA public key in DER starts with a SEQUENCE tag.
        assert_eq!(der[0], 0x30);
        assert_eq!(id_from_public_key(&der), WEB_STORE_EXTENSION_ID);
    }

    #[test]
    fn validates_ids() {
        assert!(is_valid_extension_id(DEV_A));
        assert!(!is_valid_extension_id("tooshort"));
        assert!(!is_valid_extension_id("abcdefghijklmnop1234567890abcdef"));
        assert!(!is_valid_extension_id("ABCDEFGHIJKLMNOPABCDEFGHIJKLMNOP"));
        // Chrome IDs never use the letters q to z.
        assert!(!is_valid_extension_id("abcdefghijklmnopqrstuvwxyzabcdef"));
        assert!(!is_valid_extension_id(&"z".repeat(32)));
    }

    #[test]
    fn invalid_ids_in_the_file_are_ignored() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(config::config_dir(tmp.path())).unwrap();
        std::fs::write(
            ids_file(tmp.path()),
            serde_json::json!({
                "development_extension_ids": [DEV_A, "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz", "x/\"", DEV_B]
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(development_extension_ids(tmp.path()), vec![DEV_A, DEV_B]);
        assert_eq!(ensure_ids_file(tmp.path()).unwrap(), vec![DEV_A, DEV_B]);
    }

    #[test]
    fn parses_origin() {
        assert_eq!(
            extension_id_from_origin(&format!("chrome-extension://{DEV_A}/")).as_deref(),
            Some(DEV_A)
        );
        assert_eq!(extension_id_from_origin("chrome-extension://bad/"), None);
        assert_eq!(extension_id_from_origin("https://example.com/"), None);
    }

    #[test]
    fn no_file_and_no_manifests_means_no_ids() {
        let tmp = TempDir::new().unwrap();
        assert!(development_extension_ids(tmp.path()).is_empty());
        assert!(allowed_extension_ids_with_store_id(tmp.path(), "").is_empty());
    }

    #[test]
    fn the_store_id_is_always_allowed_and_first() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(
            allowed_extension_ids(tmp.path()),
            vec![WEB_STORE_EXTENSION_ID]
        );
        add_development_extension_id(tmp.path(), DEV_A).unwrap();
        assert_eq!(
            allowed_extension_ids(tmp.path()),
            vec![WEB_STORE_EXTENSION_ID, DEV_A]
        );
        assert_eq!(
            allowed_extension_ids_with_store_id(tmp.path(), ""),
            vec![DEV_A]
        );
    }

    #[test]
    fn adding_the_store_id_as_a_development_id_is_a_no_op() {
        let tmp = TempDir::new().unwrap();
        assert!(!add_development_extension_id(tmp.path(), WEB_STORE_EXTENSION_ID).unwrap());
        assert!(development_extension_ids(tmp.path()).is_empty());
    }

    #[test]
    fn adding_persists_in_order_without_duplicates() {
        let tmp = TempDir::new().unwrap();
        assert!(add_development_extension_id(tmp.path(), DEV_A).unwrap());
        assert!(add_development_extension_id(tmp.path(), DEV_B).unwrap());
        assert!(!add_development_extension_id(tmp.path(), DEV_A).unwrap());
        assert_eq!(development_extension_ids(tmp.path()), vec![DEV_A, DEV_B]);
        let text = std::fs::read_to_string(ids_file(tmp.path())).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["development_extension_ids"][1], DEV_B);
    }

    #[test]
    fn adding_rejects_an_invalid_id_and_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        let err = add_development_extension_id(tmp.path(), "nope").unwrap_err();
        assert!(matches!(err, InstallError::InvalidExtensionId { .. }));
        assert!(!ids_file(tmp.path()).exists());
    }

    #[test]
    fn migrates_ids_from_an_existing_chrome_manifest_once() {
        let tmp = TempDir::new().unwrap();
        let chrome = manifest_targets(tmp.path()).remove(0);
        std::fs::create_dir_all(&chrome.dir).unwrap();
        std::fs::write(
            chrome.manifest_path(),
            serde_json::json!({
                "name": "com.agentmark.native",
                "allowed_origins": [format!("chrome-extension://{DEV_B}/"), "junk"]
            })
            .to_string(),
        )
        .unwrap();

        // Read-only view sees the manifest's IDs without writing.
        assert_eq!(development_extension_ids(tmp.path()), vec![DEV_B]);
        assert!(!ids_file(tmp.path()).exists());

        // Adding a new ID keeps the migrated one.
        add_development_extension_id(tmp.path(), DEV_A).unwrap();
        assert_eq!(development_extension_ids(tmp.path()), vec![DEV_B, DEV_A]);

        // Once the file exists the manifest is no longer consulted.
        std::fs::remove_file(chrome.manifest_path()).unwrap();
        assert_eq!(development_extension_ids(tmp.path()), vec![DEV_B, DEV_A]);
    }

    #[test]
    fn unreadable_file_counts_as_empty() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(config::config_dir(tmp.path())).unwrap();
        std::fs::write(ids_file(tmp.path()), "{not json").unwrap();
        assert!(development_extension_ids(tmp.path()).is_empty());
    }
}
