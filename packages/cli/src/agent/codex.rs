use std::path::PathBuf;

use super::prompt::{build_prompt, ENRICHMENT_SCHEMA};
use super::provider::{
    normalize_response, AgentError, AgentProvider, EnrichmentRequest, EnrichmentResponse,
    ProcessOutput, ProcessRunner,
};

/// Codex CLI provider. Invokes the local `codex` binary with structured
/// JSON output via a temporary schema file.
pub(crate) struct CodexProvider {
    system_prompt: Option<String>,
    runner: Box<dyn ProcessRunner>,
}

impl CodexProvider {
    pub fn new(system_prompt: Option<String>, runner: Box<dyn ProcessRunner>) -> Self {
        Self {
            system_prompt,
            runner,
        }
    }
}

impl AgentProvider for CodexProvider {
    fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse, AgentError> {
        let parts = build_prompt(request, self.system_prompt.as_deref());

        // Write schema to a temp file for codex --output-schema flag
        let schema_path = write_temp_schema()?;
        let _cleanup = TempFileGuard(schema_path.clone());

        let schema_path_str = schema_path.to_string_lossy().to_string();
        let mut args: Vec<&str> = vec![
            "exec",        // non-interactive execution
            "--ephemeral", // don't persist session history
            "--sandbox",
            "read-only", // no writes needed for enrichment
        ];

        args.push("--output-schema");
        args.push(&schema_path_str);

        let output: ProcessOutput = self
            .runner
            .run("codex", &args, &parts.user_prompt)
            .map_err(|source| AgentError::Spawn {
                provider: "codex",
                source,
            })?;

        if output.exit_code != 0 {
            let stderr = truncate_stderr(&output.stderr);
            return Err(AgentError::ProcessFailed {
                provider: "codex",
                status: output.exit_code,
                stderr,
            });
        }

        let raw_response = parse_codex_output(&output.stdout)?;
        normalize_response(raw_response, "codex")
    }
}

/// Write the enrichment JSON schema to a temp file. Returns the path.
fn write_temp_schema() -> Result<PathBuf, AgentError> {
    let mut path = std::env::temp_dir();
    let filename = format!("agentmark-schema-{}.json", std::process::id());
    path.push(filename);
    std::fs::write(&path, ENRICHMENT_SCHEMA).map_err(|source| AgentError::TempFileWrite {
        provider: "codex",
        source,
    })?;
    Ok(path)
}

/// RAII guard that removes a temp file on drop. Cleanup failures are
/// silently ignored since the primary operation already succeeded.
struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Parse Codex CLI output. Codex `exec` with `--output-schema` returns
/// structured JSON directly on stdout.
fn parse_codex_output(stdout: &str) -> Result<EnrichmentResponse, AgentError> {
    let trimmed = stdout.trim();

    // Try direct parse first
    if let Ok(resp) = serde_json::from_str::<EnrichmentResponse>(trimmed) {
        return Ok(resp);
    }

    // Codex may wrap in an envelope with a "output" or "result" field
    if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(trimmed) {
        // Try "output" field
        for key in &["output", "result"] {
            if let Some(inner) = envelope.get(*key) {
                if let Ok(resp) = serde_json::from_value::<EnrichmentResponse>(inner.clone()) {
                    return Ok(resp);
                }
                if let Some(text) = inner.as_str() {
                    if let Ok(resp) = serde_json::from_str::<EnrichmentResponse>(text) {
                        return Ok(resp);
                    }
                }
            }
        }
    }

    Err(AgentError::InvalidResponse {
        provider: "codex",
        reason: "output is not valid enrichment JSON".to_string(),
    })
}

fn truncate_stderr(stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.len() <= 500 {
        trimmed.to_string()
    } else {
        format!("{}... (truncated)", &trimmed[..500])
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::provider::ProcessOutput;
    use super::*;
    use std::io;

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
    fn codex_success_direct_json() {
        let json = r#"{"summary":"A Rust article.","suggested_tags":["rust"],"suggested_collection":null}"#;
        let runner = MockRunner::success(json);
        let provider = CodexProvider::new(None, runner);
        let result = provider.enrich(&sample_request()).unwrap();
        assert_eq!(result.summary, "A Rust article.");
        assert_eq!(result.suggested_tags, vec!["rust"]);
        assert_eq!(result.suggested_collection, None);
    }

    #[test]
    fn codex_success_with_collection() {
        let json =
            r#"{"summary":"Article.","suggested_tags":["a","b"],"suggested_collection":"tech"}"#;
        let runner = MockRunner::success(json);
        let provider = CodexProvider::new(None, runner);
        let result = provider.enrich(&sample_request()).unwrap();
        assert_eq!(result.suggested_collection, Some("tech".to_string()));
    }

    #[test]
    fn codex_normalizes_response() {
        let json = r#"{"summary":"  spaced  ","suggested_tags":["a","a","  "],"suggested_collection":"  "}"#;
        let runner = MockRunner::success(json);
        let provider = CodexProvider::new(None, runner);
        let result = provider.enrich(&sample_request()).unwrap();
        assert_eq!(result.summary, "spaced");
        assert_eq!(result.suggested_tags, vec!["a"]);
        assert_eq!(result.suggested_collection, None);
    }

    #[test]
    fn codex_spawn_error() {
        let runner = MockRunner::spawn_error();
        let provider = CodexProvider::new(None, runner);
        let err = provider.enrich(&sample_request()).unwrap_err();
        assert!(matches!(
            err,
            AgentError::Spawn {
                provider: "codex",
                ..
            }
        ));
    }

    #[test]
    fn codex_process_failed() {
        let runner = MockRunner::failure(1, "codex error");
        let provider = CodexProvider::new(None, runner);
        let err = provider.enrich(&sample_request()).unwrap_err();
        match err {
            AgentError::ProcessFailed {
                provider,
                status,
                stderr,
            } => {
                assert_eq!(provider, "codex");
                assert_eq!(status, 1);
                assert!(stderr.contains("codex error"));
            }
            _ => panic!("expected ProcessFailed"),
        }
    }

    #[test]
    fn codex_invalid_json() {
        let runner = MockRunner::success("not json");
        let provider = CodexProvider::new(None, runner);
        let err = provider.enrich(&sample_request()).unwrap_err();
        assert!(matches!(
            err,
            AgentError::InvalidResponse {
                provider: "codex",
                ..
            }
        ));
    }

    #[test]
    fn codex_blank_summary_rejected() {
        let json = r#"{"summary":"   ","suggested_tags":[],"suggested_collection":null}"#;
        let runner = MockRunner::success(json);
        let provider = CodexProvider::new(None, runner);
        let err = provider.enrich(&sample_request()).unwrap_err();
        assert!(matches!(
            err,
            AgentError::InvalidResponse {
                provider: "codex",
                ..
            }
        ));
    }

    #[test]
    fn codex_with_system_prompt() {
        let json = r#"{"summary":"ok","suggested_tags":[],"suggested_collection":null}"#;
        let runner = MockRunner::success(json);
        let provider = CodexProvider::new(Some("You are helpful.".to_string()), runner);
        let result = provider.enrich(&sample_request()).unwrap();
        assert_eq!(result.summary, "ok");
    }

    #[test]
    fn codex_command_args_captured() {
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

        let provider = CodexProvider::new(Some("Be helpful.".to_string()), runner);
        provider.enrich(&sample_request()).unwrap();

        let captured_program = program.lock().unwrap().clone().unwrap();
        assert_eq!(captured_program, "codex");

        let captured_args = args.lock().unwrap().clone().unwrap();
        assert!(captured_args.contains(&"exec".to_string()));
        assert!(captured_args.contains(&"--ephemeral".to_string()));
        assert!(captured_args.contains(&"--sandbox".to_string()));
        assert!(captured_args.contains(&"read-only".to_string()));
        assert!(captured_args.contains(&"--output-schema".to_string()));

        let captured_stdin = stdin.lock().unwrap().clone().unwrap();
        assert!(captured_stdin.contains("https://example.com"));
    }

    #[test]
    fn codex_envelope_with_output_field() {
        let inner = r#"{"summary":"test","suggested_tags":["a"],"suggested_collection":null}"#;
        let envelope = format!(r#"{{"output":{inner}}}"#);
        let runner = MockRunner::success(&envelope);
        let provider = CodexProvider::new(None, runner);
        let result = provider.enrich(&sample_request()).unwrap();
        assert_eq!(result.summary, "test");
    }

    #[test]
    fn temp_file_guard_cleans_up() {
        let mut path = std::env::temp_dir();
        path.push("agentmark-test-cleanup.json");
        std::fs::write(&path, "test").unwrap();
        assert!(path.exists());
        {
            let _guard = TempFileGuard(path.clone());
        }
        assert!(!path.exists());
    }
}
