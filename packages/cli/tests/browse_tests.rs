//! Integration tests for `agentmark list` and `agentmark show`.
//!
//! Seeds DB rows and bundles directly through library APIs, then
//! executes the CLI with `assert_cmd` against a temp HOME.

use assert_cmd::Command;
use chrono::{TimeZone, Utc};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use agentmark::bundle::{BodySections, Bundle};
use agentmark::db::{self, BookmarkRepository};
use agentmark::fetch::PageMetadata;
use agentmark::models::{Bookmark, BookmarkState, ContentStatus, SummaryStatus};

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
    bm2.user_tags.clone(); // no tags
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
