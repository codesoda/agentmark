//! HTTP page fetching and metadata extraction.
//!
//! This module is a standalone library seam for Specs 06-09. It depends only on
//! HTTP/HTML/JSON crates and has no coupling to `config`, `db`, or `models::Bookmark`.
//!
//! Public API:
//! - [`fetch_page`] — fetch a URL and return `(raw_html, PageMetadata)`.
//! - [`PageMetadata`] — extracted metadata from OG, meta tags, and JSON-LD.
//! - [`FetchError`] — typed error surface for network/fetch failures.

pub mod metadata;

pub use metadata::PageMetadata;

use reqwest::redirect::Policy;
use std::time::Duration;
use url::Url;

/// Errors that can occur during HTTP fetching.
///
/// Only network/body failures are represented here. Metadata parsing is best-effort
/// and never produces a `FetchError`.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("invalid URL \"{url}\": {reason}")]
    InvalidUrl { url: String, reason: String },

    #[error("unsupported URL scheme \"{scheme}\" in \"{url}\"")]
    UnsupportedScheme { scheme: String, url: String },

    #[error("request timed out for {url}")]
    Timeout { url: String },

    #[error("too many redirects for {url}")]
    TooManyRedirects { url: String },

    #[error("HTTP {status} for {url}")]
    HttpStatus { status: u16, url: String },

    #[error("transport error for {url}: {message}")]
    Transport { url: String, message: String },

    #[error("failed to read response body for {url}: {message}")]
    BodyRead { url: String, message: String },
}

const USER_AGENT: &str = concat!("agentmark/", env!("CARGO_PKG_VERSION"));
const TIMEOUT_SECS: u64 = 30;
const MAX_REDIRECTS: usize = 5;

/// Build a `reqwest::blocking::Client` with our standard policy.
fn build_client() -> reqwest::blocking::Client {
    build_client_with_timeout(Duration::from_secs(TIMEOUT_SECS))
}

/// Build a client with a custom timeout (used for testing).
fn build_client_with_timeout(timeout: Duration) -> reqwest::blocking::Client {
    reqwest::blocking::ClientBuilder::new()
        .user_agent(USER_AGENT)
        .timeout(timeout)
        .redirect(Policy::limited(MAX_REDIRECTS))
        .build()
        .expect("failed to build HTTP client")
}

/// Internal response context preserved for URL resolution and testing.
struct FetchResponse {
    final_url: Url,
    html: String,
}

/// Fetch a URL's HTML and extract metadata.
///
/// Returns `(raw_html, PageMetadata)` on success. Only network/HTTP failures produce
/// errors; metadata parsing is best-effort.
pub fn fetch_page(url: &str) -> Result<(String, PageMetadata), FetchError> {
    let resp = fetch_html(url, &build_client())?;
    let metadata = metadata::extract_metadata(&resp.html, &resp.final_url);
    Ok((resp.html, metadata))
}

/// Fetch with a custom client. Useful for testing with short timeouts or custom policies.
pub fn fetch_page_with_client(
    url: &str,
    client: &reqwest::blocking::Client,
) -> Result<(String, PageMetadata), FetchError> {
    let resp = fetch_html(url, client)?;
    let metadata = metadata::extract_metadata(&resp.html, &resp.final_url);
    Ok((resp.html, metadata))
}

/// Core HTTP fetch: validate URL, execute GET, classify errors, return raw HTML + final URL.
fn fetch_html(url: &str, client: &reqwest::blocking::Client) -> Result<FetchResponse, FetchError> {
    // Parse and validate URL
    let parsed = Url::parse(url).map_err(|e| FetchError::InvalidUrl {
        url: url.to_string(),
        reason: e.to_string(),
    })?;

    match parsed.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(FetchError::UnsupportedScheme {
                scheme: scheme.to_string(),
                url: url.to_string(),
            });
        }
    }

    // Execute request
    let response = client
        .get(url)
        .send()
        .map_err(|e| classify_reqwest_error(e, url))?;

    // Check status
    let status = response.status();
    let final_url = Url::parse(response.url().as_str()).unwrap_or(parsed);

    if !status.is_success() {
        return Err(FetchError::HttpStatus {
            status: status.as_u16(),
            url: url.to_string(),
        });
    }

    // Read body
    let html = response.text().map_err(|e| FetchError::BodyRead {
        url: url.to_string(),
        message: e.to_string(),
    })?;

    Ok(FetchResponse { final_url, html })
}

/// Classify a `reqwest::Error` into a typed `FetchError`.
fn classify_reqwest_error(err: reqwest::Error, url: &str) -> FetchError {
    if err.is_timeout() {
        FetchError::Timeout {
            url: url.to_string(),
        }
    } else if err.is_redirect() {
        FetchError::TooManyRedirects {
            url: url.to_string(),
        }
    } else {
        FetchError::Transport {
            url: url.to_string(),
            message: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_url_returns_error() {
        let result = fetch_page("not a url");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, FetchError::InvalidUrl { .. }));
        assert!(err.to_string().contains("not a url"));
    }

    #[test]
    fn unsupported_scheme_returns_error() {
        let result = fetch_page("ftp://example.com/file");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            FetchError::UnsupportedScheme { ref scheme, .. } if scheme == "ftp"
        ));
    }

    #[test]
    fn user_agent_constant_has_version() {
        assert!(USER_AGENT.starts_with("agentmark/"));
    }

    #[test]
    fn classify_timeout() {
        // Verify the classifier by checking the variant construction path
        let err = FetchError::Timeout {
            url: "https://example.com".to_string(),
        };
        assert!(err.to_string().contains("timed out"));
    }

    #[test]
    fn classify_http_status() {
        let err = FetchError::HttpStatus {
            status: 404,
            url: "https://example.com".to_string(),
        };
        assert!(err.to_string().contains("404"));
    }
}
