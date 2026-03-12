use assert_cmd::Command;
use chrono::{TimeZone, Utc};
use std::path::PathBuf;
use tempfile::TempDir;

use agentmark::bundle::{BodySections, Bundle};
use agentmark::db::{self, BookmarkRepository};
use agentmark::fetch::PageMetadata;
use agentmark::models::{Bookmark, BookmarkEvent, ContentStatus, SummaryStatus};

// ── Test environment ────────────────────────────────────────────────

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
        cmd.env("NO_COLOR", "1");
        cmd
    }

    fn seed_bookmark(&self, bookmark: &Bookmark, article: &str) {
        self.seed_bookmark_with_html(bookmark, article, "<html><body>original</body></html>");
    }

    fn seed_bookmark_with_html(&self, bookmark: &Bookmark, article: &str, html: &str) {
        let conn = db::open_and_migrate(&self.db_path()).unwrap();
        let repo = BookmarkRepository::new(&conn);
        repo.insert(bookmark).unwrap();

        let meta = PageMetadata {
            title: Some(bookmark.title.clone()),
            description: bookmark.description.clone(),
            author: bookmark.author.clone(),
            site_name: bookmark.site_name.clone(),
            ..Default::default()
        };
        let bundle = Bundle::create(&self.storage, bookmark, &meta, article, html, "cli").unwrap();

        // Write initial summary if bookmark has enriched status
        if bookmark.summary_status == SummaryStatus::Done {
            let sections = BodySections {
                summary: Some("Original enriched summary.".to_string()),
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
    bm.content_status = ContentStatus::Extracted;
    bm.content_hash = Some("sha256:original_hash".to_string());
    bm
}

fn get_bookmark_from_db(env: &TestEnv, id: &str) -> Bookmark {
    let conn = db::open_and_migrate(&env.db_path()).unwrap();
    let repo = BookmarkRepository::new(&conn);
    repo.get_by_id(id).unwrap().expect("bookmark should exist")
}

fn read_events(env: &TestEnv, bookmark: &Bookmark) -> Vec<BookmarkEvent> {
    let bundle = Bundle::find(&env.storage, &bookmark.saved_at, &bookmark.id).unwrap();
    let content = std::fs::read_to_string(bundle.path().join("events.jsonl")).unwrap();
    content
        .lines()
        .map(|l| BookmarkEvent::from_json_line(l).unwrap())
        .collect()
}

fn sample_html(title: &str, body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>{title}</title>
    <meta name="description" content="A test description">
    <meta name="author" content="Test Author">
</head>
<body>
<article>
<h1>{title}</h1>
<p>{body}</p>
<p>Second paragraph with additional meaningful content for extraction thresholds.</p>
<p>Third paragraph ensuring we have enough content to pass readability checks.</p>
</article>
</body>
</html>"#
    )
}

// ── Single bookmark reprocess ───────────────────────────────────────

#[test]
fn reprocess_updates_article_when_content_changed() {
    let env = TestEnv::new();
    let mut server = mockito::Server::new();
    let url = format!("{}/article", server.url());

    let bm = make_bookmark("am_reprocess1", &url, "Original Title", 5);
    env.seed_bookmark(&bm, "# Original Article\n\nOriginal content.");

    // Mock returns new content
    let new_html = sample_html(
        "Updated Title",
        "Completely new article content that differs from original.",
    );
    let _mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(&new_html)
        .create();

    let output = env
        .cmd()
        .args(["reprocess", "am_reprocess1"])
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(output.status.success(), "stderr: {stderr}");
    assert!(stderr.contains("content updated"), "stderr: {stderr}");

    // Verify article.md was updated
    let bundle = Bundle::find(&env.storage, &bm.saved_at, &bm.id).unwrap();
    let article = std::fs::read_to_string(bundle.path().join("article.md")).unwrap();
    assert!(
        article.contains("new article content"),
        "article should have new content"
    );

    // Verify DB row updated
    let db_bm = get_bookmark_from_db(&env, "am_reprocess1");
    assert_eq!(db_bm.title, "Updated Title");
    assert_ne!(db_bm.content_hash.as_deref(), Some("sha256:original_hash"));

    // Verify saved_at and id preserved
    assert_eq!(db_bm.id, "am_reprocess1");
    assert_eq!(db_bm.saved_at, bm.saved_at);
}

#[test]
fn reprocess_preserves_bundle_path_and_id() {
    let env = TestEnv::new();
    let mut server = mockito::Server::new();
    let url = format!("{}/article", server.url());

    let bm = make_bookmark("am_preserve1", &url, "Test Title", 6);
    env.seed_bookmark(&bm, "# Original\n\nContent.");

    let bundle_before = Bundle::find(&env.storage, &bm.saved_at, &bm.id).unwrap();
    let path_before = bundle_before.path().to_path_buf();

    let new_html = sample_html("Test Title", "Same enough content.");
    let _mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(&new_html)
        .create();

    let output = env
        .cmd()
        .args(["reprocess", "am_preserve1"])
        .output()
        .unwrap();
    assert!(output.status.success());

    // Bundle path should still exist at same location
    let bundle_after = Bundle::find(&env.storage, &bm.saved_at, &bm.id).unwrap();
    assert_eq!(path_before, bundle_after.path());
}

#[test]
fn reprocess_appends_reprocessed_event() {
    let env = TestEnv::new();
    let mut server = mockito::Server::new();
    let url = format!("{}/article", server.url());

    let bm = make_bookmark("am_event1", &url, "Event Test", 7);
    env.seed_bookmark(&bm, "# Article\n\nContent.");

    let new_html = sample_html("Event Test", "Same content with slight variation.");
    let _mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(&new_html)
        .create();

    let output = env.cmd().args(["reprocess", "am_event1"]).output().unwrap();
    assert!(output.status.success());

    let events = read_events(&env, &bm);
    let last_event = events.last().unwrap();
    assert_eq!(
        last_event.event_type,
        agentmark::models::EventType::Reprocessed
    );
    assert!(last_event.details.get("content_changed").is_some());
    assert!(last_event.details.get("agent").is_some());
}

#[test]
fn reprocess_invalid_id_fails() {
    let env = TestEnv::new();

    // Ensure DB exists
    let _conn = db::open_and_migrate(&env.db_path()).unwrap();

    let output = env
        .cmd()
        .args(["reprocess", "am_nonexistent"])
        .output()
        .unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

#[test]
fn reprocess_clears_stale_summary_on_content_change() {
    let env = TestEnv::new();
    let mut server = mockito::Server::new();
    let url = format!("{}/article", server.url());

    let mut bm = make_bookmark("am_stale1", &url, "Stale Summary Test", 8);
    bm.summary_status = SummaryStatus::Done;
    bm.suggested_tags = vec!["old-tag".to_string()];
    env.seed_bookmark(&bm, "# Old Article\n\nOld content.");

    // Mock returns different content
    let new_html = sample_html(
        "Stale Summary Test",
        "Completely different new content for hash change.",
    );
    let _mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(&new_html)
        .create();

    let output = env.cmd().args(["reprocess", "am_stale1"]).output().unwrap();
    assert!(output.status.success());

    let db_bm = get_bookmark_from_db(&env, "am_stale1");
    assert_eq!(db_bm.summary_status, SummaryStatus::Pending);
    assert!(
        db_bm.suggested_tags.is_empty(),
        "suggested tags should be cleared"
    );

    // Verify enrichment body sections cleared
    let bundle = Bundle::find(&env.storage, &bm.saved_at, &bm.id).unwrap();
    let sections = bundle.read_body_sections().unwrap();
    assert!(sections.summary.is_none(), "summary body should be cleared");
}

#[test]
fn reprocess_preserves_enriched_body_when_content_unchanged() {
    let env = TestEnv::new();
    let mut server = mockito::Server::new();
    let url = format!("{}/article", server.url());

    let mut bm = make_bookmark("am_preserve_body1", &url, "Preserve Test", 9);
    bm.summary_status = SummaryStatus::Done;

    // Seed with specific content that the mock will also return (same hash)
    let html_content = sample_html("Preserve Test", "The exact same article content.");
    let extraction = agentmark::extract::extract_content(&html_content);
    bm.content_hash = Some(extraction.content_hash.clone());

    env.seed_bookmark_with_html(&bm, &extraction.article_markdown, &html_content);

    // Write enriched body
    let bundle = Bundle::find(&env.storage, &bm.saved_at, &bm.id).unwrap();
    let sections = BodySections {
        summary: Some("This is an enriched summary that should survive.".to_string()),
        ..Default::default()
    };
    bundle.update_bookmark_md(&bm, &sections).unwrap();

    // Mock returns same content
    let _mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(&html_content)
        .create();

    let output = env
        .cmd()
        .args(["reprocess", "am_preserve_body1"])
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(stderr.contains("content unchanged"), "stderr: {stderr}");

    // Verify enriched body sections preserved
    let bundle = Bundle::find(&env.storage, &bm.saved_at, &bm.id).unwrap();
    let read_sections = bundle.read_body_sections().unwrap();
    assert_eq!(
        read_sections.summary.as_deref(),
        Some("This is an enriched summary that should survive.")
    );
}

// ── Batch reprocess ─────────────────────────────────────────────────

#[test]
fn reprocess_all_decline_leaves_data_untouched() {
    let env = TestEnv::new();
    let server = mockito::Server::new();
    let url = format!("{}/article", server.url());

    let bm = make_bookmark("am_decline1", &url, "Decline Test", 10);
    env.seed_bookmark(&bm, "# Article\n\nContent.");

    let output = env
        .cmd()
        .args(["reprocess", "--all"])
        .write_stdin("n\n")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("cancelled"), "stderr: {stderr}");

    // Verify bookmark unchanged
    let db_bm = get_bookmark_from_db(&env, "am_decline1");
    assert_eq!(db_bm.content_hash.as_deref(), Some("sha256:original_hash"));
}

#[test]
fn reprocess_all_accept_processes_multiple_bookmarks() {
    let env = TestEnv::new();
    let mut server = mockito::Server::new();

    let url1 = format!("{}/article1", server.url());
    let url2 = format!("{}/article2", server.url());

    let bm1 = make_bookmark("am_batch1", &url1, "Batch One", 11);
    let bm2 = make_bookmark("am_batch2", &url2, "Batch Two", 12);

    env.seed_bookmark(&bm1, "# Article One\n\nContent one.");
    env.seed_bookmark(&bm2, "# Article Two\n\nContent two.");

    let html1 = sample_html("Batch One", "Content for batch one article.");
    let html2 = sample_html("Batch Two", "Content for batch two article.");

    let _mock1 = server
        .mock("GET", "/article1")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(&html1)
        .create();
    let _mock2 = server
        .mock("GET", "/article2")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(&html2)
        .create();

    let output = env
        .cmd()
        .args(["reprocess", "--all"])
        .write_stdin("y\n")
        .output()
        .unwrap();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(stderr.contains("Reprocessed 2/2"), "stderr: {stderr}");
}

#[test]
fn reprocess_all_zero_bookmarks_prints_no_op() {
    let env = TestEnv::new();

    // Ensure DB exists with no bookmarks
    let _conn = db::open_and_migrate(&env.db_path()).unwrap();

    let output = env.cmd().args(["reprocess", "--all"]).output().unwrap();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(stderr.contains("No bookmarks"), "stderr: {stderr}");
}

#[test]
fn reprocess_all_continues_on_error() {
    let env = TestEnv::new();
    let mut server = mockito::Server::new();

    let url1 = format!("{}/good", server.url());
    // Use a URL that won't resolve (different port) for the "bad" bookmark
    let bad_url = "http://127.0.0.1:1/nonexistent";

    let bm1 = make_bookmark("am_good1", &url1, "Good Article", 13);
    let bm2 = make_bookmark("am_bad1", bad_url, "Bad Article", 14);

    env.seed_bookmark(&bm1, "# Good\n\nContent.");
    env.seed_bookmark(&bm2, "# Bad\n\nContent.");

    let html = sample_html("Good Article", "Good article content.");
    let _mock = server
        .mock("GET", "/good")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(&html)
        .create();

    let output = env
        .cmd()
        .args(["reprocess", "--all"])
        .write_stdin("y\n")
        .output()
        .unwrap();

    let stderr = String::from_utf8(output.stderr).unwrap();
    // Should fail overall (non-zero) due to the bad bookmark
    assert!(!output.status.success(), "should fail with partial errors");
    // But should report both attempted
    assert!(stderr.contains("1 failed"), "stderr: {stderr}");
}
