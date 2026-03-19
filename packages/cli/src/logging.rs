//! Logging setup for the AgentMark CLI.
//!
//! Initialises a daily-rolling file appender under `~/.agentmark/logs/`
//! using the `tracing` ecosystem. Log files are named `agentmark.YYYY-MM-DD.log`.

use std::path::Path;

use tracing_appender::rolling;
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

    let file_appender = rolling::daily(&logs_dir, "agentmark.log");

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
