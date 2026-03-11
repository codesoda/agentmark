//! Integration tests for `agentmark save`.
//!
//! Uses mockito for HTTP fixtures, tempfile for isolated HOME/storage,
//! and assert_cmd for binary-level assertions.

use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;

// ── Helpers ─────────────────────────────────────────────────────────

/// Set up a temp HOME with a valid config pointing at `storage_root`
/// and a mock server URL.
fn setup_home(tmp: &TempDir, storage_path: &Path) -> std::path::PathBuf {
    let home = tmp.path().join("home");
    let config_dir = home.join(".agentmark");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::create_dir_all(storage_path).unwrap();

    let config_content = format!(
        r#"default_agent = "claude"
storage_path = "{}"

[enrichment]
enabled = true
"#,
        storage_path.display()
    );
    std::fs::write(config_dir.join("config.toml"), config_content).unwrap();

    // Touch index.db so open_and_migrate can work
    std::fs::write(config_dir.join("index.db"), b"").unwrap();

    home
}

fn agentmark_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("agentmark").unwrap();
    cmd.env("HOME", home);
    cmd
}

/// Minimal HTML that produces extractable content.
fn sample_html() -> &'static str {
    r#"<!DOCTYPE html>
<html>
<head>
    <title>Test Page Title</title>
    <meta property="og:title" content="OG Test Title">
    <meta name="description" content="A test description">
    <meta name="author" content="Test Author">
    <link rel="canonical" href="https://example.com/canonical">
</head>
<body>
<article>
<h1>Test Article</h1>
<p>This is a substantial test article with enough content to pass extraction thresholds.
It includes multiple paragraphs and covers an interesting topic in detail.</p>
<p>The second paragraph adds even more meaningful content to the article body.</p>
</article>
</body>
</html>"#
}

/// HTML with no extractable article content.
fn empty_article_html() -> &'static str {
    "<html><head><title>Empty Page</title></head><body><nav>Menu</nav></body></html>"
}

// ── Happy path tests ────────────────────────────────────────────────

#[test]
fn save_minimal_url_creates_bundle_and_index() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    let url = format!("{}/article", server.url());
    let output = agentmark_cmd(&home)
        .args(["save", &url])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    mock.assert();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("Saved bookmark am_"), "stdout: {stdout}");
    assert!(stdout.contains("path:"), "stdout: {stdout}");

    // Verify bundle files exist
    let bundle_dirs: Vec<_> = find_bundle_dirs(&storage);
    assert_eq!(bundle_dirs.len(), 1, "expected exactly one bundle");
    let bundle_dir = &bundle_dirs[0];
    assert!(bundle_dir.join("bookmark.md").is_file());
    assert!(bundle_dir.join("article.md").is_file());
    assert!(bundle_dir.join("metadata.json").is_file());
    assert!(bundle_dir.join("source.html").is_file());
    assert!(bundle_dir.join("events.jsonl").is_file());

    // Verify article.md has content (extraction worked)
    let article = std::fs::read_to_string(bundle_dir.join("article.md")).unwrap();
    assert!(!article.trim().is_empty(), "article.md should have content");

    // Verify SQLite has the row
    let db_path = home.join(".agentmark/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn save_with_all_flags_populates_fields() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_body(sample_html())
        .create();

    let url = format!("{}/article", server.url());
    agentmark_cmd(&home)
        .args([
            "save",
            &url,
            "--tags",
            "rust,cli",
            "--collection",
            "dev",
            "--note",
            "good read",
            "--action",
            "review later",
            "--no-enrich",
        ])
        .assert()
        .success();

    // Verify bookmark fields in SQLite
    let db_path = home.join(".agentmark/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let (user_tags, collections, note, action_prompt): (
        String,
        String,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT user_tags, collections, note, action_prompt FROM bookmarks LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();

    let tags: Vec<String> = serde_json::from_str(&user_tags).unwrap();
    assert_eq!(tags, vec!["rust", "cli"]);
    let cols: Vec<String> = serde_json::from_str(&collections).unwrap();
    assert_eq!(cols, vec!["dev"]);
    assert_eq!(note.as_deref(), Some("good read"));
    assert_eq!(action_prompt.as_deref(), Some("review later"));

    // Verify bookmark.md front matter contains the same data
    let bundle_dirs = find_bundle_dirs(&storage);
    assert_eq!(bundle_dirs.len(), 1);
    let bm_md = std::fs::read_to_string(bundle_dirs[0].join("bookmark.md")).unwrap();
    assert!(bm_md.contains("rust"), "bookmark.md should contain tag");
    assert!(
        bm_md.contains("good read"),
        "bookmark.md should contain note"
    );
}

// ── Extraction warning test ─────────────────────────────────────────

#[test]
fn save_empty_extraction_warns_but_succeeds() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/empty")
        .with_status(200)
        .with_body(empty_article_html())
        .create();

    let url = format!("{}/empty", server.url());
    let output = agentmark_cmd(&home)
        .args(["save", &url])
        .assert()
        .success()
        .get_output()
        .clone();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("no readable text"),
        "should warn about empty extraction, stderr: {stderr}"
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Saved bookmark"), "should still succeed");

    // Verify content_status is "failed" in DB
    let db_path = home.join(".agentmark/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let status: String = conn
        .query_row("SELECT content_status FROM bookmarks LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(status, "failed");
}

// ── Failure path tests ──────────────────────────────────────────────

#[test]
fn save_invalid_url_fails() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let output = agentmark_cmd(&home)
        .args(["save", "not a url"])
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid URL") || stderr.contains("fetch failed"),
        "should mention URL error, stderr: {stderr}"
    );
}

#[test]
fn save_unsupported_scheme_fails() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let output = agentmark_cmd(&home)
        .args(["save", "ftp://example.com/file"])
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("unsupported") || stderr.contains("ftp"),
        "should mention unsupported scheme, stderr: {stderr}"
    );
}

#[test]
fn save_missing_config_fails() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("no-config-home");
    std::fs::create_dir_all(&home).unwrap();

    let output = agentmark_cmd(&home)
        .args(["save", "https://example.com"])
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("agentmark init"),
        "should suggest running init, stderr: {stderr}"
    );
}

#[test]
fn save_http_error_fails() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server.mock("GET", "/gone").with_status(404).create();

    let url = format!("{}/gone", server.url());
    let output = agentmark_cmd(&home)
        .args(["save", &url])
        .assert()
        .failure()
        .get_output()
        .clone();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("404"),
        "should mention HTTP status, stderr: {stderr}"
    );
}

// ── Cross-store consistency ─────────────────────────────────────────

#[test]
fn save_bundle_and_db_share_same_id() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_body(sample_html())
        .create();

    let url = format!("{}/article", server.url());
    let output = agentmark_cmd(&home)
        .args(["save", &url])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();

    // Extract ID from stdout ("Saved bookmark am_XXXXX")
    let id = stdout
        .lines()
        .find(|l| l.contains("Saved bookmark"))
        .and_then(|l| l.strip_prefix("Saved bookmark "))
        .map(|s| s.trim())
        .expect("should contain bookmark ID");
    assert!(id.starts_with("am_"), "id: {id}");

    // Verify same ID in bundle directory name
    let bundle_dirs = find_bundle_dirs(&storage);
    assert_eq!(bundle_dirs.len(), 1);
    let dir_name = bundle_dirs[0].file_name().unwrap().to_str().unwrap();
    assert!(
        dir_name.contains(id),
        "bundle dir should contain ID: dir={dir_name}, id={id}"
    );

    // Verify same ID in SQLite
    let db_path = home.join(".agentmark/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let db_id: String = conn
        .query_row("SELECT id FROM bookmarks LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(db_id, id);
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Find all leaf directories in storage that contain bookmark.md (bundle dirs).
fn find_bundle_dirs(storage: &Path) -> Vec<std::path::PathBuf> {
    let mut results = Vec::new();
    walk_for_bundles(storage, &mut results);
    results
}

fn walk_for_bundles(dir: &Path, results: &mut Vec<std::path::PathBuf>) {
    if !dir.is_dir() {
        return;
    }
    if dir.join("bookmark.md").exists() {
        results.push(dir.to_path_buf());
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            walk_for_bundles(&entry.path(), results);
        }
    }
}
