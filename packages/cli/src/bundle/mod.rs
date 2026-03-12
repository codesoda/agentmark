pub mod bookmark_md;
pub mod writer;

use std::path::{Path, PathBuf};

use crate::fetch::PageMetadata;
use crate::models::{Bookmark, BookmarkEvent};

pub use bookmark_md::BodySections;
pub use writer::BundleInput;

/// Extract body section content from an existing `bookmark.md` file.
///
/// Looks for `# Summary`, `# Suggested Next Actions`, `# Related Items`
/// headings and captures the text between them, excluding placeholder text.
fn parse_body_sections(content: &str) -> BodySections {
    let mut sections = BodySections::default();

    // Find the end of front matter
    let body = if let Some(after_open) = content.strip_prefix("---\n") {
        if let Some(end) = after_open.find("\n---\n") {
            &after_open[end + 5..]
        } else {
            content
        }
    } else {
        content
    };

    sections.summary = extract_section(body, "# Summary");
    sections.suggested_next_actions = extract_section(body, "# Suggested Next Actions");
    sections.related_items = extract_section(body, "# Related Items");

    sections
}

/// Extract content between a section heading and the next `# ` heading.
/// Returns None if the section contains only placeholder text or is empty.
fn extract_section(body: &str, heading: &str) -> Option<String> {
    let start = body.find(heading)?;
    let after_heading = &body[start + heading.len()..];

    // Find the next top-level heading or end of content
    let end = after_heading.find("\n# ").unwrap_or(after_heading.len());

    let section_text = after_heading[..end].trim();

    // Return None for placeholder text
    if section_text.is_empty()
        || section_text == "[pending enrichment]"
        || section_text == "[pending]"
    {
        return None;
    }

    Some(section_text.to_string())
}

/// Errors that can occur during bundle operations.
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("bundle directory already exists: {path}")]
    DirectoryExists { path: PathBuf },

    #[error("bundle not found: {path}")]
    BundleNotFound { path: PathBuf },

    #[error("events.jsonl missing: {path}")]
    EventsLogMissing { path: PathBuf },

    #[error("path error for {path}: {message}")]
    PathError { path: PathBuf, message: String },

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("YAML serialization error at {path}: {source}")]
    Yaml {
        path: PathBuf,
        source: serde_yaml::Error,
    },

    #[error("JSON serialization error at {path}: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
}

/// A handle to an existing bundle directory on disk.
///
/// Owns the resolved bundle path so that update operations do not need
/// to recompute it from potentially-mutated bookmark fields.
#[derive(Debug)]
pub struct Bundle {
    path: PathBuf,
}

impl Bundle {
    /// Create a new bundle on disk with all five canonical artifacts.
    ///
    /// The bundle is written to a staging directory first, then renamed
    /// to the final path for atomicity.
    pub fn create(
        storage_root: &Path,
        bookmark: &Bookmark,
        metadata: &PageMetadata,
        article_markdown: &str,
        raw_html: &str,
        capture_source: &str,
    ) -> Result<Self, BundleError> {
        let input = BundleInput {
            bookmark,
            metadata,
            article_markdown,
            raw_html,
            capture_source,
        };
        let path = writer::create_bundle(storage_root, &input)?;
        Ok(Self { path })
    }

    /// Open an existing bundle by its known path.
    pub fn open(path: PathBuf) -> Result<Self, BundleError> {
        if !path.is_dir() {
            return Err(BundleError::BundleNotFound { path });
        }
        Ok(Self { path })
    }

    /// The resolved bundle directory path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Rewrite `bookmark.md` from updated structured inputs.
    pub fn update_bookmark_md(
        &self,
        bookmark: &Bookmark,
        sections: &BodySections,
    ) -> Result<(), BundleError> {
        writer::rewrite_bookmark_md(&self.path, bookmark, sections)
    }

    /// Append a lifecycle event to `events.jsonl`.
    pub fn append_event(&self, event: &BookmarkEvent) -> Result<(), BundleError> {
        writer::append_event(&self.path, event)
    }

    /// Rewrite `article.md` with new content.
    pub fn update_article_md(&self, content: &str) -> Result<(), BundleError> {
        writer::rewrite_article_md(&self.path, content)
    }

    /// Rewrite `metadata.json` with new metadata.
    pub fn update_metadata_json(&self, metadata: &PageMetadata) -> Result<(), BundleError> {
        writer::rewrite_metadata_json(&self.path, metadata)
    }

    /// Rewrite `source.html` with new HTML.
    pub fn update_source_html(&self, html: &str) -> Result<(), BundleError> {
        writer::rewrite_source_html(&self.path, html)
    }

    /// Find an existing bundle by storage root, saved_at date, and bookmark ID.
    ///
    /// This lookup is stable even if the bookmark title changes, because it
    /// matches by the `-<id>` suffix of the directory name.
    pub fn find(
        storage_root: &Path,
        saved_at: &chrono::DateTime<chrono::Utc>,
        id: &str,
    ) -> Result<Self, BundleError> {
        let path = writer::find_bundle_dir(storage_root, saved_at, id)?;
        Ok(Self { path })
    }

    /// Rewrite `bookmark.md` preserving existing body sections.
    ///
    /// Reads the current bookmark.md, extracts body section content,
    /// then re-renders with updated front matter + preserved body.
    pub fn update_bookmark_md_preserving_body(
        &self,
        bookmark: &Bookmark,
    ) -> Result<(), BundleError> {
        let bm_path = self.path.join("bookmark.md");
        let sections = if bm_path.is_file() {
            let content = std::fs::read_to_string(&bm_path).map_err(|source| BundleError::Io {
                path: bm_path.clone(),
                source,
            })?;
            parse_body_sections(&content)
        } else {
            BodySections::default()
        };
        writer::rewrite_bookmark_md(&self.path, bookmark, &sections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::{BookmarkEvent, EventType};
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use tempfile::TempDir;

    fn test_bookmark() -> Bookmark {
        let mut bm = Bookmark::new("https://example.com/article", "Test Article Title");
        bm.id = "am_01HXYZ123456".to_string();
        bm.saved_at = Utc.with_ymd_and_hms(2026, 3, 5, 14, 30, 0).unwrap();
        bm
    }

    fn test_metadata() -> PageMetadata {
        PageMetadata {
            title: Some("Test Article Title".to_string()),
            description: Some("A test description".to_string()),
            author: Some("Test Author".to_string()),
            site_name: Some("Example".to_string()),
            ..Default::default()
        }
    }

    // --- Path generation tests ---

    #[test]
    fn bundle_dir_path_uses_date_hierarchy() {
        let root = Path::new("/storage");
        let saved_at = Utc.with_ymd_and_hms(2026, 3, 5, 14, 30, 0).unwrap();
        let path = writer::bundle_dir_path(root, &saved_at, "test-article", "am_01HXYZ");
        assert_eq!(
            path,
            PathBuf::from("/storage/2026/03/05/test-article-am_01HXYZ")
        );
    }

    #[test]
    fn bundle_dir_path_zero_pads_single_digit_month_day() {
        let root = Path::new("/storage");
        let saved_at = Utc.with_ymd_and_hms(2026, 1, 9, 0, 0, 0).unwrap();
        let path = writer::bundle_dir_path(root, &saved_at, "slug", "am_ID");
        assert_eq!(path, PathBuf::from("/storage/2026/01/09/slug-am_ID"));
    }

    #[test]
    fn bundle_dir_path_uses_bookmark_slug() {
        let root = Path::new("/s");
        let bm = test_bookmark();
        let slug = bm.slug();
        let path = writer::bundle_dir_path(root, &bm.saved_at, &slug, &bm.id);
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("test-article-title-"));
    }

    #[test]
    fn bundle_dir_path_fallback_slug() {
        let root = Path::new("/storage");
        let mut bm = Bookmark::new("https://example.com", "");
        bm.saved_at = Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();
        let slug = bm.slug();
        assert_eq!(slug, "untitled");
        let path = writer::bundle_dir_path(root, &bm.saved_at, &slug, &bm.id);
        assert!(path.to_str().unwrap().contains("untitled-"));
    }

    // --- Full bundle creation integration tests ---

    #[test]
    fn create_bundle_writes_all_five_files() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();
        let meta = test_metadata();

        let bundle = Bundle::create(
            tmp.path(),
            &bm,
            &meta,
            "# Article\n\nContent here.",
            "<html><body>Hello</body></html>",
            "cli",
        )
        .unwrap();

        let dir = bundle.path();
        assert!(dir.join("bookmark.md").is_file());
        assert!(dir.join("article.md").is_file());
        assert!(dir.join("metadata.json").is_file());
        assert!(dir.join("source.html").is_file());
        assert!(dir.join("events.jsonl").is_file());

        // Verify no extra files
        let entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn create_bundle_date_hierarchy_is_correct() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark(); // saved_at = 2026-03-05
        let meta = test_metadata();

        let bundle = Bundle::create(tmp.path(), &bm, &meta, "", "", "cli").unwrap();

        let rel = bundle.path().strip_prefix(tmp.path()).unwrap();
        let components: Vec<_> = rel
            .components()
            .map(|c| c.as_os_str().to_str().unwrap())
            .collect();
        assert_eq!(components[0], "2026");
        assert_eq!(components[1], "03");
        assert_eq!(components[2], "05");
        assert!(components[3].starts_with("test-article-title-"));
        assert!(components[3].ends_with("am_01HXYZ123456"));
    }

    #[test]
    fn bookmark_md_front_matter_roundtrips() {
        let tmp = TempDir::new().unwrap();
        let mut bm = test_bookmark();
        bm.description = Some("Desc".to_string());
        bm.user_tags = vec!["rust".to_string()];
        let meta = test_metadata();

        let bundle = Bundle::create(tmp.path(), &bm, &meta, "", "", "cli").unwrap();
        let content = std::fs::read_to_string(bundle.path().join("bookmark.md")).unwrap();

        // Extract YAML front matter
        let yaml_start = content.find("---\n").unwrap() + 4;
        let yaml_end = content[yaml_start..].find("\n---\n").unwrap() + yaml_start;
        let yaml = &content[yaml_start..yaml_end + 1];
        let roundtripped = Bookmark::from_yaml_str(yaml).unwrap();
        assert_eq!(bm, roundtripped);
    }

    #[test]
    fn article_md_matches_input_exactly() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();
        let article = "# Heading\n\nParagraph with **bold** text.\n\n- item 1\n- item 2\n";

        let bundle = Bundle::create(tmp.path(), &bm, &test_metadata(), article, "", "cli").unwrap();
        let content = std::fs::read_to_string(bundle.path().join("article.md")).unwrap();
        assert_eq!(content, article);
    }

    #[test]
    fn metadata_json_roundtrips_to_page_metadata() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();
        let meta = test_metadata();

        let bundle = Bundle::create(tmp.path(), &bm, &meta, "", "", "cli").unwrap();
        let content = std::fs::read_to_string(bundle.path().join("metadata.json")).unwrap();
        let roundtripped: PageMetadata = serde_json::from_str(&content).unwrap();
        assert_eq!(meta, roundtripped);
    }

    #[test]
    fn metadata_json_roundtrips_with_mostly_none() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();
        let meta = PageMetadata::default(); // all None

        let bundle = Bundle::create(tmp.path(), &bm, &meta, "", "", "cli").unwrap();
        let content = std::fs::read_to_string(bundle.path().join("metadata.json")).unwrap();
        let roundtripped: PageMetadata = serde_json::from_str(&content).unwrap();
        assert_eq!(meta, roundtripped);
    }

    #[test]
    fn source_html_matches_input_exactly() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();
        let html = "<html><head><title>日本語</title></head><body><p>\"quoted\" & special</p></body></html>";

        let bundle = Bundle::create(tmp.path(), &bm, &test_metadata(), "", html, "cli").unwrap();
        let content = std::fs::read_to_string(bundle.path().join("source.html")).unwrap();
        assert_eq!(content, html);
    }

    #[test]
    fn events_jsonl_has_initial_saved_event() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();

        let bundle = Bundle::create(tmp.path(), &bm, &test_metadata(), "", "", "cli").unwrap();
        let content = std::fs::read_to_string(bundle.path().join("events.jsonl")).unwrap();

        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);

        let event = BookmarkEvent::from_json_line(lines[0]).unwrap();
        assert_eq!(event.event_type, EventType::Saved);
        assert_eq!(event.details["capture_source"], "cli");
        assert_eq!(event.details["url"], "https://example.com/article");
    }

    #[test]
    fn events_jsonl_with_chrome_extension_source() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();

        let bundle = Bundle::create(
            tmp.path(),
            &bm,
            &test_metadata(),
            "",
            "",
            "chrome_extension",
        )
        .unwrap();
        let content = std::fs::read_to_string(bundle.path().join("events.jsonl")).unwrap();
        let event = BookmarkEvent::from_json_line(content.lines().next().unwrap()).unwrap();
        assert_eq!(event.details["capture_source"], "chrome_extension");
    }

    // --- Collision tests ---

    #[test]
    fn create_bundle_rejects_collision() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();
        let meta = test_metadata();

        // First create succeeds
        Bundle::create(tmp.path(), &bm, &meta, "", "", "cli").unwrap();

        // Second create with same bookmark fails
        let result = Bundle::create(tmp.path(), &bm, &meta, "", "", "cli");
        assert!(result.is_err());
        match result.unwrap_err() {
            BundleError::DirectoryExists { path } => {
                assert!(path.to_str().unwrap().contains("am_01HXYZ123456"));
            }
            other => panic!("expected DirectoryExists, got: {}", other),
        }
    }

    // --- Update tests ---

    #[test]
    fn update_bookmark_md_rewrites_content() {
        let tmp = TempDir::new().unwrap();
        let mut bm = test_bookmark();
        let meta = test_metadata();

        let bundle = Bundle::create(tmp.path(), &bm, &meta, "", "", "cli").unwrap();

        // Update bookmark fields and sections
        bm.user_tags = vec!["updated-tag".to_string()];
        let sections = BodySections {
            summary: Some("Enriched summary.".to_string()),
            suggested_next_actions: Some("- Action 1\n- Action 2".to_string()),
            related_items: None,
        };
        bundle.update_bookmark_md(&bm, &sections).unwrap();

        let content = std::fs::read_to_string(bundle.path().join("bookmark.md")).unwrap();
        assert!(content.contains("updated-tag"));
        assert!(content.contains("Enriched summary."));
        assert!(content.contains("- Action 1"));
        assert!(!content.contains("[pending enrichment]"));
        // related_items is still None so should have [pending]
        assert!(content.contains("[pending]"));
    }

    #[test]
    fn update_bookmark_md_on_missing_dir_fails() {
        let bundle = Bundle {
            path: PathBuf::from("/nonexistent/bundle"),
        };
        let bm = test_bookmark();
        let result = bundle.update_bookmark_md(&bm, &BodySections::default());
        assert!(matches!(
            result.unwrap_err(),
            BundleError::BundleNotFound { .. }
        ));
    }

    // --- Append event tests ---

    #[test]
    fn append_event_adds_line() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();

        let bundle = Bundle::create(tmp.path(), &bm, &test_metadata(), "", "", "cli").unwrap();

        let event = BookmarkEvent::new(EventType::Enriched, json!({"tags": ["rust"]}));
        bundle.append_event(&event).unwrap();

        let content = std::fs::read_to_string(bundle.path().join("events.jsonl")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        let first = BookmarkEvent::from_json_line(lines[0]).unwrap();
        assert_eq!(first.event_type, EventType::Saved);

        let second = BookmarkEvent::from_json_line(lines[1]).unwrap();
        assert_eq!(second.event_type, EventType::Enriched);
        assert_eq!(second.details["tags"], json!(["rust"]));
    }

    #[test]
    fn append_event_on_missing_events_file_fails() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();

        let bundle = Bundle::create(tmp.path(), &bm, &test_metadata(), "", "", "cli").unwrap();

        // Remove events.jsonl
        std::fs::remove_file(bundle.path().join("events.jsonl")).unwrap();

        let event = BookmarkEvent::new(EventType::Enriched, json!({}));
        let result = bundle.append_event(&event);
        assert!(matches!(
            result.unwrap_err(),
            BundleError::EventsLogMissing { .. }
        ));
    }

    #[test]
    fn open_existing_bundle() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();

        let created = Bundle::create(tmp.path(), &bm, &test_metadata(), "", "", "cli").unwrap();
        let path = created.path().to_path_buf();

        let opened = Bundle::open(path.clone()).unwrap();
        assert_eq!(opened.path(), path);
    }

    #[test]
    fn open_nonexistent_path_fails() {
        let result = Bundle::open(PathBuf::from("/nonexistent"));
        assert!(matches!(
            result.unwrap_err(),
            BundleError::BundleNotFound { .. }
        ));
    }

    // --- Edge case tests ---

    #[test]
    fn empty_article_markdown_creates_empty_file() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();

        let bundle = Bundle::create(tmp.path(), &bm, &test_metadata(), "", "", "cli").unwrap();
        let content = std::fs::read_to_string(bundle.path().join("article.md")).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn unicode_html_written_correctly() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();
        let html = "<p>日本語テスト — \"quotes\" & em-dash</p>";

        let bundle = Bundle::create(tmp.path(), &bm, &test_metadata(), "", html, "cli").unwrap();
        let content = std::fs::read_to_string(bundle.path().join("source.html")).unwrap();
        assert_eq!(content, html);
    }

    // --- Bundle find tests ---

    #[test]
    fn find_bundle_by_saved_at_and_id() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();
        let meta = test_metadata();

        let created = Bundle::create(tmp.path(), &bm, &meta, "", "", "cli").unwrap();
        let found = Bundle::find(tmp.path(), &bm.saved_at, &bm.id).unwrap();
        assert_eq!(created.path(), found.path());
    }

    #[test]
    fn find_bundle_missing_returns_error() {
        let tmp = TempDir::new().unwrap();
        let saved_at = Utc.with_ymd_and_hms(2026, 3, 5, 14, 30, 0).unwrap();
        let result = Bundle::find(tmp.path(), &saved_at, "am_nonexistent");
        assert!(matches!(
            result.unwrap_err(),
            BundleError::BundleNotFound { .. }
        ));
    }

    // --- In-place update tests ---

    #[test]
    fn update_article_md_rewrites_content() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();
        let bundle =
            Bundle::create(tmp.path(), &bm, &test_metadata(), "original", "", "cli").unwrap();

        bundle.update_article_md("updated content").unwrap();
        let content = std::fs::read_to_string(bundle.path().join("article.md")).unwrap();
        assert_eq!(content, "updated content");
    }

    #[test]
    fn update_metadata_json_rewrites_content() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();
        let bundle = Bundle::create(tmp.path(), &bm, &test_metadata(), "", "", "cli").unwrap();

        let new_meta = PageMetadata {
            title: Some("Updated Title".to_string()),
            ..Default::default()
        };
        bundle.update_metadata_json(&new_meta).unwrap();
        let content = std::fs::read_to_string(bundle.path().join("metadata.json")).unwrap();
        let roundtripped: PageMetadata = serde_json::from_str(&content).unwrap();
        assert_eq!(roundtripped, new_meta);
    }

    #[test]
    fn update_source_html_rewrites_content() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();
        let bundle = Bundle::create(tmp.path(), &bm, &test_metadata(), "", "<old>", "cli").unwrap();

        bundle.update_source_html("<new>").unwrap();
        let content = std::fs::read_to_string(bundle.path().join("source.html")).unwrap();
        assert_eq!(content, "<new>");
    }

    // --- Body preservation tests ---

    #[test]
    fn update_bookmark_md_preserving_body_keeps_enriched_content() {
        let tmp = TempDir::new().unwrap();
        let mut bm = test_bookmark();
        let meta = test_metadata();

        let bundle = Bundle::create(tmp.path(), &bm, &meta, "", "", "cli").unwrap();

        // Simulate enrichment by writing custom body sections
        let enriched_sections = BodySections {
            summary: Some("This is an enriched summary with real content.".to_string()),
            suggested_next_actions: Some("- Read follow-up article".to_string()),
            related_items: Some("- [Related page](https://example.com/related)".to_string()),
        };
        bundle.update_bookmark_md(&bm, &enriched_sections).unwrap();

        // Now update front matter only (simulating resave)
        bm.user_tags = vec!["new-tag".to_string()];
        bundle.update_bookmark_md_preserving_body(&bm).unwrap();

        // Verify body sections are preserved
        let content = std::fs::read_to_string(bundle.path().join("bookmark.md")).unwrap();
        assert!(
            content.contains("new-tag"),
            "front matter should be updated"
        );
        assert!(
            content.contains("enriched summary"),
            "summary should be preserved"
        );
        assert!(
            content.contains("Read follow-up"),
            "actions should be preserved"
        );
        assert!(
            content.contains("Related page"),
            "related items should be preserved"
        );
    }

    #[test]
    fn update_bookmark_md_preserving_body_handles_placeholder_sections() {
        let tmp = TempDir::new().unwrap();
        let mut bm = test_bookmark();
        let bundle = Bundle::create(tmp.path(), &bm, &test_metadata(), "", "", "cli").unwrap();

        // Bookmark starts with placeholder sections
        bm.user_tags = vec!["tag".to_string()];
        bundle.update_bookmark_md_preserving_body(&bm).unwrap();

        let content = std::fs::read_to_string(bundle.path().join("bookmark.md")).unwrap();
        assert!(content.contains("tag"), "front matter should be updated");
        // Placeholders should still be there (since they're the default)
        assert!(content.contains("[pending enrichment]"));
    }

    // --- Parse body sections tests ---

    #[test]
    fn parse_body_sections_extracts_enriched_content() {
        let content = "---\nid: am_test\n---\n\n# Summary\n\nEnriched summary here.\n\n# Why I Saved This\n\nMy note.\n\n# Suggested Next Actions\n\n- Do something\n\n# Related Items\n\n- [Link](url)\n";
        let sections = parse_body_sections(content);
        assert_eq!(sections.summary.as_deref(), Some("Enriched summary here."));
        assert_eq!(
            sections.suggested_next_actions.as_deref(),
            Some("- Do something")
        );
        assert_eq!(sections.related_items.as_deref(), Some("- [Link](url)"));
    }

    #[test]
    fn parse_body_sections_returns_none_for_placeholders() {
        let content = "---\nid: am_test\n---\n\n# Summary\n\n[pending enrichment]\n\n# Why I Saved This\n\n\n\n# Suggested Next Actions\n\n[pending enrichment]\n\n# Related Items\n\n[pending]\n";
        let sections = parse_body_sections(content);
        assert!(sections.summary.is_none());
        assert!(sections.suggested_next_actions.is_none());
        assert!(sections.related_items.is_none());
    }

    #[test]
    fn multiple_appended_events_each_on_own_line() {
        let tmp = TempDir::new().unwrap();
        let bm = test_bookmark();

        let bundle = Bundle::create(tmp.path(), &bm, &test_metadata(), "", "", "cli").unwrap();

        for i in 0..3 {
            let event = BookmarkEvent::new(EventType::ContentUpdated, json!({"iteration": i}));
            bundle.append_event(&event).unwrap();
        }

        let content = std::fs::read_to_string(bundle.path().join("events.jsonl")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 4); // 1 initial + 3 appended
        for line in &lines {
            BookmarkEvent::from_json_line(line).unwrap(); // all parseable
        }
    }
}
