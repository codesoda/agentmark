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
