//! Which agent CLIs are installed. Stat only: an agent is never run here.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// `(id, executable, display name)`, in the order the UIs list them.
const AGENTS: [(&str, &str, &str); 3] = [
    ("claude", "claude", "Claude Code"),
    ("codex", "codex", "Codex"),
    ("pi", "pi", "pi"),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    /// Display name. UIs show this rather than hardcoding names.
    pub name: String,
    pub found: bool,
    /// The executable found, as `dir/name` (not canonicalized).
    pub path: Option<String>,
}

/// Look each agent up on `search_path`. Always returns claude, codex, pi.
pub fn detect_agents(search_path: &OsStr) -> Vec<AgentInfo> {
    AGENTS
        .iter()
        .map(|(id, exe, name)| {
            let path = find_executable(exe, search_path);
            AgentInfo {
                id: id.to_string(),
                name: name.to_string(),
                found: path.is_some(),
                path: path.map(|p| p.display().to_string()),
            }
        })
        .collect()
}

/// The first `dir/name` on `search_path` that is an executable regular file
/// (symlinks followed).
pub(crate) fn find_executable(name: &str, search_path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(search_path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable_file(candidate))
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn make_file(dir: &Path, name: &str, mode: u32) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    fn join(dirs: &[&Path]) -> std::ffi::OsString {
        std::env::join_paths(dirs).unwrap()
    }

    #[test]
    fn reports_all_three_in_order_when_none_found() {
        let tmp = TempDir::new().unwrap();
        let agents = detect_agents(&join(&[tmp.path()]));
        let ids: Vec<_> = agents.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["claude", "codex", "pi"]);
        let names: Vec<_> = agents.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Claude Code", "Codex", "pi"]);
        assert!(agents.iter().all(|a| !a.found && a.path.is_none()));
    }

    #[test]
    fn finds_executables_and_reports_their_path() {
        let tmp = TempDir::new().unwrap();
        let bin = tmp.path().join("bin");
        let claude = make_file(&bin, "claude", 0o755);
        let agents = detect_agents(&join(&[&bin]));
        assert!(agents[0].found);
        assert_eq!(agents[0].path.as_deref(), Some(claude.to_str().unwrap()));
        assert!(!agents[1].found);
    }

    #[test]
    fn first_directory_wins() {
        let tmp = TempDir::new().unwrap();
        let first = tmp.path().join("first");
        let second = tmp.path().join("second");
        let expected = make_file(&first, "codex", 0o755);
        make_file(&second, "codex", 0o755);
        let agents = detect_agents(&join(&[&first, &second]));
        assert_eq!(agents[1].path.as_deref(), Some(expected.to_str().unwrap()));
    }

    #[test]
    fn skips_non_executable_files_and_directories() {
        let tmp = TempDir::new().unwrap();
        let first = tmp.path().join("first");
        let second = tmp.path().join("second");
        make_file(&first, "pi", 0o644);
        std::fs::create_dir_all(first.join("claude")).unwrap();
        let expected = make_file(&second, "pi", 0o700);
        let agents = detect_agents(&join(&[&first, &second]));
        assert!(!agents[0].found, "a directory named claude is not an agent");
        assert_eq!(agents[2].path.as_deref(), Some(expected.to_str().unwrap()));
    }

    #[test]
    fn follows_symlinks_but_reports_the_link_path() {
        let tmp = TempDir::new().unwrap();
        let real = make_file(&tmp.path().join("real"), "claude-real", 0o755);
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::os::unix::fs::symlink(&real, bin.join("claude")).unwrap();
        std::os::unix::fs::symlink(tmp.path().join("missing"), bin.join("codex")).unwrap();
        let agents = detect_agents(&join(&[&bin]));
        assert_eq!(
            agents[0].path.as_deref(),
            Some(bin.join("claude").to_str().unwrap())
        );
        assert!(!agents[1].found, "a dangling symlink is not an agent");
    }
}
