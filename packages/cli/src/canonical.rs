//! URL canonicalization for duplicate detection.
//!
//! Pure, deterministic URL normalization. No network access, no config, no DB.
//! Depends only on the `url` crate.

use url::Url;

/// Tracking query parameters to strip during canonicalization.
const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "utm_id",
    "utm_cid",
    "fbclid",
    "gclid",
    "ref",
    "source",
    "medium",
];

/// Canonicalize a URL for duplicate detection.
///
/// Rules applied:
/// 1. Parse and validate the URL
/// 2. Lowercase the hostname
/// 3. Remove `www.` prefix from host
/// 4. Upgrade `http` → `https` for standard web hosts (not localhost/loopback/non-default ports)
/// 5. Strip tracking query parameters (`utm_*`, `fbclid`, `gclid`, `ref`, `source`, `medium`)
/// 6. Sort remaining query parameters alphabetically
/// 7. Remove trailing slash (except for root path `/`)
/// 8. Strip fragment unless it looks like a meaningful anchor
///
/// Returns the canonical URL string or an error for invalid/unsupported URLs.
pub fn canonicalize(raw: &str) -> Result<String, CanonicalError> {
    let mut url = Url::parse(raw).map_err(|e| CanonicalError::InvalidUrl {
        url: raw.to_string(),
        reason: e.to_string(),
    })?;

    let scheme = url.scheme().to_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(CanonicalError::UnsupportedScheme {
            url: raw.to_string(),
            scheme,
        });
    }

    // Lowercase host (url crate does this, but be explicit)
    if let Some(host) = url.host_str().map(|h| h.to_lowercase()) {
        let without_www = strip_www(&host);
        url.set_host(Some(without_www))
            .map_err(|e| CanonicalError::InvalidUrl {
                url: raw.to_string(),
                reason: format!("failed to set host: {e}"),
            })?;
    }

    // Upgrade http → https for standard web hosts
    if url.scheme() == "http" && should_upgrade_scheme(&url) {
        url.set_scheme("https").ok(); // infallible for http→https
    }

    // Strip tracking params and sort remaining
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !is_tracking_param(key))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if pairs.is_empty() {
        url.set_query(None);
    } else {
        let mut sorted = pairs;
        sorted.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let query_string: String = sorted
            .iter()
            .map(|(k, v)| {
                url::form_urlencoded::Serializer::new(String::new())
                    .append_pair(k, v)
                    .finish()
            })
            .collect::<Vec<_>>()
            .join("&");
        url.set_query(Some(&query_string));
    }

    // Strip fragment unless it looks like a meaningful anchor
    if let Some(fragment) = url.fragment() {
        if !is_meaningful_fragment(fragment) {
            url.set_fragment(None);
        }
    }

    // Remove trailing slash (except root path)
    let path = url.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        url.set_path(path.trim_end_matches('/'));
    }

    Ok(url.to_string())
}

/// Check if a query parameter name is a tracking parameter.
fn is_tracking_param(key: &str) -> bool {
    let lower = key.to_lowercase();
    // Check exact matches first
    if TRACKING_PARAMS.contains(&lower.as_str()) {
        return true;
    }
    // Check utm_* prefix for any utm variant not in the list
    lower.starts_with("utm_")
}

/// Strip `www.` prefix from hostname if present.
fn strip_www(host: &str) -> &str {
    host.strip_prefix("www.").unwrap_or(host)
}

/// Determine if http should be upgraded to https.
/// Don't upgrade for localhost, loopback, or explicit non-default ports.
fn should_upgrade_scheme(url: &Url) -> bool {
    if let Some(host) = url.host_str() {
        let is_local = host == "localhost"
            || host == "127.0.0.1"
            || host == "::1"
            || host == "[::1]"
            || host.ends_with(".local")
            || host.ends_with(".localhost");
        if is_local {
            return false;
        }
    }
    // Don't upgrade if there's an explicit non-default port
    if let Some(port) = url.port() {
        if port != 80 && port != 443 {
            return false;
        }
    }
    true
}

/// Determine if a URL fragment is "meaningful" (should be preserved).
/// Meaningful fragments look like section anchors (alphanumeric with hyphens/underscores).
/// Empty fragments, hash-routed paths, and query-like fragments are stripped.
fn is_meaningful_fragment(fragment: &str) -> bool {
    if fragment.is_empty() {
        return false;
    }
    // Strip fragments that look like JS state or query-like
    if fragment.starts_with('!')
        || fragment.starts_with('/')
        || fragment.contains('=')
        || fragment.contains('?')
    {
        return false;
    }
    // Keep fragments that look like section anchors (letters, digits, hyphens, underscores, dots)
    fragment
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Errors from URL canonicalization.
#[derive(Debug, Clone)]
pub enum CanonicalError {
    InvalidUrl { url: String, reason: String },
    UnsupportedScheme { url: String, scheme: String },
}

impl std::fmt::Display for CanonicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CanonicalError::InvalidUrl { url, reason } => {
                write!(f, "invalid URL '{url}': {reason}")
            }
            CanonicalError::UnsupportedScheme { url, scheme } => {
                write!(f, "unsupported scheme '{scheme}' in URL '{url}'")
            }
        }
    }
}

impl std::error::Error for CanonicalError {}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Tracking param stripping ────────────────────────────────────

    #[test]
    fn strips_utm_params() {
        let result =
            canonicalize("https://example.com/page?utm_source=twitter&utm_medium=social&keep=1")
                .unwrap();
        assert!(!result.contains("utm_source"));
        assert!(!result.contains("utm_medium"));
        assert!(result.contains("keep=1"));
    }

    #[test]
    fn strips_fbclid_and_gclid() {
        let result =
            canonicalize("https://example.com/page?fbclid=abc123&gclid=def456&q=test").unwrap();
        assert!(!result.contains("fbclid"));
        assert!(!result.contains("gclid"));
        assert!(result.contains("q=test"));
    }

    #[test]
    fn strips_ref_source_medium() {
        let result =
            canonicalize("https://example.com/?ref=email&source=newsletter&medium=cpc").unwrap();
        assert!(!result.contains("ref="));
        assert!(!result.contains("source="));
        assert!(!result.contains("medium="));
    }

    #[test]
    fn strips_utm_with_mixed_case() {
        let result =
            canonicalize("https://example.com/page?UTM_SOURCE=twitter&Utm_Medium=social").unwrap();
        assert!(!result.contains("utm"));
        assert!(!result.contains("UTM"));
    }

    #[test]
    fn strips_unknown_utm_variants() {
        let result =
            canonicalize("https://example.com/page?utm_foo=bar&utm_custom=1&keep=1").unwrap();
        assert!(!result.contains("utm_"));
        assert!(result.contains("keep=1"));
    }

    // ── Host normalization ──────────────────────────────────────────

    #[test]
    fn lowercases_host() {
        let result = canonicalize("https://EXAMPLE.COM/Page").unwrap();
        assert!(result.starts_with("https://example.com/"));
    }

    #[test]
    fn removes_www_prefix() {
        let result = canonicalize("https://www.example.com/page").unwrap();
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn www_removal_only_exact_prefix() {
        // "wwwexample.com" should NOT be stripped
        let result = canonicalize("https://wwwexample.com/page").unwrap();
        assert!(result.contains("wwwexample.com"));
    }

    // ── Trailing slash ──────────────────────────────────────────────

    #[test]
    fn removes_trailing_slash() {
        let result = canonicalize("https://example.com/page/").unwrap();
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn preserves_root_slash() {
        let result = canonicalize("https://example.com/").unwrap();
        assert_eq!(result, "https://example.com/");
    }

    // ── Query param sorting ─────────────────────────────────────────

    #[test]
    fn sorts_query_params() {
        let result = canonicalize("https://example.com/page?z=1&a=2&m=3").unwrap();
        assert!(result.contains("a=2&m=3&z=1"));
    }

    #[test]
    fn preserves_duplicate_non_tracking_params() {
        let result = canonicalize("https://example.com/page?tag=a&tag=b").unwrap();
        assert!(result.contains("tag=a"));
        assert!(result.contains("tag=b"));
    }

    #[test]
    fn empty_query_after_stripping_removes_question_mark() {
        let result = canonicalize("https://example.com/page?utm_source=twitter").unwrap();
        assert!(!result.contains('?'));
    }

    // ── Scheme normalization ────────────────────────────────────────

    #[test]
    fn upgrades_http_to_https_for_standard_hosts() {
        let result = canonicalize("http://example.com/page").unwrap();
        assert!(result.starts_with("https://"));
    }

    #[test]
    fn does_not_upgrade_localhost() {
        let result = canonicalize("http://localhost:3000/page").unwrap();
        assert!(result.starts_with("http://localhost"));
    }

    #[test]
    fn does_not_upgrade_127_0_0_1() {
        let result = canonicalize("http://127.0.0.1:8080/page").unwrap();
        assert!(result.starts_with("http://127.0.0.1"));
    }

    #[test]
    fn does_not_upgrade_non_default_port() {
        let result = canonicalize("http://example.com:8080/page").unwrap();
        assert!(result.starts_with("http://example.com:8080"));
    }

    #[test]
    fn upgrades_http_default_port_80() {
        // Port 80 is the default for http, should upgrade
        let result = canonicalize("http://example.com:80/page").unwrap();
        assert!(result.starts_with("https://"));
    }

    // ── Fragment handling ───────────────────────────────────────────

    #[test]
    fn preserves_meaningful_fragment() {
        let result = canonicalize("https://example.com/page#section-2").unwrap();
        assert!(result.contains("#section-2"));
    }

    #[test]
    fn strips_empty_fragment() {
        let result = canonicalize("https://example.com/page#").unwrap();
        assert!(!result.contains('#'));
    }

    #[test]
    fn strips_hash_routed_fragment() {
        let result = canonicalize("https://example.com/page#!/route/path").unwrap();
        assert!(!result.contains('#'));
    }

    #[test]
    fn strips_query_like_fragment() {
        let result = canonicalize("https://example.com/page#key=value").unwrap();
        assert!(!result.contains('#'));
    }

    // ── Idempotence ─────────────────────────────────────────────────

    #[test]
    fn canonicalization_is_idempotent() {
        let urls = vec![
            "https://www.example.com/page?utm_source=x&b=2&a=1",
            "http://EXAMPLE.COM/article/",
            "https://example.com/page#section",
            "https://example.com/?q=test",
        ];
        for url in urls {
            let first = canonicalize(url).unwrap();
            let second = canonicalize(&first).unwrap();
            assert_eq!(first, second, "not idempotent for: {url}");
        }
    }

    // ── Error cases ─────────────────────────────────────────────────

    #[test]
    fn rejects_invalid_url() {
        assert!(canonicalize("not a url").is_err());
    }

    #[test]
    fn rejects_unsupported_scheme() {
        let err = canonicalize("ftp://example.com/file").unwrap_err();
        match err {
            CanonicalError::UnsupportedScheme { scheme, .. } => assert_eq!(scheme, "ftp"),
            other => panic!("expected UnsupportedScheme, got: {other}"),
        }
    }

    // ── Dedup equivalence ───────────────────────────────────────────

    #[test]
    fn urls_differing_only_by_tracking_canonicalize_same() {
        let a = canonicalize("https://example.com/page").unwrap();
        let b = canonicalize("https://example.com/page?utm_source=twitter&fbclid=abc").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn urls_differing_only_by_www_canonicalize_same() {
        let a = canonicalize("https://example.com/page").unwrap();
        let b = canonicalize("https://www.example.com/page").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn urls_differing_only_by_trailing_slash_canonicalize_same() {
        let a = canonicalize("https://example.com/page").unwrap();
        let b = canonicalize("https://example.com/page/").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn urls_differing_only_by_query_order_canonicalize_same() {
        let a = canonicalize("https://example.com/page?a=1&b=2").unwrap();
        let b = canonicalize("https://example.com/page?b=2&a=1").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn urls_differing_only_by_host_case_canonicalize_same() {
        let a = canonicalize("https://example.com/page").unwrap();
        let b = canonicalize("https://EXAMPLE.COM/page").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn urls_differing_only_by_scheme_canonicalize_same() {
        let a = canonicalize("https://example.com/page").unwrap();
        let b = canonicalize("http://example.com/page").unwrap();
        assert_eq!(a, b);
    }

    // ── Blank / edge values ─────────────────────────────────────────

    #[test]
    fn preserves_path_case() {
        let result = canonicalize("https://example.com/CamelCase/Path").unwrap();
        assert!(result.contains("/CamelCase/Path"));
    }

    #[test]
    fn handles_url_with_userinfo() {
        // url crate should handle this
        let result = canonicalize("https://user:pass@example.com/page");
        // We don't require success here, just no panic
        let _ = result;
    }

    #[test]
    fn preserves_port_in_output() {
        let result = canonicalize("https://example.com:9090/page").unwrap();
        assert!(result.contains(":9090"));
    }
}
