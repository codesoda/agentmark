use crate::models::Bookmark;

/// Placeholder text for sections pending enrichment.
const PENDING_ENRICHMENT: &str = "[pending enrichment]";
const PENDING: &str = "[pending]";

/// Optional body section content for bookmark.md rendering.
/// Fields default to placeholder text when `None`.
#[derive(Debug, Default)]
pub struct BodySections {
    pub summary: Option<String>,
    pub suggested_next_actions: Option<String>,
    pub related_items: Option<String>,
}

/// Render a complete `bookmark.md` from structured inputs.
///
/// The file consists of YAML front matter (fenced by `---`) followed by
/// markdown body sections. The same renderer is used for both initial
/// creation and subsequent updates, keeping the output deterministic.
pub fn render(bookmark: &Bookmark, sections: &BodySections) -> Result<String, serde_yaml::Error> {
    let yaml = bookmark.to_yaml_string()?;
    let mut out = String::with_capacity(yaml.len() + 512);

    // YAML front matter
    out.push_str("---\n");
    out.push_str(&yaml);
    out.push_str("---\n\n");

    // Body sections
    out.push_str("# Summary\n\n");
    out.push_str(sections.summary.as_deref().unwrap_or(PENDING_ENRICHMENT));
    out.push_str("\n\n");

    out.push_str("# Why I Saved This\n\n");
    if let Some(note) = &bookmark.note {
        out.push_str(note);
    }
    out.push_str("\n\n");

    out.push_str("# Suggested Next Actions\n\n");
    out.push_str(
        sections
            .suggested_next_actions
            .as_deref()
            .unwrap_or(PENDING_ENRICHMENT),
    );
    out.push_str("\n\n");

    out.push_str("# Related Items\n\n");
    out.push_str(sections.related_items.as_deref().unwrap_or(PENDING));
    out.push('\n');

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn test_bookmark() -> Bookmark {
        let mut bm = Bookmark::new("https://example.com/article", "Test Article Title");
        bm.id = "am_01HXYZ123456".to_string();
        bm.saved_at = Utc.with_ymd_and_hms(2026, 3, 5, 14, 30, 0).unwrap();
        bm
    }

    #[test]
    fn render_has_yaml_front_matter_fences() {
        let bm = test_bookmark();
        let md = render(&bm, &BodySections::default()).unwrap();
        assert!(md.starts_with("---\n"));
        // Find the closing fence
        let after_open = &md[4..];
        assert!(after_open.contains("\n---\n"));
    }

    #[test]
    fn front_matter_roundtrips_to_bookmark() {
        let mut bm = test_bookmark();
        bm.description = Some("A test description".to_string());
        bm.user_tags = vec!["rust".to_string(), "web".to_string()];
        bm.suggested_tags = vec!["dev".to_string()];
        bm.note = Some("Important note".to_string());

        let md = render(&bm, &BodySections::default()).unwrap();

        // Extract YAML between the fences
        let yaml_start = md.find("---\n").unwrap() + 4;
        let yaml_end = md[yaml_start..].find("\n---\n").unwrap() + yaml_start;
        let yaml = &md[yaml_start..yaml_end + 1]; // include trailing newline

        let roundtripped = Bookmark::from_yaml_str(yaml).unwrap();
        assert_eq!(bm, roundtripped);
    }

    #[test]
    fn body_contains_all_sections_in_order() {
        let bm = test_bookmark();
        let md = render(&bm, &BodySections::default()).unwrap();

        let summary_pos = md.find("# Summary").unwrap();
        let why_pos = md.find("# Why I Saved This").unwrap();
        let actions_pos = md.find("# Suggested Next Actions").unwrap();
        let related_pos = md.find("# Related Items").unwrap();

        assert!(summary_pos < why_pos);
        assert!(why_pos < actions_pos);
        assert!(actions_pos < related_pos);
    }

    #[test]
    fn default_sections_use_placeholders() {
        let bm = test_bookmark();
        let md = render(&bm, &BodySections::default()).unwrap();

        assert!(md.contains("[pending enrichment]"));
        assert!(md.contains("[pending]"));
    }

    #[test]
    fn note_present_renders_in_why_section() {
        let mut bm = test_bookmark();
        bm.note = Some("This is why I saved it.".to_string());
        let md = render(&bm, &BodySections::default()).unwrap();

        let why_pos = md.find("# Why I Saved This").unwrap();
        let actions_pos = md.find("# Suggested Next Actions").unwrap();
        let section = &md[why_pos..actions_pos];
        assert!(section.contains("This is why I saved it."));
    }

    #[test]
    fn note_absent_leaves_empty_why_section() {
        let bm = test_bookmark();
        assert!(bm.note.is_none());
        let md = render(&bm, &BodySections::default()).unwrap();

        let why_pos = md.find("# Why I Saved This").unwrap();
        let actions_pos = md.find("# Suggested Next Actions").unwrap();
        let section = &md[why_pos..actions_pos];
        // Should just be heading + blank lines
        assert!(!section.contains("[pending"));
    }

    #[test]
    fn note_with_multiline_markdown_content() {
        let mut bm = test_bookmark();
        bm.note = Some("Line one\n\n## Sub heading\n\n- item 1\n- item 2".to_string());
        let md = render(&bm, &BodySections::default()).unwrap();
        assert!(md.contains("Line one\n\n## Sub heading\n\n- item 1\n- item 2"));
    }

    #[test]
    fn custom_sections_override_placeholders() {
        let bm = test_bookmark();
        let sections = BodySections {
            summary: Some("This is a great article about Rust.".to_string()),
            suggested_next_actions: Some("- Read the follow-up post".to_string()),
            related_items: Some("- [Similar article](https://example.com)".to_string()),
        };
        let md = render(&bm, &sections).unwrap();

        assert!(md.contains("This is a great article about Rust."));
        assert!(md.contains("- Read the follow-up post"));
        assert!(md.contains("- [Similar article](https://example.com)"));
        assert!(!md.contains("[pending enrichment]"));
        assert!(!md.contains("[pending]"));
    }
}
