use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A lifecycle event for a bookmark, appended to `events.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookmarkEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,
    pub details: serde_json::Value,
}

/// The type of bookmark lifecycle event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Saved,
    Enriched,
    Resaved,
    ContentUpdated,
    Reprocessed,
}

impl BookmarkEvent {
    /// Create a new event with the given type and details.
    pub fn new(event_type: EventType, details: serde_json::Value) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            details,
        }
    }

    /// Serialize to a single-line JSON string suitable for JSONL append.
    pub fn to_jsonl(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize from a single JSON line.
    pub fn from_json_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn event_type_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&EventType::Saved).unwrap(),
            "\"saved\""
        );
        assert_eq!(
            serde_json::to_string(&EventType::Enriched).unwrap(),
            "\"enriched\""
        );
        assert_eq!(
            serde_json::to_string(&EventType::Resaved).unwrap(),
            "\"resaved\""
        );
        assert_eq!(
            serde_json::to_string(&EventType::ContentUpdated).unwrap(),
            "\"content_updated\""
        );
        assert_eq!(
            serde_json::to_string(&EventType::Reprocessed).unwrap(),
            "\"reprocessed\""
        );
    }

    #[test]
    fn unknown_event_type_fails_deserialization() {
        let result = serde_json::from_str::<EventType>("\"deleted\"");
        assert!(result.is_err());
    }

    #[test]
    fn event_json_roundtrip() {
        let event = BookmarkEvent::new(EventType::Saved, json!({"url": "https://example.com"}));
        let json = event.to_jsonl().unwrap();
        let roundtripped = BookmarkEvent::from_json_line(&json).unwrap();
        assert_eq!(event, roundtripped);
    }

    #[test]
    fn event_with_empty_details_roundtrips() {
        let event = BookmarkEvent::new(EventType::Enriched, json!({}));
        let json = event.to_jsonl().unwrap();
        let roundtripped = BookmarkEvent::from_json_line(&json).unwrap();
        assert_eq!(event.event_type, roundtripped.event_type);
        assert_eq!(event.details, roundtripped.details);
    }

    #[test]
    fn event_with_nested_details_roundtrips() {
        let details = json!({
            "changes": {
                "tags_added": ["rust", "cli"],
                "summary": "A nested summary",
                "metadata": {
                    "source": "auto",
                    "confidence": 0.95
                }
            }
        });
        let event = BookmarkEvent::new(EventType::ContentUpdated, details.clone());
        let json = event.to_jsonl().unwrap();
        let roundtripped = BookmarkEvent::from_json_line(&json).unwrap();
        assert_eq!(roundtripped.details, details);
    }

    #[test]
    fn event_jsonl_is_single_line() {
        let event = BookmarkEvent::new(
            EventType::Saved,
            json!({"url": "https://example.com", "title": "Test Page"}),
        );
        let jsonl = event.to_jsonl().unwrap();
        assert!(
            !jsonl.contains('\n'),
            "JSONL output must not contain newlines"
        );
    }

    #[test]
    fn event_timestamp_is_recent_utc() {
        let before = Utc::now();
        let event = BookmarkEvent::new(EventType::Saved, json!({}));
        let after = Utc::now();
        assert!(event.timestamp >= before);
        assert!(event.timestamp <= after);
    }

    #[test]
    fn event_details_with_arrays_preserved() {
        let details = json!({"items": [1, 2, 3], "names": ["a", "b"]});
        let event = BookmarkEvent::new(EventType::Reprocessed, details.clone());
        let json = event.to_jsonl().unwrap();
        let roundtripped = BookmarkEvent::from_json_line(&json).unwrap();
        assert_eq!(roundtripped.details["items"], json!([1, 2, 3]));
        assert_eq!(roundtripped.details["names"], json!(["a", "b"]));
    }
}
