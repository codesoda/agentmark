//! The launch steps and settings calls, over explicit paths so tests run
//! against a temp HOME. Every rule lives in `agentmark::setup`; this module
//! only turns its outcomes into sentences the screens show.

use std::path::{Path, PathBuf};

use agentmark::setup::{
    self, CliInstallOutcome, ConnectionRecord, FieldError, FolderChoice, InstallError, LinkOutcome,
    RegistrationOutcome, SettingsChanges, SettingsError, SetupEnv, SetupState,
};
use serde::{Deserialize, Serialize};

/// Where the steps read and write. Production values come from
/// [`AppPaths::from_process`]; tests build one over a temp dir.
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub env: SetupEnv,
    /// `~/.local/bin` unless `AGENTMARK_LOCAL_BIN` says otherwise.
    pub local_bin: PathBuf,
    /// The bundled CLI: `Contents/MacOS/agentmark` beside the app binary.
    pub sidecar: PathBuf,
    /// Set for a development run (`npm run tauri dev`), see [`DevRun`].
    pub dev: Option<DevRun>,
}

/// Opt in to copying a development run's CLI into the real `~/.agentmark/bin`.
pub const DEV_INSTALL_CLI_ENV: &str = "AGENTMARK_DEV_INSTALL_CLI";

/// A debug build run from cargo's `target/` folder (`npm run tauri dev`), not
/// from a bundled `.app`. Its "sidecar" is whatever `target/debug/agentmark`
/// the last cargo build left, so by default it must not replace the CLI in
/// `~/.agentmark/bin`, which may be a release copy of the same version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DevRun {
    /// `AGENTMARK_DEV_INSTALL_CLI=1`: copy it anyway.
    pub install_cli: bool,
}

impl DevRun {
    /// `Some` for a debug build whose binary is not inside a `.app` bundle.
    pub fn detect(
        debug_build: bool,
        current_exe: &Path,
        install_env: Option<&std::ffi::OsStr>,
    ) -> Option<Self> {
        let bundled = current_exe
            .ancestors()
            .any(|dir| dir.extension().is_some_and(|ext| ext == "app"));
        (debug_build && !bundled).then(|| DevRun {
            install_cli: install_env.is_some_and(|v| v == "1"),
        })
    }
}

impl AppPaths {
    pub fn from_process() -> Result<Self, String> {
        let env = SetupEnv::from_process().map_err(|e| e.to_string())?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let sidecar = sidecar_path(&exe);
        let local_bin = setup::default_local_bin(&env.home);
        let dev = DevRun::detect(
            cfg!(debug_assertions),
            &exe,
            std::env::var_os(DEV_INSTALL_CLI_ENV).as_deref(),
        );
        Ok(Self {
            env,
            local_bin,
            sidecar,
            dev,
        })
    }

    pub fn home(&self) -> &Path {
        &self.env.home
    }

    fn display(&self, path: &Path) -> String {
        setup::display_path(path, self.home())
    }
}

/// Tauri places `externalBin` entries beside the main binary, without the
/// target-triple suffix they have in `src-tauri/binaries/`.
pub fn sidecar_path(current_exe: &Path) -> PathBuf {
    let dir = current_exe.parent().unwrap_or_else(|| Path::new("."));
    dir.join(if cfg!(windows) {
        "agentmark.exe"
    } else {
        "agentmark"
    })
}

// ── Launch steps ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepId {
    Cli,
    NativeHost,
    Skill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// Finished.
    Done,
    /// Finished with something the user may want to know about.
    Warning,
    /// Cannot finish yet, and that is expected (no extension ID known).
    Waiting,
    /// Failed; the screen offers "Try again".
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepReport {
    pub id: StepId,
    pub status: StepStatus,
    /// One short sentence.
    pub summary: String,
    /// Optional second line: a path, the browsers written, or the raw error.
    pub detail: Option<String>,
}

impl StepReport {
    fn new(id: StepId, status: StepStatus, summary: impl Into<String>) -> Self {
        Self {
            id,
            status,
            summary: summary.into(),
            detail: None,
        }
    }

    fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

pub fn run_step(paths: &AppPaths, id: StepId) -> StepReport {
    match id {
        StepId::Cli => cli_step(paths),
        StepId::NativeHost => native_host_step(paths),
        StepId::Skill => skill_step(paths),
    }
}

/// Copy the bundled CLI to `~/.agentmark/bin/agentmark` (never downgrading)
/// and link it into `~/.local/bin`.
fn cli_step(paths: &AppPaths) -> StepReport {
    let id = StepId::Cli;
    if let Some(dev) = paths.dev {
        if !dev.install_cli {
            return dev_cli_skipped(paths);
        }
        if !paths.sidecar.is_file() {
            return StepReport::new(
                id,
                StepStatus::Failed,
                "Development build: there is no agentmark next to the app.",
            )
            .detail(format!(
                "Expected it at {}. Build it with `cargo build -p agentmark` (build the extension first to embed it).",
                paths.sidecar.display()
            ));
        }
    }
    if !paths.sidecar.is_file() {
        return StepReport::new(
            id,
            StepStatus::Failed,
            "The agentmark command is missing from this copy of the app.",
        )
        .detail(format!(
            "Expected it at {}. Download the app again.",
            paths.sidecar.display()
        ));
    }

    let home = paths.home();
    let stable = paths.display(&setup::stable_cli_path(home));
    let version = env!("CARGO_PKG_VERSION");
    let summary = match setup::install_cli(home, &paths.sidecar) {
        Ok(CliInstallOutcome::Installed) => format!("Installed agentmark {version}."),
        Ok(CliInstallOutcome::Updated { from }) => {
            format!("Updated agentmark from {from} to {version}.")
        }
        Ok(CliInstallOutcome::AlreadyCurrent) => format!("agentmark {version} is installed."),
        Ok(CliInstallOutcome::KeptNewer { installed }) => {
            format!("Kept the newer agentmark {installed} that is already installed.")
        }
        Err(e) => {
            return StepReport::new(id, StepStatus::Failed, "Could not install agentmark.")
                .detail(e.to_string())
        }
    };

    let link = paths.local_bin.join("agentmark");
    match setup::link_cli(home, &paths.local_bin) {
        Ok(LinkOutcome::Linked | LinkOutcome::AlreadyLinked) => {
            StepReport::new(id, StepStatus::Done, summary)
                .detail(format!("{stable}, linked from {}", paths.display(&link)))
        }
        Ok(LinkOutcome::BlockedByFile) => {
            StepReport::new(id, StepStatus::Warning, summary).detail(format!(
                "{} is a file AgentMark did not make, so it was left alone. Chrome still works.",
                paths.display(&link)
            ))
        }
        Err(e) => StepReport::new(id, StepStatus::Warning, summary).detail(format!(
            "Could not link it into {}: {e}. Chrome still works.",
            paths.display(&paths.local_bin)
        )),
    }
}

/// A development run leaves the installed CLI alone unless asked.
fn dev_cli_skipped(paths: &AppPaths) -> StepReport {
    let stable = setup::stable_cli_path(paths.home());
    let how = format!(
        "This development build does not copy {} there. Set {DEV_INSTALL_CLI_ENV}=1 to install it.",
        paths.sidecar.display()
    );
    if stable.is_file() {
        StepReport::new(
            StepId::Cli,
            StepStatus::Warning,
            "Development build: left the installed agentmark as it is.",
        )
        .detail(format!("{}. {how}", paths.display(&stable)))
    } else {
        StepReport::new(
            StepId::Cli,
            StepStatus::Warning,
            "Development build: agentmark is not installed.",
        )
        .detail(format!("Nothing at {}. {how}", paths.display(&stable)))
    }
}

/// Register the native messaging host, pointing at the stable CLI copy. The
/// store ID is always allowed, so this needs no pasted ID.
fn native_host_step(paths: &AppPaths) -> StepReport {
    native_host_step_with_store_id(paths, setup::WEB_STORE_EXTENSION_ID)
}

/// [`native_host_step`] for a given store ID. `""` (a build with no store
/// item) and no development ID gives the "waiting for an ID" report.
pub(crate) fn native_host_step_with_store_id(paths: &AppPaths, store_id: &str) -> StepReport {
    let id = StepId::NativeHost;
    let home = paths.home();
    match setup::register_native_hosts_with_store_id(home, &setup::stable_cli_path(home), store_id) {
        Ok(registrations) => {
            let written: Vec<&str> = registrations
                .iter()
                .filter(|r| r.outcome == RegistrationOutcome::Written)
                .map(|r| r.browser)
                .collect();
            let failed: Vec<String> = registrations
                .iter()
                .filter_map(|r| match &r.outcome {
                    RegistrationOutcome::Failed(reason) => Some(format!("{}: {reason}", r.browser)),
                    _ => None,
                })
                .collect();
            let summary = format!("Registered with {}.", join_names(&written));
            if failed.is_empty() {
                StepReport::new(id, StepStatus::Done, summary)
            } else {
                StepReport::new(id, StepStatus::Warning, summary)
                    .detail(format!("Could not register: {}", failed.join("; ")))
            }
        }
        Err(InstallError::NoExtensionIds) => StepReport::new(
            id,
            StepStatus::Waiting,
            "Waiting for the Chrome extension's ID.",
        )
        .detail(
            "Development build: this build has no Chrome Web Store ID, so Chrome needs the ID of an extension you load yourself.",
        ),
        Err(e) => StepReport::new(id, StepStatus::Failed, "Could not register with Chrome.")
            .detail(e.to_string()),
    }
}

fn skill_step(paths: &AppPaths) -> StepReport {
    let id = StepId::Skill;
    match setup::install_skill(paths.home()) {
        Ok(result) if result.linked.is_empty() => {
            StepReport::new(id, StepStatus::Done, "Added the agent skill.")
                .detail(paths.display(&result.canonical_dir))
        }
        Ok(result) => StepReport::new(id, StepStatus::Done, "Added the agent skill.")
            .detail(format!("For {}", join_names(&result.linked))),
        Err(e) => StepReport::new(id, StepStatus::Failed, "Could not add the agent skill.")
            .detail(e.to_string()),
    }
}

fn join_names<S: AsRef<str>>(names: &[S]) -> String {
    let names: Vec<&str> = names.iter().map(AsRef::as_ref).collect();
    match names.as_slice() {
        [] => "no browsers".to_string(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

// ── Development extension ───────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DevExtension {
    pub path: String,
    pub path_display: String,
}

/// Write the extension this build embeds to `~/.agentmark/extension/`, the
/// folder to "Load unpacked" from (same place as `install-extension`).
pub fn prepare_dev_extension(paths: &AppPaths) -> Result<DevExtension, String> {
    if !agentmark::extension::is_embedded() {
        return Err("This build does not include the extension.".to_string());
    }
    let target = agentmark::config::config_dir(paths.home()).join("extension");
    if target.exists() {
        std::fs::remove_dir_all(&target)
            .map_err(|e| format!("Could not replace {}: {e}", target.display()))?;
    }
    agentmark::extension::extract_to(&target)
        .map_err(|e| format!("Could not write {}: {e}", target.display()))?;
    Ok(DevExtension {
        path_display: paths.display(&target),
        path: target.display().to_string(),
    })
}

/// Allow a development extension ID, then register again.
pub fn add_extension_id(paths: &AppPaths, id: &str) -> Result<StepReport, String> {
    let id = id.trim();
    setup::add_development_extension_id(paths.home(), id).map_err(|e| match e {
        InstallError::InvalidExtensionId { .. } => {
            "That is not an extension ID. It is 32 letters from a to p, shown on the extension's card."
                .to_string()
        }
        other => other.to_string(),
    })?;
    Ok(native_host_step(paths))
}

pub fn connection(paths: &AppPaths) -> Option<ConnectionRecord> {
    setup::read_connection(paths.home())
}

// ── Settings ────────────────────────────────────────────────────────

pub fn read_state(paths: &AppPaths) -> SetupState {
    setup::read_state(&paths.env)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SaveResult {
    Saved { state: Box<SetupState> },
    Invalid { errors: Vec<FieldError> },
}

/// Invalid input is a result the form shows by field; only write and index
/// failures are errors.
pub fn save_settings(paths: &AppPaths, changes: &SettingsChanges) -> Result<SaveResult, String> {
    match setup::apply_settings(&paths.env, changes) {
        Ok(state) => Ok(SaveResult::Saved {
            state: Box::new(state),
        }),
        Err(SettingsError::Invalid(errors)) => Ok(SaveResult::Invalid { errors }),
        Err(e) => Err(e.to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FolderResult {
    /// `chosen`, `cancelled` or `unavailable`, as in the native message.
    pub outcome: &'static str,
    pub path: Option<String>,
    pub path_display: Option<String>,
    pub reason: Option<String>,
}

/// Shape a dialog answer like the native host's `choose_folder_result`.
pub fn folder_result(paths: &AppPaths, choice: FolderChoice) -> FolderResult {
    match choice {
        FolderChoice::Chosen(path) => FolderResult {
            outcome: "chosen",
            path_display: Some(paths.display(&path)),
            path: Some(path.display().to_string()),
            reason: None,
        },
        FolderChoice::Cancelled => FolderResult {
            outcome: "cancelled",
            path: None,
            path_display: None,
            reason: None,
        },
        FolderChoice::Unavailable { reason } => FolderResult {
            outcome: "unavailable",
            path: None,
            path_display: None,
            reason: Some(reason),
        },
    }
}

/// Where the dialog starts: the typed folder if it exists (with `~`
/// expanded), else its nearest existing parent, else home.
pub fn dialog_start(paths: &AppPaths, typed: Option<&str>) -> PathBuf {
    let home = paths.home();
    let typed = typed.map(str::trim).unwrap_or_default();
    let mut candidate = if typed == "~" {
        home.to_path_buf()
    } else if let Some(rest) = typed.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(typed)
    };
    if !candidate.is_absolute() {
        return home.to_path_buf();
    }
    while !candidate.is_dir() {
        if !candidate.pop() {
            return home.to_path_buf();
        }
    }
    candidate
}

#[cfg(test)]
#[path = "steps_tests.rs"]
mod tests;
