use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── Error types ─────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("HOME environment variable is not set")]
    HomeMissing,

    #[error("config not found at {path} — run `agentmark init` first")]
    NotFound { path: PathBuf },

    #[error("failed to read config at {path}: {source}")]
    ReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid config at {path}: {source}")]
    ParseError {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("failed to write config at {path}: {source}")]
    WriteError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("path exists but is not a directory: {path}")]
    NotADirectory { path: PathBuf },

    #[error("invalid agent: {value} (expected \"claude\" or \"codex\")")]
    InvalidAgent { value: String },

    #[error("storage path cannot be blank")]
    BlankStoragePath,

    #[error("initialization cancelled")]
    Cancelled,

    #[error("unexpected end of input")]
    UnexpectedEof,
}

// ── Config structs ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub default_agent: String,
    pub storage_path: PathBuf,
    pub system_prompt: Option<String>,
    pub log_level: Option<String>,
    pub enrichment: EnrichmentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnrichmentConfig {
    pub enabled: bool,
}

// ── Path helpers ────────────────────────────────────────────────────

/// Returns the AgentMark config root directory for a given home.
pub fn config_dir(home: &Path) -> PathBuf {
    home.join(".agentmark")
}

/// Returns the config file path for a given home.
pub fn config_file(home: &Path) -> PathBuf {
    config_dir(home).join("config.toml")
}

/// Returns the index database path for a given home.
pub fn index_db_path(home: &Path) -> PathBuf {
    config_dir(home).join("index.db")
}

/// Returns the logs directory path for a given home.
pub fn logs_dir(home: &Path) -> PathBuf {
    config_dir(home).join("logs")
}

/// Resolves a user-provided storage path to an absolute path.
///
/// - If the input is already absolute, returns it as-is.
/// - If it starts with `~/`, expands relative to `home`.
/// - Otherwise, resolves relative to `cwd`.
///
/// Does NOT call `canonicalize` — the path need not exist yet.
pub fn resolve_storage_path(input: &str, home: &Path, cwd: &Path) -> Result<PathBuf, ConfigError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::BlankStoragePath);
    }

    let path = Path::new(trimmed);
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else if let Ok(rest) = path.strip_prefix("~") {
        Ok(home.join(rest))
    } else {
        Ok(cwd.join(path))
    }
}

/// Returns the user's home directory from `HOME` env var.
pub fn home_dir() -> Result<PathBuf, ConfigError> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| ConfigError::HomeMissing)
}

// ── Validation ──────────────────────────────────────────────────────

/// Normalizes and validates an agent name.
/// Returns the canonical lowercase form or an error.
pub fn validate_agent(input: &str) -> Result<String, ConfigError> {
    let normalized = input.trim().to_lowercase();
    match normalized.as_str() {
        "claude" | "codex" => Ok(normalized),
        _ => Err(ConfigError::InvalidAgent {
            value: input.trim().to_string(),
        }),
    }
}

// ── Config rendering (TOML + comments) ──────────────────────────────

const SYSTEM_PROMPT_COMMENT: &str = r#"
# system_prompt = """
# You have access to the following local tools:
# - native notifications (alerter CLI)
# - agent-ui for presenting native macOS UI dialogs
# Include any context about your local setup here.
# """

# Log level for file logging (~/.agentmark/logs/). Overridden by AGENTMARK_LOG env var.
# log_level = "debug"
"#;

/// Renders a Config to a human-readable TOML string with commented examples.
pub fn render_config_toml(config: &Config) -> String {
    let mut out = toml::to_string_pretty(config).expect("Config serialization should not fail");

    // Remove the serialized system_prompt if it's None — toml will omit it,
    // but we add the comment block regardless.
    if config.system_prompt.is_none() {
        // Ensure the comment block appears after the top-level fields
        // and before [enrichment]
        if let Some(pos) = out.find("\n[enrichment]") {
            out.insert_str(pos, SYSTEM_PROMPT_COMMENT);
        } else {
            out.push_str(SYSTEM_PROMPT_COMMENT);
        }
    }

    out
}

// ── Load / Save ─────────────────────────────────────────────────────

impl Config {
    /// Load config from the standard location under `home`.
    pub fn load(home: &Path) -> Result<Self, ConfigError> {
        let path = config_file(home);

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ConfigError::NotFound { path });
            }
            Err(e) => {
                return Err(ConfigError::ReadError { path, source: e });
            }
        };

        toml::from_str(&contents).map_err(|e| ConfigError::ParseError { path, source: e })
    }

    /// Save config to the standard location under `home`.
    /// Creates the config directory if it doesn't exist.
    pub fn save(&self, home: &Path) -> Result<(), ConfigError> {
        let dir = config_dir(home);
        let path = config_file(home);

        ensure_dir(&dir)?;

        let rendered = render_config_toml(self);
        std::fs::write(&path, rendered).map_err(|e| ConfigError::WriteError { path, source: e })
    }
}

/// Creates a directory if it doesn't exist, erroring if the path is a file.
pub fn ensure_dir(path: &Path) -> Result<(), ConfigError> {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(ConfigError::NotADirectory {
            path: path.to_path_buf(),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => std::fs::create_dir_all(path)
            .map_err(|e| ConfigError::WriteError {
                path: path.to_path_buf(),
                source: e,
            }),
        Err(e) => Err(ConfigError::ReadError {
            path: path.to_path_buf(),
            source: e,
        }),
    }
}

/// Creates an empty file if it doesn't already exist.
/// Does NOT truncate existing files.
pub fn touch_file(path: &Path) -> Result<(), ConfigError> {
    if path.exists() {
        return Ok(());
    }
    std::fs::write(path, b"").map_err(|e| ConfigError::WriteError {
        path: path.to_path_buf(),
        source: e,
    })
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_config() -> Config {
        Config {
            default_agent: "claude".to_string(),
            storage_path: PathBuf::from("/home/user/bookmarks"),
            system_prompt: None,
            log_level: None,
            enrichment: EnrichmentConfig { enabled: true },
        }
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let config = sample_config();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn render_contains_comment_block() {
        let config = sample_config();
        let rendered = render_config_toml(&config);
        assert!(rendered.contains("# system_prompt = \"\"\""));
        assert!(rendered.contains("# - native notifications"));
    }

    #[test]
    fn render_with_comments_still_parses() {
        let config = sample_config();
        let rendered = render_config_toml(&config);
        let parsed: Config = toml::from_str(&rendered).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn render_with_system_prompt_set() {
        let mut config = sample_config();
        config.system_prompt = Some("You are a helpful assistant.".to_string());
        let rendered = render_config_toml(&config);
        let parsed: Config = toml::from_str(&rendered).unwrap();
        assert_eq!(
            parsed.system_prompt.as_deref(),
            Some("You are a helpful assistant.")
        );
    }

    #[test]
    fn save_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let config = sample_config();

        config.save(home).unwrap();
        let loaded = Config::load(home).unwrap();
        assert_eq!(config, loaded);
    }

    #[test]
    fn save_creates_config_dir() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        sample_config().save(home).unwrap();
        assert!(config_dir(home).is_dir());
        assert!(config_file(home).is_file());
    }

    #[test]
    fn saved_file_contains_enrichment_section() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        sample_config().save(home).unwrap();
        let contents = std::fs::read_to_string(config_file(home)).unwrap();
        assert!(contents.contains("[enrichment]"));
        assert!(contents.contains("enabled = true"));
    }

    #[test]
    fn saved_file_contains_absolute_storage_path() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        sample_config().save(home).unwrap();
        let contents = std::fs::read_to_string(config_file(home)).unwrap();
        assert!(contents.contains("storage_path = \"/home/user/bookmarks\""));
    }

    #[test]
    fn saved_file_contains_comment_block() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        sample_config().save(home).unwrap();
        let contents = std::fs::read_to_string(config_file(home)).unwrap();
        assert!(contents.contains("# system_prompt = \"\"\""));
    }

    #[test]
    fn load_missing_file_returns_not_found() {
        let tmp = TempDir::new().unwrap();
        let err = Config::load(tmp.path()).unwrap_err();
        assert!(matches!(err, ConfigError::NotFound { .. }));
    }

    #[test]
    fn load_malformed_toml_returns_parse_error() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let dir = config_dir(home);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(config_file(home), "not valid { toml }").unwrap();

        let err = Config::load(home).unwrap_err();
        assert!(matches!(err, ConfigError::ParseError { .. }));
    }

    // ── Path helper tests ───────────────────────────────────────────

    #[test]
    fn config_dir_under_home() {
        let home = Path::new("/Users/test");
        assert_eq!(config_dir(home), PathBuf::from("/Users/test/.agentmark"));
    }

    #[test]
    fn config_file_under_home() {
        let home = Path::new("/Users/test");
        assert_eq!(
            config_file(home),
            PathBuf::from("/Users/test/.agentmark/config.toml")
        );
    }

    #[test]
    fn index_db_under_home() {
        let home = Path::new("/Users/test");
        assert_eq!(
            index_db_path(home),
            PathBuf::from("/Users/test/.agentmark/index.db")
        );
    }

    #[test]
    fn resolve_absolute_storage_path() {
        let home = Path::new("/home/user");
        let cwd = Path::new("/tmp");
        let result = resolve_storage_path("/opt/bookmarks", home, cwd).unwrap();
        assert_eq!(result, PathBuf::from("/opt/bookmarks"));
    }

    #[test]
    fn resolve_relative_storage_path() {
        let home = Path::new("/home/user");
        let cwd = Path::new("/projects/agentmark");
        let result = resolve_storage_path("bookmarks", home, cwd).unwrap();
        assert_eq!(result, PathBuf::from("/projects/agentmark/bookmarks"));
    }

    #[test]
    fn resolve_relative_with_dot_prefix() {
        let home = Path::new("/home/user");
        let cwd = Path::new("/projects/agentmark");
        let result = resolve_storage_path("./bookmarks", home, cwd).unwrap();
        assert_eq!(result, PathBuf::from("/projects/agentmark/./bookmarks"));
    }

    #[test]
    fn resolve_tilde_storage_path() {
        let home = Path::new("/home/user");
        let cwd = Path::new("/tmp");
        let result = resolve_storage_path("~/my-bookmarks", home, cwd).unwrap();
        assert_eq!(result, PathBuf::from("/home/user/my-bookmarks"));
    }

    #[test]
    fn resolve_blank_storage_path_errors() {
        let home = Path::new("/home/user");
        let cwd = Path::new("/tmp");
        let err = resolve_storage_path("", home, cwd).unwrap_err();
        assert!(matches!(err, ConfigError::BlankStoragePath));
    }

    #[test]
    fn resolve_whitespace_only_storage_path_errors() {
        let home = Path::new("/home/user");
        let cwd = Path::new("/tmp");
        let err = resolve_storage_path("   ", home, cwd).unwrap_err();
        assert!(matches!(err, ConfigError::BlankStoragePath));
    }

    // ── Agent validation tests ──────────────────────────────────────

    #[test]
    fn validate_claude_lowercase() {
        assert_eq!(validate_agent("claude").unwrap(), "claude");
    }

    #[test]
    fn validate_codex_lowercase() {
        assert_eq!(validate_agent("codex").unwrap(), "codex");
    }

    #[test]
    fn validate_agent_case_insensitive() {
        assert_eq!(validate_agent("Claude").unwrap(), "claude");
        assert_eq!(validate_agent("CODEX").unwrap(), "codex");
    }

    #[test]
    fn validate_agent_trims_whitespace() {
        assert_eq!(validate_agent("  claude  ").unwrap(), "claude");
    }

    #[test]
    fn validate_agent_rejects_invalid() {
        assert!(matches!(
            validate_agent("chatgpt").unwrap_err(),
            ConfigError::InvalidAgent { .. }
        ));
    }

    #[test]
    fn validate_agent_rejects_empty() {
        assert!(matches!(
            validate_agent("").unwrap_err(),
            ConfigError::InvalidAgent { .. }
        ));
    }

    // ── Filesystem helper tests ─────────────────────────────────────

    #[test]
    fn ensure_dir_creates_missing() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("new-dir");
        assert!(!dir.exists());
        ensure_dir(&dir).unwrap();
        assert!(dir.is_dir());
    }

    #[test]
    fn ensure_dir_noop_if_exists() {
        let tmp = TempDir::new().unwrap();
        ensure_dir(tmp.path()).unwrap();
    }

    #[test]
    fn ensure_dir_errors_on_file() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("a-file");
        std::fs::write(&file, "data").unwrap();
        let err = ensure_dir(&file).unwrap_err();
        assert!(matches!(err, ConfigError::NotADirectory { .. }));
    }

    #[test]
    fn touch_file_creates_new() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("new.db");
        assert!(!f.exists());
        touch_file(&f).unwrap();
        assert!(f.is_file());
    }

    #[test]
    fn touch_file_preserves_existing() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("existing.db");
        std::fs::write(&f, "important data").unwrap();
        touch_file(&f).unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "important data");
    }
}
