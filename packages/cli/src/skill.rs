//! Embedded agent skill files and installation helpers.
//!
//! The skill markdown files are included at compile time via `include_str!`.

use std::path::Path;

use crate::config;

const SKILL_MD: &str = include_str!("../../skill/SKILL.md");

const SKILL_NAME: &str = "agentmark";

/// Install the skill to the canonical location and symlink into agent roots.
///
/// Canonical location: `~/.agentmark/skill/`
/// Symlinks into:
///   - `~/.agents/skills/agentmark` (shared agent root, covers Codex)
///   - `~/.claude/skills/agentmark` (Claude Code)
pub fn install_skill(home: &Path) -> Result<SkillInstallResult, Box<dyn std::error::Error>> {
    let canonical_dir = config::config_dir(home).join("skill");

    // Write skill files to canonical location
    std::fs::create_dir_all(&canonical_dir)?;
    std::fs::write(canonical_dir.join("SKILL.md"), SKILL_MD)?;

    // Symlink into agent roots
    let mut linked = Vec::new();
    let mut skipped = Vec::new();

    // Always create the shared agents root
    match link_agent_root(&home.join(".agents/skills"), &canonical_dir, true) {
        Ok(()) => linked.push("Agents (shared)".to_string()),
        Err(reason) => skipped.push(format!("Agents (shared): {reason}")),
    }

    // Only link into Claude if it's already installed (~/.claude/ exists)
    let claude_skills = home.join(".claude/skills");
    if home.join(".claude").is_dir() {
        match link_agent_root(&claude_skills, &canonical_dir, true) {
            Ok(()) => linked.push("Claude Code".to_string()),
            Err(reason) => skipped.push(format!("Claude Code: {reason}")),
        }
    } else {
        skipped.push("Claude Code: ~/.claude not found".to_string());
    }

    Ok(SkillInstallResult {
        canonical_dir,
        linked,
        skipped,
    })
}

pub struct SkillInstallResult {
    pub canonical_dir: std::path::PathBuf,
    pub linked: Vec<String>,
    pub skipped: Vec<String>,
}

fn link_agent_root(skills_root: &Path, canonical_dir: &Path, create: bool) -> Result<(), String> {
    let target = skills_root.join(SKILL_NAME);

    if skills_root.exists() && !skills_root.is_dir() {
        return Err(format!("{} is not a directory", skills_root.display()));
    }

    if !skills_root.is_dir() {
        if create {
            std::fs::create_dir_all(skills_root)
                .map_err(|e| format!("failed to create {}: {e}", skills_root.display()))?;
        } else {
            return Err(format!("{} does not exist", skills_root.display()));
        }
    }

    // Handle existing target
    if target.is_symlink() {
        std::fs::remove_file(&target)
            .map_err(|e| format!("failed to remove existing symlink: {e}"))?;
    } else if target.is_dir() {
        std::fs::remove_dir_all(&target)
            .map_err(|e| format!("failed to remove existing directory: {e}"))?;
    } else if target.exists() {
        return Err(format!(
            "{} exists and is not a directory or symlink",
            target.display()
        ));
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(canonical_dir, &target)
        .map_err(|e| format!("symlink failed: {e}"))?;

    #[cfg(not(unix))]
    return Err("symlinks not supported on this platform".to_string());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn install_creates_canonical_files_under_agentmark() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        let result = install_skill(home).unwrap();
        assert_eq!(
            result.canonical_dir,
            home.join(".agentmark/skill"),
            "Canonical dir should be under ~/.agentmark/"
        );
        assert!(result.canonical_dir.join("SKILL.md").exists());
    }

    #[test]
    fn install_creates_shared_agents_root() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        let result = install_skill(home).unwrap();

        // Shared agents root should always be created and linked
        let agents_link = home.join(".agents/skills/agentmark");
        assert!(agents_link.is_symlink());
        assert_eq!(
            std::fs::read_link(&agents_link).unwrap(),
            result.canonical_dir
        );
    }

    #[test]
    fn install_skips_claude_when_not_installed() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        // Don't create ~/.claude

        let result = install_skill(home).unwrap();
        assert!(!home.join(".claude/skills/agentmark").exists());
        assert!(
            result.skipped.iter().any(|s| s.contains("Claude")),
            "Should skip Claude: {:?}",
            result.skipped
        );
    }

    #[test]
    fn install_links_claude_when_installed() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        std::fs::create_dir_all(home.join(".claude")).unwrap();

        let result = install_skill(home).unwrap();
        assert_eq!(result.linked.len(), 2);
        assert!(home.join(".claude/skills/agentmark").is_symlink());
    }

    #[test]
    fn install_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        std::fs::create_dir_all(home.join(".claude")).unwrap();

        install_skill(home).unwrap();
        let result = install_skill(home).unwrap();
        assert_eq!(result.linked.len(), 2);
        assert!(home.join(".claude/skills/agentmark").is_symlink());
        assert!(home.join(".agents/skills/agentmark").is_symlink());
    }
}
