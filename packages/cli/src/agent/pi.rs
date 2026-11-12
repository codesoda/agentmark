use tracing::{debug, instrument, warn};

use super::prompt::{build_article_prompt, build_prompt, PromptParts};
use super::provider::{
    extract_json_from_markdown, normalize_markdown, normalize_response, truncate_stderr,
    AgentError, AgentProvider, ArticleRequest, EnrichmentRequest, EnrichmentResponse,
    ProcessOutput, ProcessRunner,
};

/// System prompt used when the user has not configured one. Replaces pi's
/// default coding-assistant prompt, which is irrelevant to bookmark processing.
const DEFAULT_SYSTEM_PROMPT: &str =
    "You are AgentMark's bookmark processing assistant. Follow the task in the user \
     message exactly and output only what it asks for.";

/// Pi CLI provider. Invokes the local `pi` binary in non-interactive print
/// mode. The model is whatever pi is configured to use by default.
pub(crate) struct PiProvider {
    system_prompt: Option<String>,
    runner: Box<dyn ProcessRunner>,
}

impl PiProvider {
    pub fn new(system_prompt: Option<String>, runner: Box<dyn ProcessRunner>) -> Self {
        Self {
            system_prompt,
            runner,
        }
    }

    /// Invoke pi once with the given prompt and return the model's text output.
    fn run(&self, parts: &PromptParts) -> Result<String, AgentError> {
        let system_prompt = parts
            .system_prompt
            .as_deref()
            .unwrap_or(DEFAULT_SYSTEM_PROMPT);
        let args = [
            "--print",            // non-interactive: process the prompt and exit
            "--no-session",       // don't pollute the user's session history
            "--no-tools",         // the page content is untrusted; no tool access
            "--no-extensions",    // plain model call, no harness extensions
            "--no-skills",        //
            "--no-context-files", // ignore AGENTS.md / CLAUDE.md from the cwd
            "--no-prompt-templates",
            "--thinking",
            "low", // transcription and tagging do not need deep reasoning
            "--system-prompt",
            system_prompt,
        ];

        let output: ProcessOutput =
            self.runner
                .run("pi", &args, &parts.user_prompt)
                .map_err(|source| AgentError::Spawn {
                    provider: "pi",
                    source,
                })?;

        if output.exit_code != 0 {
            warn!(exit_code = output.exit_code, "pi CLI failed");
            return Err(AgentError::ProcessFailed {
                provider: "pi",
                status: output.exit_code,
                stderr: truncate_stderr(&output.stderr),
            });
        }

        debug!("pi CLI returned successfully");
        Ok(output.stdout)
    }
}

impl AgentProvider for PiProvider {
    #[instrument(skip(self, request), fields(url = %request.url))]
    fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse, AgentError> {
        let parts = build_prompt(request, self.system_prompt.as_deref());
        let stdout = self.run(&parts)?;
        normalize_response(parse_pi_output(&stdout)?, "pi")
    }

    #[instrument(skip(self, request), fields(url = %request.url))]
    fn render_article(&self, request: &ArticleRequest) -> Result<String, AgentError> {
        let parts = build_article_prompt(request, self.system_prompt.as_deref());
        let stdout = self.run(&parts)?;
        normalize_markdown(&stdout, "pi")
    }
}

/// Parse pi's text output as enrichment JSON, tolerating a markdown code fence
/// or prose around the JSON object.
fn parse_pi_output(stdout: &str) -> Result<EnrichmentResponse, AgentError> {
    let trimmed = stdout.trim();

    if let Ok(resp) = serde_json::from_str::<EnrichmentResponse>(trimmed) {
        return Ok(resp);
    }
    if let Some(fenced) = extract_json_from_markdown(trimmed) {
        if let Ok(resp) = serde_json::from_str::<EnrichmentResponse>(&fenced) {
            return Ok(resp);
        }
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            if let Ok(resp) = serde_json::from_str::<EnrichmentResponse>(&trimmed[start..=end]) {
                return Ok(resp);
            }
        }
    }

    Err(AgentError::InvalidResponse {
        provider: "pi",
        reason: "output is not valid enrichment JSON".to_string(),
    })
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::io;
    use std::rc::Rc;

    /// What the provider asked the runner to execute.
    #[derive(Default)]
    struct Call {
        program: String,
        args: Vec<String>,
        stdin: String,
    }

    /// Mock runner that records the call and returns a predefined output.
    struct MockRunner {
        result: Result<(String, String, i32), io::ErrorKind>,
        call: Rc<RefCell<Call>>,
    }

    impl ProcessRunner for MockRunner {
        fn run(
            &self,
            program: &str,
            args: &[&str],
            stdin_data: &str,
        ) -> Result<ProcessOutput, io::Error> {
            *self.call.borrow_mut() = Call {
                program: program.to_string(),
                args: args.iter().map(|a| a.to_string()).collect(),
                stdin: stdin_data.to_string(),
            };
            match &self.result {
                Ok((stdout, stderr, exit_code)) => Ok(ProcessOutput {
                    stdout: stdout.clone(),
                    stderr: stderr.clone(),
                    exit_code: *exit_code,
                }),
                Err(kind) => Err(io::Error::new(*kind, "program not found")),
            }
        }
    }

    fn provider(
        system_prompt: Option<&str>,
        result: Result<(&str, &str, i32), io::ErrorKind>,
    ) -> (PiProvider, Rc<RefCell<Call>>) {
        let call = Rc::new(RefCell::new(Call::default()));
        let runner = MockRunner {
            result: result.map(|(o, e, c)| (o.to_string(), e.to_string(), c)),
            call: Rc::clone(&call),
        };
        (
            PiProvider::new(system_prompt.map(str::to_string), Box::new(runner)),
            call,
        )
    }

    fn enrich_request() -> EnrichmentRequest {
        EnrichmentRequest {
            article_content: "Rust is great.".to_string(),
            user_note: None,
            existing_tags: vec![],
            url: "https://example.com".to_string(),
            title: "Test".to_string(),
        }
    }

    fn article_request() -> ArticleRequest {
        ArticleRequest {
            url: "https://www.linkedin.com/in/someone/".to_string(),
            title: "Someone | LinkedIn".to_string(),
            page_html: "<main><h2>Someone</h2></main>".to_string(),
            fetched_html: None,
        }
    }

    const ENRICH_JSON: &str =
        r#"{"summary":"A Rust article.","suggested_tags":["rust"],"suggested_collection":null}"#;

    #[test]
    fn enrich_parses_plain_json() {
        let (p, _) = provider(None, Ok((ENRICH_JSON, "", 0)));
        let result = p.enrich(&enrich_request()).unwrap();
        assert_eq!(result.summary, "A Rust article.");
        assert_eq!(result.suggested_tags, vec!["rust"]);
        assert_eq!(result.suggested_collection, None);
    }

    #[test]
    fn enrich_parses_fenced_json() {
        let out = format!("```json\n{ENRICH_JSON}\n```\n");
        let (p, _) = provider(None, Ok((&out, "", 0)));
        assert_eq!(
            p.enrich(&enrich_request()).unwrap().summary,
            "A Rust article."
        );
    }

    #[test]
    fn enrich_parses_json_surrounded_by_prose() {
        let out = format!("Here is the result:\n{ENRICH_JSON}\nDone.");
        let (p, _) = provider(None, Ok((&out, "", 0)));
        assert_eq!(
            p.enrich(&enrich_request()).unwrap().summary,
            "A Rust article."
        );
    }

    #[test]
    fn enrich_rejects_non_json() {
        let (p, _) = provider(None, Ok(("I cannot help with that.", "", 0)));
        assert!(matches!(
            p.enrich(&enrich_request()).unwrap_err(),
            AgentError::InvalidResponse { provider: "pi", .. }
        ));
    }

    #[test]
    fn invokes_pi_in_isolated_print_mode_with_prompt_on_stdin() {
        let (p, call) = provider(None, Ok((ENRICH_JSON, "", 0)));
        p.enrich(&enrich_request()).unwrap();
        let call = call.borrow();
        assert_eq!(call.program, "pi");
        for flag in [
            "--print",
            "--no-session",
            "--no-tools",
            "--no-extensions",
            "--no-context-files",
        ] {
            assert!(call.args.iter().any(|a| a == flag), "missing {flag}");
        }
        assert!(call.stdin.contains("Rust is great."));
    }

    #[test]
    fn uses_default_system_prompt_when_none_configured() {
        let (p, call) = provider(None, Ok((ENRICH_JSON, "", 0)));
        p.enrich(&enrich_request()).unwrap();
        let call = call.borrow();
        let i = call
            .args
            .iter()
            .position(|a| a == "--system-prompt")
            .unwrap();
        assert_eq!(call.args[i + 1], DEFAULT_SYSTEM_PROMPT);
    }

    #[test]
    fn uses_configured_system_prompt() {
        let (p, call) = provider(Some("Be terse."), Ok((ENRICH_JSON, "", 0)));
        p.enrich(&enrich_request()).unwrap();
        let call = call.borrow();
        let i = call
            .args
            .iter()
            .position(|a| a == "--system-prompt")
            .unwrap();
        assert_eq!(call.args[i + 1], "Be terse.");
    }

    #[test]
    fn render_article_returns_markdown_and_sends_page_html() {
        let (p, call) = provider(None, Ok(("# Someone\n\nFounder\n", "", 0)));
        let md = p.render_article(&article_request()).unwrap();
        assert_eq!(md, "# Someone\n\nFounder\n");
        assert!(call
            .borrow()
            .stdin
            .contains("<main><h2>Someone</h2></main>"));
    }

    #[test]
    fn render_article_rejects_blank_output() {
        let (p, _) = provider(None, Ok(("  \n", "", 0)));
        assert!(matches!(
            p.render_article(&article_request()).unwrap_err(),
            AgentError::InvalidResponse { provider: "pi", .. }
        ));
    }

    #[test]
    fn process_failure_reports_status_and_stderr() {
        let (p, _) = provider(None, Ok(("", "No API key", 1)));
        match p.enrich(&enrich_request()).unwrap_err() {
            AgentError::ProcessFailed {
                provider,
                status,
                stderr,
            } => {
                assert_eq!(provider, "pi");
                assert_eq!(status, 1);
                assert_eq!(stderr, "No API key");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn spawn_error_is_reported() {
        let (p, _) = provider(None, Err(io::ErrorKind::NotFound));
        assert!(matches!(
            p.render_article(&article_request()).unwrap_err(),
            AgentError::Spawn { provider: "pi", .. }
        ));
    }
}
