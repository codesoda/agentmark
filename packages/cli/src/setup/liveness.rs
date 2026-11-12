//! `~/.agentmark/extension-connection.json`: proof the extension has
//! connected, read by the desktop app's "Waiting for Chrome…" screen.
//!
//! Writing it must never fail or slow a native request, so every error is
//! logged at debug level and swallowed.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::debug;

use super::write_file_atomic;
use crate::config;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionRecord {
    /// RFC 3339, UTC.
    pub connected_at: String,
    pub extension_id: Option<String>,
    pub extension_version: Option<String>,
    pub host_version: String,
}

impl ConnectionRecord {
    /// A record stamped now, for this host version.
    pub fn now(extension_id: Option<String>, extension_version: Option<String>) -> Self {
        Self {
            connected_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            extension_id,
            extension_version,
            host_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

fn connection_file(home: &Path) -> PathBuf {
    config::config_dir(home).join("extension-connection.json")
}

/// Write the record atomically, only when `~/.agentmark/` already exists
/// (the host never creates it). Never fails; returns whether it was written.
pub fn record_connection(home: &Path, record: &ConnectionRecord) -> bool {
    let dir = config::config_dir(home);
    if !dir.is_dir() {
        debug!(
            "skipping connection record: {} does not exist",
            dir.display()
        );
        return false;
    }
    let result = serde_json::to_vec(record)
        .map_err(std::io::Error::other)
        .and_then(|bytes| write_file_atomic(&connection_file(home), &bytes));
    match result {
        Ok(()) => {
            debug!(extension_id = ?record.extension_id, "connection recorded");
            true
        }
        Err(e) => {
            debug!("failed to record connection: {e}");
            false
        }
    }
}

/// The last connection, or `None` if there is none or it cannot be read.
pub fn read_connection(home: &Path) -> Option<ConnectionRecord> {
    let text = std::fs::read_to_string(connection_file(home)).ok()?;
    match serde_json::from_str(&text) {
        Ok(record) => Some(record),
        Err(e) => {
            debug!("ignoring unreadable connection record: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn writes_and_reads_back() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(config::config_dir(tmp.path())).unwrap();
        let record = ConnectionRecord::now(
            Some("abcdefghijklmnopabcdefghijklmnop".to_string()),
            Some("0.3.0".to_string()),
        );
        assert!(record_connection(tmp.path(), &record));
        assert_eq!(read_connection(tmp.path()), Some(record.clone()));

        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(connection_file(tmp.path())).unwrap())
                .unwrap();
        let keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        assert_eq!(
            keys.len(),
            4,
            "connected_at, extension_id, extension_version, host_version: {keys:?}"
        );
        assert!(chrono::DateTime::parse_from_rfc3339(&record.connected_at).is_ok());
        assert!(record.connected_at.ends_with('Z'));
    }

    #[test]
    fn null_fields_serialize_as_null() {
        let record = ConnectionRecord::now(None, None);
        let json = serde_json::to_value(&record).unwrap();
        assert!(json["extension_id"].is_null());
        assert!(json["extension_version"].is_null());
        assert_eq!(json["host_version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn never_creates_the_agentmark_directory() {
        let tmp = TempDir::new().unwrap();
        assert!(!record_connection(
            tmp.path(),
            &ConnectionRecord::now(None, None)
        ));
        assert!(!config::config_dir(tmp.path()).exists());
        assert_eq!(read_connection(tmp.path()), None);
    }

    #[test]
    fn write_failure_is_swallowed() {
        let tmp = TempDir::new().unwrap();
        // A directory where the file should go makes the rename fail.
        std::fs::create_dir_all(connection_file(tmp.path())).unwrap();
        assert!(!record_connection(
            tmp.path(),
            &ConnectionRecord::now(None, None)
        ));
        let names: Vec<_> = std::fs::read_dir(config::config_dir(tmp.path()))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            names,
            vec!["extension-connection.json"],
            "temp file cleaned up"
        );
    }

    #[test]
    fn unreadable_record_reads_as_none() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(config::config_dir(tmp.path())).unwrap();
        std::fs::write(connection_file(tmp.path()), "{").unwrap();
        assert_eq!(read_connection(tmp.path()), None);
    }
}
