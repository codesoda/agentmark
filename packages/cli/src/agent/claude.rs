use super::prompt::{build_article_prompt, build_prompt, PromptParts};
use tracing::{debug, instrument, warn};

use super::provider::{
    extract_json_from_markdown, normalize_markdown, normalize_response, truncate_stderr,
    AgentError, AgentProvider, ArticleRequest, EnrichmentRequest, EnrichmentResponse,
    ProcessOutput, ProcessRunner,
};

/// Claude CLI provider. Invokes the local `claude` binary in non-interactive
/// mode with structured JSON output.
pub(crate) struct ClaudeProvider {
    system_prompt: Option<String>,
    runner: Box<dyn ProcessRunner>,
}

impl ClaudeProvider {
    pub fn new(system_prompt: Option<String>, runner: Box<dyn ProcessRunner>) -> Self {
        Self {
            system_prompt,
            runner,
        }
    }
}

impl AgentProvider for ClaudeProvider {
    #[instrument(skip(self, request), fields(url = %request.url))]
    fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse, AgentError> {
        let parts = build_prompt(request, self.system_prompt.as_deref());

        let stdout = self.run(&parts)?;

        // Claude --output-format json wraps the response in a JSON object
        // with a "result" field containing the text response.
        let raw_response = parse_claude_output(&stdout)?;
        normalize_response(raw_response, "claude")
    }

    #[instrument(skip(self, request), fields(url = %request.url))]
    fn render_article(&self, request: &ArticleRequest) -> Result<String, AgentError> {
        let parts = build_article_prompt(request, self.system_prompt.as_deref());
        let stdout = self.run(&parts)?;
        // Prefer the envelope's result text; fall back to raw stdout.
        let text = extract_result_text(&stdout).unwrap_or(stdout);
        normalize_markdown(&text, "claude")
    }
}

impl ClaudeProvider {
    /// Invoke the Claude CLI once with the given prompt and return its stdout.
    fn run(&self, parts: &PromptParts) -> Result<String, AgentError> {
        let mut args: Vec<&str> = vec![
            "--print", // non-interactive, print response only
            "--output-format",
            "json", // structured JSON output
            "--max-turns",
            "1",                        // single turn, no tool loops
            "--no-session-persistence", // don't pollute user's session history
        ];

        // Apply system prompt via CLI flag if available
        if let Some(ref sp) = parts.system_prompt {
            args.push("--system-prompt");
            args.push(sp);
        }

        // Disable tool access for safety
        args.push("--allowedTools");
        args.push("");

        let output: ProcessOutput = self
            .runner
            .run("claude", &args, &parts.user_prompt)
            .map_err(|source| AgentError::Spawn {
                provider: "claude",
                source,
            })?;

        if output.exit_code != 0 {
            warn!(exit_code = output.exit_code, "claude CLI failed");
            let stderr = truncate_stderr(&output.stderr);
            return Err(AgentError::ProcessFailed {
                provider: "claude",
                status: output.exit_code,
                stderr,
            });
        }

        debug!("claude CLI returned successfully");
        Ok(output.stdout)
    }
}

/// Extract the `result` string from Claude CLI JSON output.
///
/// The CLI `--output-format json` has two known shapes:
///   1. **Streaming array** (current): a JSON array of event objects. The final
///      element with `"type": "result"` contains a `"result"` string field.
///   2. **Single envelope** (legacy): a single JSON object with a `"result"` field.
fn extract_result_text(stdout: &str) -> Option<String> {
    let trimmed = stdout.trim();
    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;

    // Streaming array: scan for the result event
    if let Some(arr) = value.as_array() {
        for item in arr.iter().rev() {
            if item.get("type").and_then(|v| v.as_str()) == Some("result") {
                return item
                    .get("result")
                    .and_then(|v| v.as_str())
                    .map(String::from);
            }
        }
        return None;
    }

    // Single envelope object
    value
        .get("result")
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Parse Claude CLI JSON output into an [`EnrichmentResponse`].
fn parse_claude_output(stdout: &str) -> Result<EnrichmentResponse, AgentError> {
    let trimmed = stdout.trim();

    // Try to extract the result text from the CLI envelope / stream
    if let Some(result_text) = extract_result_text(trimmed) {
        // The result field contains the model's text response as a string.
        // Try to parse it as our EnrichmentResponse.
        if let Ok(resp) = serde_json::from_str::<EnrichmentResponse>(&result_text) {
            return Ok(resp);
        }
        // Sometimes the model wraps JSON in markdown code fences
        if let Some(extracted) = extract_json_from_markdown(&result_text) {
            if let Ok(resp) = serde_json::from_str::<EnrichmentResponse>(&extracted) {
                return Ok(resp);
            }
        }
        return Err(AgentError::InvalidResponse {
            provider: "claude",
            reason: "result field does not contain valid enrichment JSON".to_string(),
        });
    }

    // Fallback: raw JSON output (direct EnrichmentResponse)
    if let Ok(resp) = serde_json::from_str::<EnrichmentResponse>(trimmed) {
        return Ok(resp);
    }

    Err(AgentError::InvalidResponse {
        provider: "claude",
        reason: "output is not valid JSON".to_string(),
    })
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::provider::ProcessOutput;
    use super::*;
    use std::io;

    /// Mock runner that returns a predefined output.
    struct MockRunner {
        result: Result<ProcessOutput, io::Error>,
    }

    impl MockRunner {
        fn success(stdout: &str) -> Box<Self> {
            Box::new(Self {
                result: Ok(ProcessOutput {
                    stdout: stdout.to_string(),
                    stderr: String::new(),
                    exit_code: 0,
                }),
            })
        }

        fn failure(exit_code: i32, stderr: &str) -> Box<Self> {
            Box::new(Self {
                result: Ok(ProcessOutput {
                    stdout: String::new(),
                    stderr: stderr.to_string(),
                    exit_code,
                }),
            })
        }

        fn spawn_error() -> Box<Self> {
            Box::new(Self {
                result: Err(io::Error::new(io::ErrorKind::NotFound, "program not found")),
            })
        }
    }

    impl ProcessRunner for MockRunner {
        fn run(
            &self,
            _program: &str,
            _args: &[&str],
            _stdin_data: &str,
        ) -> Result<ProcessOutput, io::Error> {
            match &self.result {
                Ok(output) => Ok(ProcessOutput {
                    stdout: output.stdout.clone(),
                    stderr: output.stderr.clone(),
                    exit_code: output.exit_code,
                }),
                Err(e) => Err(io::Error::new(e.kind(), e.to_string())),
            }
        }
    }

    fn sample_request() -> EnrichmentRequest {
        EnrichmentRequest {
            article_content: "Rust is great.".to_string(),
            user_note: None,
            existing_tags: vec![],
            url: "https://example.com".to_string(),
            title: "Test".to_string(),
        }
    }

    #[test]
    fn claude_success_with_envelope() {
        let response_json = r#"{"summary":"A Rust article.","suggested_tags":["rust"],"suggested_collection":null}"#;
        let envelope = format!(r#"{{"result":"{}"}}"#, response_json.replace('"', "\\\""));
        let runner = MockRunner::success(&envelope);
        let provider = ClaudeProvider::new(None, runner);
        let result = provider.enrich(&sample_request()).unwrap();
        assert_eq!(result.summary, "A Rust article.");
        assert_eq!(result.suggested_tags, vec!["rust"]);
        assert_eq!(result.suggested_collection, None);
    }

    #[test]
    fn claude_success_with_streaming_array() {
        let response_json = r#"{"summary":"A Rust article.","suggested_tags":["rust"],"suggested_collection":null}"#;
        let escaped = response_json.replace('"', "\\\"");
        let stream = format!(
            r#"[{{"type":"system","subtype":"init"}},{{"type":"assistant","message":{{}}}},{{"type":"result","subtype":"success","result":"{escaped}"}}]"#
        );
        let runner = MockRunner::success(&stream);
        let provider = ClaudeProvider::new(None, runner);
        let result = provider.enrich(&sample_request()).unwrap();
        assert_eq!(result.summary, "A Rust article.");
        assert_eq!(result.suggested_tags, vec!["rust"]);
    }

    #[test]
    fn claude_success_with_raw_json() {
        let json = r#"{"summary":"A Rust article.","suggested_tags":["rust"],"suggested_collection":"tech"}"#;
        let runner = MockRunner::success(json);
        let provider = ClaudeProvider::new(None, runner);
        let result = provider.enrich(&sample_request()).unwrap();
        assert_eq!(result.summary, "A Rust article.");
        assert_eq!(result.suggested_collection, Some("tech".to_string()));
    }

    #[test]
    fn claude_success_with_markdown_fenced_json() {
        let result_text = "Here is the result:\n```json\n{\"summary\":\"Test.\",\"suggested_tags\":[\"a\"],\"suggested_collection\":null}\n```";
        let envelope = serde_json::json!({"result": result_text}).to_string();
        let runner = MockRunner::success(&envelope);
        let provider = ClaudeProvider::new(None, runner);
        let result = provider.enrich(&sample_request()).unwrap();
        assert_eq!(result.summary, "Test.");
    }

    #[test]
    fn claude_normalizes_response() {
        let json = r#"{"summary":"  spaced  ","suggested_tags":["a","a","  "],"suggested_collection":"  "}"#;
        let runner = MockRunner::success(json);
        let provider = ClaudeProvider::new(None, runner);
        let result = provider.enrich(&sample_request()).unwrap();
        assert_eq!(result.summary, "spaced");
        assert_eq!(result.suggested_tags, vec!["a"]);
        assert_eq!(result.suggested_collection, None);
    }

    #[test]
    fn claude_spawn_error() {
        let runner = MockRunner::spawn_error();
        let provider = ClaudeProvider::new(None, runner);
        let err = provider.enrich(&sample_request()).unwrap_err();
        assert!(matches!(
            err,
            AgentError::Spawn {
                provider: "claude",
                ..
            }
        ));
    }

    #[test]
    fn claude_process_failed() {
        let runner = MockRunner::failure(1, "auth required");
        let provider = ClaudeProvider::new(None, runner);
        let err = provider.enrich(&sample_request()).unwrap_err();
        match err {
            AgentError::ProcessFailed {
                provider,
                status,
                stderr,
            } => {
                assert_eq!(provider, "claude");
                assert_eq!(status, 1);
                assert!(stderr.contains("auth required"));
            }
            _ => panic!("expected ProcessFailed"),
        }
    }

    #[test]
    fn claude_invalid_json() {
        let runner = MockRunner::success("not json at all");
        let provider = ClaudeProvider::new(None, runner);
        let err = provider.enrich(&sample_request()).unwrap_err();
        assert!(matches!(
            err,
            AgentError::InvalidResponse {
                provider: "claude",
                ..
            }
        ));
    }

    #[test]
    fn claude_blank_summary_rejected() {
        let json = r#"{"summary":"   ","suggested_tags":[],"suggested_collection":null}"#;
        let runner = MockRunner::success(json);
        let provider = ClaudeProvider::new(None, runner);
        let err = provider.enrich(&sample_request()).unwrap_err();
        assert!(matches!(
            err,
            AgentError::InvalidResponse {
                provider: "claude",
                ..
            }
        ));
    }

    #[test]
    fn claude_with_system_prompt() {
        // Verify the provider can be constructed with a system prompt
        // (command assembly is tested via mock runner; we just verify no panic)
        let json = r#"{"summary":"ok","suggested_tags":[],"suggested_collection":null}"#;
        let runner = MockRunner::success(json);
        let provider = ClaudeProvider::new(Some("You are helpful.".to_string()), runner);
        let result = provider.enrich(&sample_request()).unwrap();
        assert_eq!(result.summary, "ok");
    }

    #[test]
    fn claude_command_args_captured() {
        use std::sync::{Arc, Mutex};

        struct CapturingRunner {
            captured_program: Arc<Mutex<Option<String>>>,
            captured_args: Arc<Mutex<Option<Vec<String>>>>,
            captured_stdin: Arc<Mutex<Option<String>>>,
        }

        impl ProcessRunner for CapturingRunner {
            fn run(
                &self,
                program: &str,
                args: &[&str],
                stdin_data: &str,
            ) -> Result<ProcessOutput, io::Error> {
                *self.captured_program.lock().unwrap() = Some(program.to_string());
                *self.captured_args.lock().unwrap() =
                    Some(args.iter().map(|s| s.to_string()).collect());
                *self.captured_stdin.lock().unwrap() = Some(stdin_data.to_string());
                Ok(ProcessOutput {
                    stdout: r#"{"summary":"ok","suggested_tags":[],"suggested_collection":null}"#
                        .to_string(),
                    stderr: String::new(),
                    exit_code: 0,
                })
            }
        }

        let program = Arc::new(Mutex::new(None));
        let args = Arc::new(Mutex::new(None));
        let stdin = Arc::new(Mutex::new(None));

        let runner = Box::new(CapturingRunner {
            captured_program: program.clone(),
            captured_args: args.clone(),
            captured_stdin: stdin.clone(),
        });

        let provider = ClaudeProvider::new(Some("Be helpful.".to_string()), runner);
        provider.enrich(&sample_request()).unwrap();

        let captured_program = program.lock().unwrap().clone().unwrap();
        assert_eq!(captured_program, "claude");

        let captured_args = args.lock().unwrap().clone().unwrap();
        assert!(captured_args.contains(&"--print".to_string()));
        assert!(captured_args.contains(&"--output-format".to_string()));
        assert!(captured_args.contains(&"json".to_string()));
        assert!(captured_args.contains(&"--max-turns".to_string()));
        assert!(captured_args.contains(&"1".to_string()));
        assert!(captured_args.contains(&"--system-prompt".to_string()));
        assert!(captured_args.contains(&"Be helpful.".to_string()));
        assert!(captured_args.contains(&"--allowedTools".to_string()));

        let captured_stdin = stdin.lock().unwrap().clone().unwrap();
        assert!(captured_stdin.contains("https://example.com"));
        assert!(captured_stdin.contains("Rust is great."));
    }

    #[test]
    fn claude_command_args_without_system_prompt() {
        use std::sync::{Arc, Mutex};

        struct CapturingRunner {
            captured_args: Arc<Mutex<Option<Vec<String>>>>,
        }

        impl ProcessRunner for CapturingRunner {
            fn run(
                &self,
                _program: &str,
                args: &[&str],
                _stdin_data: &str,
            ) -> Result<ProcessOutput, io::Error> {
                *self.captured_args.lock().unwrap() =
                    Some(args.iter().map(|s| s.to_string()).collect());
                Ok(ProcessOutput {
                    stdout: r#"{"summary":"ok","suggested_tags":[],"suggested_collection":null}"#
                        .to_string(),
                    stderr: String::new(),
                    exit_code: 0,
                })
            }
        }

        let args = Arc::new(Mutex::new(None));
        let runner = Box::new(CapturingRunner {
            captured_args: args.clone(),
        });

        let provider = ClaudeProvider::new(None, runner);
        provider.enrich(&sample_request()).unwrap();

        let captured_args = args.lock().unwrap().clone().unwrap();
        assert!(!captured_args.contains(&"--system-prompt".to_string()));
    }
}
