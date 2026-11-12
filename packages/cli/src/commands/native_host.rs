//! Native messaging host command: `agentmark native-host`.
//!
//! Runs a long-lived loop reading Chrome native messaging frames from stdin,
//! dispatching to save/status/setup handlers, and writing responses to stdout.
//! All stdout output is protocol-only; diagnostics go to stderr.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use tracing::{debug, error, info, warn};

use crate::canonical;
use crate::commands::bookmark_detail;
use crate::commands::post_process;
use crate::commands::save::{self, DedupResult, SaveRequest};
use crate::config;
use crate::db::{self, BookmarkRepository};
use crate::enrich::ProviderFactory;
use crate::models::{BookmarkState, CaptureSource};
use crate::native::messages::{
    BookmarkChanges, BookmarkSummary, FolderOutcome, IncomingMessage, OutgoingMessage,
};
use crate::native::protocol::{self, ProtocolError};
use crate::setup::{
    self, ConnectionRecord, FolderChoice, FolderDialog, SettingsChanges, SettingsError, SetupEnv,
    SystemFolderDialog,
};

/// Longest `extension_version` recorded; longer values are truncated.
const MAX_EXTENSION_VERSION_CHARS: usize = 64;

/// Everything a request handler needs, injectable for tests.
pub(crate) struct HostContext<'a> {
    pub env: SetupEnv,
    pub provider_factory: &'a ProviderFactory,
    pub folder_dialog: &'a dyn FolderDialog,
}

// ── Public entry point ──────────────────────────────────────────────

/// Entry point for the native host using real stdin/stdout. `origin` is the
/// `chrome-extension://<id>/` argument Chrome passes, when started by Chrome.
pub fn run_native_host(origin: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "native-host starting (version {}, origin {origin:?})",
        env!("CARGO_PKG_VERSION")
    );
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    let ctx = HostContext {
        env: SetupEnv::from_process()?,
        provider_factory: &save::default_provider_factory,
        folder_dialog: &SystemFolderDialog,
    };
    let extension_id = origin.as_deref().and_then(setup::extension_id_from_origin);
    let result = run_native_host_with_io(&mut reader, &mut writer, &ctx, extension_id);
    match &result {
        Ok(()) => info!("native-host exiting normally"),
        Err(e) => error!("native-host exiting with error: {e}"),
    }
    result
}

/// Testable host loop over injectable I/O and context.
pub(crate) fn run_native_host_with_io(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    ctx: &HostContext,
    extension_id: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let home = ctx.env.home.as_path();
    // Liveness for the desktop app, best effort. `written` is the extension
    // version in the last record this process wrote (`None` = none yet).
    // Before setup `~/.agentmark/` does not exist, so the write is retried
    // after each request until setup creates it.
    let mut known_version: Option<String> = None;
    let mut written: Option<Option<String>> = None;
    if setup::record_connection(home, &ConnectionRecord::now(extension_id.clone(), None)) {
        written = Some(None);
    }

    loop {
        // Read one framed message
        let value = match protocol::read_message(reader) {
            Ok(v) => v,
            Err(ProtocolError::Eof) => {
                debug!("stdin EOF — exiting message loop");
                return Ok(());
            }
            Err(ProtocolError::MessageTooLarge { size }) => {
                warn!("received oversized message ({size} bytes), draining");
                // Drain the oversized payload to restore stream alignment
                if let Err(drain_err) = protocol::drain_payload(reader, size) {
                    error!("failed to drain oversized message: {drain_err}");
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
                warn!("received empty message (length zero)");
                let response = OutgoingMessage::error("message length is zero");
                write_response(writer, &response)?;
                continue;
            }
            Err(ProtocolError::InvalidJson(e)) => {
                warn!("received invalid JSON: {e}");
                let response = OutgoingMessage::error(format!("invalid JSON: {e}"));
                write_response(writer, &response)?;
                continue;
            }
            Err(e) => {
                error!("fatal protocol error: {e}");
                // UnexpectedEof or Io — fatal transport errors
                return Err(format!("fatal protocol error: {e}").into());
            }
        };

        // Parse into typed message
        let incoming = match IncomingMessage::from_value(value) {
            Ok(msg) => {
                debug!("received message: {msg:?}");
                msg
            }
            Err(e) => {
                warn!("failed to parse message: {e}");
                let response = OutgoingMessage::error(e.to_string());
                write_response(writer, &response)?;
                continue;
            }
        };

        let reported_version = match &incoming {
            IncomingMessage::Status {
                extension_version: Some(v),
            } => Some(truncate_chars(v, MAX_EXTENSION_VERSION_CHARS)),
            _ => None,
        };

        // Dispatch
        let response = dispatch(incoming, ctx);
        debug!("sending response: {response:?}");
        write_response(writer, &response)?;

        // Recorded after the reply so the file write never delays it.
        if reported_version.is_some() {
            known_version = reported_version;
        }
        if written.as_ref() != Some(&known_version) {
            let record = ConnectionRecord::now(extension_id.clone(), known_version.clone());
            if setup::record_connection(home, &record) {
                written = Some(known_version.clone());
            }
        }
    }
}

fn truncate_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

// ── Dispatch ────────────────────────────────────────────────────────

fn dispatch(message: IncomingMessage, ctx: &HostContext) -> OutgoingMessage {
    let home = ctx.env.home.as_path();
    // "Not set up" is a state the extension handles, so requests that need a
    // config get a typed error rather than whatever failed first.
    if requires_setup(&message) && !setup::is_set_up(home) {
        return OutgoingMessage::setup_required();
    }
    match message {
        IncomingMessage::Status {
            extension_version: _,
        } => status_response(home),
        IncomingMessage::Save {
            url,
            title,
            tags,
            collection,
            note,
            selected_text: _selected_text,
            action,
            page_html,
        } => {
            let req = build_save_request(url, title, tags, collection, note, action, page_html);
            handle_save(req, home, ctx.provider_factory)
        }
        IncomingMessage::ListCollections => handle_list_collections(home),
        IncomingMessage::List { limit, state } => handle_list(home, limit, state),
        IncomingMessage::Show { id } => handle_show(home, &id),
        IncomingMessage::Update { id, changes } => handle_update(home, &id, &changes),
        IncomingMessage::LookupUrls { urls } => handle_lookup_urls(home, &urls),
        IncomingMessage::GetSetupState => OutgoingMessage::SetupStateResult {
            state: setup::read_state(&ctx.env),
        },
        IncomingMessage::SaveSettings { settings } => handle_save_settings(&ctx.env, &settings),
        IncomingMessage::ChooseFolder { start_path } => handle_choose_folder(ctx, start_path),
    }
}

/// Requests that read or write bookmarks need a config and index.
fn requires_setup(message: &IncomingMessage) -> bool {
    match message {
        IncomingMessage::Save { .. }
        | IncomingMessage::ListCollections
        | IncomingMessage::List { .. }
        | IncomingMessage::Show { .. }
        | IncomingMessage::Update { .. }
        | IncomingMessage::LookupUrls { .. } => true,
        IncomingMessage::Status { .. }
        | IncomingMessage::GetSetupState
        | IncomingMessage::SaveSettings { .. }
        | IncomingMessage::ChooseFolder { .. } => false,
    }
}

/// Must stay cheap (the extension times out after 5 s): config read only.
fn status_response(home: &Path) -> OutgoingMessage {
    OutgoingMessage::StatusResult {
        ok: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
        initialized: setup::is_set_up(home),
    }
}

fn handle_save_settings(env: &SetupEnv, changes: &SettingsChanges) -> OutgoingMessage {
    match setup::apply_settings(env, changes) {
        Ok(state) => {
            info!("settings saved");
            OutgoingMessage::SaveSettingsResult { state }
        }
        Err(SettingsError::Invalid(errors)) => {
            info!(count = errors.len(), "settings rejected");
            OutgoingMessage::SettingsInvalid { errors }
        }
        Err(e) => {
            error!("saving settings failed: {e}");
            OutgoingMessage::error(e.to_string())
        }
    }
}

fn handle_choose_folder(ctx: &HostContext, start_path: Option<String>) -> OutgoingMessage {
    let home = ctx.env.home.as_path();
    // A start folder that does not exist is ignored rather than an error.
    let start = start_path
        .map(|raw| expand_tilde(raw.trim(), home))
        .filter(|p| p.is_dir());
    let (outcome, path, reason) = match setup::choose_folder(ctx.folder_dialog, start.as_deref()) {
        FolderChoice::Chosen(path) => (FolderOutcome::Chosen, Some(path), None),
        FolderChoice::Cancelled => (FolderOutcome::Cancelled, None, None),
        FolderChoice::Unavailable { reason } => (FolderOutcome::Unavailable, None, Some(reason)),
    };
    info!(?outcome, "folder dialog finished");
    OutgoingMessage::ChooseFolderResult {
        outcome,
        path_display: path.as_deref().map(|p| setup::display_path(p, home)),
        path: path.map(|p| p.display().to_string()),
        reason,
    }
}

fn expand_tilde(raw: &str, home: &Path) -> PathBuf {
    if raw == "~" {
        home.to_path_buf()
    } else if let Some(rest) = raw.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(raw)
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
    page_html: Option<String>,
) -> SaveRequest {
    SaveRequest {
        url,
        tags: tags.unwrap_or_default(),
        collection,
        note,
        action,
        capture_source: CaptureSource::ChromeExtension,
        provided_title: title,
        page_html,
        no_enrich: false,
        // Never make the browser wait on a model; the work runs in the background.
        defer_enrichment: true,
    }
}

fn handle_save(
    req: SaveRequest,
    home: &Path,
    provider_factory: &ProviderFactory,
) -> OutgoingMessage {
    info!("saving bookmark: url={}", req.url);
    match save::execute_save_request(home, &req, provider_factory) {
        Ok(outcome) => {
            let status = match outcome.dedup {
                DedupResult::New => "created",
                DedupResult::Unchanged => "updated",
                DedupResult::ContentChanged => "content_updated",
            };
            info!("save succeeded: id={} status={status}", outcome.id);
            if let Some(work) = outcome.post_process {
                match post_process::spawn_detached(&outcome.id, work) {
                    Ok(()) => info!("post-processing started: id={} {work:?}", outcome.id),
                    Err(e) => warn!("failed to start post-processing for {}: {e}", outcome.id),
                }
            }
            OutgoingMessage::SaveResult {
                id: outcome.id,
                path: outcome.bundle_path.display().to_string(),
                status: status.to_string(),
            }
        }
        Err(e) => {
            error!("save failed for url={}: {e}", req.url);
            OutgoingMessage::error(e.to_string())
        }
    }
}

/// Verify config exists and open the index DB. Returns an error message on failure.
fn open_repository(home: &Path) -> Result<rusqlite::Connection, String> {
    // Verify init by loading config — this surfaces "run `agentmark init` first"
    // and stops us creating a database under an uninitialized home.
    config::Config::load(home).map_err(|e| format!("not initialized: {e}"))?;

    // Migrate here too: the extension can talk to a newer binary before any
    // CLI command has upgraded the index, and the busy timeout matters because
    // background post-processing writes concurrently.
    let db_path = config::index_db_path(home);
    db::open_and_migrate(&db_path).map_err(|e| format!("failed to open database: {e}"))
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

fn handle_show(home: &Path, id: &str) -> OutgoingMessage {
    match bookmark_detail::load_bookmark_detail(home, id) {
        Ok(loaded) => OutgoingMessage::ShowResult {
            bookmark: loaded.to_detail_dto(),
        },
        Err(e) => OutgoingMessage::error(e.to_string()),
    }
}

fn handle_update(home: &Path, id: &str, changes: &BookmarkChanges) -> OutgoingMessage {
    match bookmark_detail::apply_bookmark_update(home, id, changes) {
        Ok(loaded) => OutgoingMessage::UpdateResult {
            bookmark: loaded.to_detail_dto(),
        },
        Err(e) => OutgoingMessage::error(e.to_string()),
    }
}

/// Maximum number of URLs considered in one lookup; extras are ignored.
const MAX_LOOKUP_URLS: usize = 200;

/// Report which request URLs are already bookmarked, echoing them verbatim so
/// the extension can match its own strings without knowing canonicalization.
fn handle_lookup_urls(home: &Path, urls: &[String]) -> OutgoingMessage {
    let conn = match open_repository(home) {
        Ok(c) => c,
        Err(e) => return OutgoingMessage::error(e),
    };
    let repo = BookmarkRepository::new(&conn);

    // Unparseable URLs cannot be bookmarked, so they just drop out.
    let considered = &urls[..urls.len().min(MAX_LOOKUP_URLS)];
    let pairs: Vec<(&String, String)> = considered
        .iter()
        .filter_map(|raw| canonical::canonicalize(raw).ok().map(|c| (raw, c)))
        .collect();
    let canonicals: Vec<String> = pairs.iter().map(|(_, c)| c.clone()).collect();

    let found = match repo.saved_urls(&canonicals) {
        Ok(found) => found,
        Err(e) => return OutgoingMessage::error(format!("failed to look up URLs: {e}")),
    };

    let mut seen = std::collections::HashSet::new();
    let saved: Vec<String> = pairs
        .into_iter()
        .filter(|(raw, c)| found.contains(c) && seen.insert(raw.as_str()))
        .map(|(raw, _)| raw.clone())
        .collect();
    debug!(requested = urls.len(), saved = saved.len(), "lookup_urls");
    OutgoingMessage::LookupUrlsResult { saved }
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

    /// A folder dialog that always answers the same way.
    struct FakeDialog(FolderChoice);

    impl FolderDialog for FakeDialog {
        fn pick(&self, _start: Option<&Path>) -> FolderChoice {
            self.0.clone()
        }
    }

    /// A context over `home` with no agents on the search path.
    fn test_ctx<'a>(
        home: &Path,
        factory: &'a ProviderFactory,
        dialog: &'a dyn FolderDialog,
    ) -> HostContext<'a> {
        HostContext {
            env: SetupEnv {
                home: home.to_path_buf(),
                search_path: std::ffi::OsString::new(),
            },
            provider_factory: factory,
            folder_dialog: dialog,
        }
    }

    /// Dispatch with a fake dialog that cancels.
    fn dispatch_with(
        message: IncomingMessage,
        home: &Path,
        factory: &ProviderFactory,
    ) -> OutgoingMessage {
        let dialog = FakeDialog(FolderChoice::Cancelled);
        dispatch(message, &test_ctx(home, factory, &dialog))
    }

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
        let dialog = FakeDialog(FolderChoice::Cancelled);
        let ctx = test_ctx(&home, no_factory(), &dialog);
        let result = run_native_host_with_io(&mut reader, &mut writer, &ctx, None);
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
        let response = dispatch_with(
            IncomingMessage::Status {
                extension_version: None,
            },
            &home,
            &factory,
        );
        match response {
            OutgoingMessage::StatusResult { ok, version, .. } => {
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
        let response = dispatch_with(
            IncomingMessage::Save {
                url: "https://example.com".to_string(),
                title: None,
                tags: None,
                collection: None,
                note: None,
                selected_text: None,
                page_html: None,
                action: None,
            },
            &home,
            &factory,
        );
        match response {
            OutgoingMessage::Error { message, .. } => {
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
        let response = dispatch_with(IncomingMessage::ListCollections, &home, &factory);
        match response {
            OutgoingMessage::Error { message, .. } => {
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
        let response = dispatch_with(
            IncomingMessage::List {
                limit: None,
                state: None,
            },
            &home,
            &factory,
        );
        match response {
            OutgoingMessage::Error { message, .. } => {
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

    // ── Show/Update dispatch tests ───────────────────────────────────

    #[test]
    fn dispatch_show_with_no_config_returns_error() {
        let home = std::path::PathBuf::from("/nonexistent");
        let factory: &ProviderFactory = &|_, _| {
            Err(crate::agent::AgentError::InvalidAgent {
                value: "test".to_string(),
            })
        };
        let response = dispatch_with(
            IncomingMessage::Show {
                id: "am_123".to_string(),
            },
            &home,
            &factory,
        );
        match response {
            OutgoingMessage::Error { message, .. } => {
                assert!(!message.is_empty());
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_update_with_no_config_returns_error() {
        let home = std::path::PathBuf::from("/nonexistent");
        let factory: &ProviderFactory = &|_, _| {
            Err(crate::agent::AgentError::InvalidAgent {
                value: "test".to_string(),
            })
        };
        let response = dispatch_with(
            IncomingMessage::Update {
                id: "am_123".to_string(),
                changes: crate::native::messages::BookmarkChanges {
                    user_tags: None,
                    suggested_tags: None,
                    collections: None,
                    note: None,
                    state: None,
                },
            },
            &home,
            &factory,
        );
        match response {
            OutgoingMessage::Error { message, .. } => {
                assert!(!message.is_empty());
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn show_request_returns_error_and_loop_continues() {
        let stdin = frame_messages(&[
            json!({"type": "show", "id": "am_123"}),
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
    fn update_request_returns_error_and_loop_continues() {
        let stdin = frame_messages(&[
            json!({"type": "update", "id": "am_123", "changes": {"state": "processed"}}),
            json!({"type": "status"}),
        ]);
        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["type"], "error");
        assert_eq!(responses[1]["type"], "status_result");
    }

    // ── LookupUrls tests ───────────────────────────────────────────

    /// A temp home with config + migrated DB holding one bookmark at `canonical_url`.
    fn home_with_bookmark(canonical_url: &str) -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join(".agentmark");
        std::fs::create_dir_all(&config_dir).unwrap();
        let storage = tmp.path().join("bookmarks");
        std::fs::create_dir_all(&storage).unwrap();
        std::fs::write(
            config_dir.join("config.toml"),
            format!(
                "default_agent = \"claude\"\nstorage_path = \"{}\"\n\n[enrichment]\nenabled = false\n",
                storage.display()
            ),
        )
        .unwrap();
        let conn = db::open_and_migrate(&config::index_db_path(tmp.path())).unwrap();
        let mut bm = crate::models::Bookmark::new(canonical_url, "Saved");
        bm.canonical_url = canonical_url.to_string();
        BookmarkRepository::new(&conn).insert(&bm).unwrap();
        tmp
    }

    fn no_factory() -> &'static ProviderFactory {
        &|_, _| {
            Err(crate::agent::AgentError::InvalidAgent {
                value: "test".to_string(),
            })
        }
    }

    #[test]
    fn dispatch_lookup_urls_with_no_config_returns_error() {
        let home = std::path::PathBuf::from("/nonexistent");
        let response = dispatch_with(
            IncomingMessage::LookupUrls {
                urls: vec!["https://example.com".to_string()],
            },
            &home,
            no_factory(),
        );
        match response {
            OutgoingMessage::Error { message, .. } => {
                assert!(message.contains("not initialized"), "{message}");
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_lookup_urls_returns_verbatim_saved_in_order_without_duplicates() {
        let tmp = home_with_bookmark("https://example.com/a");
        let urls: Vec<String> = [
            "https://www.example.com/a/?utm_source=x",
            "https://example.com/unsaved",
            "not a url",
            "https://example.com/a",
            "https://www.example.com/a/?utm_source=x",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let response = dispatch_with(
            IncomingMessage::LookupUrls { urls },
            tmp.path(),
            no_factory(),
        );
        assert_eq!(
            response,
            OutgoingMessage::LookupUrlsResult {
                saved: vec![
                    "https://www.example.com/a/?utm_source=x".to_string(),
                    "https://example.com/a".to_string(),
                ],
            }
        );
    }

    #[test]
    fn dispatch_lookup_urls_ignores_urls_past_the_limit() {
        let tmp = home_with_bookmark("https://example.com/a");
        let mut urls: Vec<String> = (0..MAX_LOOKUP_URLS)
            .map(|i| format!("https://example.com/unsaved/{i}"))
            .collect();
        urls.push("https://example.com/a".to_string());
        let response = dispatch_with(
            IncomingMessage::LookupUrls { urls },
            tmp.path(),
            no_factory(),
        );
        assert_eq!(
            response,
            OutgoingMessage::LookupUrlsResult { saved: Vec::new() }
        );
    }

    #[test]
    fn dispatch_lookup_urls_empty_request_returns_empty() {
        let tmp = home_with_bookmark("https://example.com/a");
        let response = dispatch_with(
            IncomingMessage::LookupUrls { urls: Vec::new() },
            tmp.path(),
            no_factory(),
        );
        assert_eq!(
            response,
            OutgoingMessage::LookupUrlsResult { saved: Vec::new() }
        );
    }

    #[test]
    fn lookup_urls_missing_field_returns_error_and_loop_continues() {
        let stdin = frame_messages(&[json!({"type": "lookup_urls"}), json!({"type": "status"})]);
        let (stdout, result) = run_host_status_only(&stdin);
        assert!(result.is_ok());

        let responses = decode_responses(&stdout);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["type"], "error");
        assert_eq!(responses[1]["type"], "status_result");
    }

    // ── Setup gate ─────────────────────────────────────────────────

    #[test]
    fn requests_needing_setup_return_setup_required_before_setup() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gated = [
            IncomingMessage::Save {
                url: "https://example.com".into(),
                title: None,
                tags: None,
                collection: None,
                note: None,
                selected_text: None,
                action: None,
                page_html: None,
            },
            IncomingMessage::ListCollections,
            IncomingMessage::List {
                limit: None,
                state: None,
            },
            IncomingMessage::Show { id: "am_1".into() },
            IncomingMessage::Update {
                id: "am_1".into(),
                changes: BookmarkChanges {
                    user_tags: None,
                    suggested_tags: None,
                    collections: None,
                    note: None,
                    state: None,
                },
            },
            IncomingMessage::LookupUrls { urls: vec![] },
        ];
        for message in gated {
            let response = dispatch_with(message.clone(), tmp.path(), no_factory());
            assert_eq!(
                response,
                OutgoingMessage::setup_required(),
                "for {message:?}"
            );
        }
        assert!(
            !tmp.path().join(".agentmark").exists(),
            "gated requests create nothing"
        );
    }

    #[test]
    fn status_reports_initialized() {
        let tmp = tempfile::TempDir::new().unwrap();
        let status = || {
            dispatch_with(
                IncomingMessage::Status {
                    extension_version: Some("0.3.0".into()),
                },
                tmp.path(),
                no_factory(),
            )
        };
        assert!(matches!(
            status(),
            OutgoingMessage::StatusResult {
                ok: true,
                initialized: false,
                ..
            }
        ));
        setup::apply_settings(
            &test_ctx(
                tmp.path(),
                no_factory(),
                &FakeDialog(FolderChoice::Cancelled),
            )
            .env,
            &SettingsChanges::default(),
        )
        .unwrap();
        assert!(matches!(
            status(),
            OutgoingMessage::StatusResult {
                initialized: true,
                ..
            }
        ));
    }

    // ── Setup messages ─────────────────────────────────────────────

    #[test]
    fn get_setup_state_works_before_setup() {
        let tmp = tempfile::TempDir::new().unwrap();
        match dispatch_with(IncomingMessage::GetSetupState, tmp.path(), no_factory()) {
            OutgoingMessage::SetupStateResult { state } => {
                assert!(!state.initialized);
                assert_eq!(state.settings.storage_path_display, "~/Documents/AgentMark");
                assert_eq!(state.agents.len(), 3);
            }
            other => panic!("expected SetupStateResult, got {other:?}"),
        }
    }

    #[test]
    fn save_settings_then_ready() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = tmp.path().join("marks");
        let changes = SettingsChanges {
            storage_path: Some(storage.display().to_string()),
            default_agent: Some("codex".into()),
            enrichment_enabled: Some(false),
            ..SettingsChanges::default()
        };
        match dispatch_with(
            IncomingMessage::SaveSettings { settings: changes },
            tmp.path(),
            no_factory(),
        ) {
            OutgoingMessage::SaveSettingsResult { state } => {
                assert!(state.initialized);
                assert_eq!(state.settings.default_agent, "codex");
                assert_eq!(state.bookmark_count, Some(0));
            }
            other => panic!("expected SaveSettingsResult, got {other:?}"),
        }
        assert!(storage.is_dir());
        let response = dispatch_with(
            IncomingMessage::LookupUrls {
                urls: vec!["https://example.com".into()],
            },
            tmp.path(),
            no_factory(),
        );
        assert_eq!(
            response,
            OutgoingMessage::LookupUrlsResult { saved: Vec::new() }
        );
    }

    #[test]
    fn save_settings_reports_field_errors_and_writes_nothing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let changes = SettingsChanges {
            storage_path: Some("relative/path".into()),
            default_agent: Some("nobody".into()),
            ..SettingsChanges::default()
        };
        match dispatch_with(
            IncomingMessage::SaveSettings { settings: changes },
            tmp.path(),
            no_factory(),
        ) {
            OutgoingMessage::SettingsInvalid { errors } => {
                let fields: Vec<_> = errors.iter().map(|e| e.field).collect();
                assert_eq!(
                    fields,
                    [
                        setup::SettingsField::StoragePath,
                        setup::SettingsField::DefaultAgent
                    ]
                );
            }
            other => panic!("expected SettingsInvalid, got {other:?}"),
        }
        assert!(!tmp.path().join(".agentmark").exists());
    }

    #[test]
    fn save_settings_write_failure_is_an_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        // A file where ~/.agentmark should be.
        std::fs::write(tmp.path().join(".agentmark"), "x").unwrap();
        let changes = SettingsChanges {
            storage_path: Some(tmp.path().join("marks").display().to_string()),
            ..SettingsChanges::default()
        };
        match dispatch_with(
            IncomingMessage::SaveSettings { settings: changes },
            tmp.path(),
            no_factory(),
        ) {
            OutgoingMessage::Error { message, code } => {
                assert!(message.starts_with("failed to write"), "{message}");
                assert_eq!(code, None);
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    // ── choose_folder ──────────────────────────────────────────────

    /// Records the start folder it was given.
    struct RecordingDialog {
        answer: FolderChoice,
        start: std::cell::RefCell<Option<Option<PathBuf>>>,
    }

    impl FolderDialog for RecordingDialog {
        fn pick(&self, start: Option<&Path>) -> FolderChoice {
            *self.start.borrow_mut() = Some(start.map(Path::to_path_buf));
            self.answer.clone()
        }
    }

    fn choose(
        home: &Path,
        answer: FolderChoice,
        start_path: Option<&str>,
    ) -> (OutgoingMessage, Option<PathBuf>) {
        let dialog = RecordingDialog {
            answer,
            start: std::cell::RefCell::new(None),
        };
        let response = dispatch(
            IncomingMessage::ChooseFolder {
                start_path: start_path.map(String::from),
            },
            &test_ctx(home, no_factory(), &dialog),
        );
        let start = dialog.start.borrow().clone().expect("dialog was shown");
        (response, start)
    }

    #[test]
    fn choose_folder_chosen_with_display_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let picked = tmp.path().join("Bookmarks/");
        let (response, _) = choose(tmp.path(), FolderChoice::Chosen(picked), None);
        assert_eq!(
            response.to_value().unwrap(),
            serde_json::json!({
                "type": "choose_folder_result",
                "outcome": "chosen",
                "path": tmp.path().join("Bookmarks").display().to_string(),
                "path_display": "~/Bookmarks",
                "reason": null
            })
        );
    }

    #[test]
    fn choose_folder_cancelled_and_unavailable() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (response, _) = choose(tmp.path(), FolderChoice::Cancelled, None);
        assert_eq!(
            response,
            OutgoingMessage::ChooseFolderResult {
                outcome: FolderOutcome::Cancelled,
                path: None,
                path_display: None,
                reason: None,
            }
        );
        let (response, _) = choose(
            tmp.path(),
            FolderChoice::Unavailable {
                reason: "No folder dialog is available on this system.".into(),
            },
            None,
        );
        assert_eq!(
            response,
            OutgoingMessage::ChooseFolderResult {
                outcome: FolderOutcome::Unavailable,
                path: None,
                path_display: None,
                reason: Some("No folder dialog is available on this system.".into()),
            }
        );
    }

    #[test]
    fn choose_folder_expands_and_checks_the_start_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("Documents")).unwrap();
        let (_, start) = choose(tmp.path(), FolderChoice::Cancelled, Some("~/Documents"));
        assert_eq!(start, Some(tmp.path().join("Documents")));
        let (_, start) = choose(tmp.path(), FolderChoice::Cancelled, Some("~/Missing"));
        assert_eq!(start, None, "a missing start folder is ignored");
        let (_, start) = choose(tmp.path(), FolderChoice::Cancelled, None);
        assert_eq!(start, None);
    }

    #[test]
    fn choose_folder_works_before_setup() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (response, _) = choose(tmp.path(), FolderChoice::Cancelled, None);
        assert!(matches!(
            response,
            OutgoingMessage::ChooseFolderResult { .. }
        ));
    }

    // ── Liveness ───────────────────────────────────────────────────

    const EXT_ID: &str = "abcdefghijklmnopabcdefghijklmnop";

    fn run_with_home(home: &Path, messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
        let mut reader = Cursor::new(frame_messages(messages));
        let mut writer = Vec::new();
        let dialog = FakeDialog(FolderChoice::Cancelled);
        let ctx = test_ctx(home, no_factory(), &dialog);
        run_native_host_with_io(&mut reader, &mut writer, &ctx, Some(EXT_ID.to_string())).unwrap();
        decode_responses(&writer)
    }

    #[test]
    fn records_connection_on_start_and_on_new_extension_version() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".agentmark")).unwrap();

        run_with_home(tmp.path(), &[]);
        let record = setup::read_connection(tmp.path()).expect("written on start");
        assert_eq!(record.extension_id.as_deref(), Some(EXT_ID));
        assert_eq!(record.extension_version, None);
        assert_eq!(record.host_version, env!("CARGO_PKG_VERSION"));

        let long = "9".repeat(100);
        let responses = run_with_home(
            tmp.path(),
            &[json!({"type": "status", "extension_version": long})],
        );
        assert_eq!(responses[0]["type"], "status_result");
        let record = setup::read_connection(tmp.path()).unwrap();
        assert_eq!(record.extension_version, Some("9".repeat(64)), "truncated");
    }

    #[test]
    fn liveness_failure_does_not_change_responses() {
        let tmp = tempfile::TempDir::new().unwrap();
        // A directory where the record should go makes every write fail.
        std::fs::create_dir_all(tmp.path().join(".agentmark/extension-connection.json")).unwrap();
        let responses = run_with_home(
            tmp.path(),
            &[
                json!({"type": "status", "extension_version": "0.3.0"}),
                json!({"type": "status"}),
            ],
        );
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["type"], "status_result");
        assert_eq!(responses[1]["type"], "status_result");
    }

    #[test]
    fn records_connection_once_setup_creates_the_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let responses = run_with_home(
            tmp.path(),
            &[
                json!({"type": "status", "extension_version": "0.3.0"}),
                json!({"type": "save_settings", "settings": {
                    "storage_path": tmp.path().join("marks").display().to_string()
                }}),
            ],
        );
        assert_eq!(responses[1]["type"], "save_settings_result");
        let record = setup::read_connection(tmp.path()).expect("written after setup");
        assert_eq!(record.extension_version.as_deref(), Some("0.3.0"));
        assert_eq!(record.extension_id.as_deref(), Some(EXT_ID));
    }

    #[test]
    fn no_liveness_file_without_agentmark_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        run_with_home(
            tmp.path(),
            &[json!({"type": "status", "extension_version": "1"})],
        );
        assert!(!tmp.path().join(".agentmark").exists());
    }
}
