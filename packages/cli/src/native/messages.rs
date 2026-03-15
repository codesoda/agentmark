//! Typed native messaging request/response contracts.
//!
//! Defines the message shapes exchanged between the Chrome extension and
//! the `agentmark native-host` process. Spec 18 consumes these types for
//! dispatch; Spec 19 should mirror them in the extension's TypeScript.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::models::BookmarkState;

/// Errors when converting a raw JSON value into a typed message.
#[derive(Debug, Error)]
pub enum MessageError {
    /// The JSON value is not an object.
    #[error("expected JSON object")]
    NotAnObject,

    /// The object has no `type` field or it is not a string.
    #[error("missing or invalid \"type\" field")]
    MissingType,

    /// The `type` value is not a recognized message type.
    #[error("unknown message type: {0}")]
    UnknownType(String),

    /// The object has the right type but fails schema validation
    /// (e.g. missing required `url` on a `save` message).
    #[error("invalid message fields: {0}")]
    InvalidFields(String),
}

/// Messages sent from the Chrome extension to the native host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IncomingMessage {
    /// Save a bookmark.
    Save {
        url: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        tags: Option<Vec<String>>,
        #[serde(default)]
        collection: Option<String>,
        #[serde(default)]
        note: Option<String>,
        #[serde(default)]
        selected_text: Option<String>,
        #[serde(default)]
        action: Option<String>,
    },
    /// Health check.
    Status,
    /// List existing collections.
    ListCollections,
    /// List recent bookmarks with optional filtering.
    List {
        #[serde(default)]
        limit: Option<u32>,
        #[serde(default)]
        state: Option<BookmarkState>,
    },
    /// Show full details for a single bookmark.
    Show { id: String },
    /// Update bookmark metadata fields.
    Update {
        id: String,
        changes: BookmarkChanges,
    },
}

/// Messages sent from the native host back to the Chrome extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutgoingMessage {
    /// Result of a successful save.
    SaveResult {
        id: String,
        path: String,
        status: String,
    },
    /// Result of a health check.
    StatusResult { ok: bool, version: String },
    /// Result of a collection listing.
    ListCollectionsResult { collections: Vec<String> },
    /// Result of a bookmark listing.
    ListResult { bookmarks: Vec<BookmarkSummary> },
    /// Result of a bookmark detail lookup.
    ShowResult { bookmark: BookmarkDetail },
    /// Result of a bookmark update.
    UpdateResult { bookmark: BookmarkDetail },
    /// Error response for any failed operation.
    Error { message: String },
}

/// Narrow DTO for bookmark list responses — only fields needed for list rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookmarkSummary {
    pub id: String,
    pub url: String,
    pub title: String,
    pub state: BookmarkState,
    pub user_tags: Vec<String>,
    pub suggested_tags: Vec<String>,
    pub saved_at: String,
}

/// Full detail DTO for a single bookmark — used by show and update responses.
/// Excludes article body to keep payloads bounded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookmarkDetail {
    pub id: String,
    pub url: String,
    pub title: String,
    pub summary: Option<String>,
    pub saved_at: String,
    pub capture_source: String,
    pub state: BookmarkState,
    pub user_tags: Vec<String>,
    pub suggested_tags: Vec<String>,
    pub collections: Vec<String>,
    pub note: Option<String>,
}

/// Typed changes for bookmark update requests.
/// All fields are optional — absent means "leave unchanged".
/// `note` uses double-option: outer `None` = leave unchanged, `Some(None)` = clear, `Some(Some(v))` = set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookmarkChanges {
    #[serde(default)]
    pub user_tags: Option<Vec<String>>,
    #[serde(default)]
    pub suggested_tags: Option<Vec<String>>,
    #[serde(default)]
    pub collections: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub note: Option<Option<String>>,
    #[serde(default)]
    pub state: Option<BookmarkState>,
}

/// Deserialize a double-option field:
/// - absent / undefined → `None` (leave unchanged)
/// - `null` → `Some(None)` (clear)
/// - `"value"` → `Some(Some("value"))` (set)
fn deserialize_double_option<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // When the field is present (even if null), serde calls this function.
    // null → Some(None), "text" → Some(Some("text"))
    let value: Option<String> = Option::deserialize(deserializer)?;
    Ok(Some(value))
}

impl OutgoingMessage {
    /// Convenience constructor for error responses.
    pub fn error(message: impl Into<String>) -> Self {
        OutgoingMessage::Error {
            message: message.into(),
        }
    }
}

impl IncomingMessage {
    /// Parse a [`serde_json::Value`] into a typed [`IncomingMessage`].
    ///
    /// This provides explicit error variants so Spec 18 can distinguish
    /// between unknown message types and malformed fields without
    /// string-matching serde error messages.
    pub fn from_value(value: serde_json::Value) -> Result<Self, MessageError> {
        let obj = value.as_object().ok_or(MessageError::NotAnObject)?;

        let type_str = obj
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or(MessageError::MissingType)?;

        match type_str {
            "save" | "status" | "list_collections" | "list" | "show" | "update" => {
                // Use serde for full field validation.
                serde_json::from_value(value)
                    .map_err(|e| MessageError::InvalidFields(e.to_string()))
            }
            other => Err(MessageError::UnknownType(other.to_string())),
        }
    }
}

impl OutgoingMessage {
    /// Serialize this message to a [`serde_json::Value`].
    pub fn to_value(&self) -> Result<serde_json::Value, MessageError> {
        serde_json::to_value(self).map_err(|e| MessageError::InvalidFields(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- IncomingMessage deserialization --

    #[test]
    fn deserialize_save_full() {
        let value = json!({
            "type": "save",
            "url": "https://example.com",
            "title": "Example",
            "tags": ["rust", "cli"],
            "collection": "reading",
            "note": "interesting",
            "selected_text": "some excerpt",
            "action": "read_later"
        });
        let msg = IncomingMessage::from_value(value).unwrap();
        assert_eq!(
            msg,
            IncomingMessage::Save {
                url: "https://example.com".into(),
                title: Some("Example".into()),
                tags: Some(vec!["rust".into(), "cli".into()]),
                collection: Some("reading".into()),
                note: Some("interesting".into()),
                selected_text: Some("some excerpt".into()),
                action: Some("read_later".into()),
            }
        );
    }

    #[test]
    fn deserialize_save_minimal() {
        let value = json!({"type": "save", "url": "https://example.com"});
        let msg = IncomingMessage::from_value(value).unwrap();
        assert_eq!(
            msg,
            IncomingMessage::Save {
                url: "https://example.com".into(),
                title: None,
                tags: None,
                collection: None,
                note: None,
                selected_text: None,
                action: None,
            }
        );
    }

    #[test]
    fn deserialize_save_with_null_optionals() {
        let value = json!({
            "type": "save",
            "url": "https://example.com",
            "title": null,
            "tags": null,
            "note": null
        });
        let msg = IncomingMessage::from_value(value).unwrap();
        match msg {
            IncomingMessage::Save {
                url,
                title,
                tags,
                note,
                ..
            } => {
                assert_eq!(url, "https://example.com");
                assert!(title.is_none());
                assert!(tags.is_none());
                assert!(note.is_none());
            }
            _ => panic!("expected Save"),
        }
    }

    #[test]
    fn deserialize_status() {
        let value = json!({"type": "status"});
        let msg = IncomingMessage::from_value(value).unwrap();
        assert_eq!(msg, IncomingMessage::Status);
    }

    #[test]
    fn reject_not_an_object() {
        let value = json!("just a string");
        let err = IncomingMessage::from_value(value).unwrap_err();
        assert!(matches!(err, MessageError::NotAnObject));
    }

    #[test]
    fn reject_missing_type() {
        let value = json!({"url": "https://example.com"});
        let err = IncomingMessage::from_value(value).unwrap_err();
        assert!(matches!(err, MessageError::MissingType));
    }

    #[test]
    fn reject_non_string_type() {
        let value = json!({"type": 42});
        let err = IncomingMessage::from_value(value).unwrap_err();
        assert!(matches!(err, MessageError::MissingType));
    }

    #[test]
    fn reject_unknown_type() {
        let value = json!({"type": "delete"});
        let err = IncomingMessage::from_value(value).unwrap_err();
        assert!(
            matches!(&err, MessageError::UnknownType(t) if t == "delete"),
            "expected UnknownType(\"delete\"), got {err:?}"
        );
    }

    #[test]
    fn reject_save_missing_url() {
        let value = json!({"type": "save", "title": "No URL"});
        let err = IncomingMessage::from_value(value).unwrap_err();
        assert!(
            matches!(err, MessageError::InvalidFields(_)),
            "expected InvalidFields, got {err:?}"
        );
    }

    #[test]
    fn reject_save_bad_tags_type() {
        let value = json!({"type": "save", "url": "https://x.com", "tags": "not-an-array"});
        let err = IncomingMessage::from_value(value).unwrap_err();
        assert!(
            matches!(err, MessageError::InvalidFields(_)),
            "expected InvalidFields, got {err:?}"
        );
    }

    // -- OutgoingMessage serialization --

    #[test]
    fn serialize_save_result() {
        let msg = OutgoingMessage::SaveResult {
            id: "abc123".into(),
            path: "/path/to/bundle".into(),
            status: "created".into(),
        };
        let value = msg.to_value().unwrap();
        assert_eq!(value["type"], "save_result");
        assert_eq!(value["id"], "abc123");
        assert_eq!(value["path"], "/path/to/bundle");
        assert_eq!(value["status"], "created");
    }

    #[test]
    fn serialize_status_result() {
        let msg = OutgoingMessage::StatusResult {
            ok: true,
            version: "0.1.0".into(),
        };
        let value = msg.to_value().unwrap();
        assert_eq!(value["type"], "status_result");
        assert_eq!(value["ok"], true);
        assert_eq!(value["version"], "0.1.0");
    }

    #[test]
    fn serialize_error() {
        let msg = OutgoingMessage::error("something went wrong");
        let value = msg.to_value().unwrap();
        assert_eq!(value["type"], "error");
        assert_eq!(value["message"], "something went wrong");
    }

    #[test]
    fn error_helper_accepts_string_and_str() {
        let from_str = OutgoingMessage::error("hello");
        let from_string = OutgoingMessage::error(String::from("hello"));
        assert_eq!(from_str, from_string);
    }

    // -- Roundtrip through serde_json::Value --

    #[test]
    fn incoming_save_serde_roundtrip() {
        let msg = IncomingMessage::Save {
            url: "https://example.com".into(),
            title: Some("Title".into()),
            tags: Some(vec!["a".into()]),
            collection: Some("work".into()),
            note: None,
            selected_text: None,
            action: None,
        };
        let value = serde_json::to_value(&msg).unwrap();
        let back = IncomingMessage::from_value(value).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn outgoing_error_with_unicode() {
        let msg = OutgoingMessage::error("エラー: 失敗しました 🚫");
        let value = msg.to_value().unwrap();
        assert_eq!(value["message"], "エラー: 失敗しました 🚫");
    }

    // -- ListCollections --

    #[test]
    fn deserialize_list_collections() {
        let value = json!({"type": "list_collections"});
        let msg = IncomingMessage::from_value(value).unwrap();
        assert_eq!(msg, IncomingMessage::ListCollections);
    }

    #[test]
    fn serialize_list_collections_result() {
        let msg = OutgoingMessage::ListCollectionsResult {
            collections: vec!["reading".into(), "work".into()],
        };
        let value = msg.to_value().unwrap();
        assert_eq!(value["type"], "list_collections_result");
        let cols = value["collections"].as_array().unwrap();
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0], "reading");
        assert_eq!(cols[1], "work");
    }

    #[test]
    fn serialize_list_collections_result_empty() {
        let msg = OutgoingMessage::ListCollectionsResult {
            collections: vec![],
        };
        let value = msg.to_value().unwrap();
        assert_eq!(value["type"], "list_collections_result");
        assert!(value["collections"].as_array().unwrap().is_empty());
    }

    // -- List --

    #[test]
    fn deserialize_list_minimal() {
        let value = json!({"type": "list"});
        let msg = IncomingMessage::from_value(value).unwrap();
        assert_eq!(
            msg,
            IncomingMessage::List {
                limit: None,
                state: None,
            }
        );
    }

    #[test]
    fn deserialize_list_with_limit_and_state() {
        let value = json!({"type": "list", "limit": 25, "state": "inbox"});
        let msg = IncomingMessage::from_value(value).unwrap();
        assert_eq!(
            msg,
            IncomingMessage::List {
                limit: Some(25),
                state: Some(BookmarkState::Inbox),
            }
        );
    }

    #[test]
    fn deserialize_list_with_all_states() {
        for (state_str, state_val) in [
            ("inbox", BookmarkState::Inbox),
            ("processed", BookmarkState::Processed),
            ("archived", BookmarkState::Archived),
        ] {
            let value = json!({"type": "list", "state": state_str});
            let msg = IncomingMessage::from_value(value).unwrap();
            assert_eq!(
                msg,
                IncomingMessage::List {
                    limit: None,
                    state: Some(state_val),
                }
            );
        }
    }

    #[test]
    fn deserialize_list_invalid_state_returns_error() {
        let value = json!({"type": "list", "state": "deleted"});
        let err = IncomingMessage::from_value(value).unwrap_err();
        assert!(
            matches!(err, MessageError::InvalidFields(_)),
            "expected InvalidFields, got {err:?}"
        );
    }

    #[test]
    fn serialize_list_result() {
        let msg = OutgoingMessage::ListResult {
            bookmarks: vec![BookmarkSummary {
                id: "am_123".into(),
                url: "https://example.com".into(),
                title: "Example".into(),
                state: BookmarkState::Inbox,
                user_tags: vec!["rust".into()],
                suggested_tags: vec!["dev".into()],
                saved_at: "2026-03-12T00:00:00Z".into(),
            }],
        };
        let value = msg.to_value().unwrap();
        assert_eq!(value["type"], "list_result");
        let bookmarks = value["bookmarks"].as_array().unwrap();
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0]["id"], "am_123");
        assert_eq!(bookmarks[0]["url"], "https://example.com");
        assert_eq!(bookmarks[0]["title"], "Example");
        assert_eq!(bookmarks[0]["state"], "inbox");
        assert_eq!(bookmarks[0]["user_tags"][0], "rust");
        assert_eq!(bookmarks[0]["suggested_tags"][0], "dev");
        assert_eq!(bookmarks[0]["saved_at"], "2026-03-12T00:00:00Z");
    }

    #[test]
    fn serialize_list_result_empty() {
        let msg = OutgoingMessage::ListResult { bookmarks: vec![] };
        let value = msg.to_value().unwrap();
        assert_eq!(value["type"], "list_result");
        assert!(value["bookmarks"].as_array().unwrap().is_empty());
    }

    #[test]
    fn deserialize_save_with_collection() {
        let value = json!({
            "type": "save",
            "url": "https://example.com",
            "collection": "research"
        });
        let msg = IncomingMessage::from_value(value).unwrap();
        match msg {
            IncomingMessage::Save { collection, .. } => {
                assert_eq!(collection, Some("research".into()));
            }
            _ => panic!("expected Save"),
        }
    }

    // -- Show --

    #[test]
    fn deserialize_show() {
        let value = json!({"type": "show", "id": "am_123"});
        let msg = IncomingMessage::from_value(value).unwrap();
        assert_eq!(
            msg,
            IncomingMessage::Show {
                id: "am_123".into()
            }
        );
    }

    #[test]
    fn deserialize_show_missing_id() {
        let value = json!({"type": "show"});
        let err = IncomingMessage::from_value(value).unwrap_err();
        assert!(matches!(err, MessageError::InvalidFields(_)));
    }

    #[test]
    fn serialize_show_result() {
        let msg = OutgoingMessage::ShowResult {
            bookmark: BookmarkDetail {
                id: "am_123".into(),
                url: "https://example.com".into(),
                title: "Example".into(),
                summary: Some("A summary".into()),
                saved_at: "2026-03-12T00:00:00Z".into(),
                capture_source: "cli".into(),
                state: BookmarkState::Inbox,
                user_tags: vec!["rust".into()],
                suggested_tags: vec!["dev".into()],
                collections: vec!["reading".into()],
                note: Some("my note".into()),
            },
        };
        let value = msg.to_value().unwrap();
        assert_eq!(value["type"], "show_result");
        assert_eq!(value["bookmark"]["id"], "am_123");
        assert_eq!(value["bookmark"]["summary"], "A summary");
        assert_eq!(value["bookmark"]["note"], "my note");
        assert_eq!(value["bookmark"]["collections"][0], "reading");
    }

    #[test]
    fn serialize_show_result_null_optionals() {
        let msg = OutgoingMessage::ShowResult {
            bookmark: BookmarkDetail {
                id: "am_456".into(),
                url: "https://example.com".into(),
                title: "Example".into(),
                summary: None,
                saved_at: "2026-03-12T00:00:00Z".into(),
                capture_source: "chrome_extension".into(),
                state: BookmarkState::Processed,
                user_tags: vec![],
                suggested_tags: vec![],
                collections: vec![],
                note: None,
            },
        };
        let value = msg.to_value().unwrap();
        assert_eq!(value["type"], "show_result");
        assert!(value["bookmark"]["summary"].is_null());
        assert!(value["bookmark"]["note"].is_null());
        assert!(value["bookmark"]["user_tags"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    // -- Update --

    #[test]
    fn deserialize_update_with_all_changes() {
        let value = json!({
            "type": "update",
            "id": "am_123",
            "changes": {
                "user_tags": ["rust", "cli"],
                "suggested_tags": ["dev"],
                "collections": ["reading"],
                "note": "updated note",
                "state": "processed"
            }
        });
        let msg = IncomingMessage::from_value(value).unwrap();
        match msg {
            IncomingMessage::Update { id, changes } => {
                assert_eq!(id, "am_123");
                assert_eq!(changes.user_tags, Some(vec!["rust".into(), "cli".into()]));
                assert_eq!(changes.suggested_tags, Some(vec!["dev".into()]));
                assert_eq!(changes.collections, Some(vec!["reading".into()]));
                assert_eq!(changes.note, Some(Some("updated note".into())));
                assert_eq!(changes.state, Some(BookmarkState::Processed));
            }
            _ => panic!("expected Update"),
        }
    }

    #[test]
    fn deserialize_update_with_empty_changes() {
        let value = json!({"type": "update", "id": "am_123", "changes": {}});
        let msg = IncomingMessage::from_value(value).unwrap();
        match msg {
            IncomingMessage::Update { id, changes } => {
                assert_eq!(id, "am_123");
                assert!(changes.user_tags.is_none());
                assert!(changes.suggested_tags.is_none());
                assert!(changes.collections.is_none());
                assert!(changes.note.is_none());
                assert!(changes.state.is_none());
            }
            _ => panic!("expected Update"),
        }
    }

    #[test]
    fn deserialize_update_note_null_clears() {
        let value = json!({"type": "update", "id": "am_123", "changes": {"note": null}});
        let msg = IncomingMessage::from_value(value).unwrap();
        match msg {
            IncomingMessage::Update { changes, .. } => {
                assert_eq!(changes.note, Some(None));
            }
            _ => panic!("expected Update"),
        }
    }

    #[test]
    fn deserialize_update_missing_id() {
        let value = json!({"type": "update", "changes": {}});
        let err = IncomingMessage::from_value(value).unwrap_err();
        assert!(matches!(err, MessageError::InvalidFields(_)));
    }

    #[test]
    fn deserialize_update_missing_changes() {
        let value = json!({"type": "update", "id": "am_123"});
        let err = IncomingMessage::from_value(value).unwrap_err();
        assert!(matches!(err, MessageError::InvalidFields(_)));
    }

    #[test]
    fn deserialize_update_invalid_state() {
        let value = json!({"type": "update", "id": "am_123", "changes": {"state": "deleted"}});
        let err = IncomingMessage::from_value(value).unwrap_err();
        assert!(matches!(err, MessageError::InvalidFields(_)));
    }

    #[test]
    fn serialize_update_result() {
        let msg = OutgoingMessage::UpdateResult {
            bookmark: BookmarkDetail {
                id: "am_123".into(),
                url: "https://example.com".into(),
                title: "Example".into(),
                summary: None,
                saved_at: "2026-03-12T00:00:00Z".into(),
                capture_source: "cli".into(),
                state: BookmarkState::Processed,
                user_tags: vec!["rust".into()],
                suggested_tags: vec![],
                collections: vec![],
                note: None,
            },
        };
        let value = msg.to_value().unwrap();
        assert_eq!(value["type"], "update_result");
        assert_eq!(value["bookmark"]["state"], "processed");
    }
}
