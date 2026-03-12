//! Tests for the root install.sh script.
//!
//! Uses a temp repo fixture with fake cargo/npm shims to avoid running
//! real builds. Tests assert filesystem outcomes, manifest contents,
//! idempotency, flag behavior, and failure paths.

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

/// Create a temp repo fixture with install.sh, fake skill files, and
/// fake packages/extension structure.
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

        // Create minimal extension structure
        let ext_dir = root.join("packages/extension");
        fs::create_dir_all(&ext_dir).unwrap();
        fs::write(ext_dir.join("package.json"), "{}").unwrap();
        fs::write(ext_dir.join("package-lock.json"), "{}").unwrap();

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

        // Fake npm: records invocations and creates dist/ for build
        let npm_shim = shims_dir.join("npm");
        fs::write(
            &npm_shim,
            r#"#!/bin/sh
# Fake npm shim for testing
echo "npm $*" >> "${NPM_LOG:-/dev/null}"
if [ "$1" = "run" ] && [ "$2" = "build" ]; then
    mkdir -p dist
    echo '{"fake": true}' > dist/manifest.json
fi
"#,
        )
        .unwrap();
        set_executable(&npm_shim);

        // Fake node
        let node_shim = shims_dir.join("node");
        fs::write(&node_shim, "#!/bin/sh\necho 'v20.0.0'\n").unwrap();
        set_executable(&node_shim);

        shims_dir
    }

    /// Run the installer with the given args, using fake shims and temp HOME.
    fn run_installer(&self, args: &[&str]) -> InstallerResult {
        let home = self.home();
        fs::create_dir_all(&home).unwrap();

        let shims = self.create_shims();
        let init_log = self.tmp.path().join("init.log");
        let npm_log = self.tmp.path().join("npm.log");

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
        cmd.env("NPM_LOG", &npm_log);
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
            npm_log,
        }
    }
}

struct InstallerResult {
    output: std::process::Output,
    home: PathBuf,
    init_log: PathBuf,
    npm_log: PathBuf,
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

    fn extension_dir(&self) -> PathBuf {
        self.home.join(".agentmark/extension")
    }

    fn native_host_manifest(&self) -> PathBuf {
        if cfg!(target_os = "macos") {
            self.home.join("Library/Application Support/Google/Chrome/NativeMessagingHosts/com.agentmark.native.json")
        } else {
            self.home
                .join(".config/google-chrome/NativeMessagingHosts/com.agentmark.native.json")
        }
    }

    fn init_was_called(&self) -> bool {
        self.init_log.exists()
            && fs::read_to_string(&self.init_log)
                .unwrap_or_default()
                .contains("init")
    }

    fn npm_invocations(&self) -> String {
        fs::read_to_string(&self.npm_log).unwrap_or_default()
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
    let output = Command::new("bash")
        .arg("-n")
        .arg(&script)
        .output()
        .expect("bash should be available");
    assert!(
        output.status.success(),
        "install.sh has syntax errors:\n{}",
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
    let result = fixture.run_installer(&["--skip-init", "--skip-extension"]);

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
    let result = fixture.run_installer(&["--skip-init", "--skip-extension"]);

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
fn installs_extension_to_durable_dir() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());

    let ext_dir = result.extension_dir();
    assert!(
        ext_dir.exists(),
        "Extension dir should exist at {:?}",
        ext_dir
    );
    assert!(
        ext_dir.join("manifest.json").exists(),
        "Extension dir should contain manifest.json"
    );

    // Verify npm was called with ci and build
    let npm_calls = result.npm_invocations();
    assert!(npm_calls.contains("ci"), "npm ci should have been called");
    assert!(
        npm_calls.contains("run build"),
        "npm run build should have been called"
    );
}

#[test]
fn writes_native_host_manifest_with_extension_id() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&[
        "--skip-init",
        "--skip-extension",
        "--extension-id",
        "abcdefghijklmnopabcdefghijklmnop",
    ]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());

    let manifest_path = result.native_host_manifest();
    assert!(
        manifest_path.exists(),
        "Native host manifest should exist at {:?}",
        manifest_path
    );

    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap())
            .expect("Manifest should be valid JSON");

    assert_eq!(manifest["name"], "com.agentmark.native");
    assert_eq!(manifest["type"], "stdio");
    assert_eq!(
        manifest["path"],
        result.installed_binary().to_str().unwrap()
    );
    assert_eq!(
        manifest["allowed_origins"][0],
        "chrome-extension://abcdefghijklmnopabcdefghijklmnop/"
    );
}

#[test]
fn delegates_skill_installation() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init", "--skip-extension"]);

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
fn runs_init_with_absolute_binary_path() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-extension"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());
    assert!(
        result.init_was_called(),
        "Init should have been called via the installed binary"
    );

    // Verify the init log shows the init command was invoked
    let log = fs::read_to_string(&result.init_log).unwrap();
    assert!(log.contains("init"), "Init log should record init call");
}

#[test]
fn skip_init_suppresses_init() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init", "--skip-extension"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());
    assert!(
        !result.init_was_called(),
        "Init should NOT have been called with --skip-init"
    );
}

#[test]
fn skip_extension_suppresses_npm() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init", "--skip-extension"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());

    let npm_calls = result.npm_invocations();
    assert!(
        npm_calls.is_empty(),
        "npm should NOT have been called with --skip-extension, but got: {npm_calls}"
    );

    assert!(
        !result.extension_dir().exists(),
        "Extension dir should not exist with --skip-extension"
    );
}

#[test]
fn idempotent_rerun() {
    let fixture = TestFixture::new();

    // First run
    let result1 = fixture.run_installer(&[
        "--skip-init",
        "--skip-extension",
        "--extension-id",
        "testextensionid",
    ]);
    assert!(result1.success(), "First run failed:\n{}", result1.stdout());

    // Second run
    let result2 = fixture.run_installer(&[
        "--skip-init",
        "--skip-extension",
        "--extension-id",
        "testextensionid",
    ]);
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
    // Manifest still valid
    assert!(result2.native_host_manifest().exists());
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
    cmd.arg("--skip-extension");
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
fn missing_npm_fails_when_extension_not_skipped() {
    let fixture = TestFixture::new();
    let home = fixture.home();
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(home.join(".claude/skills")).unwrap();
    fs::create_dir_all(home.join(".codex/skills")).unwrap();

    let shims = fixture.create_shims();

    // Remove npm shim
    fs::remove_file(shims.join("npm")).unwrap();
    // Remove node shim too
    fs::remove_file(shims.join("node")).unwrap();

    let mut cmd = Command::new("sh");
    cmd.arg(fixture.root().join("install.sh"));
    cmd.arg("--skip-init");
    cmd.env("HOME", &home);
    cmd.env("PATH", format!("{}:/usr/bin:/bin", shims.display()));
    cmd.env("NO_COLOR", "1");
    cmd.env("AGENTMARK_SHARED_SKILLS_DIR", home.join(".agents/skills"));
    cmd.env("CLAUDE_SKILLS_DIR", home.join(".claude/skills"));
    cmd.env("CODEX_SKILLS_DIR", home.join(".codex/skills"));
    cmd.current_dir(fixture.root());

    let output = cmd.output().expect("installer should execute");
    assert!(
        !output.status.success(),
        "Installer should fail without node/npm when extension not skipped"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("node is required") || stderr.contains("npm is required"),
        "Error should mention node or npm, got: {stderr}"
    );
}

#[test]
fn missing_extension_id_warns_but_succeeds() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init", "--skip-extension"]);

    assert!(
        result.success(),
        "Installer should succeed without extension ID"
    );

    let stderr = result.stderr();
    assert!(
        stderr.contains("No extension ID provided"),
        "Should warn about missing extension ID, got stderr: {stderr}"
    );

    assert!(
        !result.native_host_manifest().exists(),
        "Native host manifest should NOT be written without extension ID"
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

    let result = fixture.run_installer(&["--skip-init", "--skip-extension"]);

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
    // The test HOME's .local/bin is never on PATH in the test environment
    let result = fixture.run_installer(&["--skip-init", "--skip-extension"]);

    assert!(result.success(), "Installer should succeed");

    let stderr = result.stderr();
    assert!(
        stderr.contains("not on your PATH"),
        "Should warn about .local/bin not on PATH, got: {stderr}"
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

    let mut cmd = Command::new("sh");
    cmd.arg(fixture.root().join("install.sh"));
    cmd.arg("--skip-init");
    cmd.arg("--skip-extension");
    cmd.env("HOME", &home);
    cmd.env("PATH", format!("{}:/usr/bin:/bin", shims.display()));
    cmd.env("AGENTMARK_EXTENSION_ID", "envvarextensionid");
    cmd.env("NO_COLOR", "1");
    cmd.env("AGENTMARK_SHARED_SKILLS_DIR", home.join(".agents/skills"));
    cmd.env("CLAUDE_SKILLS_DIR", home.join(".claude/skills"));
    cmd.env("CODEX_SKILLS_DIR", home.join(".codex/skills"));
    cmd.current_dir(fixture.root());

    let output = cmd.output().expect("installer should execute");
    assert!(output.status.success(), "Installer should succeed");

    let manifest_path = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Google/Chrome/NativeMessagingHosts/com.agentmark.native.json")
    } else {
        home.join(".config/google-chrome/NativeMessagingHosts/com.agentmark.native.json")
    };
    assert!(manifest_path.exists(), "Manifest should be written");

    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(
        manifest["allowed_origins"][0],
        "chrome-extension://envvarextensionid/"
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
        stdout.contains("--skip-extension"),
        "Help should document --skip-extension"
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

    // Extension structure
    let ext_dir = root.join("packages/extension");
    fs::create_dir_all(&ext_dir).unwrap();
    fs::write(ext_dir.join("package.json"), "{}").unwrap();
    fs::write(ext_dir.join("package-lock.json"), "{}").unwrap();

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

    let npm_shim = shims_dir.join("npm");
    fs::write(&npm_shim, "#!/bin/sh\n").unwrap();
    set_executable(&npm_shim);

    let node_shim = shims_dir.join("node");
    fs::write(&node_shim, "#!/bin/sh\necho 'v20.0.0'\n").unwrap();
    set_executable(&node_shim);

    let mut cmd = Command::new("sh");
    cmd.arg(root.join("install.sh"));
    cmd.arg("--skip-init");
    cmd.arg("--skip-extension");
    cmd.env("HOME", &home);
    cmd.env("PATH", format!("{}:/usr/bin:/bin", shims_dir.display()));
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

#[test]
fn summary_mentions_extension_load_path() {
    let fixture = TestFixture::new();
    let result = fixture.run_installer(&["--skip-init"]);

    assert!(result.success(), "Installer failed:\n{}", result.stdout());

    let stdout = result.stdout();
    let ext_dir_str = result.extension_dir().to_str().unwrap().to_string();
    assert!(
        stdout.contains(&ext_dir_str),
        "Summary should mention extension install path {ext_dir_str}, got:\n{stdout}"
    );
}

#[test]
fn manifest_rewrite_with_new_extension_id() {
    let fixture = TestFixture::new();

    // First run with one ID
    let result1 = fixture.run_installer(&[
        "--skip-init",
        "--skip-extension",
        "--extension-id",
        "firstid",
    ]);
    assert!(result1.success());

    let manifest1: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(result1.native_host_manifest()).unwrap()).unwrap();
    assert_eq!(
        manifest1["allowed_origins"][0],
        "chrome-extension://firstid/"
    );

    // Second run with different ID
    let result2 = fixture.run_installer(&[
        "--skip-init",
        "--skip-extension",
        "--extension-id",
        "secondid",
    ]);
    assert!(result2.success());

    let manifest2: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(result2.native_host_manifest()).unwrap()).unwrap();
    assert_eq!(
        manifest2["allowed_origins"][0],
        "chrome-extension://secondid/"
    );
}
