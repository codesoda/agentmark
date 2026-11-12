use serde::{Deserialize, Serialize};
use std::io;
use thiserror::Error;

// ── Error types ─────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("invalid agent: {value} (expected \"claude\", \"codex\" or \"pi\")")]
    InvalidAgent { value: String },

    #[error("failed to spawn {provider} CLI: {source}")]
    Spawn {
        provider: &'static str,
        source: io::Error,
    },

    #[error("{provider} CLI exited with status {status}: {stderr}")]
    ProcessFailed {
        provider: &'static str,
        status: i32,
        stderr: String,
    },

    #[error("{provider} CLI produced invalid output: {reason}")]
    InvalidResponse {
        provider: &'static str,
        reason: String,
    },

    #[error("failed to write temporary file for {provider}: {source}")]
    TempFileWrite {
        provider: &'static str,
        source: io::Error,
    },

    #[error("failed to read output file for {provider}: {source}")]
    TempFileRead {
        provider: &'static str,
        source: io::Error,
    },
}

// ── Request / Response ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentRequest {
    pub article_content: String,
    pub user_note: Option<String>,
    pub existing_tags: Vec<String>,
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnrichmentResponse {
    pub summary: String,
    pub suggested_tags: Vec<String>,
    pub suggested_collection: Option<String>,
}

/// Request to turn a simplified HTML snapshot of a page into markdown.
#[derive(Debug, Clone)]
pub struct ArticleRequest {
    pub url: String,
    pub title: String,
    /// Simplified page HTML (see `extract::simplify_html`), not the raw capture.
    pub page_html: String,
    /// The same page as fetched without login, simplified the same way.
    /// Supplementary: it may hold content the snapshot lacks (e.g. comments).
    pub fetched_html: Option<String>,
}

// ── Provider trait ──────────────────────────────────────────────────

pub trait AgentProvider {
    fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse, AgentError>;

    /// Render a page snapshot as a faithful markdown document.
    ///
    /// Used for pages where generic article extraction does not apply
    /// (profiles, feeds, app-like pages captured from the browser).
    fn render_article(&self, request: &ArticleRequest) -> Result<String, AgentError>;
}

/// Normalizes model-written markdown: trims, unwraps a surrounding code fence,
/// and rejects blank output.
pub fn normalize_markdown(raw: &str, provider: &'static str) -> Result<String, AgentError> {
    let mut text = raw.trim();
    if text.starts_with("```") && text.ends_with("```") && text.len() >= 6 {
        if let Some(first_newline) = text.find('\n') {
            text = text[first_newline + 1..text.len() - 3].trim();
        }
    }
    if text.is_empty() {
        return Err(AgentError::InvalidResponse {
            provider,
            reason: "article markdown is blank".to_string(),
        });
    }
    Ok(format!("{text}\n"))
}

/// Extract JSON content from markdown code fences (```json ... ``` or ``` ... ```).
pub(crate) fn extract_json_from_markdown(text: &str) -> Option<String> {
    let start = text.find("```")?;
    let after_fence = &text[start + 3..];
    // Skip optional language tag on the same line
    let content_start = after_fence.find('\n')? + 1;
    let content = &after_fence[content_start..];
    let end = content.find("```")?;
    Some(content[..end].trim().to_string())
}

pub(crate) fn truncate_stderr(stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.len() <= 500 {
        trimmed.to_string()
    } else {
        let mut end = 500;
        while !trimmed.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}... (truncated)", &trimmed[..end])
    }
}

// ── Response normalization ──────────────────────────────────────────

/// Normalizes an `EnrichmentResponse`: trims whitespace, removes blank/duplicate
/// tags, and normalizes blank collection to `None`. Returns an error if the
/// summary is blank after trimming.
pub fn normalize_response(
    raw: EnrichmentResponse,
    provider: &'static str,
) -> Result<EnrichmentResponse, AgentError> {
    let summary = raw.summary.trim().to_string();
    if summary.is_empty() {
        return Err(AgentError::InvalidResponse {
            provider,
            reason: "summary is blank".to_string(),
        });
    }

    let mut seen = std::collections::HashSet::new();
    let suggested_tags: Vec<String> = raw
        .suggested_tags
        .into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty() && seen.insert(t.clone()))
        .collect();

    let suggested_collection = raw
        .suggested_collection
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());

    Ok(EnrichmentResponse {
        summary,
        suggested_tags,
        suggested_collection,
    })
}

// ── Subprocess runner seam ──────────────────────────────────────────

/// Output captured from a subprocess invocation.
pub(crate) struct ProcessOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Trait for running subprocesses, enabling test injection.
pub(crate) trait ProcessRunner {
    fn run(
        &self,
        program: &str,
        args: &[&str],
        stdin_data: &str,
    ) -> Result<ProcessOutput, io::Error>;
}

/// Default runner that spawns real subprocesses.
pub(crate) struct RealRunner;

impl ProcessRunner for RealRunner {
    fn run(
        &self,
        program: &str,
        args: &[&str],
        stdin_data: &str,
    ) -> Result<ProcessOutput, io::Error> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut child = Command::new(program)
            .args(args)
            .env("PATH", agent_search_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(ref mut stdin) = child.stdin.take() {
            stdin.write_all(stdin_data.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok(ProcessOutput {
            stdout,
            stderr,
            exit_code,
        })
    }
}

/// Directories where agent CLIs and their runtimes (node, bun) are commonly
/// installed, relative to `$HOME` unless absolute.
const EXTRA_BIN_DIRS: &[&str] = &[
    ".local/bin",
    ".volta/bin",
    ".bun/bin",
    ".cargo/bin",
    ".npm-global/bin",
    ".pi/agent/bin",
    ".claude/local",
    "/opt/homebrew/bin",
    "/usr/local/bin",
];

/// `PATH` for agent subprocesses.
///
/// Chrome starts the native host with a bare system `PATH`, so agent CLIs
/// installed in user directories would not be found. The inherited `PATH`
/// keeps priority; well-known install directories are appended.
fn agent_search_path() -> std::ffi::OsString {
    build_search_path(
        std::env::var_os("PATH"),
        std::env::var_os("HOME").map(std::path::PathBuf::from),
    )
}

fn build_search_path(
    inherited: Option<std::ffi::OsString>,
    home: Option<std::path::PathBuf>,
) -> std::ffi::OsString {
    let mut dirs: Vec<std::path::PathBuf> = inherited
        .as_deref()
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default();
    for extra in EXTRA_BIN_DIRS {
        let dir = if extra.starts_with('/') {
            std::path::PathBuf::from(extra)
        } else if let Some(ref home) = home {
            home.join(extra)
        } else {
            continue;
        };
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    std::env::join_paths(dirs).unwrap_or_else(|_| inherited.unwrap_or_default())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_path_keeps_inherited_dirs_first_and_appends_user_dirs() {
        let path = build_search_path(
            Some("/usr/bin:/bin".into()),
            Some(std::path::PathBuf::from("/Users/me")),
        );
        let dirs: Vec<_> = std::env::split_paths(&path).collect();
        assert_eq!(dirs[0], std::path::PathBuf::from("/usr/bin"));
        assert_eq!(dirs[1], std::path::PathBuf::from("/bin"));
        assert!(dirs.contains(&std::path::PathBuf::from("/Users/me/.local/bin")));
        assert!(dirs.contains(&std::path::PathBuf::from("/Users/me/.volta/bin")));
        assert!(dirs.contains(&std::path::PathBuf::from("/opt/homebrew/bin")));
    }

    #[test]
    fn search_path_does_not_duplicate_existing_dirs() {
        let path = build_search_path(
            Some("/opt/homebrew/bin:/usr/bin".into()),
            Some(std::path::PathBuf::from("/Users/me")),
        );
        let count = std::env::split_paths(&path)
            .filter(|d| d == &std::path::PathBuf::from("/opt/homebrew/bin"))
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn search_path_without_home_still_adds_system_dirs() {
        let path = build_search_path(Some("/usr/bin".into()), None);
        let dirs: Vec<_> = std::env::split_paths(&path).collect();
        assert!(dirs.contains(&std::path::PathBuf::from("/usr/local/bin")));
        assert!(!dirs.iter().any(|d| d.ends_with(".local/bin")));
    }

    #[test]
    fn normalize_markdown_trims_and_adds_trailing_newline() {
        assert_eq!(
            normalize_markdown("  # Title\n\nBody  ", "pi").unwrap(),
            "# Title\n\nBody\n"
        );
    }

    #[test]
    fn normalize_markdown_unwraps_code_fence() {
        assert_eq!(
            normalize_markdown("```markdown\n# Title\n```", "pi").unwrap(),
            "# Title\n"
        );
    }

    #[test]
    fn normalize_markdown_rejects_blank() {
        assert!(normalize_markdown("  \n ", "pi").is_err());
        assert!(normalize_markdown("```\n```", "pi").is_err());
    }

    #[test]
    fn enrichment_response_serde_roundtrip() {
        let resp = EnrichmentResponse {
            summary: "A great article about Rust.".to_string(),
            suggested_tags: vec!["rust".to_string(), "programming".to_string()],
            suggested_collection: Some("tech".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: EnrichmentResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, parsed);
    }

    #[test]
    fn enrichment_response_deserialize_without_collection() {
        let json = r#"{"summary":"test","suggested_tags":["a"],"suggested_collection":null}"#;
        let resp: EnrichmentResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.suggested_collection, None);
    }

    #[test]
    fn normalize_trims_summary() {
        let raw = EnrichmentResponse {
            summary: "  trimmed  ".to_string(),
            suggested_tags: vec![],
            suggested_collection: None,
        };
        let result = normalize_response(raw, "test").unwrap();
        assert_eq!(result.summary, "trimmed");
    }

    #[test]
    fn normalize_rejects_blank_summary() {
        let raw = EnrichmentResponse {
            summary: "   ".to_string(),
            suggested_tags: vec![],
            suggested_collection: None,
        };
        let err = normalize_response(raw, "test").unwrap_err();
        assert!(matches!(err, AgentError::InvalidResponse { .. }));
    }

    #[test]
    fn normalize_removes_blank_tags() {
        let raw = EnrichmentResponse {
            summary: "ok".to_string(),
            suggested_tags: vec!["a".to_string(), "  ".to_string(), "b".to_string()],
            suggested_collection: None,
        };
        let result = normalize_response(raw, "test").unwrap();
        assert_eq!(result.suggested_tags, vec!["a", "b"]);
    }

    #[test]
    fn normalize_deduplicates_tags_preserving_order() {
        let raw = EnrichmentResponse {
            summary: "ok".to_string(),
            suggested_tags: vec![
                "rust".to_string(),
                "wasm".to_string(),
                "rust".to_string(),
                " wasm ".to_string(),
            ],
            suggested_collection: None,
        };
        let result = normalize_response(raw, "test").unwrap();
        assert_eq!(result.suggested_tags, vec!["rust", "wasm"]);
    }

    #[test]
    fn normalize_trims_tags() {
        let raw = EnrichmentResponse {
            summary: "ok".to_string(),
            suggested_tags: vec!["  a  ".to_string()],
            suggested_collection: None,
        };
        let result = normalize_response(raw, "test").unwrap();
        assert_eq!(result.suggested_tags, vec!["a"]);
    }

    #[test]
    fn normalize_blank_collection_becomes_none() {
        let raw = EnrichmentResponse {
            summary: "ok".to_string(),
            suggested_tags: vec![],
            suggested_collection: Some("   ".to_string()),
        };
        let result = normalize_response(raw, "test").unwrap();
        assert_eq!(result.suggested_collection, None);
    }

    #[test]
    fn normalize_trims_collection() {
        let raw = EnrichmentResponse {
            summary: "ok".to_string(),
            suggested_tags: vec![],
            suggested_collection: Some("  tech  ".to_string()),
        };
        let result = normalize_response(raw, "test").unwrap();
        assert_eq!(result.suggested_collection, Some("tech".to_string()));
    }

    #[test]
    fn enrichment_request_serde_roundtrip() {
        let req = EnrichmentRequest {
            article_content: "Hello world".to_string(),
            user_note: Some("check this".to_string()),
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            existing_tags: vec!["tag1".to_string()],
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: EnrichmentRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.url, "https://example.com");
        assert_eq!(parsed.user_note, Some("check this".to_string()));
    }
}
