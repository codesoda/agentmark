//! Which pages should be saved from the browser's snapshot instead of a fetch.
//!
//! Some sites show a logged-in user far more than they show an anonymous
//! client: the fetch either fails (LinkedIn profiles answer HTTP 999) or
//! succeeds with a logged-out page that lacks most of the content (LinkedIn
//! posts). For URLs matching a rule here, the snapshot sent by the browser
//! extension is the primary content and the fetched page only supplements it.
//!
//! A rule is `host/path-prefix`. The host also matches its subdomains, and the
//! path is matched as a prefix. `example.com` alone matches the whole site.
//! Pure URL logic; depends only on the `url` crate.

use url::Url;

/// Rules that always apply. Users can add more with `snapshot_sites` in config.
pub const BUILTIN_RULES: &[&str] = &[
    "linkedin.com/in/",          // profiles
    "linkedin.com/posts/",       // full post page (share URL)
    "linkedin.com/feed/update/", // full post page (permalink from the timeline)
    "linkedin.com/company/",     // company pages
    "x.com",                     // posts and profiles render only for a browser
    "twitter.com",               // legacy host of the same site
    // Discussions are public, but generic article extraction mangles their
    // table layout; the agent renders them properly.
    "news.ycombinator.com/item",
];

/// Returns true when `url` matches a built-in rule or one of `extra_rules`.
pub fn prefers_browser_snapshot(url: &str, extra_rules: &[String]) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    BUILTIN_RULES
        .iter()
        .copied()
        .chain(extra_rules.iter().map(String::as_str))
        .any(|rule| rule_matches(rule, &parsed))
}

fn rule_matches(rule: &str, url: &Url) -> bool {
    let rule = rule.trim();
    let rule = rule
        .strip_prefix("https://")
        .or_else(|| rule.strip_prefix("http://"))
        .unwrap_or(rule);
    let rule = rule.strip_suffix('*').unwrap_or(rule);

    let (rule_host, rule_path) = match rule.find('/') {
        Some(i) => (&rule[..i], &rule[i..]),
        None => (rule, "/"),
    };
    let rule_host = rule_host.strip_prefix("www.").unwrap_or(rule_host);
    if rule_host.is_empty() {
        return false;
    }

    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    let rule_host = rule_host.to_ascii_lowercase();
    let host_matches = host == rule_host || host.ends_with(&format!(".{rule_host}"));

    host_matches && url.path().starts_with(rule_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builtin(url: &str) -> bool {
        prefers_browser_snapshot(url, &[])
    }

    #[test]
    fn linkedin_profiles_posts_and_companies_match() {
        assert!(builtin("https://www.linkedin.com/in/someone/"));
        assert!(builtin(
            "https://www.linkedin.com/posts/someone_a-post-share-123-A4om/?utm_source=share"
        ));
        assert!(builtin(
            "https://www.linkedin.com/feed/update/urn:li:activity:7525613594424631296/"
        ));
        assert!(builtin("https://www.linkedin.com/company/anthropic/"));
        assert!(builtin("https://linkedin.com/in/someone"));
        assert!(builtin("https://uk.linkedin.com/in/someone"));
    }

    #[test]
    fn x_and_twitter_match_site_wide() {
        assert!(builtin("https://x.com/jack/status/20"));
        assert!(builtin("https://x.com/jack"));
        assert!(builtin("https://twitter.com/jack/status/20"));
        assert!(builtin("https://mobile.twitter.com/jack/status/20"));
        assert!(!builtin("https://notx.com/jack/status/20"));
        assert!(!builtin("https://example.com/x.com/"));
    }

    #[test]
    fn hacker_news_discussions_match_but_listings_do_not() {
        assert!(builtin("https://news.ycombinator.com/item?id=15"));
        assert!(!builtin("https://news.ycombinator.com/"));
        assert!(!builtin("https://news.ycombinator.com/newest"));
        assert!(!builtin("https://news.ycombinator.com/user?id=pg"));
    }

    #[test]
    fn other_linkedin_pages_do_not_match() {
        assert!(!builtin("https://www.linkedin.com/feed/"));
        assert!(!builtin("https://www.linkedin.com/jobs/view/123/"));
        assert!(!builtin("https://www.linkedin.com/pulse/an-article/"));
        assert!(!builtin("https://www.linkedin.com/"));
    }

    #[test]
    fn lookalike_hosts_and_other_sites_do_not_match() {
        assert!(!builtin("https://notlinkedin.com/in/someone/"));
        assert!(!builtin("https://linkedin.com.evil.example/in/someone/"));
        assert!(!builtin("https://example.com/in/someone/"));
    }

    #[test]
    fn malformed_urls_do_not_match() {
        assert!(!builtin("not a url"));
        assert!(!builtin(""));
    }

    #[test]
    fn extra_rules_match_host_and_path_prefix() {
        let extra = vec!["example.com/app/".to_string()];
        assert!(prefers_browser_snapshot(
            "https://example.com/app/page",
            &extra
        ));
        assert!(prefers_browser_snapshot(
            "https://sub.example.com/app/",
            &extra
        ));
        assert!(!prefers_browser_snapshot(
            "https://example.com/blog/",
            &extra
        ));
    }

    #[test]
    fn extra_rule_without_path_matches_whole_site() {
        let extra = vec!["example.com".to_string()];
        assert!(prefers_browser_snapshot("https://example.com/", &extra));
        assert!(prefers_browser_snapshot(
            "https://www.example.com/any/page",
            &extra
        ));
    }

    #[test]
    fn extra_rules_tolerate_scheme_www_wildcard_and_case() {
        for rule in [
            "https://www.Example.com/app/*",
            " example.com/app/ ",
            "http://example.com/app/",
        ] {
            assert!(
                prefers_browser_snapshot("https://example.com/app/x", &[rule.to_string()]),
                "{rule}"
            );
        }
    }

    #[test]
    fn extra_rules_ignore_port_in_url() {
        let extra = vec!["127.0.0.1/in/".to_string()];
        assert!(prefers_browser_snapshot(
            "http://127.0.0.1:4321/in/someone",
            &extra
        ));
    }

    #[test]
    fn empty_rule_matches_nothing() {
        assert!(!prefers_browser_snapshot(
            "https://example.com/",
            &[String::new(), "/x".to_string()]
        ));
    }
}
