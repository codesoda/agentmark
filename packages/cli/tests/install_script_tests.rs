//! Tests for the root install.sh script.
//!
//! Uses a temp repo fixture with fake cargo and npm shims to avoid running
//! real builds. Tests assert filesystem outcomes, idempotency,
//! flag behavior, and failure paths. The fake `agentmark` binary logs its
//! arguments and can be told to report "no extension IDs" or to fail.
//!
//! Extension installation and native host manifest tests are now in
//! `src/commands/install_extension.rs` since that logic moved to the CLI.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// Path to the root install.sh in the repo.
fn install_script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("install.sh")
}

/// Create a temp repo fixture with install.sh and fake skill files.
struct TestFixture {
    tmp: TempDir,
}

impl TestFixture {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Copy install.sh
        fs::copy(install_script_path(), root.join("install.sh")).unwrap();

        // Create a minimal Cargo.toml so source resolution succeeds
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"packages/cli\"]\n",
        )
        .unwrap();

        // Create packages/cli and packages/extension directories
        fs::create_dir_all(root.join("packages/cli")).unwrap();
        fs::create_dir_all(root.join("packages/extension")).unwrap();

        TestFixture { tmp }
    }

    fn root(&self) -> &Path {
        self.tmp.path()
    }

    fn home(&self) -> PathBuf {
        self.tmp.path().join("home")
    }

    /// Create the fake tool shims directory and return its path.
    fn create_shims(&self) -> PathBuf {
        let shims_dir = self.tmp.path().join("shims");
        fs::create_dir_all(&shims_dir).unwrap();

        write_cargo_shim(&shims_dir);
        write_npm_shim(&shims_dir);
        shims_dir
    }

    fn build_log(&self) -> PathBuf {
        self.tmp.path().join("build.log")
    }

    fn build_log_contents(&self) -> String {
        fs::read_to_string(self.build_log()).unwrap_or_default()
    }

    /// Run the installer with the given args, using fake shims and temp HOME.
    fn run_installer(&self, args: &[&str]) -> InstallerResult {
        self.run_installer_with_env(args, &[])
    }

    /// Like `run_installer`, with extra environment variables.
    fn run_installer_with_env(&self, args: &[&str], env: &[(&str, &str)]) -> InstallerResult {
        let home = self.home();
        fs::create_dir_all(&home).unwrap();

        let shims = self.create_shims();
        let init_log = self.tmp.path().join("init.log");

        // Create agent skill roots so skill installer can symlink
        let claude_skills = home.join(".claude/skills");
        let codex_skills = home.join(".codex/skills");
        fs::create_dir_all(&claude_skills).unwrap();
        fs::create_dir_all(&codex_skills).unwrap();

        let mut cmd = Command::new("sh");
        cmd.arg(self.root().join("install.sh"));
        for arg in args {
            cmd.arg(arg);
        }
        cmd.env("HOME", &home);
        cmd.env("PATH", format!("{}:/usr/bin:/bin", shims.display()));
        cmd.env("AGENTMARK_INIT_LOG", &init_log);
        cmd.env("AGENTMARK_BUILD_LOG", self.build_log());
        cmd.env("NO_COLOR", "1");
        for (key, value) in env {
            cmd.env(key, value);
        }
        // Use env overrides for skill dirs so they use the temp HOME
        cmd.env("AGENTMARK_SHARED_SKILLS_DIR", home.join(".agents/skills"));
        cmd.env("CLAUDE_SKILLS_DIR", &claude_skills);
        cmd.env("CODEX_SKILLS_DIR", &codex_skills);
        cmd.current_dir(self.root());

        let output = cmd.output().expect("installer should execute");

        InstallerResult {
            output,
            home,
            init_log,
        }
    }
}

struct InstallerResult {
    output: std::process::Output,
    home: PathBuf,
    init_log: PathBuf,
}

impl InstallerResult {
    fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).to_string()
    }

    fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).to_string()
    }

    fn success(&self) -> bool {
        self.output.status.success()
    }

    fn installed_binary(&self) -> PathBuf {
        self.home.join(".agentmark/bin/agentmark")
    }

    fn symlink_path(&self) -> PathBuf {
        self.home.join(".local/bin/agentmark")
    }

    fn init_log_contents(&self) -> String {
        fs::read_to_string(&self.init_log).unwrap_or_default()
    }
}

/// The fake `agentmark` binary the cargo shim "builds". It logs every call to
/// AGENTMARK_INIT_LOG; FAKE_NO_IDS=1 makes a bare `install-extension` report
/// that no extension ID is known, FAKE_EXT_FAIL=1 makes it fail.
const FAKE_BINARY: &str = r#"#!/bin/sh
echo "agentmark-fake $*" >> "${AGENTMARK_INIT_LOG:-/dev/null}"
if [ "$1" = "install-extension" ]; then
    if [ "${FAKE_EXT_FAIL:-0}" = 1 ]; then
        echo "error: cannot write manifest" >&2
        exit 1
    fi
    if [ "${FAKE_NO_IDS:-0}" = 1 ] && [ "$#" = 1 ]; then
        echo "No extension IDs are known yet, so the native host was not registered."
        exit 0
    fi
    echo "Native host registered for Chrome at /fake/manifest.json"
fi
"#;

/// Fake cargo: logs to AGENTMARK_BUILD_LOG and writes the fake binary to
/// target/release/agentmark under the source root.
fn write_cargo_shim(shims_dir: &Path) {
    let cargo_shim = shims_dir.join("cargo");
    fs::write(
        &cargo_shim,
        format!(
            r#"#!/bin/sh
echo "cargo $*" >> "${{AGENTMARK_BUILD_LOG:-/dev/null}}"
# Find the source root by walking up from CWD
src_root="$(pwd)"
while [ ! -f "$src_root/Cargo.toml" ] && [ "$src_root" != "/" ]; do
    src_root="$(dirname "$src_root")"
done
mkdir -p "$src_root/target/release"
cat > "$src_root/target/release/agentmark" <<'FAKE_BIN'
{FAKE_BINARY}FAKE_BIN
chmod +x "$src_root/target/release/agentmark"
echo "Fake cargo build complete"
"#
        ),
    )
    .unwrap();
    set_executable(&cargo_shim);
}

/// Fake npm: logs the command and the directory it ran in.
fn write_npm_shim(shims_dir: &Path) {
    let npm_shim = shims_dir.join("npm");
    fs::write(
        &npm_shim,
        r#"#!/bin/sh
echo "npm $* in $(basename "$(dirname "$(pwd)")")/$(basename "$(pwd)")" >> "${AGENTMARK_BUILD_LOG:-/dev/null}"
"#,
    )
    .unwrap();
    set_executable(&npm_shim);
}

fn set_executable(path: &Path) {
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).unwrap();
}

// --- Shell syntax validation ---

#[test]
fn install_script_has_valid_shell_syntax() {
    let script = install_script_path();

    // Check with bash -n (catches general syntax errors)
    let output = Command::new("bash")
        .arg("-n")
        .arg(&script)
        .output()
        .expect("bash should be available");
    assert!(
        output.status.success(),
        "install.sh has syntax errors (bash -n):\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Also check with sh -n to match the #!/bin/sh shebang.
    // On Ubuntu CI, /bin/sh is dash which is stricter than bash —
    // this catches accidental bashisms that bash -n would accept.
    let output = Command::new("sh")
        .arg("-n")
        .arg(&script)
        .output()
        .expect("sh should be available");
    assert!(
        output.status.success(),
        "install.sh has syntax errors (sh -n):\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn install_script_passes_shellcheck() {
    // shellcheck may not be available in all environments
    if Command::new("shellcheck")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("shellcheck not available — skipping");
        return;
    }

    let script = install_script_path();
    let output = Command::new("shellcheck")
        .arg("--shell=sh")
        .arg(&script)
        .output()
        .expect("shellcheck should execute");
    assert!(
        output.status.success(),
        "install.sh has shellcheck issues:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// --- Success path tests ---

#[test]
fn installs_binary_to_agentmark_bin() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());
    assert!(
        result.installed_binary().exists(),
        "Binary should be installed at {:?}",
        result.installed_binary()
    );
    assert!(
        result.installed_binary().is_file(),
        "Installed binary should be a file"
    );
}

#[test]
fn creates_local_bin_symlink() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());

    let symlink = result.symlink_path();
    assert!(symlink.exists(), "Symlink should exist at {:?}", symlink);
    assert!(
        symlink.is_symlink(),
        "~/.local/bin/agentmark should be a symlink"
    );

    let target = fs::read_link(&symlink).unwrap();
    assert_eq!(
        target,
        result.installed_binary(),
        "Symlink should point to installed binary"
    );
}

#[test]
fn delegates_extension_install_to_cli() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());

    // The fake binary should have been invoked with install-extension
    let log = result.init_log_contents();
    assert!(
        log.contains("install-extension"),
        "CLI should be called with install-extension, got log: {log}"
    );
}

#[test]
fn forwards_extension_id_to_cli() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&[
        "--skip-init",
        "--extension-id",
        "abcdefghijklmnopabcdefghijklmnop",
    ]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());

    let log = result.init_log_contents();
    assert!(
        log.contains("--extension-id abcdefghijklmnopabcdefghijklmnop"),
        "Extension ID should be forwarded to CLI, got log: {log}"
    );
}

#[test]
fn extension_id_via_env_var() {
    let fixture = TestFixture::new();
    let home = fixture.home();
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(home.join(".claude/skills")).unwrap();
    fs::create_dir_all(home.join(".codex/skills")).unwrap();

    let shims = fixture.create_shims();
    let init_log = fixture.tmp.path().join("init.log");

    let mut cmd = Command::new("sh");
    cmd.arg(fixture.root().join("install.sh"));
    cmd.arg("--skip-init");
    cmd.env("HOME", &home);
    cmd.env("PATH", format!("{}:/usr/bin:/bin", shims.display()));
    cmd.env("AGENTMARK_EXTENSION_ID", "mmmmnnnnooooppppqqqqrrrrsssstttt");
    cmd.env("AGENTMARK_INIT_LOG", &init_log);
    cmd.env("NO_COLOR", "1");
    cmd.env("AGENTMARK_SHARED_SKILLS_DIR", home.join(".agents/skills"));
    cmd.env("CLAUDE_SKILLS_DIR", home.join(".claude/skills"));
    cmd.env("CODEX_SKILLS_DIR", home.join(".codex/skills"));
    cmd.current_dir(fixture.root());

    let output = cmd.output().expect("installer should execute");
    assert!(output.status.success(), "Installer should succeed");

    let log = fs::read_to_string(&init_log).unwrap_or_default();
    assert!(
        log.contains("--extension-id mmmmnnnnooooppppqqqqrrrrsssstttt"),
        "Env var extension ID should be forwarded to CLI, got log: {log}"
    );
}

#[test]
fn delegates_skill_installation_to_cli() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());

    // The fake binary should have been invoked with add-skill
    let log = result.init_log_contents();
    assert!(
        log.contains("add-skill"),
        "CLI should be called with add-skill, got log: {log}"
    );
}

#[test]
fn never_runs_init_or_prompts() {
    // Setup happens in Chrome or the app, never in the terminal.
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&[]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());

    let has_bare_init = result.init_log_contents().lines().any(|line| {
        let args = line.trim_start_matches("agentmark-fake ");
        args == "init" || args.starts_with("init ")
    });
    assert!(!has_bare_init, "init must never run");
    assert!(!result.stdout().contains("Enter the extension ID"));
}

#[test]
fn ends_by_sending_the_user_back_to_chrome() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&[]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());
    let stdout = result.stdout();
    assert!(
        stdout.contains("Done. Go back to Chrome: AgentMark finishes setting up there."),
        "got:\n{stdout}"
    );
    assert!(!stdout.contains("Development: load the extension unpacked"));
    assert!(
        !stdout.contains("No AgentMark extension in Chrome yet?"),
        "no hint without a development copy"
    );
}

#[test]
fn points_to_the_development_copy_when_one_was_extracted() {
    // The real install-extension extracts the copy; it carries the store key,
    // so the hint asks for no extension ID.
    let fixture = TestFixture::new();
    let ext_dir = fixture.home().join(".agentmark/extension");
    fs::create_dir_all(&ext_dir).unwrap();
    fs::write(ext_dir.join("manifest.json"), "{}").unwrap();
    let result = fixture.run_installer(&[]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());
    let stdout = result.stdout();
    assert!(stdout.contains("Done. Go back to Chrome"), "{stdout}");
    assert!(
        stdout.contains("No AgentMark extension in Chrome yet?"),
        "{stdout}"
    );
    assert!(stdout.contains(&ext_dir.display().to_string()), "{stdout}");
    assert!(!stdout.contains("YOUR_EXTENSION_ID"));
}

#[test]
fn shows_development_steps_when_no_extension_id_is_known() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer_with_env(&[], &[("FAKE_NO_IDS", "1")]);

    assert!(
        result.success(),
        "no ID yet is not a failure:\n{}",
        result.stdout()
    );
    let stdout = result.stdout();
    assert!(
        stdout.contains("Development: load the extension unpacked"),
        "{stdout}"
    );
    assert!(stdout.contains("Load unpacked"));
    assert!(
        stdout.contains("Then run: agentmark install-extension --extension-id YOUR_EXTENSION_ID")
    );
    assert!(!stdout.contains("Done. Go back to Chrome"));
}

#[test]
fn extension_id_skips_development_steps() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer_with_env(
        &["--extension-id", "abcdefghijklmnopabcdefghijklmnop"],
        &[("FAKE_NO_IDS", "1")],
    );

    assert!(result.success(), "Installer failed:\n{}", result.stdout());
    assert!(!result
        .stdout()
        .contains("Development: load the extension unpacked"));
    assert!(result.stdout().contains("Done. Go back to Chrome"));
}

#[test]
fn failed_registration_exits_one() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer_with_env(&[], &[("FAKE_EXT_FAIL", "1")]);

    assert_eq!(result.output.status.code(), Some(1), "{}", result.stdout());
    assert!(
        result.stderr().contains("registration failed"),
        "{}",
        result.stderr()
    );
    // The binary and the skill are still installed.
    assert!(result.installed_binary().exists());
    assert!(result.init_log_contents().contains("add-skill"));
}

#[test]
fn builds_extension_before_cli_so_it_is_embedded() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--from-source"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());
    let log = fixture.build_log_contents();
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(
        lines,
        vec![
            "npm ci in packages/extension",
            "npm run build in packages/extension",
            "cargo build --release -p agentmark",
        ],
        "build order"
    );
}

#[test]
fn missing_npm_fails() {
    if Path::new("/usr/bin/npm").exists() || Path::new("/bin/npm").exists() {
        eprintln!("npm is on the base PATH — skipping");
        return;
    }
    let fixture = TestFixture::new();
    let home = fixture.home();
    fs::create_dir_all(&home).unwrap();

    // Shims with cargo only.
    let shims = fixture.tmp.path().join("cargo_only");
    fs::create_dir_all(&shims).unwrap();
    write_cargo_shim(&shims);

    let output = Command::new("sh")
        .arg(fixture.root().join("install.sh"))
        .env("HOME", &home)
        .env("PATH", format!("{}:/usr/bin:/bin", shims.display()))
        .env("NO_COLOR", "1")
        .current_dir(fixture.root())
        .output()
        .expect("installer should execute");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("npm is required"),
        "Error should mention npm, got: {stderr}"
    );
}

#[test]
fn skip_init_is_accepted_silently() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());
    assert!(
        !result.stdout().to_lowercase().contains("init"),
        "{}",
        result.stdout()
    );
}

#[test]
fn skip_init_suppresses_init() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());

    // init should not appear in the log (install-extension will, but not init alone)
    let log = result.init_log_contents();
    let has_bare_init = log.lines().any(|line| {
        let args = line.trim_start_matches("agentmark-fake ");
        args == "init" || args.starts_with("init ")
    });
    assert!(
        !has_bare_init,
        "Init should NOT have been called with --skip-init, got log: {log}"
    );
}

#[test]
fn idempotent_rerun() {
    let fixture = TestFixture::new();

    let result1 = fixture.run_installer(&["--skip-init"]);
    assert!(result1.success(), "First run failed:\n{}", result1.stdout());

    let result2 = fixture.run_installer(&["--skip-init"]);
    assert!(
        result2.success(),
        "Second run failed:\n{}",
        result2.stdout()
    );

    // Binary still exists
    assert!(result2.installed_binary().exists());
    // Symlink still valid
    assert!(result2.symlink_path().is_symlink());
    assert_eq!(
        fs::read_link(result2.symlink_path()).unwrap(),
        result2.installed_binary()
    );
}

/// A re-install renames a new file over the binary instead of writing into
/// it, so a native host Chrome is running from the old file is left alone.
#[test]
fn reinstall_replaces_the_binary_by_rename() {
    use std::os::unix::fs::MetadataExt;
    let fixture = TestFixture::new();
    let first = fixture.run_installer(&[]);
    assert!(first.success(), "{}", first.stderr());
    let inode_before = fs::metadata(first.installed_binary()).unwrap().ino();

    let second = fixture.run_installer(&[]);
    assert!(second.success(), "{}", second.stderr());
    let installed = second.installed_binary();
    assert_ne!(fs::metadata(&installed).unwrap().ino(), inode_before);
    let mode = fs::metadata(&installed).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755);

    let names: Vec<String> = fs::read_dir(installed.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec!["agentmark".to_string()], "no temp file left");
}

/// The CLI and the app always use `$HOME/.agentmark`, so the installer does
/// too: `AGENTMARK_HOME` is not an override any more.
#[test]
fn agentmark_home_is_ignored() {
    let fixture = TestFixture::new();
    let elsewhere = fixture.root().join("elsewhere");
    let result = fixture.run_installer_with_env(
        &[],
        &[
            ("AGENTMARK_HOME", elsewhere.to_str().unwrap()),
            ("FAKE_NO_IDS", "1"),
        ],
    );
    assert!(result.success(), "{}", result.stderr());
    assert!(result.installed_binary().is_file());
    assert!(!elsewhere.exists());
    assert!(
        result.stdout().contains(&format!(
            "{}",
            result.home.join(".agentmark/extension").display()
        )),
        "{}",
        result.stdout()
    );
    assert!(!result.stdout().contains("AGENTMARK_HOME"));
}

/// Put a fake `id` that reports root on the installer's PATH.
fn pretend_to_be_root(fixture: &TestFixture) {
    let shims = fixture.root().join("shims");
    fs::create_dir_all(&shims).unwrap();
    let id = shims.join("id");
    fs::write(&id, "#!/bin/sh\necho 0\n").unwrap();
    set_executable(&id);
}

#[test]
fn refuses_to_run_as_root() {
    let fixture = TestFixture::new();
    pretend_to_be_root(&fixture);
    let result = fixture.run_installer(&[]);
    assert_eq!(result.output.status.code(), Some(1));
    assert!(result.stderr().contains("Do not run the installer as root"));
    assert!(result.stderr().contains("AGENTMARK_ALLOW_ROOT=1"));
    assert!(!result.installed_binary().exists());
}

#[test]
fn root_is_allowed_with_the_override() {
    let fixture = TestFixture::new();
    pretend_to_be_root(&fixture);
    let result = fixture.run_installer_with_env(&[], &[("AGENTMARK_ALLOW_ROOT", "1")]);
    assert!(result.success(), "{}", result.stderr());
    assert!(result.installed_binary().is_file());
}

// --- Failure path tests ---

#[test]
fn missing_cargo_fails() {
    let fixture = TestFixture::new();
    let home = fixture.home();
    fs::create_dir_all(&home).unwrap();

    // Create a shims dir with only basic tools, no cargo
    let shims = fixture.tmp.path().join("empty_shims");
    fs::create_dir_all(&shims).unwrap();

    let mut cmd = Command::new("/bin/sh");
    cmd.arg(fixture.root().join("install.sh"));
    cmd.arg("--skip-init");
    cmd.env("HOME", &home);
    cmd.env("PATH", format!("{}:/usr/bin:/bin", shims.display()));
    cmd.env("NO_COLOR", "1");
    cmd.current_dir(fixture.root());

    let output = cmd.output().expect("installer should execute");
    assert!(
        !output.status.success(),
        "Installer should fail without cargo"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cargo is required"),
        "Error should mention cargo, got: {stderr}"
    );
}

#[test]
fn existing_regular_file_at_symlink_path_warns() {
    let fixture = TestFixture::new();
    let home = fixture.home();
    fs::create_dir_all(&home).unwrap();

    // Create a regular file at the symlink path
    let local_bin = home.join(".local/bin");
    fs::create_dir_all(&local_bin).unwrap();
    fs::write(local_bin.join("agentmark"), "not a symlink").unwrap();

    let result = fixture.run_installer(&["--skip-init"]);

    assert!(result.success(), "Installer should still succeed");

    let stderr = result.stderr();
    assert!(
        stderr.contains("not a symlink"),
        "Should warn about existing regular file, got: {stderr}"
    );
}

#[test]
fn path_warning_when_local_bin_not_on_path() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init"]);

    assert!(result.success(), "Installer should succeed");

    let stderr = result.stderr();
    assert!(
        stderr.contains("not on your PATH"),
        "Should warn about .local/bin not on PATH, got: {stderr}"
    );
}

#[test]
fn help_flag_shows_usage() {
    let fixture = TestFixture::new();

    let mut cmd = Command::new("sh");
    cmd.arg(fixture.root().join("install.sh"));
    cmd.arg("--help");
    cmd.env("HOME", fixture.home());
    cmd.env("NO_COLOR", "1");

    let output = cmd.output().expect("installer should execute");
    assert!(output.status.success(), "Help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("AgentMark Installer"),
        "Help should show title"
    );
    assert!(
        stdout.contains("--skip-init"),
        "Help should document --skip-init"
    );
    assert!(
        stdout.contains("--extension-id"),
        "Help should document --extension-id"
    );
}

#[test]
fn unknown_option_fails() {
    let fixture = TestFixture::new();

    let mut cmd = Command::new("sh");
    cmd.arg(fixture.root().join("install.sh"));
    cmd.arg("--bogus");
    cmd.env("HOME", fixture.home());
    cmd.env("NO_COLOR", "1");

    let output = cmd.output().expect("installer should execute");
    assert!(
        !output.status.success(),
        "Unknown option should cause failure"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown option"),
        "Error should mention unknown option, got: {stderr}"
    );
}

#[test]
fn home_with_spaces() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("repo source");
    fs::create_dir_all(&root).unwrap();

    // Copy install.sh
    fs::copy(install_script_path(), root.join("install.sh")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"packages/cli\"]\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("packages/cli")).unwrap();
    fs::create_dir_all(root.join("packages/extension")).unwrap();

    // HOME with spaces
    let home = tmp.path().join("home dir with spaces");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(home.join(".claude/skills")).unwrap();
    fs::create_dir_all(home.join(".codex/skills")).unwrap();

    // Create shims
    let shims_dir = tmp.path().join("shims");
    fs::create_dir_all(&shims_dir).unwrap();

    write_cargo_shim(&shims_dir);
    write_npm_shim(&shims_dir);

    let init_log = tmp.path().join("init.log");

    let mut cmd = Command::new("sh");
    cmd.arg(root.join("install.sh"));
    cmd.arg("--skip-init");
    cmd.env("HOME", &home);
    cmd.env("PATH", format!("{}:/usr/bin:/bin", shims_dir.display()));
    cmd.env("AGENTMARK_INIT_LOG", &init_log);
    cmd.env("NO_COLOR", "1");
    cmd.env("AGENTMARK_SHARED_SKILLS_DIR", home.join(".agents/skills"));
    cmd.env("CLAUDE_SKILLS_DIR", home.join(".claude/skills"));
    cmd.env("CODEX_SKILLS_DIR", home.join(".codex/skills"));
    cmd.current_dir(&root);

    let output = cmd.output().expect("installer should execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Installer should handle HOME with spaces.\nstdout: {stdout}\nstderr: {stderr}"
    );

    assert!(
        home.join(".agentmark/bin/agentmark").exists(),
        "Binary should be installed even with spaces in HOME"
    );
}
