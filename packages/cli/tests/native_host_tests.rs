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
    std::fs::write(config_dir.join("index.db"), b"").unwrap();

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
