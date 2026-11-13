//! Installation steps shared by `install-extension`, `install.sh` (through
//! the CLI) and the desktop app: place the CLI at its stable path, link it
//! onto `PATH`, register the native messaging host, install the skill.
//!
//! Chrome's manifests always point at `~/.agentmark/bin/agentmark`, a real
//! copy rather than a path inside the app bundle, so moving, updating or
//! deleting the app never breaks the extension.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::{debug, info, warn};

use super::extension_ids::{ensure_ids_file, merge_ids, WEB_STORE_EXTENSION_ID};
use super::write_file_atomic;
use crate::config;

pub use crate::skill::SkillInstallResult;

/// The native messaging host name the extension connects to.
pub const NATIVE_HOST_NAME: &str = "com.agentmark.native";

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("no extension IDs are known yet, so no native host was registered")]
    NoExtensionIds,

    #[error(
        "invalid extension ID '{id}': expected 32 letters from a to p (see chrome://extensions)"
    )]
    InvalidExtensionId { id: String },

    #[error("native host registration is not supported on this OS")]
    UnsupportedOs,

    #[error("{path} is not valid UTF-8")]
    NonUtf8Path { path: PathBuf },

    #[error("cannot read the version of {path}: {reason}")]
    Version { path: PathBuf, reason: String },

    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("skill installation failed: {0}")]
    Skill(String),
}

fn io_error(path: &Path) -> impl FnOnce(std::io::Error) -> InstallError + '_ {
    move |source| InstallError::Io {
        path: path.to_path_buf(),
        source,
    }
}

// ── Paths ───────────────────────────────────────────────────────────

/// `~/.agentmark/bin/agentmark`: the path every manifest registers.
pub fn stable_cli_path(home: &Path) -> PathBuf {
    config::config_dir(home).join("bin").join("agentmark")
}

/// The binary `install-extension` registers: the stable copy when it exists,
/// otherwise the running executable (a development build).
pub fn registered_binary_path(home: &Path, current_exe: &Path) -> PathBuf {
    let stable = stable_cli_path(home);
    if stable.is_file() {
        stable
    } else {
        std::fs::canonicalize(current_exe).unwrap_or_else(|_| current_exe.to_path_buf())
    }
}

/// `AGENTMARK_LOCAL_BIN`, else `~/.local/bin` (same rule as `install.sh`).
pub fn default_local_bin(home: &Path) -> PathBuf {
    local_bin_from(std::env::var_os("AGENTMARK_LOCAL_BIN"), home)
}

fn local_bin_from(override_dir: Option<OsString>, home: &Path) -> PathBuf {
    match override_dir {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => home.join(".local").join("bin"),
    }
}

// ── Native host manifests ───────────────────────────────────────────

/// One browser's manifest location.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct BrowserTarget {
    pub browser: &'static str,
    /// The `NativeMessagingHosts` directory.
    pub dir: PathBuf,
    /// Chrome is always written; other browsers only when installed.
    pub always: bool,
}

impl BrowserTarget {
    pub fn manifest_path(&self) -> PathBuf {
        self.dir.join(format!("{NATIVE_HOST_NAME}.json"))
    }
}

/// Manifest targets for the OS this binary was built for. Release builds are
/// made on the OS they run on, so a compile-time choice is correct.
pub(super) fn manifest_targets(home: &Path) -> Vec<BrowserTarget> {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "other"
    };
    targets_for_os(home, os)
}

/// Brave and Arc paths come from community sources (contract §8.2); writing
/// only when the browser's data directory exists keeps a wrong guess harmless.
fn targets_for_os(home: &Path, os: &str) -> Vec<BrowserTarget> {
    let rows: &[(&str, &str, bool)] = match os {
        "macos" => &[
            ("Chrome", "Library/Application Support/Google/Chrome", true),
            ("Chromium", "Library/Application Support/Chromium", false),
            ("Edge", "Library/Application Support/Microsoft Edge", false),
            (
                "Brave",
                "Library/Application Support/BraveSoftware/Brave-Browser",
                false,
            ),
            ("Arc", "Library/Application Support/Arc/User Data", false),
        ],
        "linux" => &[
            ("Chrome", ".config/google-chrome", true),
            ("Chromium", ".config/chromium", false),
            ("Edge", ".config/microsoft-edge", false),
            ("Brave", ".config/BraveSoftware/Brave-Browser", false),
        ],
        _ => &[],
    };
    rows.iter()
        .map(|(browser, data_dir, always)| BrowserTarget {
            browser,
            dir: home.join(data_dir).join("NativeMessagingHosts"),
            always: *always,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum RegistrationOutcome {
    Written,
    SkippedNotInstalled,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostRegistration {
    pub browser: &'static str,
    pub manifest_path: PathBuf,
    pub outcome: RegistrationOutcome,
}

/// Write the native host manifest for every installed browser, allowing the
/// store ID and all development IDs, pointing at `binary`.
///
/// Errors only when no IDs are known or Chrome's manifest cannot be written;
/// other browsers report failures in their row.
pub fn register_native_hosts(
    home: &Path,
    binary: &Path,
) -> Result<Vec<HostRegistration>, InstallError> {
    register_native_hosts_with_store_id(home, binary, WEB_STORE_EXTENSION_ID)
}

/// [`register_native_hosts`] for a given store ID. With `""` (a build with no
/// store item) and no development IDs it returns `NoExtensionIds`.
pub fn register_native_hosts_with_store_id(
    home: &Path,
    binary: &Path,
    store_id: &str,
) -> Result<Vec<HostRegistration>, InstallError> {
    let ids = merge_ids(store_id, ensure_ids_file(home)?);
    if ids.is_empty() {
        return Err(InstallError::NoExtensionIds);
    }

    let targets = manifest_targets(home);
    if targets.is_empty() {
        return Err(InstallError::UnsupportedOs);
    }

    let binary_str = binary.to_str().ok_or_else(|| InstallError::NonUtf8Path {
        path: binary.to_path_buf(),
    })?;
    let manifest = serde_json::json!({
        "name": NATIVE_HOST_NAME,
        "description": "AgentMark native messaging host",
        "path": binary_str,
        "type": "stdio",
        "allowed_origins": ids
            .iter()
            .map(|id| format!("chrome-extension://{id}/"))
            .collect::<Vec<_>>(),
    });
    let mut text = serde_json::to_string_pretty(&manifest).expect("JSON value serializes");
    text.push('\n');

    let mut results = Vec::new();
    for target in targets {
        let manifest_path = target.manifest_path();
        let installed = target.dir.parent().is_some_and(Path::is_dir);
        let outcome = if !target.always && !installed {
            RegistrationOutcome::SkippedNotInstalled
        } else {
            let written = std::fs::create_dir_all(&target.dir)
                .and_then(|()| write_file_atomic(&manifest_path, text.as_bytes()));
            match written {
                Ok(()) => {
                    info!(browser = target.browser, path = %manifest_path.display(), "native host registered");
                    RegistrationOutcome::Written
                }
                Err(source) if target.always => {
                    return Err(InstallError::Io {
                        path: manifest_path,
                        source,
                    })
                }
                Err(e) => {
                    warn!(
                        browser = target.browser,
                        "native host registration failed: {e}"
                    );
                    RegistrationOutcome::Failed(e.to_string())
                }
            }
        };
        results.push(HostRegistration {
            browser: target.browser,
            manifest_path,
            outcome,
        });
    }
    Ok(results)
}

// ── CLI copy and link ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CliInstallOutcome {
    Installed,
    Updated {
        from: String,
    },
    AlreadyCurrent,
    /// The installed CLI is newer than `source` and was kept.
    KeptNewer {
        installed: String,
    },
}

/// Copy `source` (the app's bundled CLI) to the stable path. Never downgrades:
/// a newer installed CLI (for example from `install.sh`) is kept.
///
/// The copy is staged beside the target, cleared of anything that would stop
/// it running (macOS quarantine, see [`clear_exec_blockers`]), probed for its
/// version, and only then renamed into place.
pub fn install_cli(home: &Path, source: &Path) -> Result<CliInstallOutcome, InstallError> {
    install_cli_with(home, source, clear_exec_blockers)
}

/// Removes a staged temp file unless it was renamed into place.
struct StagedFile(PathBuf);

impl Drop for StagedFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn install_cli_with(
    home: &Path,
    source: &Path,
    secure: fn(&Path) -> Vec<String>,
) -> Result<CliInstallOutcome, InstallError> {
    let target = stable_cli_path(home);
    let bin_dir = target.parent().expect("stable path has a parent");
    std::fs::create_dir_all(bin_dir).map_err(io_error(bin_dir))?;

    // Rename over the old file later: a host running from it keeps its open inode.
    let staged = StagedFile(bin_dir.join(format!("agentmark.tmp-{}", std::process::id())));
    std::fs::copy(source, &staged.0)
        .and_then(|_| set_mode_755(&staged.0))
        .map_err(|source| InstallError::Io {
            path: target.clone(),
            source,
        })?;
    // `std::fs::copy` keeps extended attributes, quarantine included, and a
    // quarantined binary is killed on exec. Problems are logged, not fatal:
    // the version probe below shows whether the copy runs.
    for problem in secure(&staged.0) {
        warn!(path = %target.display(), "could not prepare the CLI copy: {problem}");
    }
    let source_version = binary_version(&staged.0).map_err(|e| match e {
        InstallError::Version { reason, .. } => InstallError::Version {
            path: source.to_path_buf(),
            reason,
        },
        other => other,
    })?;

    let outcome = if !target.exists() {
        CliInstallOutcome::Installed
    } else {
        // An installed binary that cannot report a version is broken: replace it.
        let installed = binary_version(&target).ok();
        let from = installed
            .as_ref()
            .map(|v| v.text.clone())
            .unwrap_or_else(|| "unknown".to_string());
        match installed.map(|v| v.parts.cmp(&source_version.parts)) {
            Some(std::cmp::Ordering::Greater) => {
                return Ok(CliInstallOutcome::KeptNewer { installed: from })
            }
            // Compare with the prepared copy: signing changes the bytes.
            Some(std::cmp::Ordering::Equal) if file_sha256(&target)? == file_sha256(&staged.0)? => {
                return Ok(CliInstallOutcome::AlreadyCurrent)
            }
            _ => CliInstallOutcome::Updated { from },
        }
    };

    std::fs::rename(&staged.0, &target).map_err(io_error(&target))?;
    info!(path = %target.display(), version = %source_version.text, ?outcome, "CLI installed");
    Ok(outcome)
}

/// Make a copied binary runnable on macOS, as `install.sh`'s `secure_binary`
/// does: remove the quarantine and provenance attributes and ad-hoc sign it.
/// Returns what failed; an attribute that is not there is not a failure.
#[cfg(target_os = "macos")]
fn clear_exec_blockers(path: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let xattr = Path::new("/usr/bin/xattr");
    if xattr.is_file() {
        for attr in ["com.apple.quarantine", "com.apple.provenance"] {
            match Command::new(xattr).arg("-d").arg(attr).arg(path).output() {
                Ok(out) if out.status.success() => {}
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    if !stderr.contains("No such xattr") {
                        problems.push(format!("removing {attr} failed: {}", stderr.trim()));
                    }
                }
                Err(e) => problems.push(format!("removing {attr} failed: {e}")),
            }
        }
    } else {
        problems.push("xattr not found, quarantine not removed".to_string());
    }

    let codesign = Path::new("/usr/bin/codesign");
    if codesign.is_file() {
        match Command::new(codesign)
            // A fixed identifier (the default is the temp file's name) keeps
            // the signed bytes the same on every run, so an unchanged CLI
            // compares equal and is not reinstalled.
            .args(["--force", "--sign", "-", "--identifier", "agentmark"])
            .arg(path)
            .output()
        {
            Ok(out) if out.status.success() => {}
            Ok(out) => problems.push(format!(
                "ad-hoc signing failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )),
            Err(e) => problems.push(format!("ad-hoc signing failed: {e}")),
        }
    } else {
        problems.push("codesign not found, copy not signed".to_string());
    }
    problems
}

/// Other systems have no quarantine to clear.
#[cfg(not(target_os = "macos"))]
fn clear_exec_blockers(_path: &Path) -> Vec<String> {
    Vec::new()
}

#[cfg(unix)]
fn set_mode_755(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn set_mode_755(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
struct Version {
    text: String,
    parts: Vec<u64>,
}

/// Run `<binary> --version` (`agentmark 0.3.0`) and parse the last word.
fn binary_version(binary: &Path) -> Result<Version, InstallError> {
    let error = |reason: String| InstallError::Version {
        path: binary.to_path_buf(),
        reason,
    };
    let output = run_retrying_busy(|| Command::new(binary).arg("--version").output())
        .map_err(|e| error(e.to_string()))?;
    if !output.status.success() {
        return Err(error(format!("exited with {}", output.status)));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = stdout.split_whitespace().last().unwrap_or_default();
    parse_version(text).ok_or_else(|| error(format!("unexpected output {:?}", stdout.trim())))
}

/// A file just copied can briefly fail to exec with ETXTBSY while another
/// thread's fork still holds a write handle to it; retry a few times.
pub(super) fn run_retrying_busy<T>(
    mut run: impl FnMut() -> std::io::Result<T>,
) -> std::io::Result<T> {
    const ETXTBSY: i32 = 26; // same value on Linux and macOS
    let mut attempt: u64 = 0;
    loop {
        match run() {
            Err(e) if e.raw_os_error() == Some(ETXTBSY) && attempt < 5 => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(20 * attempt));
            }
            other => return other,
        }
    }
}

fn parse_version(text: &str) -> Option<Version> {
    let core = text.trim_start_matches('v').split(['-', '+']).next()?;
    let parts = core
        .split('.')
        .map(|p| p.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    Some(Version {
        text: text.to_string(),
        parts,
    })
}

fn file_sha256(path: &Path) -> Result<Vec<u8>, InstallError> {
    let bytes = std::fs::read(path).map_err(io_error(path))?;
    Ok(Sha256::digest(&bytes).to_vec())
}

#[derive(Debug, Clone, PartialEq)]
pub enum LinkOutcome {
    Linked,
    AlreadyLinked,
    /// A regular file is in the way; it is never replaced.
    BlockedByFile,
}

/// Make `local_bin/agentmark` a symlink to the stable CLI path.
pub fn link_cli(home: &Path, local_bin: &Path) -> Result<LinkOutcome, InstallError> {
    let target = stable_cli_path(home);
    let link = local_bin.join("agentmark");
    std::fs::create_dir_all(local_bin).map_err(io_error(local_bin))?;

    match std::fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if std::fs::read_link(&link).ok().as_deref() == Some(target.as_path()) {
                return Ok(LinkOutcome::AlreadyLinked);
            }
            std::fs::remove_file(&link).map_err(io_error(&link))?;
        }
        Ok(_) => return Ok(LinkOutcome::BlockedByFile),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_error(&link)(e)),
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &link).map_err(io_error(&link))?;
    #[cfg(not(unix))]
    return Err(InstallError::UnsupportedOs);

    debug!(link = %link.display(), target = %target.display(), "CLI linked");
    Ok(LinkOutcome::Linked)
}

// ── Skill ───────────────────────────────────────────────────────────

/// Install or refresh the agent skill (`~/.agentmark/skill/` plus symlinks).
pub fn install_skill(home: &Path) -> Result<SkillInstallResult, InstallError> {
    crate::skill::install_skill(home).map_err(|e| InstallError::Skill(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    const DEV_A: &str = "abcdefghijklmnopabcdefghijklmnop";
    const DEV_B: &str = "ponmlkjihgfedcbaponmlkjihgfedcba";

    fn read_json(path: &Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    // ── Targets ─────────────────────────────────────────────────────

    #[test]
    fn macos_targets() {
        let targets = targets_for_os(Path::new("/Users/me"), "macos");
        let browsers: Vec<_> = targets.iter().map(|t| t.browser).collect();
        assert_eq!(browsers, ["Chrome", "Chromium", "Edge", "Brave", "Arc"]);
        assert_eq!(
            targets[0].manifest_path(),
            PathBuf::from("/Users/me/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.agentmark.native.json")
        );
        assert_eq!(
            targets[4].dir,
            PathBuf::from(
                "/Users/me/Library/Application Support/Arc/User Data/NativeMessagingHosts"
            )
        );
        assert!(targets[0].always && targets[1..].iter().all(|t| !t.always));
    }

    #[test]
    fn linux_targets() {
        let targets = targets_for_os(Path::new("/home/me"), "linux");
        let dirs: Vec<_> = targets.iter().map(|t| t.dir.clone()).collect();
        assert_eq!(
            dirs,
            [
                "/home/me/.config/google-chrome/NativeMessagingHosts",
                "/home/me/.config/chromium/NativeMessagingHosts",
                "/home/me/.config/microsoft-edge/NativeMessagingHosts",
                "/home/me/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts",
            ]
            .map(PathBuf::from)
        );
        assert!(targets_for_os(Path::new("/x"), "windows").is_empty());
    }

    // ── Registration ────────────────────────────────────────────────

    #[test]
    fn no_ids_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        let err = register_native_hosts_with_store_id(tmp.path(), Path::new("/bin/agentmark"), "")
            .unwrap_err();
        assert!(matches!(err, InstallError::NoExtensionIds));
        assert!(!manifest_targets(tmp.path())[0].manifest_path().exists());
    }

    #[test]
    fn the_store_id_alone_registers_without_flags() {
        let tmp = TempDir::new().unwrap();
        let regs = register_native_hosts(tmp.path(), Path::new("/bin/agentmark")).unwrap();
        assert_eq!(regs[0].outcome, RegistrationOutcome::Written);
        assert_eq!(
            read_json(&regs[0].manifest_path)["allowed_origins"],
            serde_json::json!([format!("chrome-extension://{WEB_STORE_EXTENSION_ID}/")])
        );
    }

    #[test]
    fn writes_chrome_always_and_others_only_when_installed() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        super::super::add_development_extension_id(home, DEV_A).unwrap();
        let targets = manifest_targets(home);
        // Pretend the second browser is installed.
        std::fs::create_dir_all(targets[1].dir.parent().unwrap()).unwrap();

        let binary = stable_cli_path(home);
        let regs = register_native_hosts(home, &binary).unwrap();
        assert_eq!(regs.len(), targets.len());
        assert_eq!(regs[0].outcome, RegistrationOutcome::Written);
        assert_eq!(regs[1].outcome, RegistrationOutcome::Written);
        assert!(regs[2..]
            .iter()
            .all(|r| r.outcome == RegistrationOutcome::SkippedNotInstalled));
        assert!(!targets[2].dir.exists());

        let manifest = read_json(&regs[0].manifest_path);
        assert_eq!(manifest["name"], NATIVE_HOST_NAME);
        assert_eq!(manifest["type"], "stdio");
        assert_eq!(manifest["path"], binary.to_str().unwrap());
        assert_eq!(
            manifest["allowed_origins"],
            serde_json::json!([
                format!("chrome-extension://{WEB_STORE_EXTENSION_ID}/"),
                format!("chrome-extension://{DEV_A}/"),
            ])
        );
        assert_eq!(read_json(&regs[1].manifest_path), manifest);
    }

    #[test]
    fn registration_keeps_every_development_id() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        super::super::add_development_extension_id(home, DEV_A).unwrap();
        register_native_hosts(home, Path::new("/bin/agentmark")).unwrap();
        super::super::add_development_extension_id(home, DEV_B).unwrap();
        let regs = register_native_hosts(home, Path::new("/bin/agentmark")).unwrap();
        let origins = read_json(&regs[0].manifest_path)["allowed_origins"].clone();
        let origins: Vec<String> = serde_json::from_value(origins).unwrap();
        assert!(origins.ends_with(&[
            format!("chrome-extension://{DEV_A}/"),
            format!("chrome-extension://{DEV_B}/"),
        ]));
    }

    #[test]
    fn registration_migrates_ids_from_an_old_manifest() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let chrome = manifest_targets(home).remove(0);
        std::fs::create_dir_all(&chrome.dir).unwrap();
        std::fs::write(
            chrome.manifest_path(),
            serde_json::json!({
                "name": NATIVE_HOST_NAME,
                "path": "/old/agentmark",
                "allowed_origins": [format!("chrome-extension://{DEV_B}/")]
            })
            .to_string(),
        )
        .unwrap();

        register_native_hosts(home, Path::new("/new/agentmark")).unwrap();
        let manifest = read_json(&chrome.manifest_path());
        assert_eq!(manifest["path"], "/new/agentmark");
        assert!(manifest["allowed_origins"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(format!("chrome-extension://{DEV_B}/"))));
        assert!(config::config_dir(home).join("native-host.json").is_file());
    }

    #[test]
    fn a_failing_optional_browser_is_reported_not_fatal() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        super::super::add_development_extension_id(home, DEV_A).unwrap();
        let targets = manifest_targets(home);
        // A file where the NativeMessagingHosts directory should go.
        std::fs::create_dir_all(targets[1].dir.parent().unwrap()).unwrap();
        std::fs::write(&targets[1].dir, "blocker").unwrap();

        let regs = register_native_hosts(home, Path::new("/bin/agentmark")).unwrap();
        assert_eq!(regs[0].outcome, RegistrationOutcome::Written);
        assert!(matches!(regs[1].outcome, RegistrationOutcome::Failed(_)));
    }

    #[test]
    fn a_failing_chrome_write_is_an_error() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        super::super::add_development_extension_id(home, DEV_A).unwrap();
        let chrome = manifest_targets(home).remove(0);
        std::fs::create_dir_all(chrome.dir.parent().unwrap()).unwrap();
        std::fs::write(&chrome.dir, "blocker").unwrap();
        let err = register_native_hosts(home, Path::new("/bin/agentmark")).unwrap_err();
        assert!(matches!(err, InstallError::Io { .. }), "{err:?}");
    }

    // ── Registered path ─────────────────────────────────────────────

    #[test]
    fn registered_path_prefers_the_stable_copy() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let exe = tmp.path().join("target/debug/agentmark");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, "").unwrap();
        assert_eq!(
            registered_binary_path(home, &exe),
            std::fs::canonicalize(&exe).unwrap()
        );

        let stable = stable_cli_path(home);
        std::fs::create_dir_all(stable.parent().unwrap()).unwrap();
        std::fs::write(&stable, "").unwrap();
        assert_eq!(registered_binary_path(home, &exe), stable);
        assert_eq!(stable, home.join(".agentmark/bin/agentmark"));
    }

    #[test]
    fn local_bin_override() {
        let home = Path::new("/home/me");
        assert_eq!(
            local_bin_from(None, home),
            PathBuf::from("/home/me/.local/bin")
        );
        assert_eq!(
            local_bin_from(Some("".into()), home),
            PathBuf::from("/home/me/.local/bin")
        );
        assert_eq!(
            local_bin_from(Some("/opt/bin".into()), home),
            PathBuf::from("/opt/bin")
        );
    }

    // ── install_cli ─────────────────────────────────────────────────

    /// A fake CLI that prints `agentmark <version>`; `tag` varies the bytes.
    fn fake_cli(dir: &Path, name: &str, version: &str, tag: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(
            &path,
            format!("#!/bin/sh\n# {tag}\necho \"agentmark {version}\"\n"),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn install_existing(home: &Path, version: &str, tag: &str) {
        let stable = stable_cli_path(home);
        fake_cli(stable.parent().unwrap(), "agentmark", version, tag);
    }

    #[test]
    fn install_cli_fresh_copy_is_executable() {
        let tmp = TempDir::new().unwrap();
        let source = fake_cli(&tmp.path().join("app"), "agentmark", "0.3.0", "a");
        let home = tmp.path().join("home");
        assert_eq!(
            install_cli(&home, &source).unwrap(),
            CliInstallOutcome::Installed
        );
        let stable = stable_cli_path(&home);
        let mode = std::fs::metadata(&stable).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
        assert_eq!(
            std::fs::read(&stable).unwrap(),
            std::fs::read(&source).unwrap()
        );
        let leftovers: Vec<_> = std::fs::read_dir(stable.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, vec![std::ffi::OsString::from("agentmark")]);
    }

    #[test]
    fn install_cli_upgrades_an_older_copy() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        install_existing(&home, "0.2.9", "old");
        let source = fake_cli(&tmp.path().join("app"), "agentmark", "0.3.0", "new");
        assert_eq!(
            install_cli(&home, &source).unwrap(),
            CliInstallOutcome::Updated {
                from: "0.2.9".to_string()
            }
        );
        assert_eq!(
            binary_version(&stable_cli_path(&home)).unwrap().text,
            "0.3.0"
        );
    }

    #[test]
    fn install_cli_never_downgrades() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        install_existing(&home, "0.10.0", "newer");
        let source = fake_cli(&tmp.path().join("app"), "agentmark", "0.9.0", "older");
        assert_eq!(
            install_cli(&home, &source).unwrap(),
            CliInstallOutcome::KeptNewer {
                installed: "0.10.0".to_string()
            }
        );
        assert_eq!(
            binary_version(&stable_cli_path(&home)).unwrap().text,
            "0.10.0"
        );
    }

    #[test]
    fn install_cli_same_version_compares_contents() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let source = fake_cli(&tmp.path().join("app"), "agentmark", "0.3.0", "build-1");
        install_cli(&home, &source).unwrap();
        assert_eq!(
            install_cli(&home, &source).unwrap(),
            CliInstallOutcome::AlreadyCurrent
        );

        let rebuilt = fake_cli(&tmp.path().join("app2"), "agentmark", "0.3.0", "build-2");
        assert_eq!(
            install_cli(&home, &rebuilt).unwrap(),
            CliInstallOutcome::Updated {
                from: "0.3.0".to_string()
            }
        );
    }

    #[test]
    fn install_cli_replaces_a_broken_copy() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let stable = stable_cli_path(&home);
        std::fs::create_dir_all(stable.parent().unwrap()).unwrap();
        std::fs::write(&stable, "garbage").unwrap();
        let source = fake_cli(&tmp.path().join("app"), "agentmark", "0.3.0", "a");
        assert_eq!(
            install_cli(&home, &source).unwrap(),
            CliInstallOutcome::Updated {
                from: "unknown".to_string()
            }
        );
    }

    #[test]
    fn install_cli_rejects_a_source_without_a_version() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("not-a-cli");
        std::fs::write(&source, "").unwrap();
        let err = install_cli(&tmp.path().join("home"), &source).unwrap_err();
        assert!(matches!(err, InstallError::Version { .. }), "{err:?}");
    }

    /// The version probe runs on the prepared copy, not on the source, so a
    /// quarantined source that cannot run itself still installs.
    #[test]
    fn install_cli_probes_the_prepared_copy() {
        fn rewrite_as_0_4_0(path: &Path) -> Vec<String> {
            std::fs::write(path, "#!/bin/sh\necho \"agentmark 0.4.0\"\n").unwrap();
            Vec::new()
        }
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        install_existing(&home, "0.3.5", "installed");
        let source = fake_cli(&tmp.path().join("app"), "agentmark", "0.3.0", "a");
        assert_eq!(
            install_cli_with(&home, &source, rewrite_as_0_4_0).unwrap(),
            CliInstallOutcome::Updated {
                from: "0.3.5".to_string()
            }
        );
    }

    #[test]
    fn install_cli_reports_but_survives_preparation_failures() {
        fn fails(_path: &Path) -> Vec<String> {
            vec!["codesign not found, copy not signed".to_string()]
        }
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let source = fake_cli(&tmp.path().join("app"), "agentmark", "0.3.0", "a");
        assert_eq!(
            install_cli_with(&home, &source, fails).unwrap(),
            CliInstallOutcome::Installed
        );
        assert!(stable_cli_path(&home).is_file());
    }

    #[test]
    fn install_cli_leaves_no_temp_file_when_kept() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        install_existing(&home, "0.10.0", "newer");
        let source = fake_cli(&tmp.path().join("app"), "agentmark", "0.9.0", "older");
        install_cli(&home, &source).unwrap();
        let names: Vec<_> = std::fs::read_dir(stable_cli_path(&home).parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("agentmark")]);
    }

    /// `std::fs::copy` keeps `com.apple.quarantine`, and a quarantined binary
    /// is killed on exec, so the installed copy must not carry it.
    #[cfg(target_os = "macos")]
    #[test]
    fn install_cli_strips_quarantine_from_the_copy() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let source = fake_cli(&tmp.path().join("app"), "agentmark", "0.3.0", "q");
        let set = Command::new("/usr/bin/xattr")
            .args([
                "-w",
                "com.apple.quarantine",
                "0081;00000000;AgentMark test;",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(set.success());

        install_cli(&home, &source).unwrap();
        let installed = stable_cli_path(&home);
        let attr = Command::new("/usr/bin/xattr")
            .args(["-p", "com.apple.quarantine"])
            .arg(&installed)
            .output()
            .unwrap();
        assert!(
            !attr.status.success(),
            "quarantine kept: {}",
            String::from_utf8_lossy(&attr.stdout)
        );
    }

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("0.3.0").unwrap().parts, vec![0, 3, 0]);
        assert_eq!(parse_version("v1.2.3-beta.1").unwrap().parts, vec![1, 2, 3]);
        assert!(parse_version("abc").is_none());
        assert!(parse_version("0.10.0").unwrap().parts > parse_version("0.9.9").unwrap().parts);
    }

    // ── link_cli ────────────────────────────────────────────────────

    #[test]
    fn link_cli_creates_replaces_and_respects_files() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let local_bin = tmp.path().join("bin");
        let link = local_bin.join("agentmark");

        assert_eq!(link_cli(&home, &local_bin).unwrap(), LinkOutcome::Linked);
        assert_eq!(std::fs::read_link(&link).unwrap(), stable_cli_path(&home));
        assert_eq!(
            link_cli(&home, &local_bin).unwrap(),
            LinkOutcome::AlreadyLinked
        );

        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink("/somewhere/else", &link).unwrap();
        assert_eq!(link_cli(&home, &local_bin).unwrap(), LinkOutcome::Linked);
        assert_eq!(std::fs::read_link(&link).unwrap(), stable_cli_path(&home));

        std::fs::remove_file(&link).unwrap();
        std::fs::write(&link, "user's own script").unwrap();
        assert_eq!(
            link_cli(&home, &local_bin).unwrap(),
            LinkOutcome::BlockedByFile
        );
        assert_eq!(std::fs::read_to_string(&link).unwrap(), "user's own script");
    }

    // ── Skill ───────────────────────────────────────────────────────

    #[test]
    fn install_skill_wraps_the_skill_installer() {
        let tmp = TempDir::new().unwrap();
        let result = install_skill(tmp.path()).unwrap();
        assert!(result.canonical_dir.join("SKILL.md").is_file());
    }
}
