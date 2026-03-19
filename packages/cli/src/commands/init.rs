use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use crate::config::{self, Config, ConfigError, EnrichmentConfig};

/// Entry point for `agentmark init` using real stdio and environment.
pub fn run_init() -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    let cwd = std::env::current_dir().map_err(|_| {
        ConfigError::BlankStoragePath // reuse; cwd failure means we can't resolve defaults
    })?;

    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut writer = std::io::stdout();

    run_init_with_io(&home, &cwd, &mut reader, &mut writer)?;
    Ok(())
}

/// Testable init implementation with injected I/O and paths.
pub fn run_init_with_io(
    home: &Path,
    cwd: &Path,
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> Result<(), ConfigError> {
    let config_path = config::config_file(home);

    // Check for existing config
    if config_path.exists() {
        writeln!(writer, "Config already exists at {}", config_path.display()).ok();
        let overwrite = prompt_yes_no(reader, writer, "Overwrite? [y/N] ", false)?;
        if !overwrite {
            writeln!(writer, "Initialization cancelled.").ok();
            return Err(ConfigError::Cancelled);
        }
    }

    // Prompt for default agent
    let default_agent = prompt_agent(reader, writer)?;

    // Prompt for storage path
    let storage_path = prompt_storage_path(reader, writer, home, cwd)?;

    // Build config
    let config = Config {
        default_agent,
        storage_path: storage_path.clone(),
        system_prompt: None,
        log_level: None,
        enrichment: EnrichmentConfig { enabled: true },
    };

    // Create directories
    let config_dir = config::config_dir(home);
    config::ensure_dir(&config_dir)?;
    config::ensure_dir(&storage_path)?;

    // Save config
    config.save(home)?;

    // Touch index.db (non-destructive)
    let db_path = config::index_db_path(home);
    config::touch_file(&db_path)?;

    // Success output
    writeln!(writer, "Initialized AgentMark:").ok();
    writeln!(writer, "  config:  {}", config::config_file(home).display()).ok();
    writeln!(writer, "  storage: {}", storage_path.display()).ok();
    writeln!(writer, "  agent:   {}", config.default_agent).ok();

    Ok(())
}

// ── Prompt helpers ──────────────────────────────────────────────────

fn read_line(reader: &mut dyn BufRead) -> Result<String, ConfigError> {
    let mut buf = String::new();
    let n = reader
        .read_line(&mut buf)
        .map_err(|_| ConfigError::UnexpectedEof)?;
    if n == 0 {
        return Err(ConfigError::UnexpectedEof);
    }
    Ok(buf)
}

fn prompt_yes_no(
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
    prompt: &str,
    default: bool,
) -> Result<bool, ConfigError> {
    write!(writer, "{prompt}").ok();
    writer.flush().ok();
    let line = read_line(reader)?;
    let trimmed = line.trim().to_lowercase();
    if trimmed.is_empty() {
        return Ok(default);
    }
    match trimmed.as_str() {
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        _ => Ok(default),
    }
}

fn prompt_agent(reader: &mut dyn BufRead, writer: &mut dyn Write) -> Result<String, ConfigError> {
    write!(writer, "Default agent [claude/codex] (claude): ").ok();
    writer.flush().ok();
    let line = read_line(reader)?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok("claude".to_string());
    }
    config::validate_agent(trimmed)
}

fn prompt_storage_path(
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
    home: &Path,
    cwd: &Path,
) -> Result<PathBuf, ConfigError> {
    write!(writer, "Bookmark storage path (./bookmarks): ").ok();
    writer.flush().ok();
    let line = read_line(reader)?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return config::resolve_storage_path("./bookmarks", home, cwd);
    }
    config::resolve_storage_path(trimmed, home, cwd)
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::TempDir;

    fn run_with_input(home: &Path, cwd: &Path, input: &str) -> Result<String, ConfigError> {
        let mut reader = Cursor::new(input.as_bytes().to_vec());
        let mut output = Vec::new();
        run_init_with_io(home, cwd, &mut reader, &mut output)?;
        Ok(String::from_utf8(output).unwrap())
    }

    #[test]
    fn happy_path_creates_config_and_dirs() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();

        let input = "claude\n./bookmarks\n";
        let output = run_with_input(&home, &cwd, input).unwrap();

        assert!(config::config_file(&home).is_file());
        assert!(config::index_db_path(&home).is_file());
        assert!(cwd.join("bookmarks").is_dir());
        assert!(output.contains("Initialized AgentMark"));
    }

    #[test]
    fn defaults_on_blank_input() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();

        let input = "\n\n"; // blank agent, blank path
        run_with_input(&home, &cwd, input).unwrap();

        let loaded = Config::load(&home).unwrap();
        assert_eq!(loaded.default_agent, "claude");
        assert_eq!(loaded.storage_path, cwd.join("./bookmarks"));
    }

    #[test]
    fn codex_agent_accepted() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();

        let input = "codex\n\n";
        run_with_input(&home, &cwd, input).unwrap();

        let loaded = Config::load(&home).unwrap();
        assert_eq!(loaded.default_agent, "codex");
    }

    #[test]
    fn case_insensitive_agent() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();

        let input = "CLAUDE\n\n";
        run_with_input(&home, &cwd, input).unwrap();

        let loaded = Config::load(&home).unwrap();
        assert_eq!(loaded.default_agent, "claude");
    }

    #[test]
    fn invalid_agent_errors() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();

        let input = "chatgpt\n\n";
        let err = run_with_input(&home, &cwd, input).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidAgent { .. }));
    }

    #[test]
    fn absolute_storage_path_preserved() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();

        let abs_path = tmp.path().join("my-bookmarks");
        let input = format!("claude\n{}\n", abs_path.display());
        run_with_input(&home, &cwd, &input).unwrap();

        let loaded = Config::load(&home).unwrap();
        assert_eq!(loaded.storage_path, abs_path);
        assert!(abs_path.is_dir());
    }

    #[test]
    fn existing_config_decline_overwrite() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();

        // Create initial config
        let input = "claude\n./bookmarks\n";
        run_with_input(&home, &cwd, input).unwrap();

        let original = std::fs::read_to_string(config::config_file(&home)).unwrap();

        // Decline overwrite
        let input = "n\n";
        let err = run_with_input(&home, &cwd, input).unwrap_err();
        assert!(matches!(err, ConfigError::Cancelled));

        // Config unchanged
        let after = std::fs::read_to_string(config::config_file(&home)).unwrap();
        assert_eq!(original, after);
    }

    #[test]
    fn existing_config_confirm_overwrite() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();

        // Create initial config with claude
        let input = "claude\n./bookmarks\n";
        run_with_input(&home, &cwd, input).unwrap();

        // Overwrite with codex
        let input = "y\ncodex\n./bookmarks2\n";
        run_with_input(&home, &cwd, input).unwrap();

        let loaded = Config::load(&home).unwrap();
        assert_eq!(loaded.default_agent, "codex");
    }

    #[test]
    fn index_db_not_truncated_on_rerun() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();

        // First run
        let input = "claude\n./bookmarks\n";
        run_with_input(&home, &cwd, input).unwrap();

        // Write data to index.db
        let db_path = config::index_db_path(&home);
        std::fs::write(&db_path, "important data").unwrap();

        // Second run with overwrite
        let input = "y\nclaude\n./bookmarks\n";
        run_with_input(&home, &cwd, input).unwrap();

        // Data preserved
        assert_eq!(std::fs::read_to_string(&db_path).unwrap(), "important data");
    }

    #[test]
    fn eof_during_prompts() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");

        let input = ""; // empty stdin
        let err = run_with_input(&home, &cwd, input).unwrap_err();
        assert!(matches!(err, ConfigError::UnexpectedEof));
    }

    #[test]
    fn storage_path_file_collision() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();

        // Create a file where storage dir should be
        let blocker = cwd.join("bookmarks");
        std::fs::write(&blocker, "not a dir").unwrap();

        let input = "claude\n./bookmarks\n";
        let err = run_with_input(&home, &cwd, input).unwrap_err();
        assert!(matches!(err, ConfigError::NotADirectory { .. }));
    }

    #[test]
    fn config_dir_file_collision() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();

        // Create a file where .agentmark dir should be
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(".agentmark"), "not a dir").unwrap();

        let input = "claude\n./bookmarks\n";
        let err = run_with_input(&home, &cwd, input).unwrap_err();
        assert!(matches!(err, ConfigError::NotADirectory { .. }));
    }

    #[test]
    fn blank_overwrite_answer_defaults_to_no() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();

        // Create initial config
        let input = "claude\n./bookmarks\n";
        run_with_input(&home, &cwd, input).unwrap();

        // Blank answer = default no
        let input = "\n";
        let err = run_with_input(&home, &cwd, input).unwrap_err();
        assert!(matches!(err, ConfigError::Cancelled));
    }
}
