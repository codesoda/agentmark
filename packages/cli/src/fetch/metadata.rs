//! HTML metadata extraction from Open Graph, standard meta tags, link tags,
//! and schema.org JSON-LD markup.

use scraper::{Html, Selector};
use url::Url;

/// Metadata extracted from a fetched HTML page.
///
/// Fields follow a priority merge: OG > schema.org JSON-LD > standard meta tags > `<title>`.
/// All fields are optional — pages with missing or malformed metadata simply produce `None`.
/// Derives serde traits for `metadata.json` serialization in Spec 08.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PageMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub site_name: Option<String>,
    pub published_at: Option<String>,
    pub canonical_url: Option<String>,
    pub favicon_url: Option<String>,
    pub hero_image_url: Option<String>,
}

/// Intermediate partial metadata from a single source. Private — used only for merging.
#[derive(Debug, Default)]
struct PartialMetadata {
    title: Option<String>,
    description: Option<String>,
    author: Option<String>,
    site_name: Option<String>,
    published_at: Option<String>,
    canonical_url: Option<String>,
    favicon_url: Option<String>,
    hero_image_url: Option<String>,
}

/// Extract metadata from HTML, resolving relative URLs against `base_url`.
///
/// Parsing is best-effort: malformed HTML or JSON-LD produces partial/empty metadata, never an error.
pub fn extract_metadata(html: &str, base_url: &Url) -> PageMetadata {
    let document = Html::parse_document(html);

    let og = extract_og(&document, base_url);
    let jsonld = extract_jsonld(&document, base_url);
    let standard = extract_standard_meta(&document, base_url);
    let title_tag = extract_title_tag(&document);

    // Merge with precedence: OG > schema.org > standard meta > <title>
    merge_metadata(&[og, jsonld, standard, title_tag])
}

// ---------------------------------------------------------------------------
// Helpers for reading DOM attributes
// ---------------------------------------------------------------------------

/// Return trimmed non-blank `content` attribute for the first `<meta>` matching the selector.
fn meta_content(document: &Html, selector_str: &str) -> Option<String> {
    let sel = Selector::parse(selector_str).ok()?;
    document.select(&sel).find_map(|el| {
        let val = el.value().attr("content")?.trim();
        non_blank(val)
    })
}

/// Return trimmed non-blank `href` for the first `<link>` matching the selector.
fn link_href(document: &Html, selector_str: &str) -> Option<String> {
    let sel = Selector::parse(selector_str).ok()?;
    document.select(&sel).find_map(|el| {
        let val = el.value().attr("href")?.trim();
        non_blank(val)
    })
}

/// Returns `None` for blank/whitespace-only strings.
fn non_blank(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Resolve a potentially relative URL against the base. Returns `None` on parse failure.
fn resolve_url(base: &Url, raw: &str) -> Option<String> {
    base.join(raw.trim()).ok().map(|u| u.to_string())
}

// ---------------------------------------------------------------------------
// Open Graph extraction
// ---------------------------------------------------------------------------

fn extract_og(document: &Html, base_url: &Url) -> PartialMetadata {
    PartialMetadata {
        title: meta_content(document, r#"meta[property="og:title"]"#),
        description: meta_content(document, r#"meta[property="og:description"]"#),
        site_name: meta_content(document, r#"meta[property="og:site_name"]"#),
        hero_image_url: meta_content(document, r#"meta[property="og:image"]"#)
            .and_then(|u| resolve_url(base_url, &u)),
        canonical_url: meta_content(document, r#"meta[property="og:url"]"#)
            .and_then(|u| resolve_url(base_url, &u)),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Standard meta / link tag extraction
// ---------------------------------------------------------------------------

fn extract_standard_meta(document: &Html, base_url: &Url) -> PartialMetadata {
    let canonical =
        link_href(document, r#"link[rel="canonical"]"#).and_then(|u| resolve_url(base_url, &u));

    // Favicon: try common rel values
    let favicon = favicon_href(document, base_url);

    PartialMetadata {
        title: meta_content(document, r#"meta[name="title"]"#),
        description: meta_content(document, r#"meta[name="description"]"#),
        author: meta_content(document, r#"meta[name="author"]"#),
        published_at: meta_content(document, r#"meta[property="article:published_time"]"#),
        canonical_url: canonical,
        favicon_url: favicon,
        ..Default::default()
    }
}

/// Try several common favicon `<link>` patterns and return the first match.
fn favicon_href(document: &Html, base_url: &Url) -> Option<String> {
    // Scraper CSS selectors: try rel="icon", rel="shortcut icon"
    for selector_str in [r#"link[rel="icon"]"#, r#"link[rel="shortcut icon"]"#] {
        if let Some(href) = link_href(document, selector_str) {
            return resolve_url(base_url, &href);
        }
    }

    // Fallback: scan all <link> elements and check rel tokens
    if let Ok(sel) = Selector::parse("link[rel]") {
        for el in document.select(&sel) {
            let rel = el.value().attr("rel").unwrap_or_default().to_lowercase();
            let tokens: Vec<&str> = rel.split_whitespace().collect();
            if tokens.contains(&"icon") {
                if let Some(href) = el.value().attr("href").and_then(non_blank) {
                    return resolve_url(base_url, &href);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// <title> tag extraction (lowest priority, title only)
// ---------------------------------------------------------------------------

fn extract_title_tag(document: &Html) -> PartialMetadata {
    let title = Selector::parse("title")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .and_then(|el| non_blank(&el.text().collect::<String>()));
    PartialMetadata {
        title,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// JSON-LD schema.org extraction
// ---------------------------------------------------------------------------

fn extract_jsonld(document: &Html, base_url: &Url) -> PartialMetadata {
    let sel = match Selector::parse(r#"script[type="application/ld+json"]"#) {
        Ok(s) => s,
        Err(_) => return PartialMetadata::default(),
    };

    let mut result = PartialMetadata::default();

    for element in document.select(&sel) {
        let text = element.text().collect::<String>();
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue, // skip malformed JSON-LD blocks
        };
        extract_from_jsonld_value(&value, base_url, &mut result);
    }

    result
}

/// Walk a JSON-LD value, handling objects, arrays, and `@graph`.
fn extract_from_jsonld_value(
    value: &serde_json::Value,
    base_url: &Url,
    result: &mut PartialMetadata,
) {
    match value {
        serde_json::Value::Object(map) => {
            // If this object has @graph, walk its entries
            if let Some(graph) = map.get("@graph") {
                extract_from_jsonld_value(graph, base_url, result);
            }
            // Try to extract fields from this object directly
            extract_jsonld_fields(value, base_url, result);
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_from_jsonld_value(item, base_url, result);
            }
        }
        _ => {}
    }
}

/// Extract common article/webpage fields from a JSON-LD object.
fn extract_jsonld_fields(obj: &serde_json::Value, base_url: &Url, result: &mut PartialMetadata) {
    // Only extract from article/webpage-like types (or untyped objects)
    if let Some(typ) = obj.get("@type").and_then(|v| v.as_str()) {
        let t = typ.to_lowercase();
        // Skip types that aren't article/webpage/newsarticle/blogposting etc.
        let article_types = [
            "article",
            "newsarticle",
            "blogposting",
            "webpage",
            "creativework",
            "technicalarticle",
            "scholarlyarticle",
            "report",
            "medicalwebpage",
        ];
        if !article_types.iter().any(|at| t.contains(at)) {
            return;
        }
    }

    // Title: headline > name
    if result.title.is_none() {
        result.title = json_string(obj, "headline").or_else(|| json_string(obj, "name"));
    }

    // Description
    if result.description.is_none() {
        result.description = json_string(obj, "description");
    }

    // Author: can be string, object with "name", or array of such
    if result.author.is_none() {
        result.author = json_author(obj);
    }

    // Site name from publisher.name
    if result.site_name.is_none() {
        result.site_name = obj
            .get("publisher")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .and_then(non_blank);
    }

    // Published date
    if result.published_at.is_none() {
        result.published_at = json_string(obj, "datePublished");
    }

    // Hero image: can be string, object with "url", or array
    if result.hero_image_url.is_none() {
        result.hero_image_url = json_image(obj, base_url);
    }
}

/// Get a non-blank string value from a JSON object field.
fn json_string(obj: &serde_json::Value, key: &str) -> Option<String> {
    obj.get(key).and_then(|v| v.as_str()).and_then(non_blank)
}

/// Extract author from string, object `{ "name": "..." }`, or array thereof.
fn json_author(obj: &serde_json::Value) -> Option<String> {
    match obj.get("author")? {
        serde_json::Value::String(s) => non_blank(s),
        serde_json::Value::Object(map) => {
            map.get("name").and_then(|n| n.as_str()).and_then(non_blank)
        }
        serde_json::Value::Array(arr) => {
            // Take the first author with a name
            arr.iter().find_map(|item| match item {
                serde_json::Value::String(s) => non_blank(s),
                serde_json::Value::Object(map) => {
                    map.get("name").and_then(|n| n.as_str()).and_then(non_blank)
                }
                _ => None,
            })
        }
        _ => None,
    }
}

/// Extract image URL from string, object `{ "url": "..." }`, or array thereof.
fn json_image(obj: &serde_json::Value, base_url: &Url) -> Option<String> {
    let raw = match obj.get("image")? {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(map) => map.get("url").and_then(|u| u.as_str())?.to_string(),
        serde_json::Value::Array(arr) => arr.iter().find_map(|item| match item {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Object(map) => map
                .get("url")
                .and_then(|u| u.as_str())
                .map(|s| s.to_string()),
            _ => None,
        })?,
        _ => return None,
    };
    resolve_url(base_url, &raw)
}

// ---------------------------------------------------------------------------
// Priority merge
// ---------------------------------------------------------------------------

/// Merge multiple partial metadata sources in order of decreasing priority.
/// The first non-None value for each field wins.
fn merge_metadata(sources: &[PartialMetadata]) -> PageMetadata {
    PageMetadata {
        title: sources.iter().find_map(|s| s.title.clone()),
        description: sources.iter().find_map(|s| s.description.clone()),
        author: sources.iter().find_map(|s| s.author.clone()),
        site_name: sources.iter().find_map(|s| s.site_name.clone()),
        published_at: sources.iter().find_map(|s| s.published_at.clone()),
        canonical_url: sources.iter().find_map(|s| s.canonical_url.clone()),
        favicon_url: sources.iter().find_map(|s| s.favicon_url.clone()),
        hero_image_url: sources.iter().find_map(|s| s.hero_image_url.clone()),
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://example.com/page").unwrap()
    }

    #[test]
    fn og_extraction_populates_all_fields() {
        let html = r#"<html><head>
            <meta property="og:title" content="OG Title">
            <meta property="og:description" content="OG Desc">
            <meta property="og:site_name" content="OG Site">
            <meta property="og:image" content="https://example.com/hero.jpg">
            <meta property="og:url" content="https://example.com/canonical">
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(m.title.as_deref(), Some("OG Title"));
        assert_eq!(m.description.as_deref(), Some("OG Desc"));
        assert_eq!(m.site_name.as_deref(), Some("OG Site"));
        assert_eq!(
            m.hero_image_url.as_deref(),
            Some("https://example.com/hero.jpg")
        );
        assert_eq!(
            m.canonical_url.as_deref(),
            Some("https://example.com/canonical")
        );
    }

    #[test]
    fn standard_meta_only() {
        let html = r#"<html><head>
            <meta name="title" content="Meta Title">
            <meta name="description" content="Meta Desc">
            <meta name="author" content="Jane Doe">
            <meta property="article:published_time" content="2024-01-15">
            <link rel="canonical" href="https://example.com/canon">
            <link rel="icon" href="/favicon.ico">
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(m.title.as_deref(), Some("Meta Title"));
        assert_eq!(m.description.as_deref(), Some("Meta Desc"));
        assert_eq!(m.author.as_deref(), Some("Jane Doe"));
        assert_eq!(m.published_at.as_deref(), Some("2024-01-15"));
        assert_eq!(
            m.canonical_url.as_deref(),
            Some("https://example.com/canon")
        );
        assert_eq!(
            m.favicon_url.as_deref(),
            Some("https://example.com/favicon.ico")
        );
    }

    #[test]
    fn title_tag_fallback() {
        let html = "<html><head><title>Fallback Title</title></head></html>";
        let m = extract_metadata(html, &base());
        assert_eq!(m.title.as_deref(), Some("Fallback Title"));
        assert!(m.description.is_none());
    }

    #[test]
    fn og_overrides_standard_meta_and_title_tag() {
        let html = r#"<html><head>
            <title>Title Tag</title>
            <meta name="title" content="Meta Title">
            <meta name="description" content="Meta Desc">
            <meta property="og:title" content="OG Title">
            <meta property="og:description" content="OG Desc">
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(m.title.as_deref(), Some("OG Title"));
        assert_eq!(m.description.as_deref(), Some("OG Desc"));
    }

    #[test]
    fn jsonld_article_extraction() {
        let html = r#"<html><head>
            <script type="application/ld+json">
            {
                "@type": "Article",
                "headline": "JSON-LD Title",
                "description": "JSON-LD Desc",
                "author": {"name": "JSON Author"},
                "publisher": {"name": "Publisher Co"},
                "datePublished": "2024-06-01",
                "image": "https://example.com/img.jpg"
            }
            </script>
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(m.title.as_deref(), Some("JSON-LD Title"));
        assert_eq!(m.description.as_deref(), Some("JSON-LD Desc"));
        assert_eq!(m.author.as_deref(), Some("JSON Author"));
        assert_eq!(m.site_name.as_deref(), Some("Publisher Co"));
        assert_eq!(m.published_at.as_deref(), Some("2024-06-01"));
        assert_eq!(
            m.hero_image_url.as_deref(),
            Some("https://example.com/img.jpg")
        );
    }

    #[test]
    fn jsonld_with_graph() {
        let html = r#"<html><head>
            <script type="application/ld+json">
            {
                "@graph": [
                    {"@type": "WebPage", "name": "Graph Page"},
                    {"@type": "Article", "headline": "Graph Article", "author": "Graph Author"}
                ]
            }
            </script>
        </head></html>"#;
        let m = extract_metadata(html, &base());
        // WebPage comes first with "name", Article comes second with "headline"
        // WebPage's name fills title first
        assert_eq!(m.title.as_deref(), Some("Graph Page"));
        assert_eq!(m.author.as_deref(), Some("Graph Author"));
    }

    #[test]
    fn jsonld_array_of_objects() {
        let html = r#"<html><head>
            <script type="application/ld+json">
            [
                {"@type": "NewsArticle", "headline": "News Title", "author": ["Alice", "Bob"]}
            ]
            </script>
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(m.title.as_deref(), Some("News Title"));
        assert_eq!(m.author.as_deref(), Some("Alice"));
    }

    #[test]
    fn jsonld_author_as_string() {
        let html = r#"<html><head>
            <script type="application/ld+json">
            {"@type": "BlogPosting", "author": "String Author"}
            </script>
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(m.author.as_deref(), Some("String Author"));
    }

    #[test]
    fn jsonld_image_as_array() {
        let html = r#"<html><head>
            <script type="application/ld+json">
            {"@type": "Article", "image": ["https://example.com/a.jpg", "https://example.com/b.jpg"]}
            </script>
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(
            m.hero_image_url.as_deref(),
            Some("https://example.com/a.jpg")
        );
    }

    #[test]
    fn jsonld_image_as_object() {
        let html = r#"<html><head>
            <script type="application/ld+json">
            {"@type": "Article", "image": {"url": "https://example.com/obj.jpg"}}
            </script>
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(
            m.hero_image_url.as_deref(),
            Some("https://example.com/obj.jpg")
        );
    }

    #[test]
    fn og_beats_jsonld_beats_standard_meta() {
        let html = r#"<html><head>
            <title>Title Tag</title>
            <meta name="description" content="Standard Desc">
            <meta name="author" content="Standard Author">
            <meta property="og:title" content="OG Title">
            <script type="application/ld+json">
            {"@type": "Article", "headline": "JSON Title", "description": "JSON Desc", "author": "JSON Author"}
            </script>
        </head></html>"#;
        let m = extract_metadata(html, &base());
        // OG title wins over JSON-LD and standard
        assert_eq!(m.title.as_deref(), Some("OG Title"));
        // JSON-LD desc wins over standard (no OG desc)
        assert_eq!(m.description.as_deref(), Some("JSON Desc"));
        // JSON-LD author wins over standard (no OG author)
        assert_eq!(m.author.as_deref(), Some("JSON Author"));
    }

    #[test]
    fn no_metadata_at_all() {
        let html = "<html><body><p>Just content</p></body></html>";
        let m = extract_metadata(html, &base());
        assert_eq!(m, PageMetadata::default());
    }

    #[test]
    fn blank_metadata_values_ignored() {
        let html = r#"<html><head>
            <meta property="og:title" content="   ">
            <meta name="description" content="">
            <title>  </title>
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert!(m.title.is_none());
        assert!(m.description.is_none());
    }

    #[test]
    fn malformed_html_does_not_panic() {
        let html = r#"<html><head><meta property="og:title" content="Found"<<<<>>>"#;
        let m = extract_metadata(html, &base());
        // scraper is lenient; may or may not find the title
        // The key assertion: no panic
        let _ = m;
    }

    #[test]
    fn malformed_jsonld_does_not_wipe_other_metadata() {
        let html = r#"<html><head>
            <meta property="og:title" content="OG Title">
            <script type="application/ld+json">{invalid json here}</script>
            <script type="application/ld+json">
            {"@type": "Article", "author": "Valid Author"}
            </script>
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(m.title.as_deref(), Some("OG Title"));
        assert_eq!(m.author.as_deref(), Some("Valid Author"));
    }

    #[test]
    fn relative_urls_resolved_against_base() {
        let html = r#"<html><head>
            <link rel="canonical" href="/article/123">
            <link rel="icon" href="/favicon.png">
            <meta property="og:image" content="/images/hero.jpg">
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(
            m.canonical_url.as_deref(),
            Some("https://example.com/article/123")
        );
        assert_eq!(
            m.favicon_url.as_deref(),
            Some("https://example.com/favicon.png")
        );
        assert_eq!(
            m.hero_image_url.as_deref(),
            Some("https://example.com/images/hero.jpg")
        );
    }

    #[test]
    fn favicon_shortcut_icon_variant() {
        let html = r#"<html><head>
            <link rel="shortcut icon" href="/short.ico">
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(
            m.favicon_url.as_deref(),
            Some("https://example.com/short.ico")
        );
    }

    #[test]
    fn favicon_multi_token_rel() {
        // Some pages use rel="shortcut icon" as two tokens
        let html = r#"<html><head>
            <link rel="apple-touch-icon icon" href="/touch.png">
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(
            m.favicon_url.as_deref(),
            Some("https://example.com/touch.png")
        );
    }

    #[test]
    fn jsonld_non_article_type_ignored() {
        let html = r#"<html><head>
            <script type="application/ld+json">
            {"@type": "Organization", "name": "Org Name"}
            </script>
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert!(m.title.is_none());
    }

    #[test]
    fn multiple_jsonld_blocks_merged() {
        let html = r#"<html><head>
            <script type="application/ld+json">
            {"@type": "Article", "headline": "Title From Block 1"}
            </script>
            <script type="application/ld+json">
            {"@type": "Article", "author": "Author From Block 2"}
            </script>
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(m.title.as_deref(), Some("Title From Block 1"));
        assert_eq!(m.author.as_deref(), Some("Author From Block 2"));
    }

    #[test]
    fn serde_roundtrip() {
        let m = PageMetadata {
            title: Some("Test".to_string()),
            description: None,
            author: Some("Author".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&m).unwrap();
        let m2: PageMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn relative_jsonld_image_resolved() {
        let html = r#"<html><head>
            <script type="application/ld+json">
            {"@type": "Article", "image": "/relative/img.jpg"}
            </script>
        </head></html>"#;
        let m = extract_metadata(html, &base());
        assert_eq!(
            m.hero_image_url.as_deref(),
            Some("https://example.com/relative/img.jpg")
        );
    }
}
