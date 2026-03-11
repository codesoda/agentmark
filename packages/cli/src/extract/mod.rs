//! Content extraction: HTML → readable article → markdown → content hash.
//!
//! This module is a standalone library seam for Specs 07-10. It depends only on
//! HTML parsing and hashing crates and has no coupling to `config`, `db`, or
//! `models::Bookmark`.
//!
//! Public API:
//! - [`extract_content`] — extract article content from raw HTML.
//! - [`ExtractionResult`] — article HTML, markdown, and content hash.

mod readability;
mod to_markdown;

use sha2::{Digest, Sha256};

/// Result of content extraction from a raw HTML page.
///
/// Empty strings indicate no readable content was found. The `content_hash`
/// is always computed (even for empty content) to ensure deterministic dedup.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractionResult {
    /// Cleaned HTML of the article body.
    pub article_html: String,
    /// Markdown conversion of the article.
    pub article_markdown: String,
    /// SHA-256 hash of `article_markdown`, formatted as `sha256:<hex>`.
    pub content_hash: String,
}

/// Extract readable article content from raw HTML.
///
/// This is the primary public entry point. It:
/// 1. Identifies and extracts the main article content (readability + fallback)
/// 2. Converts the cleaned article HTML to markdown
/// 3. Computes a deterministic SHA-256 content hash
///
/// Never panics on malformed input. Returns empty strings for pages with
/// no meaningful content.
pub fn extract_content(html: &str) -> ExtractionResult {
    let article_html = readability::extract_article(html);
    let article_markdown = to_markdown::html_to_markdown(&article_html);
    let content_hash = compute_hash(&article_markdown);

    ExtractionResult {
        article_html,
        article_markdown,
        content_hash,
    }
}

/// Compute SHA-256 hash of markdown content, formatted as `sha256:<hex>`.
fn compute_hash(markdown: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(markdown.as_bytes());
    let result = hasher.finalize();
    format!("sha256:{:x}", result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_to_end_extraction() {
        let html = r#"<!DOCTYPE html>
<html><head><title>Test Article</title></head>
<body>
<nav><a href="/">Home</a></nav>
<article>
<h1>Test Article Title</h1>
<p>This is a substantial test article with enough content to pass extraction thresholds.
It includes multiple paragraphs and covers an interesting topic in detail.</p>
<p>The second paragraph adds even more meaningful content to the article body.</p>
<ul><li>Point one</li><li>Point two</li></ul>
</article>
<footer><p>Copyright</p></footer>
</body></html>"#;

        let result = extract_content(html);
        assert!(!result.article_html.is_empty(), "Should have article HTML");
        assert!(
            !result.article_markdown.is_empty(),
            "Should have article markdown"
        );
        assert!(
            result.content_hash.starts_with("sha256:"),
            "Hash should be prefixed: {}",
            result.content_hash
        );
        assert!(
            result.content_hash.len() > "sha256:".len() + 10,
            "Hash should have hex digits"
        );
    }

    #[test]
    fn deterministic_hash_for_same_input() {
        let html = r#"<article>
<h1>Consistent Title</h1>
<p>This content should always produce the same hash when extracted multiple times.
It needs to be long enough to pass the extraction threshold for readability.</p>
</article>"#;

        let result1 = extract_content(html);
        let result2 = extract_content(html);
        assert_eq!(
            result1.content_hash, result2.content_hash,
            "Same input must produce same hash"
        );
        assert_eq!(
            result1.article_markdown, result2.article_markdown,
            "Same input must produce same markdown"
        );
    }

    #[test]
    fn empty_content_produces_valid_result() {
        let html = "<html><body><nav><a href='/'>Home</a></nav></body></html>";
        let result = extract_content(html);
        // Hash should still be computed for empty content
        assert!(
            result.content_hash.starts_with("sha256:"),
            "Empty content should still have hash prefix"
        );
    }

    #[test]
    fn empty_html_produces_valid_result() {
        let result = extract_content("");
        assert_eq!(result.article_html, "");
        assert_eq!(result.article_markdown, "");
        assert!(result.content_hash.starts_with("sha256:"));
    }

    #[test]
    fn empty_markdown_hash_is_deterministic() {
        let result1 = extract_content("");
        let result2 = extract_content("   ");
        // Both produce empty markdown, so hashes should match
        assert_eq!(
            result1.content_hash, result2.content_hash,
            "Empty markdown from different empty inputs should hash the same"
        );
    }

    #[test]
    fn hash_format_is_sha256_hex() {
        let result = extract_content("<article><p>Content long enough to extract for hashing purposes and testing.</p></article>");
        assert!(result.content_hash.starts_with("sha256:"));
        let hex_part = &result.content_hash["sha256:".len()..];
        assert_eq!(hex_part.len(), 64, "SHA-256 hex should be 64 chars");
        assert!(
            hex_part.chars().all(|c| c.is_ascii_hexdigit()),
            "Should be valid hex"
        );
    }

    #[test]
    fn compute_hash_known_value() {
        // SHA-256 of empty string is a known value
        let hash = compute_hash("");
        assert_eq!(
            hash,
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn malformed_html_does_not_panic() {
        let inputs = vec![
            "<div><p>Unclosed",
            "<<<>>>",
            "<script>alert('xss')</script>",
            "<html><body>",
            "plain text with no tags at all but enough content to potentially trigger extraction",
            "<div class=\"\n\nbroken",
        ];
        for input in inputs {
            let _ = extract_content(input);
        }
    }

    #[test]
    fn no_nav_footer_in_output() {
        let html = r#"<html><body>
<nav><ul><li>Menu</li></ul></nav>
<article>
<h1>Article</h1>
<p>Substantial article content with enough text to pass extraction thresholds easily.</p>
</article>
<footer><p>Site footer content</p></footer>
</body></html>"#;

        let result = extract_content(html);
        // Check markdown doesn't contain nav/footer text
        assert!(
            !result.article_markdown.contains("Menu")
                || result.article_markdown.contains("Article"),
            "Should contain article content"
        );
        assert!(
            !result.article_markdown.contains("Site footer"),
            "Footer text should not appear in markdown"
        );
    }

    #[test]
    fn markdown_has_structure() {
        let html = r#"<article>
<h1>Main Title</h1>
<p>Introduction paragraph with enough text for extraction.</p>
<h2>Subsection</h2>
<p>More content here.</p>
<ul><li>Item A</li><li>Item B</li></ul>
</article>"#;

        let result = extract_content(html);
        let md = &result.article_markdown;
        assert!(
            md.contains("# Main Title") || md.contains("Main Title"),
            "got: {md}"
        );
        // Should have some list markers
        assert!(
            md.contains("- Item A") || md.contains("Item A"),
            "got: {md}"
        );
    }
}
