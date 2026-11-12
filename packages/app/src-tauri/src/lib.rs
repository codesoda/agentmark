//! AgentMark desktop app: an installer, a setup screen and a settings window.
//!
//! The commands here are thin: launch steps and settings logic live in
//! `steps.rs` (over explicit paths, tested against a temp HOME), which in turn
//! calls the setup core in the `agentmark` crate. The app owns no state files
//! of its own and quits when its window closes.

pub mod steps;
pub mod updates;

use std::path::{Path, PathBuf};

use agentmark::setup::{self, ConnectionRecord, FolderChoice, FolderDialog, SettingsChanges};
use serde::Serialize;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use steps::{AppPaths, DevExtension, FolderResult, SaveResult, StepId, StepReport};

/// Resolved once at start. `Err` when `HOME` is unset: every command then
/// reports it instead of guessing a home directory.
struct Paths(Result<AppPaths, String>);

impl Paths {
    fn get(&self) -> Result<&AppPaths, String> {
        self.0
            .as_ref()
            .map_err(|e| format!("AgentMark cannot find your home folder: {e}"))
    }
}

#[derive(Serialize)]
struct AppInfo {
    version: &'static str,
    updates_enabled: bool,
}

#[tauri::command]
fn app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        updates_enabled: updates::enabled(app.config()),
    }
}

/// Blocking file work runs off the main thread so the window stays live.
#[tauri::command]
async fn run_step(paths: State<'_, Paths>, step: StepId) -> Result<StepReport, String> {
    let paths = paths.get()?.clone();
    blocking(move || steps::run_step(&paths, step)).await
}

#[tauri::command]
async fn get_setup_state(paths: State<'_, Paths>) -> Result<setup::SetupState, String> {
    let paths = paths.get()?.clone();
    blocking(move || steps::read_state(&paths)).await
}

#[tauri::command]
async fn save_settings(
    paths: State<'_, Paths>,
    changes: SettingsChanges,
) -> Result<SaveResult, String> {
    let paths = paths.get()?.clone();
    blocking(move || steps::save_settings(&paths, &changes)).await?
}

/// The system folder dialog through `tauri-plugin-dialog` (contract §14).
struct TauriFolderDialog(tauri::AppHandle);

impl FolderDialog for TauriFolderDialog {
    fn pick(&self, start: Option<&Path>) -> FolderChoice {
        let mut dialog = self
            .0
            .dialog()
            .file()
            .set_title("Choose where AgentMark keeps bookmarks");
        if let Some(start) = start {
            dialog = dialog.set_directory(start);
        }
        match dialog.blocking_pick_folder() {
            None => FolderChoice::Cancelled,
            Some(path) => match path.into_path() {
                Ok(path) => FolderChoice::Chosen(path),
                Err(e) => FolderChoice::Unavailable {
                    reason: format!("The folder dialog returned an unusable path: {e}"),
                },
            },
        }
    }
}

#[tauri::command]
async fn choose_folder(
    app: tauri::AppHandle,
    paths: State<'_, Paths>,
    start_path: Option<String>,
) -> Result<FolderResult, String> {
    let paths = paths.get()?.clone();
    blocking(move || {
        let start = steps::dialog_start(&paths, start_path.as_deref());
        let choice = setup::choose_folder(&TauriFolderDialog(app), Some(&start));
        steps::folder_result(&paths, choice)
    })
    .await
}

#[tauri::command]
fn get_connection(paths: State<'_, Paths>) -> Result<Option<ConnectionRecord>, String> {
    Ok(steps::connection(paths.get()?))
}

/// Write the development copy of the extension and show it in Finder.
#[tauri::command]
async fn show_dev_extension(
    app: tauri::AppHandle,
    paths: State<'_, Paths>,
) -> Result<DevExtension, String> {
    let paths = paths.get()?.clone();
    let dev = blocking(move || steps::prepare_dev_extension(&paths)).await??;
    // Revealing is a convenience; the path is on screen either way.
    if let Err(e) = app.opener().reveal_item_in_dir(PathBuf::from(&dev.path)) {
        tracing::debug!("could not reveal {}: {e}", dev.path);
    }
    Ok(dev)
}

#[tauri::command]
async fn add_extension_id(paths: State<'_, Paths>, id: String) -> Result<StepReport, String> {
    let paths = paths.get()?.clone();
    blocking(move || steps::add_extension_id(&paths, &id)).await?
}

/// Open a web page in the default browser. Only `https://` links.
#[tauri::command]
fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err(format!("Refusing to open {url}"));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// `chrome://` pages cannot be opened by URL from other apps on most systems;
/// on macOS, `open -a` hands the URL to Chrome itself.
#[tauri::command]
fn open_chrome_extensions() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .args(["-a", "Google Chrome", "chrome://extensions"])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            return Ok(());
        }
        Err("Chrome did not open.".to_string())
    }
    #[cfg(not(target_os = "macos"))]
    Err("Open chrome://extensions in Chrome.".to_string())
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| e.to_string())
}

/// Build and run the app.
pub fn run() {
    let context = tauri::generate_context!();
    let updates_on = updates::enabled(context.config());

    let mut builder = tauri::Builder::default();
    // Must be the first plugin: a second launch focuses the existing window.
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }
    builder = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    if updates_on {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .manage(Paths(AppPaths::from_process()))
        .invoke_handler(tauri::generate_handler![
            app_info,
            run_step,
            get_setup_state,
            save_settings,
            choose_folder,
            get_connection,
            show_dev_extension,
            add_extension_id,
            open_url,
            open_chrome_extensions,
            updates::check_for_update,
            updates::install_update,
        ])
        .run(context)
        .expect("error while running the AgentMark app");
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod config_tests;
