//! HTML simplification for model input.
//!
//! A rendered page snapshot is mostly markup the model does not need: scripts,
//! styles, icons, hashed class names and wrapper elements. `simplify_html`
//! keeps the text, the document structure and the few attributes that carry
//! meaning, which typically shrinks a page by 80-90%.

use ego_tree::NodeRef;
use scraper::{Html, Node, Selector};

/// Elements dropped together with everything inside them.
const DROPPED_TAGS: &[&str] = &[
    "head", "script", "style", "svg", "link", "meta", "noscript", "iframe", "img", "picture",
    "figure", "video", "audio", "canvas", "select", "input", "textarea", "button", "template",
    "hr",
];

/// Wrapper elements that are unwrapped unless they carry a kept attribute.
const WRAPPER_TAGS: &[&str] = &["div", "span"];

/// Attributes that carry meaning for a reader.
const KEPT_ATTRS: &[&str] = &["href", "aria-label", "role", "alt", "title"];

/// Links longer than this are tracking or search URLs; the link text is kept.
const MAX_HREF_LEN: usize = 300;

/// Reduce a page to the text-bearing skeleton of its `<body>`.
///
/// Never panics on malformed input. Returns an empty string when the page has
/// no visible text.
pub fn simplify_html(html: &str) -> String {
    let document = Html::parse_document(html);
    let body = Selector::parse("body")
        .ok()
        .and_then(|sel| document.select(&sel).next());
    let root: NodeRef<'_, Node> = match body {
        Some(body) => *body,
        None => document.tree.root(),
    };

    let mut out = String::new();
    render_children(root, &mut out);
    collapse_whitespace(&out)
}

fn render_children(node: NodeRef<'_, Node>, out: &mut String) {
    for child in node.children() {
        match child.value() {
            Node::Text(text) => push_escaped(out, text, false),
            Node::Element(el) => {
                let name = el.name();
                if DROPPED_TAGS.contains(&name) || el.attr("aria-hidden") == Some("true") {
                    continue;
                }
                if name == "br" {
                    out.push_str("<br>");
                    continue;
                }

                let mut inner = String::new();
                render_children(child, &mut inner);
                if inner.trim().is_empty() {
                    // No visible text inside: icons, spacers, empty placeholders.
                    continue;
                }

                let attrs: Vec<(&str, &str)> = KEPT_ATTRS
                    .iter()
                    .filter_map(|&key| el.attr(key).map(|value| (key, value)))
                    .filter(|(key, value)| {
                        let overlong_link = *key == "href" && value.len() > MAX_HREF_LEN;
                        !value.trim().is_empty() && !overlong_link
                    })
                    .collect();

                if WRAPPER_TAGS.contains(&name) && attrs.is_empty() {
                    // Unwrap, but keep a boundary so adjacent blocks do not run together.
                    out.push(' ');
                    out.push_str(&inner);
                    out.push(' ');
                    continue;
                }

                out.push('<');
                out.push_str(name);
                for (key, value) in attrs {
                    out.push(' ');
                    out.push_str(key);
                    out.push_str("=\"");
                    push_escaped(out, value, true);
                    out.push('"');
                }
                out.push('>');
                out.push_str(&inner);
                out.push_str("</");
                out.push_str(name);
                out.push('>');
            }
            _ => {} // comments, doctypes, processing instructions
        }
    }
}

fn push_escaped(out: &mut String, text: &str, in_attribute: bool) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if in_attribute => out.push_str("&quot;"),
            // Zero-width characters used as layout hacks.
            '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}' => {}
            _ => out.push(c),
        }
    }
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_text_structure_and_links() {
        let html = r#"<html><head><title>T</title><style>p{}</style></head><body>
            <main><h2 class="x1 y2">Ada Lovelace</h2>
            <p class="zz">Analyst &amp; Metaphysician</p>
            <a class="q" href="https://example.com/post/1" data-x="1">A post</a></main></body></html>"#;
        assert_eq!(
            simplify_html(html),
            r#"<main><h2>Ada Lovelace</h2> <p>Analyst &amp; Metaphysician</p> <a href="https://example.com/post/1">A post</a></main>"#
        );
    }

    #[test]
    fn drops_scripts_icons_buttons_and_hidden_duplicates() {
        let html = r#"<body><script>alert(1)</script><svg><path d="M0"/></svg>
            <p>Body text<button data-testid="expandable-text-button">… more</button></p>
            <span aria-hidden="true">71</span><p>71 reactions</p>
            <img src="a.png" alt="photo"><iframe src="about:blank"></iframe></body>"#;
        assert_eq!(simplify_html(html), "<p>Body text</p> <p>71 reactions</p>");
    }

    #[test]
    fn unwraps_plain_wrappers_but_keeps_block_boundaries() {
        let html = "<body><div><div><span>One</span></div><div>Two</div></div></body>";
        assert_eq!(simplify_html(html), "One Two");
    }

    #[test]
    fn keeps_wrappers_with_meaningful_attributes() {
        let html = r#"<body><div role="listitem" class="c"><span aria-label="Premium">Ada</span></div></body>"#;
        assert_eq!(
            simplify_html(html),
            r#"<div role="listitem"><span aria-label="Premium">Ada</span></div>"#
        );
    }

    #[test]
    fn removes_empty_elements() {
        let html = r#"<body><section><div componentkey="lazy"></div><ul><li></li></ul></section><p>Kept</p></body>"#;
        assert_eq!(simplify_html(html), "<p>Kept</p>");
    }

    #[test]
    fn preserves_line_breaks_inside_text() {
        let html = "<body><p>First<br><br>Second</p></body>";
        assert_eq!(simplify_html(html), "<p>First<br><br>Second</p>");
    }

    #[test]
    fn drops_overlong_hrefs_but_keeps_link_text() {
        let long = format!("https://example.com/search?q={}", "a".repeat(400));
        let html = format!(r#"<body><a href="{long}">mutual connections</a></body>"#);
        assert_eq!(simplify_html(&html), "<a>mutual connections</a>");
    }

    #[test]
    fn escapes_text_and_attribute_values() {
        let html = r#"<body><a href="https://e.com/?a=1&amp;b=&quot;x&quot;">1 &lt; 2</a></body>"#;
        assert_eq!(
            simplify_html(html),
            r#"<a href="https://e.com/?a=1&amp;b=&quot;x&quot;">1 &lt; 2</a>"#
        );
    }

    #[test]
    fn strips_zero_width_characters() {
        assert_eq!(
            simplify_html("<body><p>a\u{200b}b</p><a href=\"/x\">\u{200b}</a></body>"),
            "<p>ab</p>"
        );
    }

    #[test]
    fn empty_and_malformed_input_do_not_panic() {
        assert_eq!(simplify_html(""), "");
        assert_eq!(simplify_html("<div><p>unclosed"), "<p>unclosed</p>");
        let _ = simplify_html("<<<>>>&&&");
    }
}
