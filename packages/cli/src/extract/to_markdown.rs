//! HTML → markdown conversion for cleaned article fragments.
//!
//! Converts a limited set of article HTML tags into deterministic markdown.
//! Designed to operate on already-cleaned HTML from `readability.rs`, not raw pages.

use scraper::{ElementRef, Html, Node};

/// Allowed URL schemes for links and images.
const SAFE_SCHEMES: &[&str] = &["http:", "https:", "mailto:"];

/// Convert cleaned article HTML to markdown.
///
/// Produces deterministic output suitable for hashing. Unsupported tags are
/// treated as transparent containers (their children are rendered inline).
pub fn html_to_markdown(html: &str) -> String {
    if html.trim().is_empty() {
        return String::new();
    }

    let fragment = Html::parse_fragment(html);
    let mut buf = String::with_capacity(html.len());
    render_node_children(&fragment.root_element(), &mut buf, &Context::default());
    normalize_whitespace(&buf)
}

/// Rendering context passed through recursion.
#[derive(Default, Clone)]
struct Context {
    /// Current list nesting depth.
    list_depth: usize,
    /// Whether we're inside a <pre> block.
    in_pre: bool,
    /// Whether we're inside a blockquote.
    in_blockquote: bool,
    /// Current ordered list counter (Some for <ol>, None for <ul>).
    list_counter: Option<usize>,
}

/// Render all children of a node.
fn render_node_children(elem: &ElementRef, buf: &mut String, ctx: &Context) {
    for child in elem.children() {
        render_node(child, buf, ctx);
    }
}

/// Render a single DOM node.
fn render_node(node: ego_tree::NodeRef<Node>, buf: &mut String, ctx: &Context) {
    match node.value() {
        Node::Text(text) => {
            let s: &str = text;
            if ctx.in_pre {
                buf.push_str(s);
            } else {
                // Collapse whitespace in normal flow
                let collapsed = collapse_whitespace(s);
                if !collapsed.is_empty() {
                    buf.push_str(&collapsed);
                }
            }
        }
        Node::Element(elem) => {
            if let Some(elem_ref) = ElementRef::wrap(node) {
                render_element(&elem_ref, elem.name(), buf, ctx);
            }
        }
        _ => {}
    }
}

/// Render an HTML element as markdown.
fn render_element(elem: &ElementRef, tag: &str, buf: &mut String, ctx: &Context) {
    match tag.to_lowercase().as_str() {
        "h1" => render_heading(elem, 1, buf, ctx),
        "h2" => render_heading(elem, 2, buf, ctx),
        "h3" => render_heading(elem, 3, buf, ctx),
        "h4" => render_heading(elem, 4, buf, ctx),
        "h5" => render_heading(elem, 5, buf, ctx),
        "h6" => render_heading(elem, 6, buf, ctx),
        "p" => render_block(elem, buf, ctx, "", ""),
        "br" => buf.push('\n'),
        "hr" => {
            ensure_blank_line(buf);
            buf.push_str("---\n\n");
        }
        "blockquote" => render_blockquote(elem, buf, ctx),
        "ul" => render_list(elem, false, buf, ctx),
        "ol" => render_list(elem, true, buf, ctx),
        "li" => render_list_item(elem, buf, ctx),
        "pre" => render_pre(elem, buf, ctx),
        "code" => {
            if !ctx.in_pre {
                render_inline_code(elem, buf);
            } else {
                // Inside <pre>, just render children directly
                render_node_children(elem, buf, ctx);
            }
        }
        "a" => render_link(elem, buf, ctx),
        "img" => render_image(elem, buf),
        "strong" | "b" => render_inline_wrap(elem, "**", buf, ctx),
        "em" | "i" => render_inline_wrap(elem, "_", buf, ctx),
        "del" | "s" | "strike" => render_inline_wrap(elem, "~~", buf, ctx),
        // Transparent containers — render children inline
        _ => render_node_children(elem, buf, ctx),
    }
}

/// Render a heading (# ... ######).
fn render_heading(elem: &ElementRef, level: usize, buf: &mut String, ctx: &Context) {
    ensure_blank_line(buf);
    if ctx.in_blockquote {
        buf.push_str("> ");
    }
    for _ in 0..level {
        buf.push('#');
    }
    buf.push(' ');
    let text = collect_inline_text(elem, ctx);
    buf.push_str(text.trim());
    buf.push_str("\n\n");
}

/// Render a block element (paragraph, div, etc.).
fn render_block(elem: &ElementRef, buf: &mut String, ctx: &Context, prefix: &str, suffix: &str) {
    ensure_blank_line(buf);
    if ctx.in_blockquote {
        buf.push_str("> ");
    }
    buf.push_str(prefix);
    let text = collect_inline_text(elem, ctx);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }
    if ctx.in_blockquote {
        // Wrap long blockquote lines with > prefix
        buf.push_str(&trimmed.replace('\n', "\n> "));
    } else {
        buf.push_str(trimmed);
    }
    buf.push_str(suffix);
    buf.push_str("\n\n");
}

/// Render a blockquote.
fn render_blockquote(elem: &ElementRef, buf: &mut String, ctx: &Context) {
    ensure_blank_line(buf);
    let mut inner_ctx = ctx.clone();
    inner_ctx.in_blockquote = true;

    let mut inner = String::new();
    render_node_children(elem, &mut inner, &inner_ctx);
    let inner = inner.trim();

    if inner.is_empty() {
        return;
    }

    // Prefix each line with >
    for line in inner.lines() {
        buf.push_str("> ");
        buf.push_str(line);
        buf.push('\n');
    }
    buf.push('\n');
}

/// Render an unordered or ordered list.
fn render_list(elem: &ElementRef, ordered: bool, buf: &mut String, ctx: &Context) {
    ensure_blank_line(buf);
    let mut list_ctx = ctx.clone();
    list_ctx.list_depth = ctx.list_depth + 1;
    list_ctx.list_counter = if ordered { Some(1) } else { None };

    for child in elem.children() {
        if let Some(child_elem) = ElementRef::wrap(child) {
            if child_elem.value().name() == "li" {
                render_list_item(&child_elem, buf, &list_ctx);
                if let Some(ref mut counter) = list_ctx.list_counter {
                    *counter += 1;
                }
            }
        }
    }
    // Ensure blank line after list
    if !buf.ends_with("\n\n") {
        buf.push('\n');
    }
}

/// Render a single list item.
fn render_list_item(elem: &ElementRef, buf: &mut String, ctx: &Context) {
    let indent = "  ".repeat(ctx.list_depth.saturating_sub(1));
    let marker = match ctx.list_counter {
        Some(n) => format!("{}. ", n),
        None => "- ".to_string(),
    };

    buf.push_str(&indent);
    buf.push_str(&marker);

    // Collect item content
    let inner = collect_inline_text(elem, ctx);
    let trimmed = inner.trim();

    // Handle multi-line items by indenting continuation lines
    let continuation_indent = " ".repeat(indent.len() + marker.len());
    let mut first = true;
    for line in trimmed.lines() {
        if first {
            buf.push_str(line.trim());
            first = false;
        } else {
            buf.push('\n');
            buf.push_str(&continuation_indent);
            buf.push_str(line.trim());
        }
    }
    buf.push('\n');
}

/// Render a `<pre>` block as a fenced code block.
fn render_pre(elem: &ElementRef, buf: &mut String, ctx: &Context) {
    ensure_blank_line(buf);
    buf.push_str("```\n");

    let mut pre_ctx = ctx.clone();
    pre_ctx.in_pre = true;

    let mut inner = String::new();
    render_node_children(elem, &mut inner, &pre_ctx);

    // Trim trailing whitespace but preserve internal structure
    let trimmed = inner.trim_end();
    buf.push_str(trimmed);
    buf.push_str("\n```\n\n");
}

/// Render inline `<code>` with backticks.
fn render_inline_code(elem: &ElementRef, buf: &mut String) {
    let text: String = elem.text().collect();
    if text.is_empty() {
        return;
    }
    // Use double backticks if content contains single backtick
    if text.contains('`') {
        buf.push_str("`` ");
        buf.push_str(&text);
        buf.push_str(" ``");
    } else {
        buf.push('`');
        buf.push_str(&text);
        buf.push('`');
    }
}

/// Render a link as markdown `[text](url)` or plain text if URL is unsafe.
fn render_link(elem: &ElementRef, buf: &mut String, ctx: &Context) {
    let href = elem.value().attr("href").unwrap_or("");
    let text = collect_inline_text(elem, ctx);
    let trimmed_text = text.trim();

    if trimmed_text.is_empty() {
        return;
    }

    if is_safe_url(href) && !href.is_empty() {
        buf.push('[');
        buf.push_str(trimmed_text);
        buf.push_str("](");
        buf.push_str(href);
        buf.push(')');
    } else {
        // Unsafe or missing URL — render as plain text
        buf.push_str(trimmed_text);
    }
}

/// Render an image as markdown `![alt](src)`.
fn render_image(elem: &ElementRef, buf: &mut String) {
    let src = elem.value().attr("src").unwrap_or("");
    let alt = elem.value().attr("alt").unwrap_or("");

    if !is_safe_url(src) || src.is_empty() {
        // If there's alt text, show it as plain text
        if !alt.is_empty() {
            buf.push_str(alt);
        }
        return;
    }

    buf.push_str("![");
    buf.push_str(alt);
    buf.push_str("](");
    buf.push_str(src);
    buf.push(')');
}

/// Wrap inline content with a markdown marker (**, _, ~~).
fn render_inline_wrap(elem: &ElementRef, marker: &str, buf: &mut String, ctx: &Context) {
    let text = collect_inline_text(elem, ctx);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }
    buf.push_str(marker);
    buf.push_str(trimmed);
    buf.push_str(marker);
}

/// Collect inline text from an element, recursively processing children.
fn collect_inline_text(elem: &ElementRef, ctx: &Context) -> String {
    let mut buf = String::new();
    render_node_children(elem, &mut buf, ctx);
    buf
}

/// Check if a URL has a safe scheme.
fn is_safe_url(url: &str) -> bool {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Relative URLs are safe
    if !trimmed.contains(':') {
        return true;
    }

    // Fragment-only URLs are safe
    if trimmed.starts_with('#') {
        return true;
    }

    // Protocol-relative URLs are safe
    if trimmed.starts_with("//") {
        return true;
    }

    // Check against allowed schemes (case-insensitive)
    let lower = trimmed.to_lowercase();
    SAFE_SCHEMES.iter().any(|s| lower.starts_with(s))
}

/// Ensure the buffer ends with a blank line (for block-level separation).
fn ensure_blank_line(buf: &mut String) {
    if buf.is_empty() {
        return;
    }
    if !buf.ends_with('\n') {
        buf.push('\n');
    }
    if !buf.ends_with("\n\n") {
        buf.push('\n');
    }
}

/// Collapse runs of whitespace into a single space.
fn collapse_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_was_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }
    result
}

/// Normalize final markdown output: collapse excessive blank lines, trim.
fn normalize_whitespace(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut blank_count = 0;

    for line in md.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                result.push('\n');
            }
        } else {
            if blank_count > 0 && !result.is_empty() {
                // Ensure we have exactly one blank line between blocks
                if !result.ends_with('\n') {
                    result.push('\n');
                }
            }
            blank_count = 0;
            result.push_str(line);
            result.push('\n');
        }
    }

    let trimmed = result.trim().to_string();
    if trimmed.is_empty() {
        return String::new();
    }
    // End with a single trailing newline
    format!("{}\n", trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_headings() {
        let html = "<h1>Title</h1><h2>Subtitle</h2><h3>Section</h3>";
        let md = html_to_markdown(html);
        assert!(md.contains("# Title"), "h1 → # Title: got {md}");
        assert!(md.contains("## Subtitle"), "h2 → ## Subtitle");
        assert!(md.contains("### Section"), "h3 → ### Section");
    }

    #[test]
    fn converts_paragraphs() {
        let html = "<p>First paragraph.</p><p>Second paragraph.</p>";
        let md = html_to_markdown(html);
        assert!(md.contains("First paragraph."));
        assert!(md.contains("Second paragraph."));
        // Paragraphs should be separated by blank lines
        assert!(md.contains("First paragraph.\n\nSecond paragraph."));
    }

    #[test]
    fn converts_unordered_list() {
        let html = "<ul><li>Alpha</li><li>Beta</li><li>Gamma</li></ul>";
        let md = html_to_markdown(html);
        assert!(md.contains("- Alpha"), "got: {md}");
        assert!(md.contains("- Beta"));
        assert!(md.contains("- Gamma"));
    }

    #[test]
    fn converts_ordered_list() {
        let html = "<ol><li>First</li><li>Second</li><li>Third</li></ol>";
        let md = html_to_markdown(html);
        assert!(md.contains("1. First"), "got: {md}");
        assert!(md.contains("2. Second"));
        assert!(md.contains("3. Third"));
    }

    #[test]
    fn converts_links() {
        let html = r#"<p>Visit <a href="https://example.com">Example</a> for more.</p>"#;
        let md = html_to_markdown(html);
        assert!(md.contains("[Example](https://example.com)"), "got: {md}");
    }

    #[test]
    fn converts_images() {
        let html = r#"<img src="https://example.com/photo.jpg" alt="A photo">"#;
        let md = html_to_markdown(html);
        assert!(
            md.contains("![A photo](https://example.com/photo.jpg)"),
            "got: {md}"
        );
    }

    #[test]
    fn converts_blockquotes() {
        let html = "<blockquote><p>A wise quote.</p></blockquote>";
        let md = html_to_markdown(html);
        assert!(md.contains("> A wise quote."), "got: {md}");
    }

    #[test]
    fn converts_code_blocks() {
        let html = "<pre><code>fn main() {\n    println!(\"hello\");\n}</code></pre>";
        let md = html_to_markdown(html);
        assert!(md.contains("```\nfn main()"), "got: {md}");
        assert!(md.contains("```"), "Should have closing fence");
    }

    #[test]
    fn converts_inline_code() {
        let html = "<p>Use <code>cargo build</code> to compile.</p>";
        let md = html_to_markdown(html);
        assert!(md.contains("`cargo build`"), "got: {md}");
    }

    #[test]
    fn converts_bold_and_italic() {
        let html = "<p><strong>Bold</strong> and <em>italic</em> text.</p>";
        let md = html_to_markdown(html);
        assert!(md.contains("**Bold**"), "got: {md}");
        assert!(md.contains("_italic_"));
    }

    #[test]
    fn strips_javascript_links() {
        let html = r#"<p><a href="javascript:alert('xss')">Click me</a></p>"#;
        let md = html_to_markdown(html);
        assert!(!md.contains("javascript:"), "Should strip javascript:");
        assert!(md.contains("Click me"), "Should keep visible text");
        assert!(!md.contains('['), "Should not be a markdown link");
    }

    #[test]
    fn strips_unsafe_schemes() {
        let html = r#"<a href="vbscript:foo">Link</a>"#;
        let md = html_to_markdown(html);
        assert!(!md.contains("vbscript:"));
        assert!(md.contains("Link"));
    }

    #[test]
    fn preserves_safe_relative_urls() {
        let html = r#"<a href="/about">About</a>"#;
        let md = html_to_markdown(html);
        assert!(md.contains("[About](/about)"), "got: {md}");
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(html_to_markdown(""), "");
        assert_eq!(html_to_markdown("   "), "");
    }

    #[test]
    fn handles_image_without_src() {
        let html = r#"<img alt="No source">"#;
        let md = html_to_markdown(html);
        // Should show alt text as plain text
        assert!(md.contains("No source"), "got: {md}");
        assert!(!md.contains("!["), "Should not be markdown image");
    }

    #[test]
    fn handles_image_without_alt() {
        let html = r#"<img src="https://example.com/photo.jpg">"#;
        let md = html_to_markdown(html);
        assert!(
            md.contains("![](https://example.com/photo.jpg)"),
            "got: {md}"
        );
    }

    #[test]
    fn normalizes_excessive_whitespace() {
        let html = "<p>  Hello   world  </p>\n\n\n\n<p>Next</p>";
        let md = html_to_markdown(html);
        // Should not have more than one blank line between paragraphs
        assert!(!md.contains("\n\n\n"), "No triple newlines, got: {md}");
        assert!(md.contains("Hello world"));
    }

    #[test]
    fn safe_url_checks() {
        assert!(is_safe_url("https://example.com"));
        assert!(is_safe_url("http://example.com"));
        assert!(is_safe_url("mailto:user@example.com"));
        assert!(is_safe_url("/relative/path"));
        assert!(is_safe_url("#fragment"));
        assert!(is_safe_url("//cdn.example.com/img.png"));
        assert!(!is_safe_url("javascript:alert(1)"));
        assert!(!is_safe_url("JAVASCRIPT:alert(1)"));
        assert!(!is_safe_url("vbscript:foo"));
        assert!(!is_safe_url("data:text/html,<script>alert(1)</script>"));
        assert!(!is_safe_url(""));
    }

    #[test]
    fn unsupported_tags_render_as_transparent() {
        let html = "<div><span>Hello</span> <section>World</section></div>";
        let md = html_to_markdown(html);
        assert!(md.contains("Hello"), "got: {md}");
        assert!(md.contains("World"));
    }

    #[test]
    fn pre_preserves_whitespace() {
        let html = "<pre>  indented\n    more indented\n</pre>";
        let md = html_to_markdown(html);
        assert!(md.contains("  indented"), "got: {md}");
        assert!(md.contains("    more indented"));
    }

    #[test]
    fn inline_code_with_backtick() {
        let html = "<p>Use <code>it`s</code> carefully.</p>";
        let md = html_to_markdown(html);
        assert!(md.contains("`` it`s ``"), "got: {md}");
    }
}
