//! Integration tests for the `fetch` module.
//!
//! Uses `mockito` for deterministic local HTTP fixtures — no real network calls.

use agentmark::fetch::{self, FetchError};
use std::time::Duration;

/// Build a client with short timeout for testing.
fn test_client() -> reqwest::blocking::Client {
    reqwest::blocking::ClientBuilder::new()
        .user_agent("agentmark-test/0.1.0")
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap()
}

/// Build a client with very short timeout for timeout tests.
fn short_timeout_client() -> reqwest::blocking::Client {
    reqwest::blocking::ClientBuilder::new()
        .user_agent("agentmark-test/0.1.0")
        .timeout(Duration::from_millis(200))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap()
}

#[test]
fn fetch_success_returns_html_and_metadata() {
    let html = r#"<html><head>
        <title>Test Page</title>
        <meta property="og:title" content="OG Test">
        <meta name="description" content="A test page">
    </head><body>Hello</body></html>"#;

    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/page")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(html)
        .create();

    let url = format!("{}/page", server.url());
    let (returned_html, metadata) = fetch::fetch_page_with_client(&url, &test_client()).unwrap();

    mock.assert();
    assert_eq!(returned_html, html);
    assert_eq!(metadata.title.as_deref(), Some("OG Test"));
    assert_eq!(metadata.description.as_deref(), Some("A test page"));
}

#[test]
fn fetch_sends_user_agent_header() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/ua")
        .match_header("user-agent", "agentmark-test/0.1.0")
        .with_status(200)
        .with_body("<html></html>")
        .create();

    let url = format!("{}/ua", server.url());
    let _ = fetch::fetch_page_with_client(&url, &test_client());
    mock.assert();
}

#[test]
fn fetch_follows_redirects() {
    let mut server = mockito::Server::new();

    let final_html = "<html><head><title>Final</title></head></html>";

    let mock_final = server
        .mock("GET", "/final")
        .with_status(200)
        .with_body(final_html)
        .create();

    let mock_redirect = server
        .mock("GET", "/start")
        .with_status(302)
        .with_header("location", &format!("{}/final", server.url()))
        .create();

    let url = format!("{}/start", server.url());
    let (html, metadata) = fetch::fetch_page_with_client(&url, &test_client()).unwrap();

    mock_redirect.assert();
    mock_final.assert();
    assert_eq!(html, final_html);
    assert_eq!(metadata.title.as_deref(), Some("Final"));
}

#[test]
fn fetch_403_returns_http_status_error() {
    let mut server = mockito::Server::new();
    let mock = server.mock("GET", "/forbidden").with_status(403).create();

    let url = format!("{}/forbidden", server.url());
    let result = fetch::fetch_page_with_client(&url, &test_client());
    mock.assert();

    let err = result.unwrap_err();
    match err {
        FetchError::HttpStatus { status, .. } => assert_eq!(status, 403),
        other => panic!("expected HttpStatus, got: {other}"),
    }
}

#[test]
fn fetch_404_returns_http_status_error() {
    let mut server = mockito::Server::new();
    let mock = server.mock("GET", "/missing").with_status(404).create();

    let url = format!("{}/missing", server.url());
    let result = fetch::fetch_page_with_client(&url, &test_client());
    mock.assert();

    let err = result.unwrap_err();
    match err {
        FetchError::HttpStatus { status, .. } => assert_eq!(status, 404),
        other => panic!("expected HttpStatus, got: {other}"),
    }
}

#[test]
fn fetch_500_returns_http_status_error() {
    let mut server = mockito::Server::new();
    let mock = server.mock("GET", "/error").with_status(500).create();

    let url = format!("{}/error", server.url());
    let result = fetch::fetch_page_with_client(&url, &test_client());
    mock.assert();

    let err = result.unwrap_err();
    match err {
        FetchError::HttpStatus { status, .. } => assert_eq!(status, 500),
        other => panic!("expected HttpStatus, got: {other}"),
    }
}

#[test]
fn fetch_timeout_returns_timeout_error() {
    let _server = mockito::Server::new();
    // mockito doesn't directly support delays, but we can test by pointing at
    // a port where nothing is listening (connection refused is also a transport error).
    // Instead, test with invalid URL that will definitely timeout.

    // Use a non-routable IP to trigger timeout with a very short timeout client
    let result = fetch::fetch_page_with_client("http://192.0.2.1:1/slow", &short_timeout_client());

    let err = result.unwrap_err();
    // This will be either Timeout or Transport depending on OS behavior
    match err {
        FetchError::Timeout { .. } | FetchError::Transport { .. } => {}
        other => panic!("expected Timeout or Transport, got: {other}"),
    }
}

#[test]
fn fetch_connection_refused_returns_transport_error() {
    // Use a port that is almost certainly not listening
    let result = fetch::fetch_page_with_client("http://127.0.0.1:1/nope", &short_timeout_client());

    let err = result.unwrap_err();
    match err {
        FetchError::Transport { .. } | FetchError::Timeout { .. } => {}
        other => panic!("expected Transport, got: {other}"),
    }
}

#[test]
fn fetch_invalid_url() {
    let result = fetch::fetch_page("not a valid url");
    let err = result.unwrap_err();
    assert!(matches!(err, FetchError::InvalidUrl { .. }));
}

#[test]
fn fetch_unsupported_scheme() {
    let result = fetch::fetch_page("ftp://example.com/file");
    let err = result.unwrap_err();
    assert!(matches!(err, FetchError::UnsupportedScheme { .. }));
}

#[test]
fn fetch_empty_body_returns_empty_html_and_default_metadata() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/empty")
        .with_status(200)
        .with_body("")
        .create();

    let url = format!("{}/empty", server.url());
    let (html, metadata) = fetch::fetch_page_with_client(&url, &test_client()).unwrap();

    mock.assert();
    assert_eq!(html, "");
    assert!(metadata.title.is_none());
    assert!(metadata.description.is_none());
}

#[test]
fn fetch_metadata_extraction_uses_final_url_for_relative_resolution() {
    let mut server = mockito::Server::new();

    let final_html = r#"<html><head>
        <link rel="canonical" href="/canonical-path">
        <link rel="icon" href="/favicon.ico">
    </head></html>"#;

    let mock_final = server
        .mock("GET", "/final")
        .with_status(200)
        .with_body(final_html)
        .create();

    let mock_redirect = server
        .mock("GET", "/start")
        .with_status(301)
        .with_header("location", &format!("{}/final", server.url()))
        .create();

    let url = format!("{}/start", server.url());
    let (_, metadata) = fetch::fetch_page_with_client(&url, &test_client()).unwrap();

    mock_redirect.assert();
    mock_final.assert();

    // Relative URLs should be resolved against the final URL (after redirect)
    let expected_base = server.url();
    assert_eq!(
        metadata.canonical_url.as_deref(),
        Some(format!("{expected_base}/canonical-path").as_str())
    );
    assert_eq!(
        metadata.favicon_url.as_deref(),
        Some(format!("{expected_base}/favicon.ico").as_str())
    );
}
