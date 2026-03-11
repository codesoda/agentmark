use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn agentmark() -> Command {
    Command::cargo_bin("agentmark").unwrap()
}

#[test]
fn init_happy_path() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let work = tmp.path().join("work");
    fs::create_dir_all(&work).unwrap();

    agentmark()
        .arg("init")
        .env("HOME", &home)
        .current_dir(&work)
        .write_stdin("claude\n./bookmarks\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("Initialized AgentMark"));

    // Verify config file
    let config_path = home.join(".agentmark/config.toml");
    assert!(config_path.is_file());
    let contents = fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("default_agent = \"claude\""));
    assert!(contents.contains("[enrichment]"));
    assert!(contents.contains("enabled = true"));
    assert!(contents.contains("# system_prompt"));

    // Verify storage path is absolute
    assert!(contents.contains("storage_path = \""));
    assert!(!contents.contains("storage_path = \"./"));

    // Verify index.db
    assert!(home.join(".agentmark/index.db").is_file());

    // Verify storage dir
    assert!(work.join("bookmarks").is_dir());
}

#[test]
fn init_defaults_on_empty_input() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let work = tmp.path().join("work");
    fs::create_dir_all(&work).unwrap();

    agentmark()
        .arg("init")
        .env("HOME", &home)
        .current_dir(&work)
        .write_stdin("\n\n")
        .assert()
        .success();

    let contents = fs::read_to_string(home.join(".agentmark/config.toml")).unwrap();
    assert!(contents.contains("default_agent = \"claude\""));
}

#[test]
fn init_existing_config_decline_overwrite() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let work = tmp.path().join("work");
    fs::create_dir_all(&work).unwrap();

    // First init
    agentmark()
        .arg("init")
        .env("HOME", &home)
        .current_dir(&work)
        .write_stdin("claude\n./bookmarks\n")
        .assert()
        .success();

    let original = fs::read_to_string(home.join(".agentmark/config.toml")).unwrap();

    // Decline overwrite
    agentmark()
        .arg("init")
        .env("HOME", &home)
        .current_dir(&work)
        .write_stdin("n\n")
        .assert()
        .failure();

    let after = fs::read_to_string(home.join(".agentmark/config.toml")).unwrap();
    assert_eq!(
        original, after,
        "config should not change after declined overwrite"
    );
}

#[test]
fn init_existing_config_confirm_overwrite() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let work = tmp.path().join("work");
    fs::create_dir_all(&work).unwrap();

    // First init with claude
    agentmark()
        .arg("init")
        .env("HOME", &home)
        .current_dir(&work)
        .write_stdin("claude\n./bookmarks\n")
        .assert()
        .success();

    // Overwrite with codex
    agentmark()
        .arg("init")
        .env("HOME", &home)
        .current_dir(&work)
        .write_stdin("y\ncodex\n./bookmarks2\n")
        .assert()
        .success();

    let contents = fs::read_to_string(home.join(".agentmark/config.toml")).unwrap();
    assert!(contents.contains("default_agent = \"codex\""));
}

#[test]
fn init_preserves_existing_index_db() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let work = tmp.path().join("work");
    fs::create_dir_all(&work).unwrap();

    // First init
    agentmark()
        .arg("init")
        .env("HOME", &home)
        .current_dir(&work)
        .write_stdin("claude\n./bookmarks\n")
        .assert()
        .success();

    // Write data to index.db
    let db_path = home.join(".agentmark/index.db");
    fs::write(&db_path, "important data").unwrap();

    // Second init with overwrite
    agentmark()
        .arg("init")
        .env("HOME", &home)
        .current_dir(&work)
        .write_stdin("y\nclaude\n./bookmarks\n")
        .assert()
        .success();

    assert_eq!(fs::read_to_string(&db_path).unwrap(), "important data");
}

#[test]
fn init_help_shows_description() {
    agentmark()
        .arg("init")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Initialize"));
}
