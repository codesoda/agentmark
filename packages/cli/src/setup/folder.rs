//! The system folder dialog, behind a trait so tests never open one.
//!
//! The dialog program is a separate process (`osascript` on macOS, `zenity` or
//! `kdialog` on Linux), found on the agent search path because Chrome starts
//! the host with a bare `PATH`. When there is no dialog the UI falls back to
//! a typed path, so "unavailable" is an outcome, not an error.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tracing::debug;

use super::agents::find_executable;
use super::install::run_retrying_busy;

const PROMPT: &str = "Choose where AgentMark keeps bookmarks";
const DIALOG_TIMEOUT: Duration = Duration::from_secs(300);
const TIMED_OUT: &str = "Timed out waiting for the folder dialog.";
const NO_DIALOG: &str = "No folder dialog is available on this system.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderChoice {
    Chosen(PathBuf),
    Cancelled,
    Unavailable { reason: String },
}

/// Shows a folder picker. `start` is an existing directory to open in.
pub trait FolderDialog {
    fn pick(&self, start: Option<&Path>) -> FolderChoice;
}

/// The real dialog for this OS.
pub struct SystemFolderDialog;

impl FolderDialog for SystemFolderDialog {
    fn pick(&self, start: Option<&Path>) -> FolderChoice {
        let search_path = crate::agent::provider::agent_search_path();
        if cfg!(target_os = "macos") {
            let Some(program) = find_executable("osascript", &search_path) else {
                return unavailable(NO_DIALOG);
            };
            match run_dialog(&program, &macos_args(start), DIALOG_TIMEOUT) {
                Ok(out) => interpret_macos(&out),
                Err(reason) => FolderChoice::Unavailable { reason },
            }
        } else if cfg!(target_os = "linux") {
            let display = std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty());
            let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
            if !display && !wayland {
                return unavailable(NO_DIALOG);
            }
            let (program, args) = if let Some(p) = find_executable("zenity", &search_path) {
                (p, zenity_args(start))
            } else if let Some(p) = find_executable("kdialog", &search_path) {
                (p, kdialog_args(start))
            } else {
                return unavailable(NO_DIALOG);
            };
            match run_dialog(&program, &args, DIALOG_TIMEOUT) {
                Ok(out) => interpret_linux(&out),
                Err(reason) => FolderChoice::Unavailable { reason },
            }
        } else {
            unavailable(NO_DIALOG)
        }
    }
}

fn unavailable(reason: &str) -> FolderChoice {
    FolderChoice::Unavailable {
        reason: reason.to_string(),
    }
}

/// What a finished dialog process reported.
#[derive(Debug, Clone, PartialEq)]
struct DialogOutput {
    /// `None` when killed by a signal.
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// AppleScript strings use `"` and `\` escapes.
fn applescript_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn macos_args(start: Option<&Path>) -> Vec<String> {
    let mut choose = format!(
        "POSIX path of (choose folder with prompt {}",
        applescript_string(PROMPT)
    );
    if let Some(start) = start {
        choose.push_str(&format!(
            " default location (POSIX file {})",
            applescript_string(&start.display().to_string())
        ));
    }
    choose.push(')');
    vec![
        "-e".to_string(),
        "activate".to_string(),
        "-e".to_string(),
        choose,
    ]
}

fn zenity_args(start: Option<&Path>) -> Vec<String> {
    let mut args = vec![
        "--file-selection".to_string(),
        "--directory".to_string(),
        format!("--title={PROMPT}"),
    ];
    if let Some(start) = start {
        // The trailing slash makes zenity open inside the folder.
        args.push(format!("--filename={}/", start.display()));
    }
    args
}

fn kdialog_args(start: Option<&Path>) -> Vec<String> {
    let mut args = vec!["--getexistingdirectory".to_string()];
    if let Some(start) = start {
        args.push(start.display().to_string());
    }
    args
}

/// `osascript` exits non-zero with error -128 when the user cancels.
fn interpret_macos(out: &DialogOutput) -> FolderChoice {
    let path = out.stdout.trim();
    match out.code {
        Some(0) if !path.is_empty() => FolderChoice::Chosen(PathBuf::from(path)),
        Some(code) if code != 0 && out.stderr.contains("-128") => FolderChoice::Cancelled,
        _ => FolderChoice::Unavailable {
            reason: dialog_failure(out),
        },
    }
}

/// zenity and kdialog exit 1 when the user cancels.
fn interpret_linux(out: &DialogOutput) -> FolderChoice {
    let path = out.stdout.trim();
    match out.code {
        Some(0) if !path.is_empty() => FolderChoice::Chosen(PathBuf::from(path)),
        Some(1) => FolderChoice::Cancelled,
        _ => FolderChoice::Unavailable {
            reason: dialog_failure(out),
        },
    }
}

fn dialog_failure(out: &DialogOutput) -> String {
    let detail = out.stderr.trim();
    if detail.is_empty() {
        "The folder dialog did not open.".to_string()
    } else {
        format!("The folder dialog did not open: {detail}")
    }
}

/// Run the dialog, killing it after `timeout`. `Err` is the unavailable reason.
fn run_dialog(program: &Path, args: &[String], timeout: Duration) -> Result<DialogOutput, String> {
    debug!(program = %program.display(), "opening folder dialog");
    let mut child = run_retrying_busy(|| {
        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    })
    .map_err(|e| format!("The folder dialog did not open: {e}"))?;

    // Drain both pipes on threads so a chatty dialog cannot block on a full pipe.
    let stdout = child.stdout.take().map(spawn_reader);
    let stderr = child.stderr.take().map(spawn_reader);

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(TIMED_OUT.to_string());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("The folder dialog did not open: {e}")),
        }
    };

    let collect = |h: Option<std::thread::JoinHandle<String>>| {
        h.and_then(|h| h.join().ok()).unwrap_or_default()
    };
    Ok(DialogOutput {
        code: status.code(),
        stdout: collect(stdout),
        stderr: collect(stderr),
    })
}

fn spawn_reader<R: Read + Send + 'static>(mut pipe: R) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn out(code: Option<i32>, stdout: &str, stderr: &str) -> DialogOutput {
        DialogOutput {
            code,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        }
    }

    #[test]
    fn macos_args_with_and_without_start() {
        assert_eq!(
            macos_args(None),
            vec![
                "-e",
                "activate",
                "-e",
                "POSIX path of (choose folder with prompt \"Choose where AgentMark keeps bookmarks\")"
            ]
        );
        let args = macos_args(Some(Path::new("/Users/me/My \"Docs\"")));
        assert_eq!(
            args[3],
            "POSIX path of (choose folder with prompt \"Choose where AgentMark keeps bookmarks\" \
             default location (POSIX file \"/Users/me/My \\\"Docs\\\"\"))"
        );
    }

    #[test]
    fn linux_args() {
        assert_eq!(
            zenity_args(Some(Path::new("/home/me"))),
            vec![
                "--file-selection",
                "--directory",
                "--title=Choose where AgentMark keeps bookmarks",
                "--filename=/home/me/"
            ]
        );
        assert_eq!(zenity_args(None).len(), 3);
        assert_eq!(
            kdialog_args(Some(Path::new("/home/me"))),
            vec!["--getexistingdirectory", "/home/me"]
        );
    }

    #[test]
    fn macos_outcomes() {
        assert_eq!(
            interpret_macos(&out(Some(0), "/Users/me/Marks/\n", "")),
            FolderChoice::Chosen(PathBuf::from("/Users/me/Marks/"))
        );
        assert_eq!(
            interpret_macos(&out(Some(1), "", "execution error: User canceled. (-128)")),
            FolderChoice::Cancelled
        );
        assert!(matches!(
            interpret_macos(&out(Some(1), "", "execution error: no UI (-1743)")),
            FolderChoice::Unavailable { reason } if reason.contains("-1743")
        ));
        assert!(matches!(
            interpret_macos(&out(None, "", "")),
            FolderChoice::Unavailable { .. }
        ));
    }

    #[test]
    fn linux_outcomes() {
        assert_eq!(
            interpret_linux(&out(Some(0), "/home/me/marks\n", "")),
            FolderChoice::Chosen(PathBuf::from("/home/me/marks"))
        );
        assert_eq!(
            interpret_linux(&out(Some(1), "", "")),
            FolderChoice::Cancelled
        );
        assert!(matches!(
            interpret_linux(&out(Some(255), "", "cannot open display")),
            FolderChoice::Unavailable { .. }
        ));
    }

    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("fake-dialog");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn run_dialog_captures_output_and_exit_code() {
        let tmp = TempDir::new().unwrap();
        let program = script(tmp.path(), "echo \"$2\"; echo oops >&2; exit 3");
        let result = run_dialog(
            &program,
            &["a".to_string(), "b c".to_string()],
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(result, out(Some(3), "b c\n", "oops\n"));
    }

    #[test]
    fn run_dialog_kills_after_timeout() {
        let tmp = TempDir::new().unwrap();
        let program = script(tmp.path(), "exec sleep 30");
        let started = Instant::now();
        let err = run_dialog(&program, &[], Duration::from_millis(200)).unwrap_err();
        assert_eq!(err, TIMED_OUT);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn run_dialog_reports_a_missing_program() {
        let err = run_dialog(
            Path::new("/nonexistent/dialog"),
            &[],
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(err.starts_with("The folder dialog did not open"), "{err}");
    }
}
