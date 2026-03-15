//! Integration tests for `agentmark list`, `agentmark show`, `agentmark search`,
//! `agentmark tag`, `agentmark collections`, and `agentmark open`.
//!
//! Seeds DB rows and bundles directly through library APIs, then
//! executes the CLI with `assert_cmd` against a temp HOME.

use assert_cmd::Command;
use chrono::{TimeZone, Utc};
use std::path::PathBuf;
use tempfile::TempDir;

use agentmark::bundle::{BodySections, Bundle};
use agentmark::db::{self, BookmarkRepository};
use agentmark::fetch::PageMetadata;
use agentmark::models::{Bookmark, BookmarkState};

// ── Test infrastructure ─────────────────────────────────────────────

struct TestEnv {
    _tmp: TempDir,
    home: PathBuf,
    storage: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let storage = tmp.path().join("storage");
        let config_dir = home.join(".agentmark");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&storage).unwrap();

        let config_content = format!(
            "default_agent = \"claude\"\nstorage_path = \"{}\"\n\n[enrichment]\nenabled = false\n",
            storage.display()
        );
        std::fs::write(config_dir.join("config.toml"), config_content).unwrap();

        TestEnv {
            _tmp: tmp,
            home,
            storage,
        }
    }

    fn db_path(&self) -> PathBuf {
        self.home.join(".agentmark/index.db")
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("agentmark").unwrap();
        cmd.env("HOME", &self.home);
        cmd.env("NO_COLOR", "1"); // Deterministic output
        cmd
    }

    /// Insert a bookmark into the DB and create a bundle on disk.
    fn seed_bookmark(&self, bookmark: &Bookmark, article: &str, summary: Option<&str>) {
        let conn = db::open_and_migrate(&self.db_path()).unwrap();
        let repo = BookmarkRepository::new(&conn);
        repo.insert(bookmark).unwrap();

        let meta = PageMetadata {
            title: Some(bookmark.title.clone()),
            ..Default::default()
        };
        let bundle = Bundle::create(
            &self.storage,
            bookmark,
            &meta,
            article,
            "<html></html>",
            "cli",
        )
        .unwrap();

        if let Some(sum) = summary {
            let sections = BodySections {
                summary: Some(sum.to_string()),
                ..Default::default()
            };
            bundle.update_bookmark_md(bookmark, &sections).unwrap();
        }
    }
}

fn make_bookmark(id: &str, url: &str, title: &str, day: u32) -> Bookmark {
    let mut bm = Bookmark::new(url, title);
    bm.id = id.to_string();
    bm.saved_at = Utc.with_ymd_and_hms(2026, 3, day, 12, 0, 0).unwrap();
    bm
}

// ── List tests ──────────────────────────────────────────────────────

#[test]
fn list_reverse_chronological() {
    let env = TestEnv::new();
    let bm1 = make_bookmark("am_01AAA", "https://a.com", "Article A", 1);
    let bm2 = make_bookmark("am_02BBB", "https://b.com", "Article B", 5);
    let bm3 = make_bookmark("am_03CCC", "https://c.com", "Article C", 3);
    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);
    env.seed_bookmark(&bm3, "", None);

    let output = env.cmd().args(["list"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    // B (day 5) should come before C (day 3) before A (day 1)
    let pos_b = stdout.find("Article B").expect("should contain Article B");
    let pos_c = stdout.find("Article C").expect("should contain Article C");
    let pos_a = stdout.find("Article A").expect("should contain Article A");
    assert!(pos_b < pos_c, "B should appear before C");
    assert!(pos_c < pos_a, "C should appear before A");
}

#[test]
fn list_no_bookmarks_friendly_message() {
    let env = TestEnv::new();
    // Ensure DB exists
    let conn = db::open_and_migrate(&env.db_path()).unwrap();
    drop(conn);

    let output = env.cmd().args(["list"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No bookmarks found"));
}

#[test]
fn list_with_collection_filter() {
    let env = TestEnv::new();
    let mut bm1 = make_bookmark("am_01AAA", "https://a.com", "In Dev", 1);
    bm1.collections = vec!["dev".to_string()];
    let bm2 = make_bookmark("am_02BBB", "https://b.com", "No Collection", 2);
    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);

    let output = env
        .cmd()
        .args(["list", "--collection", "dev"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("In Dev"));
    assert!(!stdout.contains("No Collection"));
}

#[test]
fn list_with_tag_filter_user_tags() {
    let env = TestEnv::new();
    let mut bm1 = make_bookmark("am_01AAA", "https://a.com", "Rust Article", 1);
    bm1.user_tags = vec!["rust".to_string()];
    let bm2 = make_bookmark("am_02BBB", "https://b.com", "Python Article", 2);
    let _ = &bm2; // no tags
    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);

    let output = env.cmd().args(["list", "--tag", "rust"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Rust Article"));
    assert!(!stdout.contains("Python Article"));
}

#[test]
fn list_with_tag_filter_suggested_tags() {
    let env = TestEnv::new();
    let mut bm1 = make_bookmark("am_01AAA", "https://a.com", "AI Article", 1);
    bm1.suggested_tags = vec!["ai".to_string()];
    let bm2 = make_bookmark("am_02BBB", "https://b.com", "Other", 2);
    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);

    let output = env.cmd().args(["list", "--tag", "ai"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("AI Article"));
    assert!(!stdout.contains("Other"));
}

#[test]
fn list_with_state_filter() {
    let env = TestEnv::new();
    let bm1 = make_bookmark("am_01AAA", "https://a.com", "Inbox Item", 1);
    let mut bm2 = make_bookmark("am_02BBB", "https://b.com", "Archived Item", 2);
    bm2.state = BookmarkState::Archived;
    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);

    let output = env
        .cmd()
        .args(["list", "--state", "archived"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Archived Item"));
    assert!(!stdout.contains("Inbox Item"));
}

#[test]
fn list_with_limit() {
    let env = TestEnv::new();
    for i in 0..5 {
        let bm = make_bookmark(
            &format!("am_0{i}XXX"),
            &format!("https://{i}.com"),
            &format!("Article {i}"),
            i + 1,
        );
        env.seed_bookmark(&bm, "", None);
    }

    let output = env.cmd().args(["list", "--limit", "2"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2);
}

#[test]
fn list_limit_zero() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_01AAA", "https://a.com", "Article", 1);
    env.seed_bookmark(&bm, "", None);

    let output = env.cmd().args(["list", "--limit", "0"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No bookmarks found"));
}

#[test]
fn list_combined_filters() {
    let env = TestEnv::new();

    let mut bm1 = make_bookmark("am_01AAA", "https://a.com", "Match Both", 1);
    bm1.user_tags = vec!["rust".to_string()];
    bm1.state = BookmarkState::Processed;

    let mut bm2 = make_bookmark("am_02BBB", "https://b.com", "Match Tag Only", 2);
    bm2.user_tags = vec!["rust".to_string()];
    bm2.state = BookmarkState::Inbox;

    let mut bm3 = make_bookmark("am_03CCC", "https://c.com", "Match State Only", 3);
    bm3.state = BookmarkState::Processed;

    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);
    env.seed_bookmark(&bm3, "", None);

    let output = env
        .cmd()
        .args(["list", "--tag", "rust", "--state", "processed"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Match Both"));
    assert!(!stdout.contains("Match Tag Only"));
    assert!(!stdout.contains("Match State Only"));
}

#[test]
fn list_shows_tags_in_output() {
    let env = TestEnv::new();
    let mut bm = make_bookmark("am_01AAA", "https://a.com", "Tagged Article", 1);
    bm.user_tags = vec!["rust".to_string(), "cli".to_string()];
    env.seed_bookmark(&bm, "", None);

    let output = env.cmd().args(["list"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[rust, cli]"));
}

#[test]
fn list_shows_state_in_output() {
    let env = TestEnv::new();
    let mut bm = make_bookmark("am_01AAA", "https://a.com", "Processed Article", 1);
    bm.state = BookmarkState::Processed;
    env.seed_bookmark(&bm, "", None);

    let output = env.cmd().args(["list"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("processed"));
}

// ── Show tests ──────────────────────────────────────────────────────

#[test]
fn show_displays_bookmark_details() {
    let env = TestEnv::new();
    let mut bm = make_bookmark("am_01AAA", "https://example.com/article", "Test Article", 5);
    bm.description = Some("A great article".to_string());
    bm.author = Some("Author Name".to_string());
    bm.site_name = Some("Example.com".to_string());
    bm.user_tags = vec!["rust".to_string()];
    bm.collections = vec!["dev".to_string()];
    bm.note = Some("My note".to_string());
    env.seed_bookmark(&bm, "Article content here.", Some("A nice summary."));

    let output = env.cmd().args(["show", "am_01AAA"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("Test Article"));
    assert!(stdout.contains("am_01AAA"));
    assert!(stdout.contains("https://example.com/article"));
    assert!(stdout.contains("Author Name"));
    assert!(stdout.contains("Example.com"));
    assert!(stdout.contains("rust"));
    assert!(stdout.contains("dev"));
    assert!(stdout.contains("My note"));
    assert!(stdout.contains("A nice summary."));
    assert!(stdout.contains("Article content here."));
}

#[test]
fn show_full_includes_all_article() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_01AAA", "https://a.com", "Long Article", 1);
    let article = (0..30)
        .map(|i| format!("Line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    env.seed_bookmark(&bm, &article, None);

    // Default (no --full) should truncate
    let output = env.cmd().args(["show", "am_01AAA"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("more lines"));

    // --full should include everything
    let output = env
        .cmd()
        .args(["show", "am_01AAA", "--full"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Line 29"));
    assert!(!stdout.contains("more lines"));
}

#[test]
fn show_pending_summary() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_01AAA", "https://a.com", "No Summary", 1);
    env.seed_bookmark(&bm, "some article", None); // No summary

    let output = env.cmd().args(["show", "am_01AAA"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[enrichment pending]"));
}

#[test]
fn show_empty_article() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_01AAA", "https://a.com", "Empty Article", 1);
    env.seed_bookmark(&bm, "", None);

    let output = env.cmd().args(["show", "am_01AAA"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[no article content]"));
}

#[test]
fn show_missing_bookmark_fails() {
    let env = TestEnv::new();
    let conn = db::open_and_migrate(&env.db_path()).unwrap();
    drop(conn);

    env.cmd()
        .args(["show", "am_NONEXISTENT"])
        .assert()
        .failure();
}

#[test]
fn show_missing_bundle_fails_with_drift_error() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_01AAA", "https://a.com", "Drifted", 1);

    // Insert into DB but don't create bundle
    let conn = db::open_and_migrate(&env.db_path()).unwrap();
    let repo = BookmarkRepository::new(&conn);
    repo.insert(&bm).unwrap();
    drop(conn);

    let output = env.cmd().args(["show", "am_01AAA"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("drift") || stderr.contains("not found"),
        "should mention drift or not found: {stderr}"
    );
}

// ── Search tests ─────────────────────────────────────────────────────

#[test]
fn search_finds_by_title() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_01AAA", "https://a.com", "Quantum Computing Basics", 1);
    env.seed_bookmark(&bm, "", None);

    let output = env.cmd().args(["search", "quantum"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("Quantum Computing Basics"));
}

#[test]
fn search_finds_by_note() {
    let env = TestEnv::new();
    let mut bm = make_bookmark("am_01AAA", "https://a.com", "Generic Title", 1);
    bm.note = Some("fascinating deep learning research".to_string());
    env.seed_bookmark(&bm, "", None);

    let output = env.cmd().args(["search", "fascinating"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("Generic Title"));
}

#[test]
fn search_finds_by_user_tag() {
    let env = TestEnv::new();
    let mut bm = make_bookmark("am_01AAA", "https://a.com", "Some Article", 1);
    bm.user_tags = vec!["rustlang".to_string()];
    let bm2 = make_bookmark("am_02BBB", "https://b.com", "Other Article", 2);
    env.seed_bookmark(&bm, "", None);
    env.seed_bookmark(&bm2, "", None);

    let output = env.cmd().args(["search", "rustlang"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Some Article"));
    assert!(!stdout.contains("Other Article"));
}

#[test]
fn search_finds_by_suggested_tag() {
    let env = TestEnv::new();
    let mut bm = make_bookmark("am_01AAA", "https://a.com", "Tagged Article", 1);
    bm.suggested_tags = vec!["machinelearning".to_string()];
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .args(["search", "machinelearning"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Tagged Article"));
}

#[test]
fn search_scopes_by_collection() {
    let env = TestEnv::new();
    let mut bm1 = make_bookmark("am_01AAA", "https://a.com", "Quantum In Dev", 1);
    bm1.collections = vec!["dev".to_string()];
    let mut bm2 = make_bookmark("am_02BBB", "https://b.com", "Quantum In Research", 2);
    bm2.collections = vec!["research".to_string()];
    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);

    let output = env
        .cmd()
        .args(["search", "quantum", "--collection", "dev"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Quantum In Dev"));
    assert!(!stdout.contains("Quantum In Research"));
}

#[test]
fn search_respects_limit() {
    let env = TestEnv::new();
    for i in 0..5 {
        let bm = make_bookmark(
            &format!("am_0{i}XXX"),
            &format!("https://{i}.com"),
            &format!("Quantum Article {i}"),
            i + 1,
        );
        env.seed_bookmark(&bm, "", None);
    }

    let output = env
        .cmd()
        .args(["search", "quantum", "--limit", "2"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2);
}

#[test]
fn search_no_results_message() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_01AAA", "https://a.com", "Unrelated Article", 1);
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .args(["search", "nonexistentkeyword"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("No results found"));
}

#[test]
fn search_empty_db_no_results() {
    let env = TestEnv::new();
    // Ensure DB exists
    let conn = db::open_and_migrate(&env.db_path()).unwrap();
    drop(conn);

    let output = env.cmd().args(["search", "anything"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("No results found"));
}

#[test]
fn search_limit_zero_no_results() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_01AAA", "https://a.com", "Quantum Article", 1);
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .args(["search", "quantum", "--limit", "0"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("No results found"));
}

#[test]
fn search_whitespace_query_no_results() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_01AAA", "https://a.com", "Quantum Article", 1);
    env.seed_bookmark(&bm, "", None);

    let output = env.cmd().args(["search", "   "]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("No results found"));
}

#[test]
fn search_quoted_phrase() {
    let env = TestEnv::new();
    let bm1 = make_bookmark("am_01AAA", "https://a.com", "Deep Learning Fundamentals", 1);
    let bm2 = make_bookmark(
        "am_02BBB",
        "https://b.com",
        "Deep Sea Fishing and Learning",
        2,
    );
    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);

    let output = env
        .cmd()
        .args(["search", "\"deep learning\""])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Deep Learning Fundamentals"));
    // The phrase "deep learning" doesn't appear adjacent in bm2's title
    assert!(!stdout.contains("Deep Sea Fishing"));
}

#[test]
fn search_boolean_and() {
    let env = TestEnv::new();
    let bm1 = make_bookmark("am_01AAA", "https://a.com", "Rust Programming Guide", 1);
    let bm2 = make_bookmark("am_02BBB", "https://b.com", "Python Programming Guide", 2);
    let bm3 = make_bookmark("am_03CCC", "https://c.com", "Rust Cooking Recipes", 3);
    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);
    env.seed_bookmark(&bm3, "", None);

    let output = env
        .cmd()
        .args(["search", "rust AND programming"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Rust Programming Guide"));
    assert!(!stdout.contains("Python Programming"));
    assert!(!stdout.contains("Rust Cooking"));
}

#[test]
fn search_relevance_ordering() {
    let env = TestEnv::new();
    // bm1: "quantum" in title only — strong match
    let bm1 = make_bookmark(
        "am_01AAA",
        "https://a.com",
        "Quantum Computing Revolution",
        1,
    );
    // bm2: "quantum" in note only — weaker match
    let mut bm2 = make_bookmark("am_02BBB", "https://b.com", "Generic Tech Article", 2);
    bm2.note = Some("mentions quantum briefly".to_string());
    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);

    let output = env.cmd().args(["search", "quantum"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let pos_strong = stdout
        .find("Quantum Computing Revolution")
        .expect("should find strong match");
    let pos_weak = stdout
        .find("Generic Tech Article")
        .expect("should find weak match");
    assert!(
        pos_strong < pos_weak,
        "Title match should rank above note-only match"
    );
}

#[test]
fn search_malformed_fts_fails() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_01AAA", "https://a.com", "Article", 1);
    env.seed_bookmark(&bm, "", None);

    let output = env.cmd().args(["search", "AND OR NOT"]).output().unwrap();
    assert!(!output.status.success(), "Malformed FTS syntax should fail");
}

#[test]
fn search_without_config_fails_with_guidance() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    // No .agentmark dir or config

    let output = Command::cargo_bin("agentmark")
        .unwrap()
        .env("HOME", &home)
        .env("NO_COLOR", "1")
        .args(["search", "anything"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("init") || stderr.contains("config"),
        "Should mention init or config: {stderr}"
    );
}

#[test]
fn search_collection_with_wildcard_chars() {
    let env = TestEnv::new();
    let mut bm1 = make_bookmark("am_01AAA", "https://a.com", "Quantum Special", 1);
    bm1.collections = vec!["my_collection%".to_string()];
    let mut bm2 = make_bookmark("am_02BBB", "https://b.com", "Quantum Normal", 2);
    bm2.collections = vec!["other".to_string()];
    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);

    let output = env
        .cmd()
        .args(["search", "quantum", "--collection", "my_collection%"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Quantum Special"));
    assert!(!stdout.contains("Quantum Normal"));
}

#[test]
fn search_uses_list_format() {
    let env = TestEnv::new();
    let mut bm = make_bookmark("am_01AAA", "https://a.com", "Quantum Tagged", 1);
    bm.user_tags = vec!["physics".to_string()];
    bm.state = BookmarkState::Processed;
    env.seed_bookmark(&bm, "", None);

    let output = env.cmd().args(["search", "quantum"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Should contain same fields as list output: date, state, title, tags
    assert!(stdout.contains("2026-03-01"));
    assert!(stdout.contains("processed"));
    assert!(stdout.contains("Quantum Tagged"));
    assert!(stdout.contains("[physics]"));
}

// ── Tag tests ───────────────────────────────────────────────────────

/// Helper: read a bookmark row from the DB by ID.
fn get_bookmark_from_db(env: &TestEnv, id: &str) -> Bookmark {
    let conn = db::open_and_migrate(&env.db_path()).unwrap();
    let repo = BookmarkRepository::new(&conn);
    repo.get_by_id(id).unwrap().expect("bookmark should exist")
}

/// Helper: read the user_tags from bookmark.md front matter in a bundle.
fn read_bundle_user_tags(env: &TestEnv, bookmark: &Bookmark) -> Vec<String> {
    let bundle = Bundle::find(&env.storage, &bookmark.saved_at, &bookmark.id).unwrap();
    let content = std::fs::read_to_string(bundle.path().join("bookmark.md")).unwrap();
    // Parse YAML front matter
    let yaml_start = content.find("---\n").unwrap() + 4;
    let yaml_end = content[yaml_start..].find("\n---\n").unwrap() + yaml_start;
    let yaml = &content[yaml_start..yaml_end + 1];
    let parsed = Bookmark::from_yaml_str(yaml).unwrap();
    parsed.user_tags
}

#[test]
fn tag_adds_new_tags() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_TAG01", "https://a.com", "Tag Test", 1);
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .args(["tag", "am_TAG01", "rust", "cli"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("rust"));
    assert!(stdout.contains("cli"));

    // Verify DB state
    let updated = get_bookmark_from_db(&env, "am_TAG01");
    assert_eq!(updated.user_tags, vec!["rust", "cli"]);

    // Verify bundle state
    let bundle_tags = read_bundle_user_tags(&env, &updated);
    assert_eq!(bundle_tags, vec!["rust", "cli"]);
}

#[test]
fn tag_deduplicates_existing_tags() {
    let env = TestEnv::new();
    let mut bm = make_bookmark("am_TAG02", "https://b.com", "Dedup Test", 2);
    bm.user_tags = vec!["rust".to_string()];
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .args(["tag", "am_TAG02", "rust", "new-tag"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let updated = get_bookmark_from_db(&env, "am_TAG02");
    assert_eq!(updated.user_tags, vec!["rust", "new-tag"]);
}

#[test]
fn tag_deduplicates_repeated_input() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_TAG03", "https://c.com", "Repeat Input", 3);
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .args(["tag", "am_TAG03", "rust", "rust", "cli"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let updated = get_bookmark_from_db(&env, "am_TAG03");
    assert_eq!(updated.user_tags, vec!["rust", "cli"]);
}

#[test]
fn tag_remove_removes_tags() {
    let env = TestEnv::new();
    let mut bm = make_bookmark("am_TAG04", "https://d.com", "Remove Test", 4);
    bm.user_tags = vec!["rust".to_string(), "cli".to_string(), "tools".to_string()];
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .args(["tag", "am_TAG04", "--remove", "cli"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let updated = get_bookmark_from_db(&env, "am_TAG04");
    assert_eq!(updated.user_tags, vec!["rust", "tools"]);

    let bundle_tags = read_bundle_user_tags(&env, &updated);
    assert_eq!(bundle_tags, vec!["rust", "tools"]);
}

#[test]
fn tag_remove_absent_is_idempotent() {
    let env = TestEnv::new();
    let mut bm = make_bookmark("am_TAG05", "https://e.com", "Idempotent Remove", 5);
    bm.user_tags = vec!["rust".to_string()];
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .args(["tag", "am_TAG05", "--remove", "nonexistent"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let updated = get_bookmark_from_db(&env, "am_TAG05");
    assert_eq!(updated.user_tags, vec!["rust"]);
}

#[test]
fn tag_remove_all_results_in_empty() {
    let env = TestEnv::new();
    let mut bm = make_bookmark("am_TAG06", "https://f.com", "Remove All", 6);
    bm.user_tags = vec!["rust".to_string(), "cli".to_string()];
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .args(["tag", "am_TAG06", "--remove", "rust", "cli"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("(none)"));

    let updated = get_bookmark_from_db(&env, "am_TAG06");
    assert!(updated.user_tags.is_empty());
}

#[test]
fn tag_invalid_id_fails() {
    let env = TestEnv::new();
    // Seed DB so config exists
    let bm = make_bookmark("am_TAG07", "https://g.com", "Exists", 7);
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .args(["tag", "am_NONEXISTENT", "rust"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"));
}

#[test]
fn tag_mixed_add_and_remove_rejected() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_TAGMIX", "https://g.com", "Mixed", 8);
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .args(["tag", "am_TAGMIX", "rust", "--remove", "old-tag"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "tag with both positional tags and --remove should fail"
    );
}

#[test]
fn tag_without_config_fails_with_guidance() {
    // Use a fresh temp dir with no config
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    let output = Command::cargo_bin("agentmark")
        .unwrap()
        .env("HOME", &home)
        .env("NO_COLOR", "1")
        .args(["tag", "am_SOME", "rust"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn tag_preserves_enriched_body() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_TAG08", "https://h.com", "Body Preserve", 8);
    env.seed_bookmark(&bm, "", Some("Enriched summary content"));

    let output = env
        .cmd()
        .args(["tag", "am_TAG08", "new-tag"])
        .output()
        .unwrap();
    assert!(output.status.success());

    // Verify enriched summary is preserved
    let updated = get_bookmark_from_db(&env, "am_TAG08");
    let bundle = Bundle::find(&env.storage, &updated.saved_at, &updated.id).unwrap();
    let sections = bundle.read_body_sections().unwrap();
    assert_eq!(
        sections.summary.as_deref(),
        Some("Enriched summary content")
    );
}

// ── Collections tests ───────────────────────────────────────────────

#[test]
fn collections_lists_with_counts() {
    let env = TestEnv::new();
    let mut bm1 = make_bookmark("am_COL01", "https://a.com", "Col A", 1);
    bm1.collections = vec!["tech".to_string(), "rust".to_string()];
    let mut bm2 = make_bookmark("am_COL02", "https://b.com", "Col B", 2);
    bm2.collections = vec!["tech".to_string()];
    let mut bm3 = make_bookmark("am_COL03", "https://c.com", "Col C", 3);
    bm3.collections = vec!["rust".to_string(), "news".to_string()];
    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);
    env.seed_bookmark(&bm3, "", None);

    let output = env.cmd().args(["collections"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Alphabetical order
    assert!(stdout.contains("news  (1 bookmarks)"));
    assert!(stdout.contains("rust  (2 bookmarks)"));
    assert!(stdout.contains("tech  (2 bookmarks)"));
}

#[test]
fn collections_empty_db() {
    let env = TestEnv::new();
    // Need to seed at least initialize the DB
    let conn = db::open_and_migrate(&env.db_path()).unwrap();
    drop(conn);

    let output = env.cmd().args(["collections"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No collections found."));
}

#[test]
fn collections_ignores_empty_collection_arrays() {
    let env = TestEnv::new();
    let bm1 = make_bookmark("am_COL04", "https://a.com", "No Coll", 1);
    let mut bm2 = make_bookmark("am_COL05", "https://b.com", "Has Coll", 2);
    bm2.collections = vec!["tech".to_string()];
    env.seed_bookmark(&bm1, "", None);
    env.seed_bookmark(&bm2, "", None);

    let output = env.cmd().args(["collections"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("tech  (1 bookmarks)"));
    // Should only have one line of output
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1);
}

#[test]
fn collections_names_with_spaces() {
    let env = TestEnv::new();
    let mut bm = make_bookmark("am_COL06", "https://a.com", "Space Coll", 1);
    bm.collections = vec!["my projects".to_string()];
    env.seed_bookmark(&bm, "", None);

    let output = env.cmd().args(["collections"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("my projects  (1 bookmarks)"));
}

// ── Open tests ──────────────────────────────────────────────────────

#[test]
fn open_valid_id_launches_opener() {
    let env = TestEnv::new();
    let bm = make_bookmark(
        "am_OPEN01",
        "https://example.com/article?q=1&r=2#frag",
        "Open Test",
        1,
    );
    env.seed_bookmark(&bm, "", None);

    // Create a fake opener script that writes the URL to a file
    let url_log = env.home.join("opened_url.txt");
    let script_path = env.home.join("fake_opener.sh");
    std::fs::write(
        &script_path,
        format!("#!/bin/sh\necho \"$1\" > {:?}\n", url_log),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let output = env
        .cmd()
        .env("AGENTMARK_OPENER", script_path.to_str().unwrap())
        .args(["open", "am_OPEN01"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Opened"));
    assert!(stdout.contains("example.com"));

    // Verify the URL was passed correctly (including query/fragment)
    let logged = std::fs::read_to_string(&url_log).unwrap();
    assert_eq!(logged.trim(), "https://example.com/article?q=1&r=2#frag");
}

#[test]
fn open_invalid_id_fails() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_OPEN02", "https://a.com", "Exists", 1);
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .env("AGENTMARK_OPENER", "true") // won't be reached
        .args(["open", "am_NONEXISTENT"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"));
}

#[test]
fn open_launcher_failure_reports_error() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_OPEN03", "https://a.com", "Fail Open", 1);
    env.seed_bookmark(&bm, "", None);

    // Use `false` as the opener — it always exits non-zero
    let output = env
        .cmd()
        .env("AGENTMARK_OPENER", "false")
        .args(["open", "am_OPEN03"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("failed to open"));
}

#[test]
fn open_missing_opener_reports_error() {
    let env = TestEnv::new();
    let bm = make_bookmark("am_OPEN04", "https://a.com", "Missing Opener", 1);
    env.seed_bookmark(&bm, "", None);

    let output = env
        .cmd()
        .env("AGENTMARK_OPENER", "/nonexistent/opener/binary")
        .args(["open", "am_OPEN04"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("failed to open"));
}
