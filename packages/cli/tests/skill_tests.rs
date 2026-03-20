//! Tests for the agentmark skill package (packages/skill/).
//!
//! Validates:
//! - Skill markdown content matches implemented CLI commands
//! - install-skill.sh works correctly in temp HOME environments
//! - Shell syntax is valid

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

fn skill_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("skill")
}

fn skill_content() -> String {
    fs::read_to_string(skill_dir().join("SKILL.md")).expect("SKILL.md should exist")
}

// --- Markdown content presence tests ---

#[test]
fn skill_documents_all_cli_commands() {
    let content = skill_content();
    let required_commands = [
        "agentmark init",
        "agentmark save",
        "agentmark list",
        "agentmark show",
        "agentmark search",
        "agentmark tag",
        "agentmark collections",
        "agentmark open",
        "agentmark reprocess",
    ];
    for cmd in &required_commands {
        assert!(
            content.contains(cmd),
            "Skill doc must mention command: {cmd}"
        );
    }
}

#[test]
fn skill_documents_save_flags() {
    let content = skill_content();
    let required_flags = [
        "--tags",
        "--collection",
        "--note",
        "--action",
        "--no-enrich",
    ];
    for flag in &required_flags {
        assert!(
            content.contains(flag),
            "Skill doc must mention save flag: {flag}"
        );
    }
}

#[test]
fn skill_documents_list_flags() {
    let content = skill_content();
    for flag in &["--collection", "--tag", "--state", "--limit"] {
        assert!(
            content.contains(flag),
            "Skill doc must mention list flag: {flag}"
        );
    }
}

#[test]
fn skill_documents_show_full_flag() {
    let content = skill_content();
    assert!(content.contains("--full"), "Skill doc must mention --full");
}

#[test]
fn skill_documents_tag_remove_flag() {
    let content = skill_content();
    assert!(
        content.contains("--remove"),
        "Skill doc must mention --remove for tag command"
    );
}

#[test]
fn skill_documents_reprocess_all_flag() {
    let content = skill_content();
    assert!(
        content.contains("--all"),
        "Skill doc must mention --all for reprocess"
    );
}

#[test]
fn skill_documents_bundle_structure() {
    let content = skill_content();
    let required_files = [
        "bookmark.md",
        "article.md",
        "metadata.json",
        "source.html",
        "events.jsonl",
    ];
    for file in &required_files {
        assert!(
            content.contains(file),
            "Skill doc must mention bundle file: {file}"
        );
    }
}

#[test]
fn skill_documents_bundle_path_convention() {
    let content = skill_content();
    assert!(
        content.contains("<storage_path>/<YYYY>/<MM>/<DD>/"),
        "Skill doc must describe bundle path convention"
    );
}

#[test]
fn skill_documents_bookmark_states() {
    let content = skill_content();
    for state in &["inbox", "processed", "archived"] {
        assert!(
            content.contains(state),
            "Skill doc must mention bookmark state: {state}"
        );
    }
}

#[test]
fn skill_documents_bootstrap_url() {
    let content = skill_content();
    assert!(
        content.contains("https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh"),
        "Skill doc must include bootstrap install URL"
    );
}

#[test]
fn skill_documents_system_prompt_guidance() {
    let content = skill_content();
    assert!(
        content.contains("system_prompt"),
        "Skill doc must mention system_prompt"
    );
    assert!(
        content.contains("config.toml"),
        "Skill doc must reference config.toml for system_prompt"
    );
}

#[test]
fn skill_documents_trigger_conditions() {
    let content = skill_content();
    assert!(
        content.contains("When to Use"),
        "Skill doc must include trigger conditions section"
    );
}

#[test]
fn skill_documents_bookmark_data_model() {
    let content = skill_content();
    let required_fields = [
        "user_tags",
        "suggested_tags",
        "content_status",
        "summary_status",
        "content_hash",
        "capture_source",
    ];
    for field in &required_fields {
        assert!(
            content.contains(field),
            "Skill doc must mention bookmark field: {field}"
        );
    }
}

#[test]
fn skill_does_not_mention_nonexistent_commands() {
    let content = skill_content();
    // The PRD mentions "related" but no such command exists
    assert!(
        !content.contains("agentmark related"),
        "Skill doc must not mention nonexistent 'related' command"
    );
}

// --- Shell syntax validation ---

#[test]
fn install_script_has_valid_shell_syntax() {
    let script = skill_dir().join("install-skill.sh");
    let output = Command::new("sh")
        .arg("-n")
        .arg(&script)
        .output()
        .expect("sh should be available");
    assert!(
        output.status.success(),
        "install-skill.sh has syntax errors:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// --- Installer integration tests ---

fn run_installer(
    source_dir: &Path,
    home: &Path,
    shared_skills: Option<&Path>,
    claude_skills: Option<&Path>,
    codex_skills: Option<&Path>,
) -> std::process::Output {
    let script = source_dir.join("install-skill.sh");
    let mut cmd = Command::new("sh");
    cmd.arg(&script);
    cmd.env("HOME", home);
    if let Some(p) = shared_skills {
        cmd.env("AGENTMARK_SHARED_SKILLS_DIR", p);
    }
    if let Some(p) = claude_skills {
        cmd.env("CLAUDE_SKILLS_DIR", p);
    }
    if let Some(p) = codex_skills {
        cmd.env("CODEX_SKILLS_DIR", p);
    }
    cmd.output().expect("installer should execute")
}

#[test]
fn installer_creates_canonical_root_and_symlinks_both_agents() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    let shared = home.join(".agents/skills");
    let claude = home.join(".claude/skills");
    let codex = home.join(".codex/skills");
    fs::create_dir_all(&claude).unwrap();
    fs::create_dir_all(&codex).unwrap();

    let output = run_installer(
        &skill_dir(),
        home,
        Some(&shared),
        Some(&claude),
        Some(&codex),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "Installer failed:\n{stdout}");

    // Canonical root exists with SKILL.md
    let canonical = shared.join("agentmark");
    assert!(canonical.join("SKILL.md").exists());

    // Agent roots are symlinks to canonical
    let claude_link = claude.join("agentmark");
    assert!(claude_link.is_symlink());
    assert_eq!(fs::read_link(&claude_link).unwrap(), canonical);

    let codex_link = codex.join("agentmark");
    assert!(codex_link.is_symlink());
    assert_eq!(fs::read_link(&codex_link).unwrap(), canonical);
}

#[test]
fn installer_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    let shared = home.join(".agents/skills");
    let claude = home.join(".claude/skills");
    fs::create_dir_all(&claude).unwrap();

    // Run twice
    let output1 = run_installer(
        &skill_dir(),
        home,
        Some(&shared),
        Some(&claude),
        None::<&Path>.map(|p| p),
    );
    assert!(output1.status.success());

    let output2 = run_installer(
        &skill_dir(),
        home,
        Some(&shared),
        Some(&claude),
        None::<&Path>.map(|p| p),
    );
    assert!(output2.status.success());

    // Still correct after second run
    let canonical = shared.join("agentmark");
    assert!(canonical.join("SKILL.md").exists());
    let claude_link = claude.join("agentmark");
    assert!(claude_link.is_symlink());
    assert_eq!(fs::read_link(&claude_link).unwrap(), canonical);
}

#[test]
fn installer_works_with_only_claude_root() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    let shared = home.join(".agents/skills");
    let claude = home.join(".claude/skills");
    let codex = home.join("nonexistent-codex-skills");
    fs::create_dir_all(&claude).unwrap();
    // codex dir intentionally not created

    let output = run_installer(
        &skill_dir(),
        home,
        Some(&shared),
        Some(&claude),
        Some(&codex),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "Installer failed:\n{stdout}");
    assert!(stdout.contains("LINKED"));
    assert!(stdout.contains("SKIPPED"));

    // Claude linked, codex not
    assert!(claude.join("agentmark").is_symlink());
    assert!(!codex.join("agentmark").exists());
}

#[test]
fn installer_works_with_no_agent_roots() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    let shared = home.join(".agents/skills");
    let claude = home.join("no-claude");
    let codex = home.join("no-codex");
    // Neither agent dir created

    let output = run_installer(
        &skill_dir(),
        home,
        Some(&shared),
        Some(&claude),
        Some(&codex),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "Installer succeeded:\n{stdout}");

    // Canonical root still created
    assert!(shared.join("agentmark/SKILL.md").exists());

    // Summary shows no links
    assert!(stdout.contains("Agent roots linked: 0"));
    assert!(stdout.contains("No agent skill roots were detected"));
}

#[test]
fn installer_replaces_existing_symlink() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    let shared = home.join(".agents/skills");
    let claude = home.join(".claude/skills");
    fs::create_dir_all(&claude).unwrap();

    // Create a stale symlink
    let stale_target = home.join("stale");
    fs::create_dir_all(&stale_target).unwrap();
    symlink(&stale_target, claude.join("agentmark")).unwrap();

    let output = run_installer(
        &skill_dir(),
        home,
        Some(&shared),
        Some(&claude),
        None::<&Path>.map(|p| p),
    );
    assert!(output.status.success());

    // Symlink now points to canonical
    let canonical = shared.join("agentmark");
    let link = claude.join("agentmark");
    assert!(link.is_symlink());
    assert_eq!(fs::read_link(&link).unwrap(), canonical);
}

#[test]
fn installer_replaces_existing_directory() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    let shared = home.join(".agents/skills");
    let claude = home.join(".claude/skills");
    fs::create_dir_all(&claude).unwrap();

    // Create an existing agentmark directory (from prior manual install)
    fs::create_dir_all(claude.join("agentmark")).unwrap();
    fs::write(claude.join("agentmark/old-file.txt"), "old").unwrap();

    let output = run_installer(
        &skill_dir(),
        home,
        Some(&shared),
        Some(&claude),
        None::<&Path>.map(|p| p),
    );
    assert!(output.status.success());

    // Now a symlink, not a directory
    let link = claude.join("agentmark");
    assert!(link.is_symlink());
}

#[test]
fn installer_fails_when_shared_root_is_a_file() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    let shared = home.join(".agents/skills");

    // Create shared path as a file, not a directory
    fs::create_dir_all(shared.parent().unwrap()).unwrap();
    fs::write(&shared, "not a directory").unwrap();

    let output = run_installer(
        &skill_dir(),
        home,
        Some(&shared),
        None::<&Path>.map(|p| p),
        None::<&Path>.map(|p| p),
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a directory"));
}

#[test]
fn installer_skips_when_agent_root_is_a_file() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    let shared = home.join(".agents/skills");
    let claude = home.join(".claude/skills");

    // Create claude skills path as a file
    fs::create_dir_all(claude.parent().unwrap()).unwrap();
    fs::write(&claude, "not a directory").unwrap();

    let output = run_installer(
        &skill_dir(),
        home,
        Some(&shared),
        Some(&claude),
        None::<&Path>.map(|p| p),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("SKIPPED"));

    // Canonical root still installed
    assert!(shared.join("agentmark/SKILL.md").exists());
}

#[test]
fn installer_handles_env_override_roots() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    let custom_shared = home.join("custom/shared/skills");
    let custom_claude = home.join("custom/claude/skills");
    fs::create_dir_all(&custom_claude).unwrap();

    let output = run_installer(
        &skill_dir(),
        home,
        Some(&custom_shared),
        Some(&custom_claude),
        None::<&Path>.map(|p| p),
    );
    assert!(output.status.success());

    assert!(custom_shared.join("agentmark/SKILL.md").exists());
    assert!(custom_claude.join("agentmark").is_symlink());
}

#[test]
fn installer_handles_paths_with_spaces() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("path with spaces");
    fs::create_dir_all(&home).unwrap();
    let shared = home.join(".agents/skills");
    let claude = home.join(".claude/skills");
    fs::create_dir_all(&claude).unwrap();

    let output = run_installer(
        &skill_dir(),
        &home,
        Some(&shared),
        Some(&claude),
        None::<&Path>.map(|p| p),
    );
    assert!(
        output.status.success(),
        "Installer should handle paths with spaces"
    );
    assert!(shared.join("agentmark/SKILL.md").exists());
    assert!(claude.join("agentmark").is_symlink());
}
