//! `agentmark init`: terminal front end for the setup core.
//!
//! Setup normally happens in the extension or the desktop app; this keeps a
//! terminal path working. It reads the state and writes settings only through
//! `setup`, so the config is edited the same way every caller edits it.

use std::io::{BufRead, Write};
use std::path::Path;

use thiserror::Error;
use tracing::instrument;

use crate::cli::InitArgs;
use crate::config::{self, ConfigError};
use crate::setup::{self, FieldError, SettingsChanges, SettingsError, SetupEnv};

#[derive(Debug, Error)]
pub enum InitError {
    #[error("{0}")]
    Config(#[from] ConfigError),

    /// One `<field>: <message>` per error. `main` prefixes the first line with
    /// `error: `, so the later lines carry their own prefix.
    #[error("{}", format_field_errors(.0))]
    Invalid(Vec<FieldError>),

    #[error("{0}")]
    Settings(SettingsError),

    #[error("cannot read the current directory: {0}")]
    Cwd(std::io::Error),
}

impl From<SettingsError> for InitError {
    fn from(e: SettingsError) -> Self {
        match e {
            SettingsError::Invalid(errors) => InitError::Invalid(errors),
            other => InitError::Settings(other),
        }
    }
}

fn format_field_errors(errors: &[FieldError]) -> String {
    errors
        .iter()
        .map(|e| format!("{}: {}", e.field.as_str(), e.message))
        .collect::<Vec<_>>()
        .join("\nerror: ")
}

/// Entry point for `agentmark init` using real stdio and environment.
pub fn run_init(args: InitArgs) -> Result<(), Box<dyn std::error::Error>> {
    let env = SetupEnv::from_process()?;
    let cwd = std::env::current_dir().map_err(InitError::Cwd)?;

    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut writer = std::io::stdout();

    run_init_with_io(&env, &cwd, &args, &mut reader, &mut writer)?;
    Ok(())
}

/// Testable init over injected environment, working directory and I/O.
#[instrument(skip_all)]
pub fn run_init_with_io(
    env: &SetupEnv,
    cwd: &Path,
    args: &InitArgs,
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> Result<(), InitError> {
    let state = setup::read_state(env);
    let home = env.home.as_path();

    let changes = if args.is_non_interactive() {
        SettingsChanges {
            storage_path: args
                .storage_path
                .as_deref()
                .map(|p| resolve_folder(p, home, cwd)),
            default_agent: args.agent.clone(),
            enrichment_enabled: args.summaries.as_deref().map(|s| s == "on"),
            ..SettingsChanges::default()
        }
    } else {
        if state.initialized {
            let update = prompt_yes_no(reader, writer, "Update existing settings? [y/N] ", false)?;
            if !update {
                writeln!(writer, "Nothing changed.").ok();
                return Ok(());
            }
        } else if let Some(problem) = &state.config_problem {
            writeln!(
                writer,
                "The existing config cannot be used ({problem}).\n\
                 It will be kept beside the new one as config.toml.invalid-<time>."
            )
            .ok();
        }

        let settings = &state.settings;
        let agent = prompt_line(
            reader,
            writer,
            &format!(
                "Default agent [claude/codex/pi] ({}): ",
                settings.default_agent
            ),
        )?;
        let folder = prompt_line(
            reader,
            writer,
            &format!("Bookmark folder ({}): ", settings.storage_path_display),
        )?;
        let summaries_prompt = if settings.enrichment_enabled {
            "Write a summary and tags after each save? [Y/n] "
        } else {
            "Write a summary and tags after each save? [y/N] "
        };
        let summaries = prompt_yes_no(
            reader,
            writer,
            summaries_prompt,
            settings.enrichment_enabled,
        )?;

        SettingsChanges {
            storage_path: folder.map(|p| resolve_folder(&p, home, cwd)),
            default_agent: agent,
            enrichment_enabled: Some(summaries),
            ..SettingsChanges::default()
        }
    };

    let new_state = setup::apply_settings(env, &changes)?;
    let s = &new_state.settings;
    writeln!(writer, "Initialized AgentMark:").ok();
    writeln!(writer, "  config:  {}", config::config_file(home).display()).ok();
    writeln!(writer, "  storage: {}", s.storage_path).ok();
    writeln!(writer, "  agent:   {}", s.default_agent).ok();
    let on_off = if s.enrichment_enabled { "on" } else { "off" };
    writeln!(writer, "  summaries: {on_off}").ok();
    Ok(())
}

/// The core rejects relative paths, so the terminal resolves them against the
/// working directory first, as `init` always has. Blank input is passed on so
/// the core reports it.
fn resolve_folder(input: &str, home: &Path, cwd: &Path) -> String {
    match config::resolve_storage_path(input, home, cwd) {
        Ok(path) => path.display().to_string(),
        Err(_) => input.to_string(),
    }
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

/// Ask once; blank input means "keep the default" (`None`).
fn prompt_line(
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
    prompt: &str,
) -> Result<Option<String>, ConfigError> {
    write!(writer, "{prompt}").ok();
    writer.flush().ok();
    let line = read_line(reader)?;
    let trimmed = line.trim();
    Ok((!trimmed.is_empty()).then(|| trimmed.to_string()))
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
    match trimmed.as_str() {
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        _ => Ok(default),
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::setup::{FieldErrorCode, SettingsField};
    use std::io::Cursor;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use tempfile::TempDir;

    struct Fixture {
        _tmp: TempDir,
        env: SetupEnv,
        cwd: PathBuf,
        bin: PathBuf,
    }

    fn fixture() -> Fixture {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("work");
        let bin = tmp.path().join("bin");
        for dir in [&home, &cwd, &bin] {
            std::fs::create_dir_all(dir).unwrap();
        }
        Fixture {
            env: SetupEnv {
                home,
                search_path: bin.clone().into_os_string(),
            },
            cwd,
            bin,
            _tmp: tmp,
        }
    }

    impl Fixture {
        fn run(&self, args: &InitArgs, input: &str) -> Result<String, InitError> {
            let mut reader = Cursor::new(input.as_bytes().to_vec());
            let mut output = Vec::new();
            run_init_with_io(&self.env, &self.cwd, args, &mut reader, &mut output)?;
            Ok(String::from_utf8(output).unwrap())
        }

        fn interactive(&self, input: &str) -> Result<String, InitError> {
            self.run(&InitArgs::default(), input)
        }

        fn config(&self) -> Config {
            Config::load(&self.env.home).unwrap()
        }

        fn add_agent(&self, name: &str) {
            let path = self.bin.join(name);
            std::fs::write(&path, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn happy_path_creates_config_folder_and_index() {
        let f = fixture();
        let output = f.interactive("claude\n./bookmarks\ny\n").unwrap();
        assert!(config::config_file(&f.env.home).is_file());
        assert!(config::index_db_path(&f.env.home).is_file());
        assert!(f.cwd.join("bookmarks").is_dir());
        assert!(output.contains("Initialized AgentMark"));
        assert!(output.contains("  summaries: on"));
        assert_eq!(f.config().storage_path, f.cwd.join("bookmarks"));
    }

    #[test]
    fn blank_answers_use_the_state_defaults() {
        let f = fixture();
        let output = f.interactive("\n\n\n").unwrap();
        assert!(output.contains("Default agent [claude/codex/pi] (claude): "));
        assert!(output.contains("Bookmark folder (~/Documents/AgentMark): "));
        assert!(
            output.contains("[y/N]"),
            "summaries default off with no agent"
        );
        let config = f.config();
        assert_eq!(config.default_agent, "claude");
        assert_eq!(config.storage_path, f.env.home.join("Documents/AgentMark"));
        assert!(!config.enrichment.enabled);
    }

    #[test]
    fn defaults_follow_the_agents_found() {
        let f = fixture();
        f.add_agent("codex");
        let output = f.interactive("\n\n\n").unwrap();
        assert!(output.contains("(codex): "));
        assert!(output.contains("[Y/n]"));
        assert_eq!(f.config().default_agent, "codex");
        assert!(f.config().enrichment.enabled);
    }

    #[test]
    fn agent_is_case_insensitive() {
        let f = fixture();
        f.interactive("CODEX\n\nn\n").unwrap();
        assert_eq!(f.config().default_agent, "codex");
        assert!(!f.config().enrichment.enabled);
    }

    #[test]
    fn invalid_agent_is_a_field_error() {
        let f = fixture();
        let err = f.interactive("chatgpt\n\n\n").unwrap_err();
        match &err {
            InitError::Invalid(errors) => {
                assert_eq!(errors[0].field, SettingsField::DefaultAgent);
                assert_eq!(errors[0].code, FieldErrorCode::InvalidAgent);
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
        assert!(err.to_string().starts_with("default_agent: "));
        assert!(!config::config_file(&f.env.home).exists());
    }

    #[test]
    fn several_errors_print_one_line_each() {
        let err = InitError::Invalid(vec![
            FieldError {
                field: SettingsField::StoragePath,
                index: None,
                code: FieldErrorCode::Blank,
                message: "a".into(),
            },
            FieldError {
                field: SettingsField::DefaultAgent,
                index: None,
                code: FieldErrorCode::InvalidAgent,
                message: "b".into(),
            },
        ]);
        assert_eq!(
            format!("error: {err}"),
            "error: storage_path: a\nerror: default_agent: b"
        );
    }

    #[test]
    fn absolute_and_tilde_folders() {
        let f = fixture();
        let abs = f.cwd.join("abs-marks");
        f.interactive(&format!("claude\n{}\n\n", abs.display()))
            .unwrap();
        assert_eq!(f.config().storage_path, abs);

        f.interactive("y\n\n~/Marks\n\n").unwrap();
        assert_eq!(f.config().storage_path, f.env.home.join("Marks"));
    }

    #[test]
    fn existing_setup_decline_changes_nothing() {
        let f = fixture();
        f.interactive("claude\n./bookmarks\n\n").unwrap();
        let original = std::fs::read_to_string(config::config_file(&f.env.home)).unwrap();

        let output = f.interactive("n\n").unwrap();
        assert!(output.contains("Update existing settings? [y/N] "));
        assert!(output.contains("Nothing changed."));
        let blank = f.interactive("\n").unwrap();
        assert!(blank.contains("Nothing changed."), "blank means no");

        let after = std::fs::read_to_string(config::config_file(&f.env.home)).unwrap();
        assert_eq!(original, after);
    }

    #[test]
    fn existing_setup_update_keeps_current_values_as_defaults() {
        let f = fixture();
        f.interactive("codex\n./bookmarks\ny\n").unwrap();
        let output = f.interactive("y\n\n\nn\n").unwrap();
        assert!(output.contains("(codex): "));
        assert!(output.contains("Bookmark folder ("));
        let config = f.config();
        assert_eq!(config.default_agent, "codex");
        assert_eq!(config.storage_path, f.cwd.join("bookmarks"));
        assert!(!config.enrichment.enabled);
    }

    #[test]
    fn rerun_keeps_the_index() {
        let f = fixture();
        f.interactive("claude\n./bookmarks\n\n").unwrap();
        let conn = crate::db::open_and_migrate(&config::index_db_path(&f.env.home)).unwrap();
        let bm = crate::models::Bookmark::new("https://example.com", "Kept");
        crate::db::BookmarkRepository::new(&conn)
            .insert(&bm)
            .unwrap();
        drop(conn);

        f.interactive("y\n\n\n\n").unwrap();
        let state = setup::read_state(&f.env);
        assert_eq!(state.bookmark_count, Some(1));
    }

    #[test]
    fn eof_during_prompts() {
        let f = fixture();
        let err = f.interactive("").unwrap_err();
        assert!(matches!(err, InitError::Config(ConfigError::UnexpectedEof)));
        let err = f.interactive("claude\n").unwrap_err();
        assert!(matches!(err, InitError::Config(ConfigError::UnexpectedEof)));
    }

    #[test]
    fn storage_path_file_collision() {
        let f = fixture();
        std::fs::write(f.cwd.join("bookmarks"), "not a dir").unwrap();
        let err = f.interactive("claude\n./bookmarks\n\n").unwrap_err();
        match err {
            InitError::Invalid(errors) => assert_eq!(errors[0].code, FieldErrorCode::NotADirectory),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn config_dir_file_collision() {
        let f = fixture();
        std::fs::write(f.env.home.join(".agentmark"), "not a dir").unwrap();
        let err = f.interactive("claude\n./bookmarks\n\n").unwrap_err();
        assert!(
            matches!(err, InitError::Settings(SettingsError::Write { .. })),
            "{err:?}"
        );
    }

    #[test]
    fn invalid_config_is_replaced_with_a_note() {
        let f = fixture();
        std::fs::create_dir_all(f.env.home.join(".agentmark")).unwrap();
        std::fs::write(config::config_file(&f.env.home), "broken = [").unwrap();
        let output = f.interactive("\n\n\n").unwrap();
        assert!(
            output.contains("The existing config cannot be used"),
            "{output}"
        );
        assert!(Config::load(&f.env.home).is_ok());
    }

    fn invalid_copies(home: &Path) -> usize {
        std::fs::read_dir(home.join(".agentmark"))
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("config.toml.invalid-")
            })
            .count()
    }

    /// A hand-written config without `[enrichment]` keeps its folder.
    #[test]
    fn config_missing_a_table_keeps_its_folder() {
        let f = fixture();
        let store = f.cwd.join("store");
        std::fs::create_dir_all(f.env.home.join(".agentmark")).unwrap();
        std::fs::write(
            config::config_file(&f.env.home),
            format!("storage_path = \"{}\"\n", store.display()),
        )
        .unwrap();
        let args = InitArgs {
            agent: Some("codex".into()),
            ..InitArgs::default()
        };
        f.run(&args, "").unwrap();
        let config = f.config();
        assert_eq!(config.storage_path, store);
        assert_eq!(config.default_agent, "codex");
        assert_eq!(invalid_copies(&f.env.home), 1);
    }

    /// `log_level = 5` cannot be fixed by the fields init writes: the file
    /// is kept beside a fresh one, as the note promises, and init succeeds.
    #[test]
    fn unrepairable_config_is_kept_beside_a_new_one() {
        let f = fixture();
        let store = f.cwd.join("store");
        std::fs::create_dir_all(f.env.home.join(".agentmark")).unwrap();
        std::fs::write(
            config::config_file(&f.env.home),
            format!(
                "default_agent = \"pi\"\nstorage_path = \"{}\"\nlog_level = 5\n[enrichment]\nenabled = true\n",
                store.display()
            ),
        )
        .unwrap();
        let output = f.interactive("\n\n\n").unwrap();
        assert!(output.contains("config.toml.invalid-<time>"), "{output}");
        assert!(output.contains("(pi): "), "{output}");
        let config = f.config();
        assert_eq!(config.storage_path, store);
        assert_eq!(config.default_agent, "pi");
        assert!(config.enrichment.enabled);
        assert_eq!(invalid_copies(&f.env.home), 1);
    }

    // ── Non-interactive ────────────────────────────────────────────

    #[test]
    fn flags_skip_every_prompt() {
        let f = fixture();
        let args = InitArgs {
            storage_path: Some("marks".into()),
            agent: Some("pi".into()),
            summaries: Some("off".into()),
            yes: false,
        };
        let output = f.run(&args, "").unwrap();
        assert!(!output.contains("?"), "no prompts: {output}");
        let config = f.config();
        assert_eq!(config.storage_path, f.cwd.join("marks"), "relative to cwd");
        assert_eq!(config.default_agent, "pi");
        assert!(!config.enrichment.enabled);
    }

    #[test]
    fn yes_alone_uses_defaults_and_updates_without_asking() {
        let f = fixture();
        let yes = InitArgs {
            yes: true,
            ..InitArgs::default()
        };
        f.run(&yes, "").unwrap();
        assert_eq!(
            f.config().storage_path,
            f.env.home.join("Documents/AgentMark")
        );

        let switch = InitArgs {
            agent: Some("codex".into()),
            ..InitArgs::default()
        };
        let output = f.run(&switch, "").unwrap();
        assert!(!output.contains("Update existing settings"));
        let config = f.config();
        assert_eq!(config.default_agent, "codex");
        assert_eq!(
            config.storage_path,
            f.env.home.join("Documents/AgentMark"),
            "kept"
        );
    }

    #[test]
    fn blank_flag_path_is_reported() {
        let f = fixture();
        let args = InitArgs {
            storage_path: Some("  ".into()),
            ..InitArgs::default()
        };
        match f.run(&args, "").unwrap_err() {
            InitError::Invalid(errors) => assert_eq!(errors[0].code, FieldErrorCode::Blank),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }
}
