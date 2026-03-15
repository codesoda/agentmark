//! Tests for the root install.sh script.
//!
//! Uses a temp repo fixture with fake cargo shims to avoid running
//! real builds. Tests assert filesystem outcomes, idempotency,
//! flag behavior, and failure paths.
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

/// Path to the skill source directory.
fn skill_source_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("skill")
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

        // Create packages/cli directory
        fs::create_dir_all(root.join("packages/cli")).unwrap();

        // Copy skill files
        let skill_src = skill_source_dir();
        let skill_dest = root.join("packages/skill");
        fs::create_dir_all(&skill_dest).unwrap();
        fs::copy(skill_src.join("SKILL.md"), skill_dest.join("SKILL.md")).unwrap();
        fs::copy(
            skill_src.join("agentmark.md"),
            skill_dest.join("agentmark.md"),
        )
        .unwrap();
        fs::copy(
            skill_src.join("install-skill.sh"),
            skill_dest.join("install-skill.sh"),
        )
        .unwrap();

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

        // Fake cargo: creates a fake binary at target/release/agentmark
        // The fake binary logs all invocations to AGENTMARK_INIT_LOG.
        let cargo_shim = shims_dir.join("cargo");
        fs::write(
            &cargo_shim,
            r#"#!/bin/sh
# Fake cargo shim for testing
# Find the source root by walking up from CWD
src_root="$(pwd)"
while [ ! -f "$src_root/Cargo.toml" ] && [ "$src_root" != "/" ]; do
    src_root="$(dirname "$src_root")"
done
mkdir -p "$src_root/target/release"
cat > "$src_root/target/release/agentmark" <<'FAKE_BIN'
#!/bin/sh
echo "agentmark-fake $*" >> "${AGENTMARK_INIT_LOG:-/dev/null}"
FAKE_BIN
chmod +x "$src_root/target/release/agentmark"
echo "Fake cargo build complete"
"#,
        )
        .unwrap();
        set_executable(&cargo_shim);

        shims_dir
    }

    /// Run the installer with the given args, using fake shims and temp HOME.
    fn run_installer(&self, args: &[&str]) -> InstallerResult {
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
        cmd.env("NO_COLOR", "1");
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
fn delegates_skill_installation() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());

    // Canonical skill should be installed
    let canonical = result.home.join(".agents/skills/agentmark");
    assert!(
        canonical.join("SKILL.md").exists(),
        "Canonical SKILL.md should exist"
    );

    // Agent root symlinks should exist
    let claude_link = result.home.join(".claude/skills/agentmark");
    assert!(claude_link.is_symlink(), "Claude skill should be symlinked");
    assert_eq!(fs::read_link(&claude_link).unwrap(), canonical);
}

#[test]
fn skips_init_in_non_interactive_mode() {
    // When stdin is not a TTY (as in test/CI/curl|bash), init is skipped
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&[]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());

    let has_bare_init = result.init_log_contents().lines().any(|line| {
        let args = line.trim_start_matches("agentmark-fake ");
        args == "init" || args.starts_with("init ")
    });
    assert!(
        !has_bare_init,
        "Init should be skipped in non-interactive mode"
    );

    let stdout = result.stdout();
    assert!(
        stdout.contains("Non-interactive") || stdout.contains("non-interactive"),
        "Should mention non-interactive skip, got:\n{stdout}"
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

    // Copy skill files
    let skill_src = skill_source_dir();
    let skill_dest = root.join("packages/skill");
    fs::create_dir_all(&skill_dest).unwrap();
    fs::copy(skill_src.join("SKILL.md"), skill_dest.join("SKILL.md")).unwrap();
    fs::copy(
        skill_src.join("agentmark.md"),
        skill_dest.join("agentmark.md"),
    )
    .unwrap();
    fs::copy(
        skill_src.join("install-skill.sh"),
        skill_dest.join("install-skill.sh"),
    )
    .unwrap();

    // HOME with spaces
    let home = tmp.path().join("home dir with spaces");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(home.join(".claude/skills")).unwrap();
    fs::create_dir_all(home.join(".codex/skills")).unwrap();

    // Create shims
    let shims_dir = tmp.path().join("shims");
    fs::create_dir_all(&shims_dir).unwrap();

    let cargo_shim = shims_dir.join("cargo");
    fs::write(
        &cargo_shim,
        r#"#!/bin/sh
src_root="$(pwd)"
while [ ! -f "$src_root/Cargo.toml" ] && [ "$src_root" != "/" ]; do
    src_root="$(dirname "$src_root")"
done
mkdir -p "$src_root/target/release"
cat > "$src_root/target/release/agentmark" <<'FAKE_BIN'
#!/bin/sh
echo "agentmark-fake $*" >> "${AGENTMARK_INIT_LOG:-/dev/null}"
FAKE_BIN
chmod +x "$src_root/target/release/agentmark"
"#,
    )
    .unwrap();
    set_executable(&cargo_shim);

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
