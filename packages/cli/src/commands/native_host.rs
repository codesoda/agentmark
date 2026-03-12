//! Native messaging host command: `agentmark native-host`.
//!
//! Runs a long-lived loop reading Chrome native messaging frames from stdin,
//! dispatching to save/status handlers, and writing responses to stdout.
//! All stdout output is protocol-only; diagnostics go to stderr.

use std::io::{self, Read, Write};
use std::path::Path;

use crate::commands::save::{self, DedupResult, SaveRequest};
use crate::config;
use crate::db::BookmarkRepository;
use crate::enrich::ProviderFactory;
use crate::models::{BookmarkState, CaptureSource};
use crate::native::messages::{BookmarkSummary, IncomingMessage, OutgoingMessage};
use crate::native::protocol::{self, ProtocolError};

// ── Public entry point ──────────────────────────────────────────────

/// Entry point for `agentmark native-host` using real stdin/stdout.
pub fn run_native_host() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    let home = config::home_dir()?;
    run_native_host_with_io(
        &mut reader,
        &mut writer,
        &home,
        &save::default_provider_factory,
    )
}

/// Testable host loop over injectable I/O, home dir, and provider factory.
pub(crate) fn run_native_host_with_io(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    home: &Path,
    provider_factory: &ProviderFactory,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // Read one framed message
        let value = match protocol::read_message(reader) {
            Ok(v) => v,
            Err(ProtocolError::Eof) => return Ok(()),
            Err(ProtocolError::MessageTooLarge { size }) => {
                // Drain the oversized payload to restore stream alignment
                if let Err(drain_err) = protocol::drain_payload(reader, size) {
                    return Err(
                        format!("fatal: failed to drain oversized message: {drain_err}").into(),
                    );
                }
                let response = OutgoingMessage::error(format!(
                    "message too large ({size} bytes, max {})",
                    protocol::MAX_MESSAGE_SIZE
                ));
                write_response(writer, &response)?;
                continue;
            }
            Err(ProtocolError::EmptyMessage) => {
                let response = OutgoingMessage::error("message length is zero");
                write_response(writer, &response)?;
                continue;
            }
            Err(ProtocolError::InvalidJson(e)) => {
                let response = OutgoingMessage::error(format!("invalid JSON: {e}"));
                write_response(writer, &response)?;
                continue;
            }
            Err(e) => {
                // UnexpectedEof or Io — fatal transport errors
                return Err(format!("fatal protocol error: {e}").into());
            }
        };

        // Parse into typed message
        let incoming = match IncomingMessage::from_value(value) {
            Ok(msg) => msg,
            Err(e) => {
                let response = OutgoingMessage::error(e.to_string());
                write_response(writer, &response)?;
                continue;
            }
        };

        // Dispatch
        let response = dispatch(incoming, home, provider_factory);
        write_response(writer, &response)?;
    }
}

// ── Dispatch ────────────────────────────────────────────────────────

fn dispatch(
    message: IncomingMessage,
    home: &Path,
    provider_factory: &ProviderFactory,
) -> OutgoingMessage {
    match message {
        IncomingMessage::Status => status_response(),
        IncomingMessage::Save {
            url,
            title,
            tags,
            collection,
            note,
            selected_text: _selected_text,
            action,
        } => {
            let req = build_save_request(url, title, tags, collection, note, action);
            handle_save(req, home, provider_factory)
        }
        IncomingMessage::ListCollections => handle_list_collections(home),
        IncomingMessage::List { limit, state } => handle_list(home, limit, state),
    }
}

fn status_response() -> OutgoingMessage {
    OutgoingMessage::StatusResult {
        ok: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// Map incoming native save fields into a transport-neutral SaveRequest.
/// `selected_text` is intentionally not persisted in this spec — it is
/// carried through the wire boundary but not stored as a bookmark field.
fn build_save_request(
    url: String,
    title: Option<String>,
    tags: Option<Vec<String>>,
    collection: Option<String>,
    note: Option<String>,
    action: Option<String>,
) -> SaveRequest {
    SaveRequest {
        url,
        tags: tags.unwrap_or_default(),
        collection,
        note,
        action,
        capture_source: CaptureSource::ChromeExtension,
        provided_title: title,
        no_enrich: false,
    }
}

fn handle_save(
    req: SaveRequest,
    home: &Path,
    provider_factory: &ProviderFactory,
) -> OutgoingMessage {
    match save::execute_save_request(home, &req, provider_factory) {
        Ok(outcome) => {
            let status = match outcome.dedup {
                DedupResult::New => "created",
                DedupResult::Unchanged => "updated",
                DedupResult::ContentChanged => "content_updated",
            };
            OutgoingMessage::SaveResult {
                id: outcome.id,
                path: outcome.bundle_path.display().to_string(),
                status: status.to_string(),
            }
        }
        Err(e) => OutgoingMessage::error(e.to_string()),
    }
}

/// Verify config exists and open the index DB. Returns an error message on failure.
fn open_repository(home: &Path) -> Result<rusqlite::Connection, String> {
    // Verify init by loading config — this surfaces "run `agentmark init` first"
    config::Config::load(home).map_err(|e| format!("not initialized: {e}"))?;

    let db_path = config::index_db_path(home);
    rusqlite::Connection::open(&db_path).map_err(|e| format!("failed to open database: {e}"))
}

fn handle_list_collections(home: &Path) -> OutgoingMessage {
    let conn = match open_repository(home) {
        Ok(c) => c,
        Err(e) => return OutgoingMessage::error(e),
    };
    let repo = BookmarkRepository::new(&conn);
    match repo.list_collections() {
        Ok(pairs) => {
            let names: Vec<String> = pairs.into_iter().map(|(name, _count)| name).collect();
            OutgoingMessage::ListCollectionsResult { collections: names }
        }
        Err(e) => OutgoingMessage::error(format!("failed to list collections: {e}")),
    }
}

/// Maximum number of bookmarks returned in a single list response.
const MAX_LIST_LIMIT: u32 = 100;
/// Default number of bookmarks when no limit is specified.
const DEFAULT_LIST_LIMIT: u32 = 50;

fn handle_list(home: &Path, limit: Option<u32>, state: Option<BookmarkState>) -> OutgoingMessage {
    let conn = match open_repository(home) {
        Ok(c) => c,
        Err(e) => return OutgoingMessage::error(e),
    };
    let repo = BookmarkRepository::new(&conn);

    let clamped_limit = limit
        .map(|l| l.min(MAX_LIST_LIMIT))
        .unwrap_or(DEFAULT_LIST_LIMIT) as usize;

    match repo.list(clamped_limit, 0, None, None, state.as_ref()) {
        Ok(bookmarks) => {
            let summaries: Vec<BookmarkSummary> = bookmarks
                .into_iter()
                .map(|b| BookmarkSummary {
                    id: b.id,
                    url: b.url,
                    title: b.title,
                    state: b.state,
                    user_tags: b.user_tags,
                    suggested_tags: b.suggested_tags,
                    saved_at: b.saved_at.to_rfc3339(),
                })
                .collect();
            OutgoingMessage::ListResult {
                bookmarks: summaries,
            }
        }
        Err(e) => OutgoingMessage::error(format!("failed to list bookmarks: {e}")),
    }
}

// ── Response writer ─────────────────────────────────────────────────

fn write_response(
    writer: &mut dyn Write,
    response: &OutgoingMessage,
) -> Result<(), Box<dyn std::error::Error>> {
    let value = response
        .to_value()
        .map_err(|e| format!("failed to serialize response: {e}"))?;
    protocol::write_message(writer, &value)
        .map_err(|e| format!("fatal: failed to write response: {e}"))?;
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::protocol;
    use serde_json::json;
    use std::io::Cursor;

    // ── Test helpers ────────────────────────────────────────────────

    /// Frame one or more JSON values into length-prefixed bytes for stdin.
    fn frame_messages(messages: &[serde_json::Value]) -> Vec<u8> {
        let mut buf = Vec::new();
        for msg in messages {
            protocol::write_message(&mut buf, msg).unwrap();
        }
        buf
    }

    /// Decode all length-prefixed responses from stdout bytes.
    fn decode_responses(stdout: &[u8]) -> Vec<serde_json::Value> {
        let mut cursor = Cursor::new(stdout);
        let mut responses = Vec::new();
        loop {
            match protocol::read_message(&mut cursor) {
                Ok(v) => responses.push(v),
                Err(ProtocolError::Eof) => break,
                Err(e) => panic!("unexpected error decoding response: {e:?}"),
            }
        }
        responses
    }

    /// Run the host loop with in-memory I/O against a no-op home dir
    /// (status-only — save requires real config/DB).
    fn run_host_status_only(
        stdin_bytes: &[u8],
    ) -> (Vec<u8>, Result<(), Box<dyn std::error::Error>>) {
        let mut reader = Cursor::new(stdin_bytes.to_vec());
        let mut writer = Vec::new();
        let home = std::path::PathBuf::from("/nonexistent");
        let factory: &ProviderFactory = &|_, _| {
            Err(crate::agent::AgentError::InvalidAgent {
                value: "test".to_string(),
            })
        };
        let result = run_native_host_with_io(&mut reader, &mut writer, &home, factory);
        (writer, result)
    }

    // ── Status tests ───────────────────────────────────────────────

    #[test]
    fn status_request_returns_status_result() {
        let stdin = frame_messages(&[json!({"type": "status"})]);
        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0]["type"], "status_result");
        assert_eq!(responses[0]["ok"], true);
        assert_eq!(responses[0]["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn empty_stdin_exits_cleanly() {
        let (stdout, result) = run_host_status_only(&[]);
        assert!(result.is_ok());
        assert!(stdout.is_empty());
    }

    // ── Multi-message tests ────────────────────────────────────────

    #[test]
    fn multiple_status_requests() {
        let stdin = frame_messages(&[json!({"type": "status"}), json!({"type": "status"})]);
        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["type"], "status_result");
        assert_eq!(responses[1]["type"], "status_result");
    }

    // ── Error recovery tests ───────────────────────────────────────

    #[test]
    fn malformed_json_returns_error_and_loop_continues() {
        // Frame malformed JSON followed by a valid status request
        let bad_payload = b"not json{{{";
        let mut stdin = Vec::new();
        stdin.extend_from_slice(&(bad_payload.len() as u32).to_le_bytes());
        stdin.extend_from_slice(bad_payload);
        // Append valid status
        let status_frame = frame_messages(&[json!({"type": "status"})]);
        stdin.extend_from_slice(&status_frame);

        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["type"], "error");
        assert!(responses[0]["message"].as_str().unwrap().contains("JSON"));
        assert_eq!(responses[1]["type"], "status_result");
    }

    #[test]
    fn unknown_type_returns_error_and_loop_continues() {
        let stdin = frame_messages(&[json!({"type": "delete"}), json!({"type": "status"})]);
        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["type"], "error");
        assert!(responses[0]["message"].as_str().unwrap().contains("delete"));
        assert_eq!(responses[1]["type"], "status_result");
    }

    #[test]
    fn missing_type_returns_error_and_loop_continues() {
        let stdin = frame_messages(&[
            json!({"url": "https://example.com"}),
            json!({"type": "status"}),
        ]);
        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["type"], "error");
        assert_eq!(responses[1]["type"], "status_result");
    }

    #[test]
    fn empty_message_returns_error_and_loop_continues() {
        // Zero-length frame followed by valid status
        let mut stdin = vec![0x00, 0x00, 0x00, 0x00]; // length = 0
        stdin.extend_from_slice(&frame_messages(&[json!({"type": "status"})]));

        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["type"], "error");
        assert!(responses[0]["message"].as_str().unwrap().contains("zero"));
        assert_eq!(responses[1]["type"], "status_result");
    }

    #[test]
    fn oversized_message_drained_and_loop_continues() {
        // Create a frame with length > MAX_MESSAGE_SIZE but provide the payload bytes
        let oversized_len: u32 = protocol::MAX_MESSAGE_SIZE + 1;
        let mut stdin = Vec::new();
        stdin.extend_from_slice(&oversized_len.to_le_bytes());
        // Write exactly oversized_len bytes of junk payload
        stdin.extend(vec![0x20; oversized_len as usize]);
        // Append valid status
        stdin.extend_from_slice(&frame_messages(&[json!({"type": "status"})]));

        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["type"], "error");
        assert!(responses[0]["message"]
            .as_str()
            .unwrap()
            .contains("too large"));
        assert_eq!(responses[1]["type"], "status_result");
    }

    // ── Fatal error tests ──────────────────────────────────────────

    #[test]
    fn partial_prefix_is_fatal() {
        // Only 2 bytes of prefix, then EOF
        let (stdout, result) = run_host_status_only(&[0x0A, 0x00]);
        assert!(result.is_err());
        assert!(stdout.is_empty());
    }

    #[test]
    fn partial_payload_is_fatal() {
        // Full prefix saying 10 bytes, but only 3 bytes follow
        let mut stdin = vec![0x0A, 0x00, 0x00, 0x00]; // length = 10
        stdin.extend_from_slice(b"abc");
        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_err());
        assert!(stdout.is_empty());
    }

    // ── Save error path tests (no real config) ─────────────────────

    #[test]
    fn save_with_missing_config_returns_error_and_loop_continues() {
        let stdin = frame_messages(&[
            json!({"type": "save", "url": "https://example.com"}),
            json!({"type": "status"}),
        ]);
        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["type"], "error");
        assert_eq!(responses[1]["type"], "status_result");
    }

    #[test]
    fn save_with_invalid_fields_returns_error_and_loop_continues() {
        let stdin = frame_messages(&[
            json!({"type": "save", "url": "https://example.com", "tags": "not-an-array"}),
            json!({"type": "status"}),
        ]);
        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["type"], "error");
        assert_eq!(responses[1]["type"], "status_result");
    }

    // ── Dispatch unit tests ────────────────────────────────────────

    #[test]
    fn dispatch_status_returns_status_result() {
        let home = std::path::PathBuf::from("/nonexistent");
        let factory: &ProviderFactory = &|_, _| {
            Err(crate::agent::AgentError::InvalidAgent {
                value: "test".to_string(),
            })
        };
        let response = dispatch(IncomingMessage::Status, &home, &factory);
        match response {
            OutgoingMessage::StatusResult { ok, version } => {
                assert!(ok);
                assert_eq!(version, env!("CARGO_PKG_VERSION"));
            }
            other => panic!("expected StatusResult, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_save_with_no_config_returns_error() {
        let home = std::path::PathBuf::from("/nonexistent");
        let factory: &ProviderFactory = &|_, _| {
            Err(crate::agent::AgentError::InvalidAgent {
                value: "test".to_string(),
            })
        };
        let response = dispatch(
            IncomingMessage::Save {
                url: "https://example.com".to_string(),
                title: None,
                tags: None,
                collection: None,
                note: None,
                selected_text: None,
                action: None,
            },
            &home,
            &factory,
        );
        match response {
            OutgoingMessage::Error { message } => {
                assert!(!message.is_empty());
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_list_collections_with_no_db_returns_error() {
        let home = std::path::PathBuf::from("/nonexistent");
        let factory: &ProviderFactory = &|_, _| {
            Err(crate::agent::AgentError::InvalidAgent {
                value: "test".to_string(),
            })
        };
        let response = dispatch(IncomingMessage::ListCollections, &home, &factory);
        match response {
            OutgoingMessage::Error { message } => {
                assert!(!message.is_empty());
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_list_with_no_config_returns_error() {
        let home = std::path::PathBuf::from("/nonexistent");
        let factory: &ProviderFactory = &|_, _| {
            Err(crate::agent::AgentError::InvalidAgent {
                value: "test".to_string(),
            })
        };
        let response = dispatch(
            IncomingMessage::List {
                limit: None,
                state: None,
            },
            &home,
            &factory,
        );
        match response {
            OutgoingMessage::Error { message } => {
                assert!(
                    message.contains("not initialized"),
                    "expected init error, got: {message}"
                );
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn list_request_returns_error_and_loop_continues() {
        let stdin = frame_messages(&[json!({"type": "list"}), json!({"type": "status"})]);
        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 2);
        // list fails because /nonexistent has no config
        assert_eq!(responses[0]["type"], "error");
        assert_eq!(responses[1]["type"], "status_result");
    }

    #[test]
    fn list_collections_request_returns_error_and_loop_continues() {
        let stdin = frame_messages(&[
            json!({"type": "list_collections"}),
            json!({"type": "status"}),
        ]);
        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 2);
        // list_collections fails because /nonexistent has no DB
        assert_eq!(responses[0]["type"], "error");
        assert_eq!(responses[1]["type"], "status_result");
    }
}
