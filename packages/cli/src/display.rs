//! Terminal display helpers for browse/search output.
//!
//! All formatting helpers are pure functions that return strings.
//! Only command-level code should print. Color/styling is opt-in
//! based on terminal capability and `NO_COLOR` convention.

use crate::models::{Bookmark, BookmarkState};

/// Default terminal width when detection fails.
const DEFAULT_WIDTH: usize = 80;

/// Minimum width below which we stop trying to truncate and just let it wrap.
const MIN_WIDTH: usize = 40;

// ── Terminal capability detection ───────────────────────────────────

/// Detect the terminal width, falling back to `DEFAULT_WIDTH`.
pub fn terminal_width() -> usize {
    // Use a simple approach: check COLUMNS env var, fall back to default.
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(w) = cols.parse::<usize>() {
            if w >= MIN_WIDTH {
                return w;
            }
        }
    }
    DEFAULT_WIDTH
}

/// Whether color output is enabled (stdout is a tty and NO_COLOR is not set).
pub fn color_enabled() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal() && std::env::var("NO_COLOR").is_err()
}

// ── String truncation ───────────────────────────────────────────────

/// Truncate a string to at most `max_chars` characters.
/// If truncated, appends `…` (the ellipsis replaces the last char slot).
pub fn truncate(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else if max_chars == 1 {
        "…".to_string()
    } else {
        let mut result: String = chars[..max_chars - 1].iter().collect();
        result.push('…');
        result
    }
}

// ── Tag merging ─────────────────────────────────────────────────────

/// Merge user_tags and suggested_tags into a single deduplicated list.
/// User tags come first, then suggested tags not already present.
pub fn merge_tags(user_tags: &[String], suggested_tags: &[String]) -> Vec<String> {
    let mut merged = user_tags.to_vec();
    for t in suggested_tags {
        if !merged.contains(t) {
            merged.push(t.clone());
        }
    }
    merged
}

/// Format a list of tags as `[tag1, tag2]` or empty string if none.
pub fn format_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        String::new()
    } else {
        format!("[{}]", tags.join(", "))
    }
}

// ── State display ───────────────────────────────────────────────────

/// Format bookmark state for display.
pub fn format_state(state: &BookmarkState) -> &'static str {
    match state {
        BookmarkState::Inbox => "inbox",
        BookmarkState::Processed => "processed",
        BookmarkState::Archived => "archived",
    }
}

// ── List row formatting ─────────────────────────────────────────────

/// Format a single list row: `date  title  [tags]  state`
///
/// Width-aware: title is truncated to fit available space.
pub fn format_list_row(bookmark: &Bookmark, width: usize) -> String {
    let date = bookmark.saved_at.format("%Y-%m-%d").to_string();
    let state = format_state(&bookmark.state);
    let merged = merge_tags(&bookmark.user_tags, &bookmark.suggested_tags);
    let tags = format_tags(&merged);

    // Fixed parts: date(10) + 2 spaces + state(max 9) + 2 spaces = ~23 min
    // Tags get up to their natural length; title gets the remainder
    let fixed_overhead = date.len() + 2 + state.len() + 2;
    let tags_with_sep = if tags.is_empty() {
        0
    } else {
        tags.len() + 2 // 2 spaces before tags
    };
    let total_fixed = fixed_overhead + tags_with_sep;

    let title_budget = if width > total_fixed + 5 {
        width - total_fixed
    } else {
        // Narrow terminal — give title at least 10 chars, let it wrap
        60.min(width.saturating_sub(4))
    };

    let title = truncate(&bookmark.title, title_budget);

    if tags.is_empty() {
        format!("{date}  {title}  {state}")
    } else {
        format!("{date}  {title}  {tags}  {state}")
    }
}

/// Format the complete list output for a set of bookmarks.
///
/// Returns an empty-state message if the list is empty.
pub fn format_list(bookmarks: &[Bookmark], width: usize) -> String {
    if bookmarks.is_empty() {
        return "No bookmarks found.".to_string();
    }

    bookmarks
        .iter()
        .map(|b| format_list_row(b, width))
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Show detail formatting ──────────────────────────────────────────

/// A container for show command data from both DB and bundle.
pub struct ShowDetail<'a> {
    pub bookmark: &'a Bookmark,
    pub summary: Option<String>,
    pub article: Option<String>,
    pub full: bool,
}

/// Format the full detail view for a bookmark.
pub fn format_show(detail: &ShowDetail<'_>, use_color: bool) -> String {
    let bm = detail.bookmark;
    let mut out = String::new();

    let header = |label: &str| -> String {
        if use_color {
            format!("\x1b[1m{label}\x1b[0m")
        } else {
            label.to_string()
        }
    };

    // Title
    out.push_str(&header(&bm.title));
    out.push('\n');

    // ID and URL
    out.push_str(&format!("ID:     {}\n", bm.id));
    out.push_str(&format!("URL:    {}\n", bm.url));
    if bm.canonical_url != bm.url {
        out.push_str(&format!("Canon:  {}\n", bm.canonical_url));
    }

    // Metadata
    out.push_str(&format!(
        "Saved:  {}\n",
        bm.saved_at.format("%Y-%m-%d %H:%M:%S UTC")
    ));
    out.push_str(&format!("State:  {}\n", format_state(&bm.state)));

    if let Some(ref author) = bm.author {
        out.push_str(&format!("Author: {author}\n"));
    }
    if let Some(ref site) = bm.site_name {
        out.push_str(&format!("Site:   {site}\n"));
    }
    if let Some(ref desc) = bm.description {
        out.push_str(&format!("Desc:   {desc}\n"));
    }
    if let Some(ref pub_at) = bm.published_at {
        out.push_str(&format!("Pub:    {pub_at}\n"));
    }

    // Tags
    let merged = merge_tags(&bm.user_tags, &bm.suggested_tags);
    if !merged.is_empty() {
        out.push_str(&format!("Tags:   {}\n", merged.join(", ")));
    }

    // Collections
    if !bm.collections.is_empty() {
        out.push_str(&format!("Colls:  {}\n", bm.collections.join(", ")));
    }

    // Note
    if let Some(ref note) = bm.note {
        out.push_str(&format!("\n{}\n{note}\n", header("Note")));
    }

    // Action prompt
    if let Some(ref action) = bm.action_prompt {
        out.push_str(&format!("\n{}\n{action}\n", header("Action")));
    }

    // Summary
    out.push('\n');
    out.push_str(&header("Summary"));
    out.push('\n');
    match &detail.summary {
        Some(s) if !s.is_empty() => out.push_str(s),
        _ => out.push_str("[enrichment pending]"),
    }
    out.push('\n');

    // Article preview/full
    out.push('\n');
    out.push_str(&header("Article"));
    out.push('\n');
    match &detail.article {
        Some(article) if !article.is_empty() => {
            if detail.full {
                out.push_str(article);
            } else {
                let lines: Vec<&str> = article.lines().collect();
                let preview_lines = lines.len().min(20);
                for line in &lines[..preview_lines] {
                    out.push_str(line);
                    out.push('\n');
                }
                if lines.len() > 20 {
                    out.push_str(&format!(
                        "\n... ({} more lines, use --full to see all)\n",
                        lines.len() - 20
                    ));
                }
            }
        }
        _ => {
            out.push_str("[no article content]");
            out.push('\n');
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Bookmark;

    fn test_bookmark(title: &str) -> Bookmark {
        Bookmark::new("https://example.com", title)
    }

    // ── Truncation tests ────────────────────────────────────────────

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_long_string_adds_ellipsis() {
        assert_eq!(truncate("hello world", 5), "hell…");
    }

    #[test]
    fn truncate_zero_returns_empty() {
        assert_eq!(truncate("hello", 0), "");
    }

    #[test]
    fn truncate_one_returns_ellipsis() {
        assert_eq!(truncate("hello", 1), "…");
    }

    #[test]
    fn truncate_handles_unicode() {
        // 4 chars: 日本語テ + truncation
        assert_eq!(truncate("日本語テスト", 4), "日本語…");
    }

    // ── Tag merging tests ───────────────────────────────────────────

    #[test]
    fn merge_tags_empty_both() {
        let result = merge_tags(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn merge_tags_deduplicates() {
        let user = vec!["rust".to_string(), "cli".to_string()];
        let suggested = vec!["cli".to_string(), "tools".to_string()];
        let result = merge_tags(&user, &suggested);
        assert_eq!(result, vec!["rust", "cli", "tools"]);
    }

    #[test]
    fn merge_tags_user_first() {
        let user = vec!["a".to_string()];
        let suggested = vec!["b".to_string()];
        let result = merge_tags(&user, &suggested);
        assert_eq!(result, vec!["a", "b"]);
    }

    // ── Format tags tests ───────────────────────────────────────────

    #[test]
    fn format_tags_empty() {
        assert_eq!(format_tags(&[]), "");
    }

    #[test]
    fn format_tags_single() {
        assert_eq!(format_tags(&["rust".to_string()]), "[rust]");
    }

    #[test]
    fn format_tags_multiple() {
        let tags = vec!["rust".to_string(), "cli".to_string()];
        assert_eq!(format_tags(&tags), "[rust, cli]");
    }

    // ── List row formatting tests ───────────────────────────────────

    #[test]
    fn format_list_row_basic() {
        let bm = test_bookmark("My Article");
        let row = format_list_row(&bm, 80);
        assert!(row.contains("My Article"));
        assert!(row.contains("inbox"));
        // Date is included
        assert!(row.len() > 20);
    }

    #[test]
    fn format_list_row_with_tags() {
        let mut bm = test_bookmark("Test");
        bm.user_tags = vec!["rust".to_string()];
        let row = format_list_row(&bm, 80);
        assert!(row.contains("[rust]"));
    }

    #[test]
    fn format_list_row_no_tags() {
        let bm = test_bookmark("Test");
        let row = format_list_row(&bm, 80);
        assert!(!row.contains('['));
    }

    #[test]
    fn format_list_row_narrow_width_no_panic() {
        let bm = test_bookmark("A Very Long Article Title That Should Get Truncated");
        let row = format_list_row(&bm, 40);
        assert!(!row.is_empty());
    }

    #[test]
    fn format_list_empty_returns_message() {
        let output = format_list(&[], 80);
        assert_eq!(output, "No bookmarks found.");
    }

    // ── Show formatting tests ───────────────────────────────────────

    #[test]
    fn format_show_no_color() {
        let bm = test_bookmark("Test Article");
        let detail = ShowDetail {
            bookmark: &bm,
            summary: Some("A summary.".to_string()),
            article: Some("Line 1\nLine 2\n".to_string()),
            full: false,
        };
        let output = format_show(&detail, false);
        assert!(output.contains("Test Article"));
        assert!(output.contains("A summary."));
        assert!(output.contains("Line 1"));
        // No ANSI escapes
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn format_show_with_color() {
        let bm = test_bookmark("Test");
        let detail = ShowDetail {
            bookmark: &bm,
            summary: None,
            article: None,
            full: false,
        };
        let output = format_show(&detail, true);
        assert!(output.contains("\x1b[1m"));
    }

    #[test]
    fn format_show_pending_summary() {
        let bm = test_bookmark("Test");
        let detail = ShowDetail {
            bookmark: &bm,
            summary: None,
            article: Some("content".to_string()),
            full: false,
        };
        let output = format_show(&detail, false);
        assert!(output.contains("[enrichment pending]"));
    }

    #[test]
    fn format_show_empty_article() {
        let bm = test_bookmark("Test");
        let detail = ShowDetail {
            bookmark: &bm,
            summary: Some("sum".to_string()),
            article: Some(String::new()),
            full: false,
        };
        let output = format_show(&detail, false);
        assert!(output.contains("[no article content]"));
    }

    #[test]
    fn format_show_preview_limits_to_20_lines() {
        let bm = test_bookmark("Test");
        let article = (0..30)
            .map(|i| format!("Line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let detail = ShowDetail {
            bookmark: &bm,
            summary: None,
            article: Some(article),
            full: false,
        };
        let output = format_show(&detail, false);
        assert!(output.contains("Line 0"));
        assert!(output.contains("Line 19"));
        assert!(!output.contains("Line 20\n"));
        assert!(output.contains("more lines"));
    }

    #[test]
    fn format_show_full_includes_all_lines() {
        let bm = test_bookmark("Test");
        let article = (0..30)
            .map(|i| format!("Line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let detail = ShowDetail {
            bookmark: &bm,
            summary: None,
            article: Some(article),
            full: true,
        };
        let output = format_show(&detail, false);
        assert!(output.contains("Line 29"));
        assert!(!output.contains("more lines"));
    }

    #[test]
    fn format_show_article_under_20_lines_shows_all() {
        let bm = test_bookmark("Test");
        let article = "Line 1\nLine 2\nLine 3\n";
        let detail = ShowDetail {
            bookmark: &bm,
            summary: None,
            article: Some(article.to_string()),
            full: false,
        };
        let output = format_show(&detail, false);
        assert!(output.contains("Line 3"));
        assert!(!output.contains("more lines"));
    }
}
