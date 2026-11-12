//! Setup core: first-run setup, settings, and installation steps.
//!
//! One implementation with three callers: the native host exposes it as
//! messages (`get_setup_state`, `save_settings`, `choose_folder`), `agentmark
//! init` drives it from a terminal, and the desktop app calls it directly.
//! The contract for every type and rule here is `docs/setup-contract.md`.
//!
//! Nothing else may write `config.toml`: all writes go through
//! [`apply_settings`], which edits the file in place so user comments and
//! unknown keys survive.

mod agents;
mod config_edit;
mod extension_ids;
mod folder;
mod install;
mod liveness;

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, instrument, warn};

use crate::config::{self, Config, ConfigError};
use crate::db::{self, BookmarkRepository};
use crate::snapshot_rules;

pub use agents::{detect_agents, AgentInfo};
pub use extension_ids::{
    add_development_extension_id, allowed_extension_ids, allowed_extension_ids_with_store_id,
    development_extension_ids, extension_id_from_origin, is_valid_extension_id,
    WEB_STORE_EXTENSION_ID,
};
pub use folder::{FolderChoice, FolderDialog, SystemFolderDialog};
pub use install::{
    default_local_bin, install_cli, install_skill, link_cli, register_native_hosts,
    register_native_hosts_with_store_id, registered_binary_path, stable_cli_path,
    CliInstallOutcome, HostRegistration, InstallError, LinkOutcome, RegistrationOutcome,
    SkillInstallResult, NATIVE_HOST_NAME,
};
pub use liveness::{read_connection, record_connection, ConnectionRecord};

// ── Limits (contract §5.1) ──────────────────────────────────────────

const MAX_STORAGE_PATH_BYTES: usize = 4096;
const MAX_SYSTEM_PROMPT_CHARS: usize = 20_000;
const MAX_SNAPSHOT_SITES: usize = 100;
const MAX_SNAPSHOT_SITE_CHARS: usize = 200;

/// Agent IDs in the order they are offered and auto-selected.
const AGENT_ORDER: [&str; 3] = ["claude", "codex", "pi"];

// ── Types ───────────────────────────────────────────────────────────

/// Where setup runs: the home directory and the `PATH` used to find agent CLIs.
#[derive(Debug, Clone)]
pub struct SetupEnv {
    pub home: PathBuf,
    /// PATH used to find agent CLIs. Production: the agent layer's search path,
    /// so a host started by Chrome with a bare `PATH` still finds them.
    pub search_path: OsString,
}

impl SetupEnv {
    /// The real environment: `HOME` and the agent search path.
    pub fn from_process() -> Result<Self, ConfigError> {
        Ok(Self {
            home: config::home_dir()?,
            search_path: crate::agent::provider::agent_search_path(),
        })
    }
}

/// Everything a setup or settings screen needs, in one read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetupState {
    pub initialized: bool,
    pub config_problem: Option<String>,
    pub version: String,
    pub platform: PlatformInfo,
    pub settings: Settings,
    pub default_storage_path: String,
    pub default_storage_path_display: String,
    /// Always three entries, in the order claude, codex, pi.
    pub agents: Vec<AgentInfo>,
    pub builtin_snapshot_sites: Vec<String>,
    pub bookmark_count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
}

/// Current settings, or the proposed defaults before setup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub storage_path: String,
    pub storage_path_display: String,
    pub default_agent: String,
    pub enrichment_enabled: bool,
    pub system_prompt: Option<String>,
    /// User sites only; the built-in rules are reported separately.
    pub snapshot_sites: Vec<String>,
}

/// A partial settings update. An absent field keeps its current value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SettingsChanges {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrichment_enabled: Option<bool>,
    /// Outer `None` = unchanged, `Some(None)` = clear, `Some(Some(v))` = set.
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub system_prompt: Option<Option<String>>,
    /// Replaces the whole user list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_sites: Option<Vec<String>>,
}

/// Present-but-null must differ from absent: `null` clears the value.
fn double_option<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldError {
    pub field: SettingsField,
    /// Only set for a `snapshot_sites` entry.
    pub index: Option<usize>,
    pub code: FieldErrorCode,
    /// A sentence the UI shows as-is.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsField {
    StoragePath,
    DefaultAgent,
    EnrichmentEnabled,
    SystemPrompt,
    SnapshotSites,
}

impl SettingsField {
    /// The wire name, also used in `agentmark init` error lines.
    pub fn as_str(self) -> &'static str {
        match self {
            SettingsField::StoragePath => "storage_path",
            SettingsField::DefaultAgent => "default_agent",
            SettingsField::EnrichmentEnabled => "enrichment_enabled",
            SettingsField::SystemPrompt => "system_prompt",
            SettingsField::SnapshotSites => "snapshot_sites",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldErrorCode {
    Blank,
    NotAbsolute,
    TooLong,
    NotADirectory,
    CannotCreate,
    NotWritable,
    InvalidAgent,
    TooMany,
    InvalidSite,
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("invalid settings")]
    Invalid(Vec<FieldError>),

    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to open the index: {0}")]
    Index(#[from] crate::db::DbError),
}

// ── Read ────────────────────────────────────────────────────────────

/// True when `config.toml` exists and loads.
pub fn is_set_up(home: &Path) -> bool {
    Config::load(home).is_ok()
}

/// Read the whole setup state. Never fails: problems are reported in the state.
#[instrument(skip(env), fields(home = %env.home.display()))]
pub fn read_state(env: &SetupEnv) -> SetupState {
    let home = &env.home;
    let agents = detect_agents(&env.search_path);
    let loaded = Config::load(home);

    let config_problem = match &loaded {
        Ok(_) | Err(ConfigError::NotFound { .. }) => None,
        Err(e) => Some(e.to_string()),
    };

    let (initialized, settings, bookmark_count) = match &loaded {
        Ok(config) => {
            let count = count_bookmarks(home);
            (true, settings_from_config(config, home), count)
        }
        Err(_) => (
            false,
            fallback_values(home, &agents).to_settings(home),
            None,
        ),
    };

    let default_storage = default_storage_path(home);
    SetupState {
        initialized,
        config_problem,
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: PlatformInfo {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        },
        settings,
        default_storage_path: default_storage.display().to_string(),
        default_storage_path_display: display_path(&default_storage, home),
        agents,
        builtin_snapshot_sites: snapshot_rules::BUILTIN_RULES
            .iter()
            .map(|s| s.to_string())
            .collect(),
        bookmark_count,
    }
}

/// Opening the index also migrates it, the same as any other request does.
fn count_bookmarks(home: &Path) -> Option<u64> {
    let conn = match db::open_and_migrate(&config::index_db_path(home)) {
        Ok(conn) => conn,
        Err(e) => {
            warn!("cannot open index to count bookmarks: {e}");
            return None;
        }
    };
    match BookmarkRepository::new(&conn).count_bookmarks() {
        Ok(n) => Some(n as u64),
        Err(e) => {
            warn!("cannot count bookmarks: {e}");
            None
        }
    }
}

fn settings_from_config(config: &Config, home: &Path) -> Settings {
    Values {
        storage_path: config.storage_path.clone(),
        default_agent: config.default_agent.clone(),
        enrichment_enabled: config.enrichment.enabled,
        system_prompt: config.system_prompt.clone(),
        snapshot_sites: config.snapshot_sites.clone(),
    }
    .to_settings(home)
}

/// `~/Documents/AgentMark` on every OS.
pub fn default_storage_path(home: &Path) -> PathBuf {
    home.join("Documents").join("AgentMark")
}

/// `~` for home, `~/rest` under home, otherwise the path unchanged.
pub fn display_path(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Show the folder dialog. A chosen path loses any trailing `/` (except `/`).
pub fn choose_folder(dialog: &dyn FolderDialog, start: Option<&Path>) -> FolderChoice {
    match dialog.pick(start) {
        FolderChoice::Chosen(path) => {
            let text = path.to_string_lossy();
            let trimmed = text.trim_end_matches('/');
            if trimmed.is_empty() {
                FolderChoice::Chosen(PathBuf::from("/"))
            } else {
                FolderChoice::Chosen(PathBuf::from(trimmed))
            }
        }
        other => other,
    }
}

// ── Values ──────────────────────────────────────────────────────────

/// The five settings as stored, before display formatting.
#[derive(Debug, Clone, PartialEq)]
struct Values {
    storage_path: PathBuf,
    default_agent: String,
    enrichment_enabled: bool,
    system_prompt: Option<String>,
    snapshot_sites: Vec<String>,
}

impl Values {
    fn to_settings(&self, home: &Path) -> Settings {
        Settings {
            storage_path: self.storage_path.display().to_string(),
            storage_path_display: display_path(&self.storage_path, home),
            default_agent: self.default_agent.clone(),
            enrichment_enabled: self.enrichment_enabled,
            system_prompt: self.system_prompt.clone(),
            snapshot_sites: self.snapshot_sites.clone(),
        }
    }
}

/// Proposed values before setup: the first agent found, and summaries on only
/// when an agent can write them.
fn default_values(home: &Path, agents: &[AgentInfo]) -> Values {
    let found = agents.iter().find(|a| a.found);
    Values {
        storage_path: default_storage_path(home),
        default_agent: found
            .map(|a| a.id.clone())
            .unwrap_or_else(|| AGENT_ORDER[0].to_string()),
        enrichment_enabled: found.is_some(),
        system_prompt: None,
        snapshot_sites: Vec::new(),
    }
}

/// Values to start from when `config.toml` does not load: the proposed
/// defaults, with every key from the file that is valid on its own kept, so a
/// file that only lacks a table does not lose the user's folder.
fn fallback_values(home: &Path, agents: &[AgentInfo]) -> Values {
    let defaults = default_values(home, agents);
    match std::fs::read_to_string(config::config_file(home)) {
        Ok(text) => config_edit::values_from_partial_config(&text, defaults),
        Err(_) => defaults,
    }
}

// ── Write ───────────────────────────────────────────────────────────

/// Validate and apply a partial settings update, then return the new state.
///
/// Every field is validated before anything is written. Then, in order: the
/// storage folder, `~/.agentmark/`, `config.toml`, and the index.
#[instrument(skip(env, changes), fields(home = %env.home.display()))]
pub fn apply_settings(
    env: &SetupEnv,
    changes: &SettingsChanges,
) -> Result<SetupState, SettingsError> {
    let home = &env.home;
    let (current, set_up) = match Config::load(home) {
        Ok(config) => (
            Values {
                storage_path: config.storage_path,
                default_agent: config.default_agent,
                enrichment_enabled: config.enrichment.enabled,
                system_prompt: config.system_prompt,
                snapshot_sites: config.snapshot_sites,
            },
            true,
        ),
        Err(_) => (
            fallback_values(home, &detect_agents(&env.search_path)),
            false,
        ),
    };

    let values = validate(home, &current, set_up, changes).map_err(SettingsError::Invalid)?;
    debug!(storage = %values.storage_path.display(), agent = %values.default_agent, "settings valid");

    let config_dir = config::config_dir(home);
    std::fs::create_dir_all(&config_dir).map_err(|source| SettingsError::Write {
        path: config_dir.clone(),
        source,
    })?;
    config_edit::write_config(home, &values)?;
    db::open_and_migrate(&config::index_db_path(home))?;

    Ok(read_state(env))
}

/// Check every field and return the resulting values, or all errors found.
/// The folder is created and probed only when everything else is valid, and
/// only when it changes or setup is not done yet: changing another setting
/// must not fail (or create a stray folder) because the current folder is on
/// a drive that is not plugged in.
fn validate(
    home: &Path,
    current: &Values,
    set_up: bool,
    changes: &SettingsChanges,
) -> Result<Values, Vec<FieldError>> {
    let mut errors = Vec::new();

    let storage_path = match &changes.storage_path {
        Some(raw) => match parse_storage_path(raw, home) {
            Ok(path) => Some(path),
            Err(e) => {
                errors.push(e);
                None
            }
        },
        None => Some(current.storage_path.clone()),
    };
    if let Some(path) = &storage_path {
        if path.exists() && !path.is_dir() {
            errors.push(field_error(
                SettingsField::StoragePath,
                FieldErrorCode::NotADirectory,
                format!("{} is a file, not a folder.", path.display()),
            ));
        }
    }

    let default_agent = match &changes.default_agent {
        Some(raw) => match config::validate_agent(raw) {
            Ok(agent) => agent,
            Err(_) => {
                errors.push(field_error(
                    SettingsField::DefaultAgent,
                    FieldErrorCode::InvalidAgent,
                    format!(
                        "\"{}\" is not a supported agent. Choose claude, codex or pi.",
                        raw.trim()
                    ),
                ));
                current.default_agent.clone()
            }
        },
        None => current.default_agent.clone(),
    };

    let enrichment_enabled = changes
        .enrichment_enabled
        .unwrap_or(current.enrichment_enabled);

    let system_prompt = match &changes.system_prompt {
        None => current.system_prompt.clone(),
        Some(None) => None,
        Some(Some(raw)) => {
            let trimmed = raw.trim();
            if trimmed.chars().count() > MAX_SYSTEM_PROMPT_CHARS {
                errors.push(field_error(
                    SettingsField::SystemPrompt,
                    FieldErrorCode::TooLong,
                    "Summary instructions can be at most 20,000 characters.".to_string(),
                ));
            }
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
    };

    let snapshot_sites = match &changes.snapshot_sites {
        Some(raw) => match normalize_sites(raw) {
            Ok(sites) => sites,
            Err(site_errors) => {
                errors.extend(site_errors);
                Vec::new()
            }
        },
        None => current.snapshot_sites.clone(),
    };

    let storage_path = match storage_path {
        Some(path) if errors.is_empty() => path,
        _ => return Err(errors),
    };
    let folder_changes = !set_up || storage_path != current.storage_path;
    if folder_changes {
        if let Err(e) = prepare_storage_folder(&storage_path) {
            return Err(vec![e]);
        }
    }

    Ok(Values {
        storage_path,
        default_agent,
        enrichment_enabled,
        system_prompt,
        snapshot_sites,
    })
}

fn field_error(field: SettingsField, code: FieldErrorCode, message: String) -> FieldError {
    FieldError {
        field,
        index: None,
        code,
        message,
    }
}

/// String checks only: trim, expand `~`, require an absolute path.
fn parse_storage_path(raw: &str, home: &Path) -> Result<PathBuf, FieldError> {
    let error = |code, message: String| field_error(SettingsField::StoragePath, code, message);
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(error(
            FieldErrorCode::Blank,
            "Choose a folder for your bookmarks.".to_string(),
        ));
    }
    let path = if trimmed == "~" {
        home.to_path_buf()
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(trimmed)
    };
    // The host and the app have no meaningful working directory to resolve
    // a relative path against, so the core rejects them.
    if !path.is_absolute() {
        return Err(error(
            FieldErrorCode::NotAbsolute,
            format!("Use a full path such as ~/Documents/AgentMark, not \"{trimmed}\"."),
        ));
    }
    if path.as_os_str().len() > MAX_STORAGE_PATH_BYTES {
        return Err(error(
            FieldErrorCode::TooLong,
            "This folder path is too long.".to_string(),
        ));
    }
    // Drops a trailing `/` and `.` segments so the stored path is stable.
    Ok(path.components().collect())
}

/// Create the folder and prove it is writable with a throwaway file.
fn prepare_storage_folder(path: &Path) -> Result<(), FieldError> {
    let error = |code, message: String| field_error(SettingsField::StoragePath, code, message);
    if let Err(e) = std::fs::create_dir_all(path) {
        return Err(error(
            FieldErrorCode::CannotCreate,
            format!("AgentMark cannot create {}: {e}.", path.display()),
        ));
    }
    let probe = path.join(format!(".agentmark-write-test-{}", std::process::id()));
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .and_then(|mut f| f.write_all(b"ok"));
    let removed = std::fs::remove_file(&probe);
    if written.is_err() || removed.is_err() {
        return Err(error(
            FieldErrorCode::NotWritable,
            format!("AgentMark cannot write to {}.", path.display()),
        ));
    }
    Ok(())
}

/// Normalize the user's site rules. Duplicates and built-in rules are dropped
/// silently; anything unusable is an error at its index.
fn normalize_sites(raw: &[String]) -> Result<Vec<String>, Vec<FieldError>> {
    let mut errors = Vec::new();
    if raw.len() > MAX_SNAPSHOT_SITES {
        errors.push(field_error(
            SettingsField::SnapshotSites,
            FieldErrorCode::TooMany,
            format!("You can add at most {MAX_SNAPSHOT_SITES} sites."),
        ));
    }

    let mut sites: Vec<String> = Vec::new();
    for (index, entry) in raw.iter().enumerate() {
        match normalize_site(entry) {
            Ok(site) => {
                let builtin = snapshot_rules::BUILTIN_RULES.contains(&site.as_str());
                if !builtin && !sites.contains(&site) {
                    sites.push(site);
                }
            }
            Err((code, message)) => errors.push(FieldError {
                field: SettingsField::SnapshotSites,
                index: Some(index),
                code,
                message,
            }),
        }
    }

    if errors.is_empty() {
        Ok(sites)
    } else {
        Err(errors)
    }
}

/// One `host/path-prefix` rule: scheme stripped, host lowercased.
fn normalize_site(entry: &str) -> Result<String, (FieldErrorCode, String)> {
    let invalid = || {
        (
            FieldErrorCode::InvalidSite,
            format!("\"{}\" is not a host/path rule.", entry.trim()),
        )
    };
    let trimmed = entry.trim();
    let lower_prefix = trimmed.get(..8).unwrap_or(trimmed).to_ascii_lowercase();
    let rest = if lower_prefix.starts_with("https://") {
        &trimmed[8..]
    } else if lower_prefix.starts_with("http://") {
        &trimmed[7..]
    } else {
        trimmed
    };
    if rest.is_empty() || rest.chars().any(char::is_whitespace) {
        return Err(invalid());
    }
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let host = host.to_ascii_lowercase();
    let host_ok = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-');
    if !host_ok {
        return Err(invalid());
    }
    let site = format!("{host}{path}");
    if site.chars().count() > MAX_SNAPSHOT_SITE_CHARS {
        return Err((
            FieldErrorCode::TooLong,
            format!("Site rules can be at most {MAX_SNAPSHOT_SITE_CHARS} characters."),
        ));
    }
    Ok(site)
}

// ── Shared file helper ──────────────────────────────────────────────

/// Write `contents` to `path` via a sibling `*.tmp-<pid>` file and a rename,
/// so readers never see a torn file.
pub(crate) fn write_file_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no file name"))?;
    let mut tmp_name = file_name.to_os_string();
    tmp_name.push(format!(".tmp-{}", std::process::id()));
    let tmp = path.with_file_name(tmp_name);

    let result = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
