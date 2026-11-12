//! Logging setup for the AgentMark CLI.
//!
//! Initialises a daily-rolling file appender under `~/.agentmark/logs/`
//! using the `tracing` ecosystem. Log files are named `agentmark.YYYY-MM-DD.log`
//! (`tracing-appender` joins prefix, date and suffix with dots). Builds before
//! 0.3.0 wrote `agentmark-.YYYY-MM-DD..log`; those files are left as they are.

use std::path::Path;

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, EnvFilter};

/// Initialise file-based tracing for a given home directory.
///
/// Creates `~/.agentmark/logs/` if it doesn't exist, then registers a
/// daily-rolling file appender. Priority for the log level filter:
///
/// 1. `AGENTMARK_LOG` env var (highest)
/// 2. `log_level` field in `config.toml`
/// 3. `"info"` (default)
pub fn init(home: &Path, config_level: Option<&str>) {
    let logs_dir = crate::config::logs_dir(home);

    // Best-effort directory creation — if it fails, the appender will
    // simply fail to open files and tracing becomes a no-op.
    let _ = std::fs::create_dir_all(&logs_dir);

    let file_appender = file_appender(&logs_dir);

    let filter = EnvFilter::try_from_env("AGENTMARK_LOG")
        .unwrap_or_else(|_| EnvFilter::new(config_level.unwrap_or("info")));

    fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_writer(file_appender)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .init();
}

/// The daily file appender. The appender adds the dots itself, so the
/// prefix and suffix carry none.
fn file_appender(logs_dir: &Path) -> RollingFileAppender {
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("agentmark")
        .filename_suffix("log")
        .build(logs_dir)
        .expect("failed to create log file appender")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn log_files_are_named_agentmark_date_log() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut appender = file_appender(tmp.path());
        appender.write_all(b"line\n").unwrap();
        appender.flush().unwrap();
        let names: Vec<String> = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
        let name = &names[0];
        // agentmark.YYYY-MM-DD.log
        assert!(
            name.starts_with("agentmark.") && name.ends_with(".log"),
            "{name}"
        );
        assert_eq!(name.len(), "agentmark.".len() + 10 + ".log".len(), "{name}");
        assert!(!name.contains(".."), "{name}");
    }
}
