//! Binary-level tests for `agentmark init`.
//!
//! Answers are always given explicitly: the defaults depend on which agent
//! CLIs the machine has installed.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use std::fs;
use tempfile::TempDir;

fn agentmark() -> Command {
    Command::cargo_bin("agentmark").unwrap()
}

/// A temp home and working directory.
fn dirs() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let work = tmp.path().join("work");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&work).unwrap();
    (tmp, home, work)
}

fn init(home: &std::path::Path, work: &std::path::Path, stdin: &str) -> assert_cmd::assert::Assert {
    agentmark()
        .arg("init")
        .env("HOME", home)
        .current_dir(work)
        .write_stdin(stdin.to_string())
        .assert()
}

#[test]
fn init_happy_path() {
    let (_tmp, home, work) = dirs();

    init(&home, &work, "claude\n./bookmarks\ny\n")
        .success()
        .stdout(predicates::str::contains("Initialized AgentMark"))
        .stdout(predicates::str::contains("summaries: on"));

    // Verify config file
    let config_path = home.join(".agentmark/config.toml");
    assert!(config_path.is_file());
    let contents = fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("default_agent = \"claude\""));
    assert!(contents.contains("[enrichment]"));
    assert!(contents.contains("enabled = true"));
    assert!(contents.contains("# system_prompt"));

    // Relative input is resolved against the working directory (canonical,
    // as the process sees it: /var is /private/var on macOS).
    let cwd = fs::canonicalize(&work).unwrap();
    assert!(contents.contains(&format!(
        "storage_path = \"{}\"",
        cwd.join("bookmarks").display()
    )));

    // The index is created and migrated.
    assert!(home.join(".agentmark/index.db").is_file());
    assert!(work.join("bookmarks").is_dir());
}

#[test]
fn init_default_folder_is_documents_agentmark() {
    let (_tmp, home, work) = dirs();

    init(&home, &work, "claude\n\nn\n").success();

    let contents = fs::read_to_string(home.join(".agentmark/config.toml")).unwrap();
    assert!(contents.contains("default_agent = \"claude\""));
    let default = home.join("Documents/AgentMark");
    assert!(contents.contains(&format!("storage_path = \"{}\"", default.display())));
    assert!(contents.contains("enabled = false"));
    assert!(default.is_dir());
}

#[test]
fn init_existing_config_decline_changes_nothing() {
    let (_tmp, home, work) = dirs();
    init(&home, &work, "claude\n./bookmarks\ny\n").success();
    let original = fs::read_to_string(home.join(".agentmark/config.toml")).unwrap();

    // Declining is not an error any more: nothing to do, exit 0.
    init(&home, &work, "n\n")
        .success()
        .stdout(predicates::str::contains("Nothing changed."));

    let after = fs::read_to_string(home.join(".agentmark/config.toml")).unwrap();
    assert_eq!(original, after, "config should not change after declining");
}

#[test]
fn init_existing_config_update() {
    let (_tmp, home, work) = dirs();
    init(&home, &work, "claude\n./bookmarks\ny\n").success();

    init(&home, &work, "y\ncodex\n./bookmarks2\nn\n").success();

    let contents = fs::read_to_string(home.join(".agentmark/config.toml")).unwrap();
    assert!(contents.contains("default_agent = \"codex\""));
    assert!(contents.contains("bookmarks2"));
    assert!(contents.contains("enabled = false"));
}

#[test]
fn init_update_keeps_comments_and_bookmarks() {
    let (_tmp, home, work) = dirs();
    init(&home, &work, "claude\n./bookmarks\ny\n").success();

    // A user comment and a saved bookmark must both survive a re-run.
    let config_path = home.join(".agentmark/config.toml");
    let edited = format!(
        "# my own note\n{}",
        fs::read_to_string(&config_path).unwrap()
    );
    fs::write(&config_path, edited).unwrap();
    let db_path = home.join(".agentmark/index.db");
    let conn = agentmark::db::open_and_migrate(&db_path).unwrap();
    let bm = agentmark::models::Bookmark::new("https://example.com/kept", "Kept");
    agentmark::db::BookmarkRepository::new(&conn)
        .insert(&bm)
        .unwrap();
    drop(conn);

    init(&home, &work, "y\nclaude\n./bookmarks\ny\n").success();

    assert!(fs::read_to_string(&config_path)
        .unwrap()
        .starts_with("# my own note\n"));
    let conn = agentmark::db::open_and_migrate(&db_path).unwrap();
    let count = agentmark::db::BookmarkRepository::new(&conn)
        .count_bookmarks()
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn init_non_interactive_flags() {
    let (_tmp, home, work) = dirs();

    agentmark()
        .args([
            "init",
            "--storage-path",
            "marks",
            "--agent",
            "pi",
            "--summaries",
            "off",
        ])
        .env("HOME", &home)
        .current_dir(&work)
        .assert()
        .success()
        .stdout(predicates::str::contains("summaries: off"))
        .stdout(predicates::str::contains("?").not());

    let contents = fs::read_to_string(home.join(".agentmark/config.toml")).unwrap();
    assert!(contents.contains("default_agent = \"pi\""));
    let cwd = fs::canonicalize(&work).unwrap();
    assert!(contents.contains(&format!(
        "storage_path = \"{}\"",
        cwd.join("marks").display()
    )));
}

#[test]
fn init_validation_errors_go_to_stderr_one_per_line() {
    let (_tmp, home, work) = dirs();

    agentmark()
        .args(["init", "--storage-path", " ", "--agent", "gpt"])
        .env("HOME", &home)
        .current_dir(&work)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("error: storage_path: "))
        .stderr(predicates::str::contains("\nerror: default_agent: "));

    assert!(!home.join(".agentmark/config.toml").exists());
}

#[test]
fn init_bad_flag_value_is_a_usage_error() {
    agentmark()
        .args(["init", "--summaries", "sometimes"])
        .assert()
        .code(2);
}

#[test]
fn init_help_shows_description() {
    agentmark()
        .arg("init")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Initialize"))
        .stdout(predicates::str::contains("--storage-path"));
}
