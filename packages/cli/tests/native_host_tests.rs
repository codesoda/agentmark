//! Integration tests for `agentmark native-host`.
//!
//! Uses mockito for HTTP fixtures, tempfile for isolated HOME/storage,
//! and assert_cmd for binary-level assertions with raw stdin/stdout framing.

use assert_cmd::Command;
use serde_json::json;
use std::io::Cursor;
use std::path::Path;
use tempfile::TempDir;

// ── Helpers ─────────────────────────────────────────────────────────

fn setup_home(tmp: &TempDir, storage_path: &Path) -> std::path::PathBuf {
    let home = tmp.path().join("home");
    let config_dir = home.join(".agentmark");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::create_dir_all(storage_path).unwrap();

    let config_content = format!(
        r#"default_agent = "claude"
storage_path = "{}"

[enrichment]
enabled = false
"#,
        storage_path.display()
    );
    std::fs::write(config_dir.join("config.toml"), config_content).unwrap();

    // Create a properly-migrated SQLite DB (not an empty file)
    let db_path = config_dir.join("index.db");
    agentmark::db::open_and_migrate(&db_path).unwrap();

    home
}

fn agentmark_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("agentmark").unwrap();
    cmd.env("HOME", home);
    cmd
}

/// Frame a JSON value into length-prefixed bytes.
fn frame(value: &serde_json::Value) -> Vec<u8> {
    let payload = serde_json::to_vec(value).unwrap();
    let mut buf = Vec::new();
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(&payload);
    buf
}

/// Frame multiple messages into a single byte stream.
fn frame_all(messages: &[serde_json::Value]) -> Vec<u8> {
    let mut buf = Vec::new();
    for msg in messages {
        buf.extend_from_slice(&frame(msg));
    }
    buf
}

/// Decode all length-prefixed responses from stdout bytes.
fn decode_responses(stdout: &[u8]) -> Vec<serde_json::Value> {
    let mut cursor = Cursor::new(stdout);
    let mut responses = Vec::new();
    loop {
        // Read 4-byte length prefix
        let mut prefix = [0u8; 4];
        use std::io::Read;
        match cursor.read_exact(&mut prefix) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => panic!("decode error: {e}"),
        }
        let len = u32::from_le_bytes(prefix) as usize;
        let mut payload = vec![0u8; len];
        cursor.read_exact(&mut payload).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        responses.push(value);
    }
    responses
}

/// Save a bookmark via native-host framed protocol and return the save_result response.
fn save_bookmark(home: &Path, url: &str, title: &str, tags: &[&str]) -> serde_json::Value {
    let stdin = frame(&json!({
        "type": "save",
        "url": url,
        "title": title,
        "tags": tags
    }));
    let output = agentmark_cmd(home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["type"], "save_result");
    responses[0].clone()
}

fn sample_html() -> &'static str {
    r#"<!DOCTYPE html>
<html>
<head>
    <title>Test Page Title</title>
    <meta name="description" content="A test description">
    <meta name="author" content="Test Author">
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

// ── Tests ───────────────────────────────────────────────────────────

#[test]
fn empty_stdin_exits_cleanly() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(Vec::<u8>::new())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(output.is_empty(), "stdout should be empty on clean EOF");
}

#[test]
fn status_request_returns_status_result() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let stdin = frame(&json!({"type": "status"}));
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 1, "expected exactly one response");
    assert_eq!(responses[0]["type"], "status_result");
    assert_eq!(responses[0]["ok"], true);
    assert!(
        responses[0]["version"].as_str().is_some(),
        "version should be a string"
    );
}

#[test]
fn save_request_returns_save_result_and_creates_bundle() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    let url = format!("{}/article", server.url());
    let stdin = frame(&json!({
        "type": "save",
        "url": url,
        "title": "Test Title",
        "tags": ["rust", "cli"]
    }));

    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["type"], "save_result");
    assert_eq!(responses[0]["status"], "created");
    assert!(responses[0]["id"].as_str().unwrap().starts_with("am_"));
    assert!(responses[0]["path"].as_str().is_some());

    // Verify bundle exists
    let bundle_dirs = find_bundle_dirs(&storage);
    assert_eq!(bundle_dirs.len(), 1);
    assert!(bundle_dirs[0].join("bookmark.md").is_file());
    assert!(bundle_dirs[0].join("article.md").is_file());
    assert!(bundle_dirs[0].join("events.jsonl").is_file());

    // Verify DB row exists with chrome_extension capture source
    let db_path = home.join(".agentmark/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let (capture_source, user_tags): (String, String) = conn
        .query_row(
            "SELECT capture_source, user_tags FROM bookmarks LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(capture_source, "chrome_extension");
    let tags: Vec<String> = serde_json::from_str(&user_tags).unwrap();
    assert_eq!(tags, vec!["rust", "cli"]);

    // Verify bundle events record chrome_extension
    let events_path = bundle_dirs[0].join("events.jsonl");
    let events_content = std::fs::read_to_string(&events_path).unwrap();
    assert!(
        events_content.contains("chrome_extension"),
        "events should record chrome_extension capture source"
    );
}

#[test]
fn malformed_json_returns_error() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    // Send malformed JSON frame
    let bad_payload = b"not json{{{";
    let mut stdin = Vec::new();
    stdin.extend_from_slice(&(bad_payload.len() as u32).to_le_bytes());
    stdin.extend_from_slice(bad_payload);

    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["type"], "error");
    assert!(responses[0]["message"].as_str().unwrap().contains("JSON"));
}

#[test]
fn multi_message_sequence_works() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let stdin = frame_all(&[json!({"type": "status"}), json!({"type": "status"})]);

    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["type"], "status_result");
    assert_eq!(responses[1]["type"], "status_result");
}

#[test]
fn malformed_then_valid_continues_loop() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    // Malformed JSON followed by valid status
    let bad_payload = b"not json";
    let mut stdin = Vec::new();
    stdin.extend_from_slice(&(bad_payload.len() as u32).to_le_bytes());
    stdin.extend_from_slice(bad_payload);
    stdin.extend_from_slice(&frame(&json!({"type": "status"})));

    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["type"], "error");
    assert_eq!(responses[1]["type"], "status_result");
}

#[test]
fn stdout_contains_only_protocol_frames() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let stdin = frame(&json!({"type": "status"}));
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Decode all frames — should consume all stdout bytes
    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 1);

    // Re-encode to verify we consumed everything
    let mut expected_len = 0usize;
    for resp in &responses {
        let payload = serde_json::to_vec(resp).unwrap();
        expected_len += 4 + payload.len();
    }
    assert_eq!(
        output.len(),
        expected_len,
        "stdout should contain only framed protocol bytes, no trailing junk"
    );
}

// ── List integration tests ──────────────────────────────────────────

#[test]
fn list_empty_db_returns_empty_bookmarks() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let stdin = frame(&json!({"type": "list"}));
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["type"], "list_result");
    let bookmarks = responses[0]["bookmarks"].as_array().unwrap();
    assert!(bookmarks.is_empty(), "empty DB should return empty list");
}

#[test]
fn list_returns_saved_bookmarks_with_correct_fields() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/page1")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    let url = format!("{}/page1", server.url());
    save_bookmark(&home, &url, "Page One", &["rust", "testing"]);

    // Now list
    let stdin = frame(&json!({"type": "list"}));
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["type"], "list_result");
    let bookmarks = responses[0]["bookmarks"].as_array().unwrap();
    assert_eq!(bookmarks.len(), 1);

    let b = &bookmarks[0];
    assert!(
        b["id"].as_str().unwrap().starts_with("am_"),
        "id should be prefixed"
    );
    assert_eq!(b["url"], url);
    // Title comes from the fetched HTML <title> tag, not the provided title
    assert_eq!(b["title"], "Test Page Title");
    assert_eq!(b["state"], "inbox");
    let user_tags: Vec<&str> = b["user_tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(user_tags, vec!["rust", "testing"]);
    assert!(b["suggested_tags"].as_array().unwrap().is_empty());
    assert!(
        b["saved_at"].as_str().is_some(),
        "saved_at should be a string"
    );
}

#[test]
fn list_returns_bookmarks_ordered_by_saved_at_desc() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock1 = server
        .mock("GET", "/first")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();
    let _mock2 = server
        .mock("GET", "/second")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    let url1 = format!("{}/first", server.url());
    let url2 = format!("{}/second", server.url());
    save_bookmark(&home, &url1, "First", &[]);
    save_bookmark(&home, &url2, "Second", &[]);

    let stdin = frame(&json!({"type": "list"}));
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    let bookmarks = responses[0]["bookmarks"].as_array().unwrap();
    assert_eq!(bookmarks.len(), 2);
    // Most recent first — check by URL since both pages have the same HTML title
    let urls: Vec<&str> = bookmarks
        .iter()
        .map(|b| b["url"].as_str().unwrap())
        .collect();
    assert!(
        urls[0].ends_with("/second"),
        "most recent bookmark should be first"
    );
    assert!(
        urls[1].ends_with("/first"),
        "older bookmark should be second"
    );
}

#[test]
fn list_with_state_filter_returns_matching_bookmarks() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/filtered")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    let url = format!("{}/filtered", server.url());
    save_bookmark(&home, &url, "Inbox Item", &[]);

    // All new saves are inbox state — filtering by "processed" should be empty
    let stdin = frame(&json!({"type": "list", "state": "processed"}));
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses[0]["type"], "list_result");
    let bookmarks = responses[0]["bookmarks"].as_array().unwrap();
    assert!(
        bookmarks.is_empty(),
        "no bookmarks should match processed state"
    );

    // Filtering by "inbox" should return the saved bookmark
    let stdin2 = frame(&json!({"type": "list", "state": "inbox"}));
    let output2 = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin2)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses2 = decode_responses(&output2);
    let bookmarks2 = responses2[0]["bookmarks"].as_array().unwrap();
    assert_eq!(bookmarks2.len(), 1);
    assert_eq!(bookmarks2[0]["state"], "inbox");
}

#[test]
fn list_with_limit_clamps_results() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock1 = server
        .mock("GET", "/a")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();
    let _mock2 = server
        .mock("GET", "/b")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    save_bookmark(&home, &format!("{}/a", server.url()), "A", &[]);
    save_bookmark(&home, &format!("{}/b", server.url()), "B", &[]);

    // limit=1 should return only one
    let stdin = frame(&json!({"type": "list", "limit": 1}));
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    let bookmarks = responses[0]["bookmarks"].as_array().unwrap();
    assert_eq!(bookmarks.len(), 1);
    // Most recent should be returned
    assert!(
        bookmarks[0]["url"].as_str().unwrap().ends_with("/b"),
        "most recent bookmark should be returned"
    );
}

#[test]
fn list_with_limit_zero_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/zero")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    save_bookmark(&home, &format!("{}/zero", server.url()), "Zero", &[]);

    let stdin = frame(&json!({"type": "list", "limit": 0}));
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    let bookmarks = responses[0]["bookmarks"].as_array().unwrap();
    assert!(bookmarks.is_empty(), "limit=0 should return empty list");
}

#[test]
fn list_then_status_continues_loop() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let stdin = frame_all(&[json!({"type": "list"}), json!({"type": "status"})]);
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["type"], "list_result");
    assert_eq!(responses[1]["type"], "status_result");
}

// ── Show integration tests ──────────────────────────────────────────

fn show_bookmark(home: &Path, id: &str) -> serde_json::Value {
    let stdin = frame(&json!({"type": "show", "id": id}));
    let output = agentmark_cmd(home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 1);
    responses[0].clone()
}

fn update_bookmark(home: &Path, id: &str, changes: serde_json::Value) -> serde_json::Value {
    let stdin = frame(&json!({"type": "update", "id": id, "changes": changes}));
    let output = agentmark_cmd(home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 1);
    responses[0].clone()
}

#[test]
fn show_returns_detail_for_saved_bookmark() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/show-test")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    let url = format!("{}/show-test", server.url());
    let save_resp = save_bookmark(&home, &url, "Show Test", &["tag1"]);
    let id = save_resp["id"].as_str().unwrap();

    let resp = show_bookmark(&home, id);
    assert_eq!(resp["type"], "show_result");
    let bm = &resp["bookmark"];
    assert_eq!(bm["id"], id);
    assert_eq!(bm["url"], url);
    assert_eq!(bm["state"], "inbox");
    assert_eq!(bm["capture_source"], "chrome_extension");
    let user_tags: Vec<&str> = bm["user_tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(user_tags, vec!["tag1"]);
    assert!(bm["suggested_tags"].as_array().unwrap().is_empty());
    assert!(bm["saved_at"].as_str().is_some());
    assert!(bm["collections"].as_array().is_some());
}

#[test]
fn show_unknown_id_returns_error() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let resp = show_bookmark(&home, "am_nonexistent");
    assert_eq!(resp["type"], "error");
    assert!(
        resp["message"].as_str().unwrap().contains("not found"),
        "error message should mention not found"
    );
}

// ── Update integration tests ────────────────────────────────────────

#[test]
fn update_note_persists_to_db_and_bundle() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/update-note")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    let url = format!("{}/update-note", server.url());
    let save_resp = save_bookmark(&home, &url, "Update Note", &[]);
    let id = save_resp["id"].as_str().unwrap();

    let resp = update_bookmark(&home, id, json!({"note": "my note"}));
    assert_eq!(resp["type"], "update_result");
    assert_eq!(resp["bookmark"]["note"], "my note");

    // Verify via show
    let show_resp = show_bookmark(&home, id);
    assert_eq!(show_resp["bookmark"]["note"], "my note");

    // Verify in DB
    let db_path = home.join(".agentmark/index.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let note: Option<String> = conn
        .query_row("SELECT note FROM bookmarks WHERE id = ?", [id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(note, Some("my note".to_string()));

    // Verify in bundle
    let bundle_dirs = find_bundle_dirs(&storage);
    assert_eq!(bundle_dirs.len(), 1);
    let bm_md = std::fs::read_to_string(bundle_dirs[0].join("bookmark.md")).unwrap();
    assert!(bm_md.contains("my note"), "bundle should contain the note");
}

#[test]
fn update_state_persists() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/update-state")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    let url = format!("{}/update-state", server.url());
    let save_resp = save_bookmark(&home, &url, "Update State", &[]);
    let id = save_resp["id"].as_str().unwrap();

    let resp = update_bookmark(&home, id, json!({"state": "processed"}));
    assert_eq!(resp["type"], "update_result");
    assert_eq!(resp["bookmark"]["state"], "processed");

    // Verify via show
    let show_resp = show_bookmark(&home, id);
    assert_eq!(show_resp["bookmark"]["state"], "processed");
}

#[test]
fn update_tags_accept_reject_persists_separate_arrays() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/update-tags")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    let url = format!("{}/update-tags", server.url());
    let save_resp = save_bookmark(&home, &url, "Tag Test", &["original"]);
    let id = save_resp["id"].as_str().unwrap();

    // Simulate accepting a suggested tag by setting both arrays
    let resp = update_bookmark(
        &home,
        id,
        json!({
            "user_tags": ["original", "accepted"],
            "suggested_tags": ["remaining"]
        }),
    );
    assert_eq!(resp["type"], "update_result");

    let bm = &resp["bookmark"];
    let user_tags: Vec<&str> = bm["user_tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(user_tags, vec!["original", "accepted"]);
    let suggested: Vec<&str> = bm["suggested_tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(suggested, vec!["remaining"]);

    // Verify via show that arrays stayed separate
    let show_resp = show_bookmark(&home, id);
    let show_user: Vec<&str> = show_resp["bookmark"]["user_tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(show_user, vec!["original", "accepted"]);
}

#[test]
fn update_collections_clear_and_note_null() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/clear-test")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    let url = format!("{}/clear-test", server.url());
    let save_resp = save_bookmark(&home, &url, "Clear Test", &[]);
    let id = save_resp["id"].as_str().unwrap();

    // First set a note and collection
    update_bookmark(
        &home,
        id,
        json!({"note": "temp note", "collections": ["reading"]}),
    );

    // Now clear them
    let resp = update_bookmark(&home, id, json!({"note": null, "collections": []}));
    assert_eq!(resp["type"], "update_result");
    assert!(resp["bookmark"]["note"].is_null());
    assert!(resp["bookmark"]["collections"]
        .as_array()
        .unwrap()
        .is_empty());

    // Verify persistence
    let show_resp = show_bookmark(&home, id);
    assert!(show_resp["bookmark"]["note"].is_null());
    assert!(show_resp["bookmark"]["collections"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn update_missing_id_returns_error() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let resp = update_bookmark(&home, "am_nonexistent", json!({"note": "test"}));
    assert_eq!(resp["type"], "error");
    assert!(resp["message"].as_str().unwrap().contains("not found"));
}

#[test]
fn update_malformed_then_valid_continues_loop() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    // Send an update with invalid state, then a valid status request
    let stdin = frame_all(&[
        json!({"type": "update", "id": "am_bad", "changes": {"state": "deleted"}}),
        json!({"type": "status"}),
    ]);
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["type"], "error");
    assert_eq!(responses[1]["type"], "status_result");
}

#[test]
fn show_then_update_then_show_round_trip() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/roundtrip")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    let url = format!("{}/roundtrip", server.url());
    let save_resp = save_bookmark(&home, &url, "Roundtrip", &["init"]);
    let id = save_resp["id"].as_str().unwrap();

    // Show, update, show in one stream
    let stdin = frame_all(&[
        json!({"type": "show", "id": id}),
        json!({"type": "update", "id": id, "changes": {"note": "updated", "state": "processed"}}),
        json!({"type": "show", "id": id}),
    ]);
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["type"], "show_result");
    assert_eq!(responses[0]["bookmark"]["state"], "inbox");
    assert!(responses[0]["bookmark"]["note"].is_null());

    assert_eq!(responses[1]["type"], "update_result");
    assert_eq!(responses[1]["bookmark"]["state"], "processed");
    assert_eq!(responses[1]["bookmark"]["note"], "updated");

    assert_eq!(responses[2]["type"], "show_result");
    assert_eq!(responses[2]["bookmark"]["state"], "processed");
    assert_eq!(responses[2]["bookmark"]["note"], "updated");
}

// ── Other edge case tests ───────────────────────────────────────────

#[test]
fn list_with_invalid_state_returns_error() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let stdin = frame(&json!({"type": "list", "state": "deleted"}));
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["type"], "error");
}

// ── Fetch-failure fallback (login walls / bot blocking) ─────────────

/// LinkedIn answers HTTP 999 to non-browser clients. The extension sends a
/// snapshot of the live tab, which must be used instead of failing the save.
#[test]
fn save_uses_page_html_snapshot_when_fetch_is_blocked() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server.mock("GET", "/in/someone").with_status(999).create();

    let url = format!("{}/in/someone", server.url());
    let stdin = frame(&json!({
        "type": "save",
        "url": url,
        "title": "Tab Title",
        "page_html": sample_html()
    }));

    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["type"], "save_result", "{:?}", responses[0]);
    assert_eq!(responses[0]["status"], "created");

    let bundle_dirs = find_bundle_dirs(&storage);
    assert_eq!(bundle_dirs.len(), 1);
    let article = std::fs::read_to_string(bundle_dirs[0].join("article.md")).unwrap();
    assert!(article.contains("substantial test article"), "{article}");
    let source = std::fs::read_to_string(bundle_dirs[0].join("source.html")).unwrap();
    assert_eq!(source, sample_html());

    let conn = rusqlite::Connection::open(home.join(".agentmark/index.db")).unwrap();
    let (title, content_status): (String, String) = conn
        .query_row(
            "SELECT title, content_status FROM bookmarks LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(title, "Test Page Title");
    assert_eq!(content_status, "extracted");
}

#[test]
fn save_without_snapshot_still_saves_when_fetch_is_blocked() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let _mock = server.mock("GET", "/in/someone").with_status(999).create();

    let url = format!("{}/in/someone", server.url());
    let response = save_bookmark(&home, &url, "Someone | LinkedIn", &[]);
    assert_eq!(response["status"], "created");

    let conn = rusqlite::Connection::open(home.join(".agentmark/index.db")).unwrap();
    let (title, content_status): (String, String) = conn
        .query_row(
            "SELECT title, content_status FROM bookmarks LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(title, "Someone | LinkedIn");
    assert_eq!(content_status, "failed");
}

#[test]
fn blocked_resave_without_snapshot_keeps_existing_content() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let mut server = mockito::Server::new();
    let ok = server
        .mock("GET", "/article")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();
    let url = format!("{}/article", server.url());
    let first = save_bookmark(&home, &url, "Test", &[]);
    assert_eq!(first["status"], "created");

    ok.remove();
    let _blocked = server.mock("GET", "/article").with_status(999).create();
    let second = save_bookmark(&home, &url, "Test", &["later"]);
    assert_eq!(second["status"], "updated");
    assert_eq!(second["id"], first["id"]);

    let bundle_dirs = find_bundle_dirs(&storage);
    assert_eq!(bundle_dirs.len(), 1);
    let article = std::fs::read_to_string(bundle_dirs[0].join("article.md")).unwrap();
    assert!(article.contains("substantial test article"), "{article}");
}

// ── Storage folder change ───────────────────────────────────────────

/// After the storage folder changes, older bookmarks' files stay in the old
/// folder. Show and update say so plainly; saving the page again writes a
/// fresh bundle in the new folder under the same id, keeping user fields.
#[test]
fn storage_folder_change_resave_writes_fresh_bundle_and_show_explains() {
    let tmp = TempDir::new().unwrap();
    let old_storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &old_storage);

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();
    let url = format!("{}/article", server.url());
    let saved = save_bookmark(&home, &url, "T", &["keep-me"]);
    let id = saved["id"].as_str().unwrap().to_string();

    let new_storage = tmp.path().join("Other");
    std::fs::create_dir_all(&new_storage).unwrap();
    std::fs::write(
        home.join(".agentmark/config.toml"),
        format!(
            "default_agent = \"claude\"\nstorage_path = \"{}\"\n\n[enrichment]\nenabled = false\n",
            new_storage.display()
        ),
    )
    .unwrap();

    for reply in [
        show_bookmark(&home, &id),
        update_bookmark(&home, &id, json!({ "note": "n" })),
    ] {
        assert_eq!(reply["type"], "error", "{reply:?}");
        let message = reply["message"].as_str().unwrap();
        assert!(
            message.contains(&new_storage.display().to_string()),
            "{message}"
        );
        assert!(
            message.contains("folder AgentMark used before"),
            "{message}"
        );
        assert!(!message.contains("drift"), "{message}");
    }

    let resaved = save_bookmark(&home, &url, "T", &[]);
    assert_eq!(resaved["id"], id.as_str(), "{resaved:?}");
    let bundle_dirs = find_bundle_dirs(&new_storage);
    assert_eq!(bundle_dirs.len(), 1);
    let article = std::fs::read_to_string(bundle_dirs[0].join("article.md")).unwrap();
    assert!(article.contains("substantial test article"), "{article}");

    let shown = show_bookmark(&home, &id);
    assert_eq!(shown["type"], "show_result", "{shown:?}");
    assert_eq!(shown["bookmark"]["user_tags"], json!(["keep-me"]));
}

// ── Snapshot-preferred sites ────────────────────────────────────────

/// Like `setup_home`, with the mock server registered as a snapshot-preferred site.
fn setup_home_with_snapshot_site(
    tmp: &TempDir,
    storage_path: &Path,
    rule: &str,
) -> std::path::PathBuf {
    let home = setup_home(tmp, storage_path);
    let config = format!(
        "default_agent = \"claude\"\nstorage_path = \"{}\"\nsnapshot_sites = [\"{rule}\"]\n\n[enrichment]\nenabled = false\n",
        storage_path.display()
    );
    std::fs::write(home.join(".agentmark/config.toml"), config).unwrap();
    home
}

fn logged_in_snapshot() -> &'static str {
    r#"<!DOCTYPE html><html><head><title>Post | Feed</title></head><body><main>
<p>The logged-in view of the post, with every comment the reader expanded on the page.</p>
<p>A second paragraph so the snapshot has enough readable text to extract.</p>
</main></body></html>"#
}

fn public_page() -> &'static str {
    r#"<!DOCTYPE html><html><head><title>Public Post Title</title>
<link rel="canonical" href="https://example.com/posts/stable-permalink">
</head><body><p>Sign in to see more.</p><article><p>The public view of the post.</p></article></body></html>"#
}

fn save_with_snapshot(home: &Path, url: &str) -> serde_json::Value {
    let stdin = frame(&json!({
        "type": "save",
        "url": url,
        "title": "Tab Title",
        "page_html": logged_in_snapshot()
    }));
    let output = agentmark_cmd(home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let responses = decode_responses(&output);
    assert_eq!(responses[0]["type"], "save_result", "{:?}", responses[0]);
    responses[0].clone()
}

/// For a listed site the browser snapshot is the content even though the
/// fetch succeeds; the fetched page is kept beside it and supplies metadata.
#[test]
fn listed_site_saves_snapshot_and_keeps_fetched_page_as_supplement() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home_with_snapshot_site(&tmp, &storage, "127.0.0.1/posts/");

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/posts/a-post")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(public_page())
        .create();

    save_with_snapshot(&home, &format!("{}/posts/a-post", server.url()));

    let bundle_dirs = find_bundle_dirs(&storage);
    assert_eq!(bundle_dirs.len(), 1);
    assert_eq!(
        std::fs::read_to_string(bundle_dirs[0].join("source.html")).unwrap(),
        logged_in_snapshot()
    );
    assert_eq!(
        std::fs::read_to_string(bundle_dirs[0].join("fetched.html")).unwrap(),
        public_page()
    );
    let article = std::fs::read_to_string(bundle_dirs[0].join("article.md")).unwrap();
    assert!(article.contains("logged-in view"), "{article}");
    assert!(!article.contains("Sign in"), "{article}");

    let conn = rusqlite::Connection::open(home.join(".agentmark/index.db")).unwrap();
    let (title, canonical): (String, String) = conn
        .query_row(
            "SELECT title, canonical_url FROM bookmarks LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(title, "Public Post Title");
    assert!(
        canonical.contains("example.com/posts/stable-permalink"),
        "{canonical}"
    );
}

#[test]
fn listed_site_saves_snapshot_alone_when_fetch_is_blocked() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home_with_snapshot_site(&tmp, &storage, "127.0.0.1/in/");

    let mut server = mockito::Server::new();
    let _mock = server.mock("GET", "/in/someone").with_status(999).create();

    save_with_snapshot(&home, &format!("{}/in/someone", server.url()));

    let bundle_dirs = find_bundle_dirs(&storage);
    assert_eq!(bundle_dirs.len(), 1);
    assert_eq!(
        std::fs::read_to_string(bundle_dirs[0].join("source.html")).unwrap(),
        logged_in_snapshot()
    );
    assert!(!bundle_dirs[0].join("fetched.html").exists());
}

/// Sites that are not listed keep the fetched page; the snapshot is only a fallback.
#[test]
fn unlisted_site_ignores_snapshot_when_fetch_succeeds() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home_with_snapshot_site(&tmp, &storage, "127.0.0.1/posts/");

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();

    save_with_snapshot(&home, &format!("{}/article", server.url()));

    let bundle_dirs = find_bundle_dirs(&storage);
    assert_eq!(
        std::fs::read_to_string(bundle_dirs[0].join("source.html")).unwrap(),
        sample_html()
    );
    assert!(!bundle_dirs[0].join("fetched.html").exists());
}

/// A login wall answers the fetch with HTTP 200 on a different URL. Its title
/// and canonical URL must not leak into the bookmark, or every walled page
/// would be named after, and deduplicated into, the login page.
#[test]
fn listed_site_ignores_fetch_redirected_to_login_page() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home_with_snapshot_site(&tmp, &storage, "127.0.0.1/company/");

    let mut server = mockito::Server::new();
    let _redirect = server
        .mock("GET", "/company/acme/about")
        .with_status(302)
        .with_header("location", "/login")
        .create();
    let _login = server
        .mock("GET", "/login")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(
            r#"<html><head><title>Login, Sign in</title>
<link rel="canonical" href="https://example.com/login"></head><body><p>Sign in</p></body></html>"#,
        )
        .create();

    save_with_snapshot(&home, &format!("{}/company/acme/about", server.url()));

    let bundle_dirs = find_bundle_dirs(&storage);
    assert_eq!(bundle_dirs.len(), 1);
    assert!(!bundle_dirs[0].join("fetched.html").exists());

    let conn = rusqlite::Connection::open(home.join(".agentmark/index.db")).unwrap();
    let (title, canonical): (String, String) = conn
        .query_row(
            "SELECT title, canonical_url FROM bookmarks LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(title, "Post | Feed");
    assert!(canonical.contains("/company/acme/about"), "{canonical}");
    assert!(!canonical.contains("login"), "{canonical}");
}

/// Some single-page apps answer the CLI's fetch with HTTP 200 on the same URL
/// but serve a generic shell titled with the bare site name and no canonical
/// URL. That shell is not the page, so the snapshot's metadata must win.
#[test]
fn listed_site_ignores_app_shell_without_canonical() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home_with_snapshot_site(&tmp, &storage, "127.0.0.1/r/");

    let mut server = mockito::Server::new();
    let _shell = server
        .mock("GET", "/r/programming/comments/abc/a_post/")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(
            r#"<!DOCTYPE html><html><head><title>Reddit</title></head>
<body><div id="root"></div><script src="/app.js"></script></body></html>"#,
        )
        .create();

    save_with_snapshot(
        &home,
        &format!("{}/r/programming/comments/abc/a_post/", server.url()),
    );

    let bundle_dirs = find_bundle_dirs(&storage);
    assert_eq!(bundle_dirs.len(), 1);
    assert!(!bundle_dirs[0].join("fetched.html").exists());

    let conn = rusqlite::Connection::open(home.join(".agentmark/index.db")).unwrap();
    let title: String = conn
        .query_row("SELECT title FROM bookmarks LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(title, "Post | Feed");
}

/// When neither the snapshot nor a trusted fetch has a title, the tab title
/// the extension sent names the bookmark, not the app shell's site name.
#[test]
fn listed_site_uses_tab_title_over_app_shell_when_snapshot_has_no_title() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home_with_snapshot_site(&tmp, &storage, "127.0.0.1/r/");

    let mut server = mockito::Server::new();
    let _shell = server
        .mock("GET", "/r/x/comments/1/p/")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body("<html><head><title>Reddit</title></head><body></body></html>")
        .create();

    let stdin = frame(&json!({
        "type": "save",
        "url": format!("{}/r/x/comments/1/p/", server.url()),
        "title": "Tab Title",
        "page_html": "<html><body><main><p>The logged-in view of the post, with enough readable text to extract.</p><p>A second paragraph for the extractor.</p></main></body></html>"
    }));
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(decode_responses(&output)[0]["type"], "save_result");

    let conn = rusqlite::Connection::open(home.join(".agentmark/index.db")).unwrap();
    let title: String = conn
        .query_row("SELECT title FROM bookmarks LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(title, "Tab Title");
}

/// Re-saving a snapshot page is "unchanged" only if the whole page is the same,
/// not merely the region generic extraction happens to pick as the article.
#[test]
fn snapshot_resave_detects_changes_outside_the_extracted_article() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home_with_snapshot_site(&tmp, &storage, "127.0.0.1/in/");

    let mut server = mockito::Server::new();
    let _mock = server.mock("GET", "/in/someone").with_status(999).create();
    let url = format!("{}/in/someone", server.url());

    let save = |html: &str| -> serde_json::Value {
        let stdin = frame(&json!({ "type": "save", "url": url, "title": "T", "page_html": html }));
        let output = agentmark_cmd(&home)
            .arg("native-host")
            .write_stdin(stdin)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        decode_responses(&output)[0].clone()
    };
    let page = |sidebar: &str| {
        format!(
            r#"<html><head><title>Someone</title></head><body><article>
<p>This is the main article region with enough readable text for extraction to choose it.</p>
<p>It has a second paragraph so that the extractor is confident about the selection.</p>
</article><aside><ul><li>{sidebar}</li></ul></aside></body></html>"#
        )
    };

    assert_eq!(save(&page("Experience: Acme"))["status"], "created");
    assert_eq!(save(&page("Experience: Acme"))["status"], "updated");
    assert_eq!(
        save(&page("Experience: Globex"))["status"],
        "content_updated"
    );

    let bundle_dirs = find_bundle_dirs(&storage);
    assert_eq!(bundle_dirs.len(), 1);
    let source = std::fs::read_to_string(bundle_dirs[0].join("source.html")).unwrap();
    assert!(source.contains("Globex"), "{source}");
}

// ── Lookup URLs ─────────────────────────────────────────────────────

#[test]
fn lookup_urls_returns_verbatim_saved_urls() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    let conn = agentmark::db::open_and_migrate(&home.join(".agentmark/index.db")).unwrap();
    let mut bm = agentmark::models::Bookmark::new("https://example.com/post", "Saved Post");
    bm.canonical_url = "https://example.com/post".to_string();
    agentmark::db::BookmarkRepository::new(&conn)
        .insert(&bm)
        .unwrap();
    drop(conn);

    let tracked = "https://www.example.com/post?utm_source=x";
    let stdin = frame(&json!({
        "type": "lookup_urls",
        "urls": [
            "https://example.com/unsaved",
            tracked,
            "not a url",
            "https://example.com/post",
        ]
    }));
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(responses.len(), 1);
    assert_eq!(
        responses[0],
        json!({
            "type": "lookup_urls_result",
            "saved": [tracked, "https://example.com/post"]
        })
    );
}

#[test]
fn lookup_urls_migrates_an_unmigrated_index() {
    // The host may run before any CLI command has created or upgraded the
    // schema; handlers must migrate rather than fail on a missing table.
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);
    std::fs::write(home.join(".agentmark/index.db"), b"").unwrap();

    let stdin = frame(&json!({"type": "lookup_urls", "urls": ["https://example.com/x"]}));
    let output = agentmark_cmd(&home)
        .arg("native-host")
        .write_stdin(stdin)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let responses = decode_responses(&output);
    assert_eq!(
        responses,
        vec![json!({"type": "lookup_urls_result", "saved": []})]
    );
}

// ── Setup states ────────────────────────────────────────────────────

const EXTENSION_ID: &str = "abcdefghijklmnopabcdefghijklmnop";

/// A temp HOME with nothing in it (AgentMark never set up).
fn bare_home(tmp: &TempDir) -> std::path::PathBuf {
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    home
}

/// Run the host over `messages` and return the decoded responses. `path`
/// becomes the host's `PATH` (the folder dialog and agents are found on it).
fn run_host(
    home: &Path,
    path: Option<&Path>,
    messages: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    let mut cmd = agentmark_cmd(home);
    cmd.arg("native-host");
    if let Some(path) = path {
        cmd.env("PATH", path);
    }
    let output = cmd
        .write_stdin(frame_all(messages))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    decode_responses(&output)
}

fn setup_required_error() -> serde_json::Value {
    json!({
        "type": "error",
        "message": "not initialized: AgentMark is not set up yet. Finish setup in the extension or the AgentMark app.",
        "code": "setup_required"
    })
}

#[test]
fn before_setup_status_says_not_initialized_and_requests_need_setup() {
    let tmp = TempDir::new().unwrap();
    let home = bare_home(&tmp);

    let responses = run_host(
        &home,
        None,
        &[
            json!({"type": "status", "extension_version": "0.3.0"}),
            json!({"type": "save", "url": "https://example.com"}),
            json!({"type": "list"}),
            json!({"type": "list_collections"}),
            json!({"type": "show", "id": "am_1"}),
            json!({"type": "update", "id": "am_1", "changes": {}}),
            json!({"type": "lookup_urls", "urls": ["https://example.com"]}),
        ],
    );

    assert_eq!(responses[0]["type"], "status_result");
    assert_eq!(responses[0]["ok"], true);
    assert_eq!(responses[0]["initialized"], false);
    for response in &responses[1..] {
        assert_eq!(response, &setup_required_error());
    }
    // The binary's logger creates ~/.agentmark/logs; setup files must not appear.
    assert!(!home.join(".agentmark/config.toml").exists());
    assert!(!home.join(".agentmark/index.db").exists());
}

#[test]
fn after_setup_status_says_initialized_and_old_status_still_works() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);

    // An old extension sends a bare status and ignores the new key.
    let responses = run_host(&home, None, &[json!({"type": "status"})]);
    assert_eq!(
        responses[0],
        json!({"type": "status_result", "ok": true, "version": env!("CARGO_PKG_VERSION"), "initialized": true})
    );
}

#[test]
fn get_setup_state_before_setup_has_every_key() {
    let tmp = TempDir::new().unwrap();
    let home = bare_home(&tmp);

    let responses = run_host(&home, None, &[json!({"type": "get_setup_state"})]);
    assert_eq!(responses[0]["type"], "setup_state_result");
    let state = &responses[0]["state"];
    assert_eq!(state["initialized"], false);
    assert!(state["config_problem"].is_null());
    assert_eq!(state["version"], env!("CARGO_PKG_VERSION"));
    assert!(state["platform"]["os"].is_string());
    assert_eq!(
        state["default_storage_path_display"],
        "~/Documents/AgentMark"
    );
    assert_eq!(
        state["default_storage_path"],
        home.join("Documents/AgentMark").display().to_string()
    );
    assert_eq!(
        state["settings"]["storage_path_display"],
        "~/Documents/AgentMark"
    );
    assert!(state["settings"]["system_prompt"].is_null());
    assert_eq!(state["settings"]["snapshot_sites"], json!([]));
    let ids: Vec<_> = state["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, ["claude", "codex", "pi"]);
    assert_eq!(state["builtin_snapshot_sites"][0], "linkedin.com/in/");
    assert!(state["bookmark_count"].is_null());
    assert!(!home.join("Documents").exists());
}

#[test]
fn save_settings_field_errors_write_nothing() {
    let tmp = TempDir::new().unwrap();
    let home = bare_home(&tmp);

    let responses = run_host(
        &home,
        None,
        &[json!({"type": "save_settings", "settings": {
            "storage_path": "bookmarks",
            "default_agent": "gpt",
            "snapshot_sites": ["ok.com", "exa mple.com"]
        }})],
    );
    assert_eq!(
        responses[0],
        json!({"type": "settings_invalid", "errors": [
            {"field": "storage_path", "index": null, "code": "not_absolute",
             "message": "Use a full path such as ~/Documents/AgentMark, not \"bookmarks\"."},
            {"field": "default_agent", "index": null, "code": "invalid_agent",
             "message": "\"gpt\" is not a supported agent. Choose claude, codex or pi."},
            {"field": "snapshot_sites", "index": 1, "code": "invalid_site",
             "message": "\"exa mple.com\" is not a host/path rule."}
        ]})
    );
    assert!(!home.join(".agentmark/config.toml").exists());
    assert!(!home.join(".agentmark/index.db").exists());
}

#[test]
fn save_settings_reports_a_folder_that_is_not_writable() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new().unwrap();
    let home = bare_home(&tmp);
    let locked = tmp.path().join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(locked.join("probe"), "x").is_ok() {
        eprintln!("running as root — permissions not enforced, skipping");
        return;
    }

    let responses = run_host(
        &home,
        None,
        &[
            json!({"type": "save_settings", "settings": {"storage_path": locked.display().to_string()}}),
        ],
    );
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(responses[0]["type"], "settings_invalid");
    assert_eq!(responses[0]["errors"][0]["code"], "not_writable");
    assert!(!home.join(".agentmark/config.toml").exists());
}

#[test]
fn save_settings_wrong_type_is_a_parse_error() {
    let tmp = TempDir::new().unwrap();
    let home = bare_home(&tmp);
    let responses = run_host(
        &home,
        None,
        &[json!({"type": "save_settings", "settings": {"enrichment_enabled": "yes"}})],
    );
    assert_eq!(responses[0]["type"], "error");
    assert!(responses[0]["message"]
        .as_str()
        .unwrap()
        .starts_with("invalid message fields: "));
}

#[test]
fn save_settings_keeps_comments_and_unknown_keys() {
    let tmp = TempDir::new().unwrap();
    let storage = tmp.path().join("bookmarks");
    let home = setup_home(&tmp, &storage);
    let config_path = home.join(".agentmark/config.toml");
    let original = std::fs::read_to_string(&config_path).unwrap();
    std::fs::write(
        &config_path,
        format!("# written by hand\nfuture_key = \"kept\"\n{original}"),
    )
    .unwrap();

    let responses = run_host(
        &home,
        None,
        &[
            json!({"type": "save_settings", "settings": {"default_agent": "codex", "system_prompt": "Be brief."}}),
        ],
    );
    assert_eq!(responses[0]["type"], "save_settings_result");
    assert_eq!(responses[0]["state"]["settings"]["default_agent"], "codex");
    assert_eq!(
        responses[0]["state"]["settings"]["storage_path"],
        storage.display().to_string(),
        "absent keys keep their value"
    );
    let text = std::fs::read_to_string(&config_path).unwrap();
    assert!(text.starts_with("# written by hand\n"), "{text}");
    assert!(text.contains("future_key = \"kept\""));
    assert!(text.contains("default_agent = \"codex\""));
    assert!(text.contains("system_prompt = \"Be brief.\""));
}

// ── choose_folder (real dialog seam, fake dialog program) ───────────

/// The dialog program this OS uses, or `None` where there is no dialog.
fn dialog_program() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("osascript")
    } else if cfg!(target_os = "linux") {
        Some("zenity")
    } else {
        None
    }
}

/// A PATH holding only a fake dialog program running `body`.
fn fake_dialog_path(tmp: &TempDir, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = tmp.path().join("fake-bin");
    std::fs::create_dir_all(&dir).unwrap();
    let program = dir.join(dialog_program().unwrap());
    std::fs::write(&program, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

fn choose_folder(home: &Path, path: &Path, start: Option<&str>) -> serde_json::Value {
    let mut request = json!({"type": "choose_folder"});
    if let Some(start) = start {
        request["start_path"] = json!(start);
    }
    let mut cmd = agentmark_cmd(home);
    cmd.arg("native-host")
        .env("PATH", path)
        // zenity is only used when a display is available.
        .env("DISPLAY", ":0")
        .env_remove("WAYLAND_DISPLAY");
    let output = cmd
        .write_stdin(frame(&request))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    decode_responses(&output).remove(0)
}

#[test]
fn choose_folder_chosen_through_the_dialog_program() {
    if dialog_program().is_none() {
        return;
    }
    let tmp = TempDir::new().unwrap();
    let home = bare_home(&tmp);
    std::fs::create_dir_all(home.join("Documents")).unwrap();
    let args_log = tmp.path().join("dialog-args.txt");
    let path = fake_dialog_path(
        &tmp,
        &format!(
            "printf '%s\\n' \"$@\" > '{}'\necho '{}/Picked/'",
            args_log.display(),
            home.display()
        ),
    );

    let response = choose_folder(&home, &path, Some("~/Documents"));
    assert_eq!(
        response,
        json!({
            "type": "choose_folder_result",
            "outcome": "chosen",
            "path": home.join("Picked").display().to_string(),
            "path_display": "~/Picked",
            "reason": null
        })
    );
    // The expanded start folder reached the dialog.
    let args = std::fs::read_to_string(&args_log).unwrap();
    assert!(
        args.contains(&home.join("Documents").display().to_string()),
        "{args}"
    );
}

#[test]
fn choose_folder_cancelled_through_the_dialog_program() {
    if dialog_program().is_none() {
        return;
    }
    let tmp = TempDir::new().unwrap();
    let home = bare_home(&tmp);
    // osascript reports cancel as error -128; zenity exits 1.
    let body = if cfg!(target_os = "macos") {
        "echo 'execution error: User canceled. (-128)' >&2; exit 1"
    } else {
        "exit 1"
    };
    let path = fake_dialog_path(&tmp, body);
    let response = choose_folder(&home, &path, None);
    assert_eq!(
        response,
        json!({"type": "choose_folder_result", "outcome": "cancelled", "path": null, "path_display": null, "reason": null})
    );
}

#[test]
fn choose_folder_unavailable_without_a_dialog_program() {
    // The agent search path also appends these system directories.
    for dir in ["/opt/homebrew/bin", "/usr/local/bin"] {
        for program in ["osascript", "zenity", "kdialog"] {
            if Path::new(dir).join(program).exists() {
                eprintln!("{dir}/{program} exists — skipping");
                return;
            }
        }
    }
    let tmp = TempDir::new().unwrap();
    let home = bare_home(&tmp);
    let empty = tmp.path().join("empty-bin");
    std::fs::create_dir_all(&empty).unwrap();
    let response = choose_folder(&home, &empty, None);
    assert_eq!(response["type"], "choose_folder_result");
    assert_eq!(response["outcome"], "unavailable");
    assert!(response["path"].is_null());
    assert_eq!(
        response["reason"],
        "No folder dialog is available on this system."
    );
}

// ── End to end ──────────────────────────────────────────────────────

/// Drives the real binary the way Chrome does (origin as the only argument)
/// from a bare HOME: status → get_setup_state → save_settings → save →
/// lookup_urls, then checks the liveness file.
#[test]
fn first_run_end_to_end_over_the_native_protocol() {
    let tmp = TempDir::new().unwrap();
    let home = bare_home(&tmp);
    let storage = tmp.path().join("My Bookmarks");

    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(sample_html())
        .create();
    let url = format!("{}/article", server.url());

    let messages = [
        json!({"type": "status", "extension_version": "0.3.0"}),
        json!({"type": "get_setup_state"}),
        json!({"type": "save_settings", "settings": {
            "storage_path": storage.display().to_string(),
            "default_agent": "claude",
            "enrichment_enabled": false
        }}),
        json!({"type": "status", "extension_version": "0.3.0"}),
        json!({"type": "save", "url": url, "title": "Test Title"}),
        json!({"type": "lookup_urls", "urls": [url, "https://example.com/unsaved"]}),
    ];
    let output = Command::cargo_bin("agentmark")
        .unwrap()
        .arg(format!("chrome-extension://{EXTENSION_ID}/"))
        .env("HOME", &home)
        .write_stdin(frame_all(&messages))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let responses = decode_responses(&output);
    assert_eq!(responses.len(), messages.len());

    assert_eq!(responses[0]["type"], "status_result");
    assert_eq!(responses[0]["initialized"], false);

    assert_eq!(responses[1]["type"], "setup_state_result");
    assert_eq!(responses[1]["state"]["initialized"], false);

    assert_eq!(
        responses[2]["type"], "save_settings_result",
        "{}",
        responses[2]
    );
    let state = &responses[2]["state"];
    assert_eq!(state["initialized"], true);
    assert_eq!(
        state["settings"]["storage_path"],
        storage.display().to_string()
    );
    assert_eq!(state["bookmark_count"], 0);

    assert_eq!(responses[3]["initialized"], true);

    assert_eq!(responses[4]["type"], "save_result", "{}", responses[4]);
    assert_eq!(responses[4]["status"], "created");
    assert_eq!(find_bundle_dirs(&storage).len(), 1);

    assert_eq!(
        responses[5],
        json!({"type": "lookup_urls_result", "saved": [url]})
    );

    let liveness: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(home.join(".agentmark/extension-connection.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(liveness["extension_id"], EXTENSION_ID);
    assert_eq!(liveness["extension_version"], "0.3.0");
    assert_eq!(liveness["host_version"], env!("CARGO_PKG_VERSION"));
    assert!(liveness["connected_at"].as_str().unwrap().ends_with('Z'));
}
