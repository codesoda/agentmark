use chrono::{DateTime, Utc};
use deunicode::deunicode;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Current bookmark schema version. Bump when the on-disk format changes.
pub const BOOKMARK_SCHEMA_VERSION: u32 = 1;

/// Maximum slug length in characters before truncation.
const MAX_SLUG_LEN: usize = 60;

/// Fallback slug when the title produces no usable characters.
const FALLBACK_SLUG: &str = "untitled";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub url: String,
    pub canonical_url: String,
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub site_name: Option<String>,
    pub published_at: Option<String>,
    pub saved_at: DateTime<Utc>,
    pub capture_source: CaptureSource,
    pub user_tags: Vec<String>,
    pub suggested_tags: Vec<String>,
    pub collections: Vec<String>,
    pub note: Option<String>,
    pub action_prompt: Option<String>,
    pub state: BookmarkState,
    pub content_status: ContentStatus,
    pub summary_status: SummaryStatus,
    pub content_hash: Option<String>,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSource {
    #[default]
    Cli,
    ChromeExtension,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookmarkState {
    #[default]
    Inbox,
    Processed,
    Archived,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentStatus {
    #[default]
    Pending,
    Extracted,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryStatus {
    #[default]
    Pending,
    Done,
    Failed,
}

/// Generate a bookmark ID: `am_` prefix + ULID.
fn generate_id() -> String {
    format!("am_{}", Ulid::new())
}

impl Bookmark {
    /// Create a new bookmark with sensible defaults for CLI usage.
    ///
    /// Does not validate URLs or trim input — that belongs to later pipeline stages.
    pub fn new(url: impl Into<String>, title: impl Into<String>) -> Self {
        let url = url.into();
        let title = title.into();
        Self {
            id: generate_id(),
            canonical_url: url.clone(),
            url,
            title,
            description: None,
            author: None,
            site_name: None,
            published_at: None,
            saved_at: Utc::now(),
            capture_source: CaptureSource::default(),
            user_tags: Vec::new(),
            suggested_tags: Vec::new(),
            collections: Vec::new(),
            note: None,
            action_prompt: None,
            state: BookmarkState::default(),
            content_status: ContentStatus::default(),
            summary_status: SummaryStatus::default(),
            content_hash: None,
            schema_version: BOOKMARK_SCHEMA_VERSION,
        }
    }

    /// Generate a filesystem-safe slug from the bookmark title.
    ///
    /// - Transliterates Unicode to ASCII
    /// - Lowercases
    /// - Replaces non-alphanumeric runs with a single `-`
    /// - Trims leading/trailing hyphens
    /// - Truncates to [`MAX_SLUG_LEN`] characters without leaving a trailing hyphen
    /// - Falls back to `"untitled"` if nothing usable remains
    pub fn slug(&self) -> String {
        slugify(&self.title)
    }

    /// Serialize to a YAML string.
    pub fn to_yaml_string(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// Deserialize from a YAML string.
    pub fn from_yaml_str(s: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(s)
    }

    /// Serialize to a JSON string.
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize from a JSON string.
    pub fn from_json_str(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

/// Pure slug generation function. Exported as a method on [`Bookmark`] via `slug()`.
fn slugify(title: &str) -> String {
    let ascii = deunicode(title);
    let lowered = ascii.to_lowercase();

    let mut slug = String::with_capacity(lowered.len());
    let mut prev_was_sep = true; // treat start as separator to trim leading hyphens

    for ch in lowered.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            prev_was_sep = false;
        } else if !prev_was_sep {
            slug.push('-');
            prev_was_sep = true;
        }
    }

    // Trim trailing hyphen
    let slug = slug.trim_end_matches('-');

    if slug.is_empty() {
        return FALLBACK_SLUG.to_string();
    }

    // Truncate to MAX_SLUG_LEN without leaving a trailing hyphen
    if slug.len() <= MAX_SLUG_LEN {
        slug.to_string()
    } else {
        let truncated = &slug[..MAX_SLUG_LEN];
        truncated.trim_end_matches('-').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    // --- Constructor defaults ---

    #[test]
    fn new_sets_id_with_am_prefix() {
        let bm = Bookmark::new("https://example.com", "Example");
        assert!(bm.id.starts_with("am_"), "id should start with am_");
    }

    #[test]
    fn new_id_suffix_is_valid_ulid() {
        let bm = Bookmark::new("https://example.com", "Example");
        let suffix = &bm.id[3..];
        assert!(
            suffix.parse::<Ulid>().is_ok(),
            "id suffix should parse as ULID, got: {}",
            suffix
        );
    }

    #[test]
    fn new_generates_unique_ids() {
        let a = Bookmark::new("https://a.com", "A");
        let b = Bookmark::new("https://b.com", "B");
        let c = Bookmark::new("https://c.com", "C");
        assert_ne!(a.id, b.id);
        assert_ne!(b.id, c.id);
        assert_ne!(a.id, c.id);
    }

    #[test]
    fn new_sets_canonical_url_to_input_url() {
        let bm = Bookmark::new("https://example.com/path?q=1", "Example");
        assert_eq!(bm.canonical_url, "https://example.com/path?q=1");
    }

    #[test]
    fn new_preserves_url_and_title_without_trimming() {
        let bm = Bookmark::new("  https://example.com  ", "  My Title  ");
        assert_eq!(bm.url, "  https://example.com  ");
        assert_eq!(bm.title, "  My Title  ");
    }

    #[test]
    fn new_defaults_capture_source_to_cli() {
        let bm = Bookmark::new("https://example.com", "Example");
        assert_eq!(bm.capture_source, CaptureSource::Cli);
    }

    #[test]
    fn new_defaults_state_to_inbox() {
        let bm = Bookmark::new("https://example.com", "Example");
        assert_eq!(bm.state, BookmarkState::Inbox);
    }

    #[test]
    fn new_defaults_content_status_to_pending() {
        let bm = Bookmark::new("https://example.com", "Example");
        assert_eq!(bm.content_status, ContentStatus::Pending);
    }

    #[test]
    fn new_defaults_summary_status_to_pending() {
        let bm = Bookmark::new("https://example.com", "Example");
        assert_eq!(bm.summary_status, SummaryStatus::Pending);
    }

    #[test]
    fn new_defaults_schema_version() {
        let bm = Bookmark::new("https://example.com", "Example");
        assert_eq!(bm.schema_version, BOOKMARK_SCHEMA_VERSION);
        assert_eq!(bm.schema_version, 1);
    }

    #[test]
    fn new_defaults_optional_fields_to_none() {
        let bm = Bookmark::new("https://example.com", "Example");
        assert_eq!(bm.description, None);
        assert_eq!(bm.author, None);
        assert_eq!(bm.site_name, None);
        assert_eq!(bm.published_at, None);
        assert_eq!(bm.note, None);
        assert_eq!(bm.action_prompt, None);
        assert_eq!(bm.content_hash, None);
    }

    #[test]
    fn new_defaults_vec_fields_to_empty() {
        let bm = Bookmark::new("https://example.com", "Example");
        assert!(bm.user_tags.is_empty());
        assert!(bm.suggested_tags.is_empty());
        assert!(bm.collections.is_empty());
    }

    #[test]
    fn new_saved_at_is_recent_utc() {
        let before = Utc::now();
        let bm = Bookmark::new("https://example.com", "Example");
        let after = Utc::now();
        assert!(bm.saved_at >= before);
        assert!(bm.saved_at <= after);
    }

    #[test]
    fn new_with_blank_url_does_not_panic() {
        let bm = Bookmark::new("", "Title");
        assert_eq!(bm.url, "");
        assert_eq!(bm.canonical_url, "");
    }

    #[test]
    fn new_with_blank_title_does_not_panic() {
        let bm = Bookmark::new("https://example.com", "");
        assert_eq!(bm.title, "");
    }

    // --- Enum serialization ---

    #[test]
    fn capture_source_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&CaptureSource::Cli).unwrap(),
            "\"cli\""
        );
        assert_eq!(
            serde_json::to_string(&CaptureSource::ChromeExtension).unwrap(),
            "\"chrome_extension\""
        );
    }

    #[test]
    fn bookmark_state_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&BookmarkState::Inbox).unwrap(),
            "\"inbox\""
        );
        assert_eq!(
            serde_json::to_string(&BookmarkState::Processed).unwrap(),
            "\"processed\""
        );
        assert_eq!(
            serde_json::to_string(&BookmarkState::Archived).unwrap(),
            "\"archived\""
        );
    }

    #[test]
    fn content_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&ContentStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&ContentStatus::Extracted).unwrap(),
            "\"extracted\""
        );
        assert_eq!(
            serde_json::to_string(&ContentStatus::Failed).unwrap(),
            "\"failed\""
        );
    }

    #[test]
    fn summary_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&SummaryStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&SummaryStatus::Done).unwrap(),
            "\"done\""
        );
        assert_eq!(
            serde_json::to_string(&SummaryStatus::Failed).unwrap(),
            "\"failed\""
        );
    }

    #[test]
    fn unknown_enum_value_fails_deserialization() {
        let result = serde_json::from_str::<CaptureSource>("\"browser\"");
        assert!(result.is_err());

        let result = serde_json::from_str::<BookmarkState>("\"deleted\"");
        assert!(result.is_err());

        let result = serde_json::from_str::<ContentStatus>("\"unknown\"");
        assert!(result.is_err());

        let result = serde_json::from_str::<SummaryStatus>("\"skipped\"");
        assert!(result.is_err());
    }

    // --- YAML roundtrip ---

    #[test]
    fn yaml_roundtrip() {
        let mut bm = Bookmark::new("https://example.com", "YAML Test");
        bm.description = Some("A description".to_string());
        bm.author = Some("Author".to_string());
        bm.user_tags = vec!["rust".to_string(), "web".to_string()];
        bm.suggested_tags = vec!["dev".to_string()];
        bm.collections = vec!["reading-list".to_string()];
        bm.note = Some("Important page".to_string());

        let yaml = bm.to_yaml_string().unwrap();
        let roundtripped = Bookmark::from_yaml_str(&yaml).unwrap();
        assert_eq!(bm, roundtripped);
    }

    #[test]
    fn yaml_with_all_none_fields_roundtrips() {
        let bm = Bookmark::new("https://example.com", "Minimal");
        let yaml = bm.to_yaml_string().unwrap();
        let roundtripped = Bookmark::from_yaml_str(&yaml).unwrap();
        assert_eq!(bm, roundtripped);
    }

    #[test]
    fn yaml_invalid_input_returns_error() {
        let result = Bookmark::from_yaml_str("not: valid: yaml: [");
        assert!(result.is_err());
    }

    #[test]
    fn yaml_missing_required_field_returns_error() {
        let yaml = "id: am_test\nurl: https://example.com\n";
        let result = Bookmark::from_yaml_str(yaml);
        assert!(result.is_err());
    }

    // --- JSON roundtrip ---

    #[test]
    fn json_roundtrip() {
        let mut bm = Bookmark::new("https://example.com", "JSON Test");
        bm.description = Some("Desc".to_string());
        bm.site_name = Some("Example".to_string());
        bm.published_at = Some("2026-01-01".to_string());
        bm.content_hash = Some("abc123".to_string());
        bm.state = BookmarkState::Processed;
        bm.content_status = ContentStatus::Extracted;
        bm.summary_status = SummaryStatus::Done;

        let json = bm.to_json_string().unwrap();
        let roundtripped = Bookmark::from_json_str(&json).unwrap();
        assert_eq!(bm, roundtripped);
    }

    #[test]
    fn json_invalid_input_returns_error() {
        let result = Bookmark::from_json_str("{broken json");
        assert!(result.is_err());
    }

    #[test]
    fn json_missing_required_field_returns_error() {
        let json = r#"{"id":"am_test","url":"https://example.com"}"#;
        let result = Bookmark::from_json_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn json_vec_fields_serialize_as_arrays() {
        let bm = Bookmark::new("https://example.com", "Test");
        let json = bm.to_json_string().unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value["user_tags"].is_array());
        assert!(value["suggested_tags"].is_array());
        assert!(value["collections"].is_array());
        assert_eq!(value["user_tags"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn json_contains_correct_enum_strings() {
        let bm = Bookmark::new("https://example.com", "Test");
        let json = bm.to_json_string().unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["capture_source"], "cli");
        assert_eq!(value["state"], "inbox");
        assert_eq!(value["content_status"], "pending");
        assert_eq!(value["summary_status"], "pending");
    }

    // --- Slug generation ---

    #[test]
    fn slug_basic_title() {
        let bm = Bookmark::new("https://example.com", "Hello World");
        assert_eq!(bm.slug(), "hello-world");
    }

    #[test]
    fn slug_special_characters() {
        let bm = Bookmark::new("https://example.com", "Hello, World! How's it going?");
        assert_eq!(bm.slug(), "hello-world-how-s-it-going");
    }

    #[test]
    fn slug_unicode_transliteration() {
        let bm = Bookmark::new("https://example.com", "Über die Brücke");
        assert_eq!(bm.slug(), "uber-die-brucke");
    }

    #[test]
    fn slug_cjk_transliteration() {
        let bm = Bookmark::new("https://example.com", "日本語テスト");
        let slug = bm.slug();
        // deunicode transliterates CJK; should produce something non-empty
        assert!(!slug.is_empty());
        assert_ne!(slug, FALLBACK_SLUG);
    }

    #[test]
    fn slug_empty_title_returns_fallback() {
        let bm = Bookmark::new("https://example.com", "");
        assert_eq!(bm.slug(), "untitled");
    }

    #[test]
    fn slug_whitespace_only_title_returns_fallback() {
        let bm = Bookmark::new("https://example.com", "   ");
        assert_eq!(bm.slug(), "untitled");
    }

    #[test]
    fn slug_punctuation_only_title_returns_fallback() {
        let bm = Bookmark::new("https://example.com", "---!!!???...");
        assert_eq!(bm.slug(), "untitled");
    }

    #[test]
    fn slug_emoji_only_title() {
        let bm = Bookmark::new("https://example.com", "🎉🚀🔥");
        // deunicode transliterates emoji to text representations
        let slug = bm.slug();
        assert!(!slug.is_empty());
    }

    #[test]
    fn slug_repeated_separators_collapsed() {
        let bm = Bookmark::new("https://example.com", "hello   ---   world");
        assert_eq!(bm.slug(), "hello-world");
    }

    #[test]
    fn slug_leading_trailing_separators_trimmed() {
        let bm = Bookmark::new("https://example.com", "---hello world---");
        assert_eq!(bm.slug(), "hello-world");
    }

    #[test]
    fn slug_long_title_truncated() {
        let long_title = "a".repeat(100);
        let bm = Bookmark::new("https://example.com", &long_title);
        let slug = bm.slug();
        assert!(slug.len() <= MAX_SLUG_LEN);
        assert_eq!(slug.len(), MAX_SLUG_LEN);
    }

    #[test]
    fn slug_truncation_does_not_leave_trailing_hyphen() {
        // Create a title that would have a hyphen right at the truncation boundary
        let title = format!("{} {}", "a".repeat(59), "b".repeat(10));
        let bm = Bookmark::new("https://example.com", &title);
        let slug = bm.slug();
        assert!(slug.len() <= MAX_SLUG_LEN);
        assert!(!slug.ends_with('-'));
    }

    #[test]
    fn slug_path_separators_not_leaked() {
        let bm = Bookmark::new("https://example.com", "path/to\\something");
        let slug = bm.slug();
        assert!(!slug.contains('/'));
        assert!(!slug.contains('\\'));
    }

    #[test]
    fn slug_is_deterministic() {
        let slug1 = slugify("Hello World!");
        let slug2 = slugify("Hello World!");
        assert_eq!(slug1, slug2);
    }
}
