//! In-app updates from GitHub Releases.
//!
//! OFF until the owner switches them on. The one place to do that is
//! `plugins.updater` in `tauri.conf.json`: set `pubkey` (the public half of
//! `cargo tauri signer generate`) and `endpoints` (for example
//! `https://github.com/codesoda/agentmark/releases/latest/download/latest.json`),
//! then add the `TAURI_SIGNING_PRIVATE_KEY` secret so the release workflow
//! signs the update artifacts. With either left empty the updater plugin is
//! not registered and these commands report that updates are off.

use serde::Serialize;
use tauri_plugin_updater::UpdaterExt;

/// True when `plugins.updater` has a public key and at least one endpoint.
pub fn enabled(config: &tauri::Config) -> bool {
    config
        .plugins
        .0
        .get("updater")
        .is_some_and(updater_config_enabled)
}

fn updater_config_enabled(value: &serde_json::Value) -> bool {
    let pubkey = value.get("pubkey").and_then(|v| v.as_str()).unwrap_or("");
    let endpoints = value
        .get("endpoints")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|e| e.as_str())
                .any(|e| !e.trim().is_empty())
        })
        .unwrap_or(false);
    !pubkey.trim().is_empty() && endpoints
}

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

const UPDATES_OFF: &str = "Updates are off in this build.";

#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    if !enabled(app.config()) {
        return Err(UPDATES_OFF.to_string());
    }
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(update.map(|u| UpdateInfo {
        version: u.version,
        notes: u.body,
    }))
}

/// Download and install the update, then restart into it.
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    if !enabled(app.config()) {
        return Err(UPDATES_OFF.to_string());
    }
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "AgentMark is up to date.".to_string())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn off_without_key_or_endpoints() {
        assert!(!updater_config_enabled(
            &json!({"pubkey": "", "endpoints": []})
        ));
        assert!(!updater_config_enabled(
            &json!({"pubkey": "", "endpoints": ["https://x/latest.json"]})
        ));
        assert!(!updater_config_enabled(
            &json!({"pubkey": "KEY", "endpoints": []})
        ));
        assert!(!updater_config_enabled(
            &json!({"pubkey": "KEY", "endpoints": [" "]})
        ));
        assert!(!updater_config_enabled(&json!({})));
    }

    #[test]
    fn on_with_key_and_endpoint() {
        assert!(updater_config_enabled(
            &json!({"pubkey": "KEY", "endpoints": ["https://x/latest.json"]})
        ));
    }

    /// The shipped config must keep updates off until the owner adds a key.
    #[test]
    fn shipped_config_has_updates_off() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert!(!updater_config_enabled(&conf["plugins"]["updater"]));
        assert_eq!(conf["bundle"]["createUpdaterArtifacts"], json!(false));
    }
}
