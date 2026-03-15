use serde::{Deserialize, Serialize};
use std::io;
use thiserror::Error;

// ── Error types ─────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("invalid agent: {value} (expected \"claude\" or \"codex\")")]
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

// ── Provider trait ──────────────────────────────────────────────────

pub trait AgentProvider {
    fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse, AgentError>;
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

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
