mod claude;
mod codex;
pub mod prompt;
pub mod provider;

pub use provider::{AgentError, AgentProvider, EnrichmentRequest, EnrichmentResponse};

use claude::ClaudeProvider;
use codex::CodexProvider;
use provider::RealRunner;
use tracing::{debug, instrument};

/// Creates an agent provider based on the configured `default_agent`.
///
/// Normalizes blank `system_prompt` to `None`. Returns a typed error
/// if the agent name is not recognized.
#[instrument(skip(system_prompt), fields(%default_agent))]
pub fn create_provider(
    default_agent: &str,
    system_prompt: Option<&str>,
) -> Result<Box<dyn AgentProvider>, AgentError> {
    let agent = default_agent.trim().to_lowercase();
    let sp = system_prompt
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    debug!(normalized = %agent, "creating agent provider");
    match agent.as_str() {
        "claude" => Ok(Box::new(ClaudeProvider::new(sp, Box::new(RealRunner)))),
        "codex" => Ok(Box::new(CodexProvider::new(sp, Box::new(RealRunner)))),
        _ => Err(AgentError::InvalidAgent {
            value: default_agent.to_string(),
        }),
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_creates_claude_provider() {
        let provider = create_provider("claude", None);
        assert!(provider.is_ok());
    }

    #[test]
    fn factory_creates_codex_provider() {
        let provider = create_provider("codex", None);
        assert!(provider.is_ok());
    }

    #[test]
    fn factory_case_insensitive() {
        assert!(create_provider("Claude", None).is_ok());
        assert!(create_provider("CODEX", None).is_ok());
    }

    #[test]
    fn factory_trims_whitespace() {
        assert!(create_provider("  claude  ", None).is_ok());
    }

    #[test]
    fn factory_rejects_invalid_agent() {
        match create_provider("chatgpt", None) {
            Err(AgentError::InvalidAgent { value }) => {
                assert_eq!(value, "chatgpt");
            }
            Ok(_) => panic!("expected InvalidAgent error"),
            Err(e) => panic!("expected InvalidAgent, got: {e}"),
        }
    }

    #[test]
    fn factory_rejects_empty_agent() {
        match create_provider("", None) {
            Err(AgentError::InvalidAgent { .. }) => {}
            Ok(_) => panic!("expected InvalidAgent error"),
            Err(e) => panic!("expected InvalidAgent, got: {e}"),
        }
    }

    #[test]
    fn factory_normalizes_blank_system_prompt() {
        // Should succeed even with blank system_prompt
        let provider = create_provider("claude", Some("   "));
        assert!(provider.is_ok());
    }

    #[test]
    fn factory_preserves_system_prompt() {
        let provider = create_provider("claude", Some("Be helpful."));
        assert!(provider.is_ok());
    }

    #[test]
    fn factory_handles_none_system_prompt() {
        let provider = create_provider("codex", None);
        assert!(provider.is_ok());
    }
}
