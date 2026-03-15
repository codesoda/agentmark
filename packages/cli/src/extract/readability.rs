//! Readability-style article extraction with semantic fallback.
//!
//! Accepts raw HTML and returns cleaned article HTML suitable for markdown
//! conversion. Uses the `readability` crate as a baseline, falling back to
//! semantic container extraction (`<article>`, `<main>`, `[role="main"]`)
//! when readability returns too little content.

use scraper::{Html, Selector};

/// Minimum character threshold for readability output before triggering fallback.
const MIN_CONTENT_LENGTH: usize = 100;

/// Tags that should be removed from extracted article HTML.
const NOISY_TAGS: &[&str] = &[
    "nav", "aside", "footer", "header", "script", "style", "noscript", "form", "iframe", "svg",
    "button", "input", "select", "textarea",
];

/// Attributes that should be stripped from retained elements.
const UNSAFE_ATTRS: &[&str] = &[
    "style",
    "onclick",
    "onload",
    "onerror",
    "onmouseover",
    "onmouseout",
    "onfocus",
    "onblur",
    "onsubmit",
    "onchange",
    "onkeydown",
    "onkeyup",
    "onkeypress",
];

/// Extract article HTML from raw page HTML.
///
/// Returns cleaned HTML of the article body, or an empty string if no
/// meaningful content can be identified. Never panics on malformed input.
pub fn extract_article(html: &str) -> String {
    if html.trim().is_empty() {
        return String::new();
    }

    // Try readability crate first
    let readability_result = try_readability(html);
    if text_length(&readability_result) >= MIN_CONTENT_LENGTH {
        return cleanup_html(&readability_result);
    }

    // Fallback: try semantic containers
    let fallback = try_semantic_fallback(html);
    if text_length(&fallback) >= MIN_CONTENT_LENGTH {
        return cleanup_html(&fallback);
    }

    // Last resort: use body text if it has any content
    let body = try_body_fallback(html);
    if !body.trim().is_empty() {
        return cleanup_html(&body);
    }

    String::new()
}

/// Try the readability crate for article extraction.
fn try_readability(html: &str) -> String {
    // The readability crate needs a mutable Read source
    let mut cursor = std::io::Cursor::new(html.as_bytes());
    match readability::extractor::extract(
        &mut cursor,
        &url::Url::parse("https://example.com").unwrap(),
    ) {
        Ok(product) => product.content,
        Err(_) => String::new(),
    }
}

/// Try semantic container elements as fallback.
fn try_semantic_fallback(html: &str) -> String {
    let document = Html::parse_document(html);

    // Try selectors in priority order
    let selectors = [
        "article",
        "main",
        "[role=\"main\"]",
        ".post-content",
        ".entry-content",
    ];

    for sel_str in &selectors {
        if let Ok(selector) = Selector::parse(sel_str) {
            if let Some(element) = document.select(&selector).next() {
                return element.html();
            }
        }
    }

    String::new()
}

/// Last-resort fallback: extract body content.
fn try_body_fallback(html: &str) -> String {
    let document = Html::parse_document(html);
    if let Ok(selector) = Selector::parse("body") {
        if let Some(body) = document.select(&selector).next() {
            return body.html();
        }
    }
    String::new()
}

/// Remove noisy elements and unsafe attributes from HTML.
fn cleanup_html(html: &str) -> String {
    let document = Html::parse_fragment(html);
    let mut output = String::with_capacity(html.len());
    render_cleaned(&document.root_element(), &mut output);
    output
}

/// Recursively render cleaned HTML, skipping noisy tags and unsafe attributes.
fn render_cleaned(node: &scraper::ElementRef, output: &mut String) {
    use scraper::Node;

    for child in node.children() {
        match child.value() {
            Node::Text(text) => {
                output.push_str(text);
            }
            Node::Element(elem) => {
                let tag = elem.name().to_lowercase();

                // Skip noisy tags entirely
                if NOISY_TAGS.contains(&tag.as_str()) {
                    continue;
                }

                let child_ref = scraper::ElementRef::wrap(child);
                if let Some(child_elem) = child_ref {
                    // Write opening tag with filtered attributes
                    output.push('<');
                    output.push_str(&tag);

                    for (attr_name, attr_val) in elem.attrs() {
                        let lower_name = attr_name.to_lowercase();
                        if UNSAFE_ATTRS.contains(&lower_name.as_str()) {
                            continue;
                        }
                        // Skip event handlers not in our list
                        if lower_name.starts_with("on") {
                            continue;
                        }
                        output.push(' ');
                        output.push_str(attr_name);
                        output.push_str("=\"");
                        output.push_str(&html_escape_attr(attr_val));
                        output.push('"');
                    }
                    output.push('>');

                    // Recurse into children
                    render_cleaned(&child_elem, output);

                    // Close tag (skip void elements)
                    if !is_void_element(&tag) {
                        output.push_str("</");
                        output.push_str(&tag);
                        output.push('>');
                    }
                }
            }
            _ => {}
        }
    }
}

/// Minimal HTML attribute escaping.
fn html_escape_attr(val: &str) -> String {
    val.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Check if a tag is a void element (self-closing).
fn is_void_element(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

/// Count visible text characters in an HTML string (rough heuristic).
fn text_length(html: &str) -> usize {
    let fragment = Html::parse_fragment(html);
    fragment.root_element().text().map(|t| t.trim().len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_article_from_blog_post() {
        let html = r#"<!DOCTYPE html>
<html><head><title>Test</title></head>
<body>
<nav><a href="/">Home</a><a href="/about">About</a></nav>
<article>
<h1>My Blog Post</h1>
<p>This is a substantial blog post with enough content to pass the minimum threshold.
It discusses many interesting topics including technology, science, and art.
The post goes on to explore various themes in great detail.</p>
<p>Another paragraph with more content to ensure we have enough text for extraction.</p>
</article>
<footer><p>Copyright 2024</p></footer>
</body></html>"#;

        let result = extract_article(html);
        assert!(!result.is_empty(), "Should extract article content");
        assert!(
            result.contains("My Blog Post") || result.contains("blog post"),
            "Should contain article text"
        );
        // Nav and footer should be stripped by cleanup
        assert!(!result.contains("<nav>"), "Should not contain nav");
        assert!(!result.contains("<footer>"), "Should not contain footer");
    }

    #[test]
    fn strips_nav_sidebar_footer() {
        let html = r#"<div>
<nav><ul><li>Menu 1</li><li>Menu 2</li></ul></nav>
<aside><p>Sidebar content</p></aside>
<main>
<h1>Main Article</h1>
<p>This is the main content of the page with enough text to be meaningful.
It contains multiple sentences and paragraphs of real content that should be extracted.</p>
</main>
<footer><p>Footer content here</p></footer>
</div>"#;

        let result = extract_article(html);
        assert!(!result.contains("<nav>"), "Nav should be stripped");
        assert!(!result.contains("<aside>"), "Aside should be stripped");
        assert!(!result.contains("<footer>"), "Footer should be stripped");
        assert!(
            result.contains("Main Article") || result.contains("main content"),
            "Main content should be preserved"
        );
    }

    #[test]
    fn handles_mostly_media_page() {
        let html = r#"<html><body>
<article>
<h1>Photo Gallery</h1>
<img src="photo1.jpg" alt="A beautiful sunset">
<img src="photo2.jpg" alt="Mountain landscape">
<p>Captions for the photos above.</p>
</article>
</body></html>"#;

        let result = extract_article(html);
        // Should extract whatever text exists
        assert!(
            result.contains("Photo Gallery") || result.contains("Captions"),
            "Should preserve available text"
        );
    }

    #[test]
    fn handles_no_article_content() {
        let html = "<html><body><nav><a href='/'>Home</a></nav></body></html>";
        let result = extract_article(html);
        // May be empty or minimal — should not panic
        assert!(
            result.is_empty() || text_length(&result) < MIN_CONTENT_LENGTH,
            "Should return empty or minimal content for nav-only page"
        );
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(extract_article(""), "");
        assert_eq!(extract_article("   "), "");
    }

    #[test]
    fn handles_malformed_html() {
        let html = "<div><p>Unclosed paragraph<p>Another one<span>Nested unclosed";
        // Should not panic
        let _ = extract_article(html);
    }

    #[test]
    fn strips_script_and_style_tags() {
        let html = r#"<article>
<h1>Article Title</h1>
<script>alert('xss')</script>
<style>.foo { color: red; }</style>
<p>This is the real content of the article with enough text to be meaningful and pass thresholds.</p>
</article>"#;

        let result = extract_article(html);
        assert!(!result.contains("<script>"), "Script should be stripped");
        assert!(!result.contains("<style>"), "Style should be stripped");
        assert!(
            !result.contains("alert"),
            "Script content should be stripped"
        );
    }

    #[test]
    fn strips_inline_event_handlers() {
        let html = r#"<article>
<h1>Article</h1>
<p onclick="alert('xss')" onmouseover="hack()">Content with enough text for extraction threshold to be met easily here.</p>
</article>"#;

        let result = extract_article(html);
        assert!(!result.contains("onclick"), "onclick should be stripped");
        assert!(
            !result.contains("onmouseover"),
            "onmouseover should be stripped"
        );
    }

    #[test]
    fn semantic_fallback_finds_main() {
        // Content that readability might not score well but has clear semantic structure
        let html = r#"<!DOCTYPE html><html><body>
<header><h1>Site Name</h1></header>
<main>
<p>This is the main content area with substantial text that should be found by the semantic fallback.
It contains enough content to pass the minimum threshold for extraction.</p>
</main>
</body></html>"#;

        let result = extract_article(html);
        assert!(
            result.contains("main content area"),
            "Should find content in <main>"
        );
    }

    #[test]
    fn text_length_counts_visible_text() {
        assert_eq!(text_length("<p>Hello</p>"), 5);
        assert_eq!(text_length("<div><p>  Hello  </p><p>World</p></div>"), 10);
        assert_eq!(text_length(""), 0);
    }
}
