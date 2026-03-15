//! Low-level Chrome native messaging framing.
//!
//! Chrome's native messaging protocol uses a 4-byte little-endian length
//! prefix followed by a JSON payload. This module implements read/write
//! helpers that are generic over `std::io::Read` and `std::io::Write`.

use std::io::{self, Read, Write};
use thiserror::Error;

/// Defensive maximum message size (1 MiB).
///
/// Chrome's native messaging transport has a 1 MB limit for messages sent
/// by the native host and 4 GB for messages from Chrome. We cap reads at
/// 1 MiB to protect against corrupted or hostile length prefixes while
/// still covering any realistic `selected_text` payload.
pub const MAX_MESSAGE_SIZE: u32 = 1024 * 1024;

/// Errors that can occur during native message framing.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// Clean EOF — stdin closed before any prefix bytes were read.
    /// The host loop should exit gracefully on this variant.
    #[error("end of input")]
    Eof,

    /// Partial read — stdin closed mid-prefix or mid-payload.
    #[error("unexpected end of input (read {read} of {expected} bytes)")]
    UnexpectedEof { expected: usize, read: usize },

    /// Declared payload length exceeds `MAX_MESSAGE_SIZE`.
    #[error("message too large ({size} bytes, max {MAX_MESSAGE_SIZE})")]
    MessageTooLarge { size: u32 },

    /// Declared payload length is zero.
    #[error("message length is zero")]
    EmptyMessage,

    /// Payload bytes are not valid JSON.
    #[error("invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    /// I/O error during read or write.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Read a single length-prefixed JSON message from `reader`.
///
/// Returns [`ProtocolError::Eof`] when stdin closes cleanly before any
/// prefix bytes, allowing the caller to distinguish normal shutdown from
/// a corrupted stream.
pub fn read_message(reader: &mut dyn Read) -> Result<serde_json::Value, ProtocolError> {
    // Read the 4-byte little-endian length prefix.
    let mut prefix = [0u8; 4];
    match reader.read(&mut prefix) {
        Ok(0) => return Err(ProtocolError::Eof),
        Ok(n) if n < 4 => {
            // Got some bytes but not a full prefix. Try to read the rest.
            let remaining = &mut prefix[n..];
            match reader.read_exact(remaining) {
                Ok(()) => {} // got the full prefix now
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    return Err(ProtocolError::UnexpectedEof {
                        expected: 4,
                        read: n,
                    });
                }
                Err(e) => return Err(ProtocolError::Io(e)),
            }
        }
        Ok(_) => {} // got all 4 bytes
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            return Err(ProtocolError::Eof);
        }
        Err(e) => return Err(ProtocolError::Io(e)),
    }

    let length = u32::from_le_bytes(prefix);

    // Validate length before allocating.
    if length == 0 {
        return Err(ProtocolError::EmptyMessage);
    }
    if length > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::MessageTooLarge { size: length });
    }

    // Read exactly `length` bytes of payload.
    let mut buf = vec![0u8; length as usize];
    let mut total_read = 0usize;
    while total_read < buf.len() {
        match reader.read(&mut buf[total_read..]) {
            Ok(0) => {
                return Err(ProtocolError::UnexpectedEof {
                    expected: length as usize,
                    read: total_read,
                });
            }
            Ok(n) => total_read += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(ProtocolError::Io(e)),
        }
    }

    // Deserialize JSON.
    let value: serde_json::Value = serde_json::from_slice(&buf)?;
    Ok(value)
}

/// Drain exactly `size` bytes from `reader`, discarding them.
///
/// Used to recover stream alignment after a `MessageTooLarge` error.
/// Returns `Ok(())` if all bytes were consumed, or a `ProtocolError`
/// if the stream ends or fails before all bytes are read.
pub fn drain_payload(reader: &mut dyn Read, size: u32) -> Result<(), ProtocolError> {
    let mut remaining = size as u64;
    let mut scratch = [0u8; 8192];
    while remaining > 0 {
        let to_read = (remaining as usize).min(scratch.len());
        match reader.read(&mut scratch[..to_read]) {
            Ok(0) => {
                return Err(ProtocolError::UnexpectedEof {
                    expected: size as usize,
                    read: (size as u64 - remaining) as usize,
                });
            }
            Ok(n) => remaining -= n as u64,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(ProtocolError::Io(e)),
        }
    }
    Ok(())
}

/// Write a single length-prefixed JSON message to `writer` and flush.
///
/// Serializes `value` to JSON bytes, writes the 4-byte little-endian
/// length prefix followed by the payload, and flushes. This ensures
/// Chrome receives the complete message without buffering delays.
pub fn write_message(
    writer: &mut dyn Write,
    value: &serde_json::Value,
) -> Result<(), ProtocolError> {
    let payload = serde_json::to_vec(value)?;

    let length = payload.len() as u32;
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    /// Helper: frame a JSON value into length-prefixed bytes.
    fn frame(value: &serde_json::Value) -> Vec<u8> {
        let payload = serde_json::to_vec(value).unwrap();
        let mut buf = Vec::new();
        buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(&payload);
        buf
    }

    #[test]
    fn write_produces_correct_length_prefix() {
        let value = json!({"type": "status"});
        let mut buf = Vec::new();
        write_message(&mut buf, &value).unwrap();

        let payload = serde_json::to_vec(&value).unwrap();
        let expected_prefix = (payload.len() as u32).to_le_bytes();
        assert_eq!(&buf[..4], &expected_prefix);
        assert_eq!(&buf[4..], &payload);
    }

    #[test]
    fn read_parses_correct_length_prefix() {
        let original = json!({"type": "status"});
        let framed = frame(&original);
        let mut cursor = Cursor::new(framed);
        let parsed = read_message(&mut cursor).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn roundtrip_write_then_read() {
        let value = json!({
            "type": "save",
            "url": "https://example.com",
            "title": "Test",
            "tags": ["rust", "cli"]
        });
        let mut buf = Vec::new();
        write_message(&mut buf, &value).unwrap();
        let mut cursor = Cursor::new(buf);
        let result = read_message(&mut cursor).unwrap();
        assert_eq!(result, value);
    }

    #[test]
    fn roundtrip_with_multibyte_utf8() {
        let value = json!({"note": "日本語テスト 🦀"});
        let mut buf = Vec::new();
        write_message(&mut buf, &value).unwrap();
        let mut cursor = Cursor::new(buf);
        let result = read_message(&mut cursor).unwrap();
        assert_eq!(result, value);
    }

    #[test]
    fn eof_before_any_prefix_bytes() {
        let mut cursor = Cursor::new(Vec::<u8>::new());
        let err = read_message(&mut cursor).unwrap_err();
        assert!(
            matches!(err, ProtocolError::Eof),
            "expected Eof, got {err:?}"
        );
    }

    #[test]
    fn eof_after_partial_prefix() {
        // Only 2 of 4 prefix bytes.
        let mut cursor = Cursor::new(vec![0x0A, 0x00]);
        let err = read_message(&mut cursor).unwrap_err();
        assert!(
            matches!(
                err,
                ProtocolError::UnexpectedEof {
                    expected: 4,
                    read: 2
                }
            ),
            "expected UnexpectedEof(4, 2), got {err:?}"
        );
    }

    #[test]
    fn eof_after_partial_payload() {
        // Prefix says 10 bytes, but only 3 bytes of payload follow.
        let mut buf = vec![0x0A, 0x00, 0x00, 0x00]; // length = 10
        buf.extend_from_slice(b"abc"); // only 3 bytes
        let mut cursor = Cursor::new(buf);
        let err = read_message(&mut cursor).unwrap_err();
        assert!(
            matches!(
                err,
                ProtocolError::UnexpectedEof {
                    expected: 10,
                    read: 3
                }
            ),
            "expected UnexpectedEof(10, 3), got {err:?}"
        );
    }

    #[test]
    fn zero_length_rejected() {
        let buf = vec![0x00, 0x00, 0x00, 0x00];
        let mut cursor = Cursor::new(buf);
        let err = read_message(&mut cursor).unwrap_err();
        assert!(
            matches!(err, ProtocolError::EmptyMessage),
            "expected EmptyMessage, got {err:?}"
        );
    }

    #[test]
    fn oversized_length_rejected() {
        // 2 MiB = 2 * 1024 * 1024 = 0x00200000
        let size: u32 = 2 * 1024 * 1024;
        let buf = size.to_le_bytes().to_vec();
        let mut cursor = Cursor::new(buf);
        let err = read_message(&mut cursor).unwrap_err();
        assert!(
            matches!(err, ProtocolError::MessageTooLarge { size: s } if s == size),
            "expected MessageTooLarge, got {err:?}"
        );
    }

    #[test]
    fn invalid_json_rejected() {
        let bad_payload = b"not json at all";
        let mut buf = (bad_payload.len() as u32).to_le_bytes().to_vec();
        buf.extend_from_slice(bad_payload);
        let mut cursor = Cursor::new(buf);
        let err = read_message(&mut cursor).unwrap_err();
        assert!(
            matches!(err, ProtocolError::InvalidJson(_)),
            "expected InvalidJson, got {err:?}"
        );
    }

    #[test]
    fn exact_max_size_accepted() {
        // A message exactly at MAX_MESSAGE_SIZE should be accepted (if valid JSON).
        // We won't allocate a full 1 MiB of valid JSON here, but verify the boundary
        // by testing MAX_MESSAGE_SIZE - 1 isn't rejected.
        let size = MAX_MESSAGE_SIZE;
        // Just verify the check passes; we'll get InvalidJson since
        // we won't write valid JSON, but it should NOT be MessageTooLarge.
        let mut buf = size.to_le_bytes().to_vec();
        buf.extend(vec![0x20; size as usize]); // spaces are not valid JSON
        let mut cursor = Cursor::new(buf);
        let err = read_message(&mut cursor).unwrap_err();
        assert!(
            matches!(err, ProtocolError::InvalidJson(_)),
            "expected InvalidJson (not MessageTooLarge), got {err:?}"
        );
    }

    #[test]
    fn write_flush_is_called() {
        // A writer that tracks whether flush was called.
        struct FlushTracker {
            buf: Vec<u8>,
            flushed: bool,
        }
        impl Write for FlushTracker {
            fn write(&mut self, data: &[u8]) -> io::Result<usize> {
                self.buf.extend_from_slice(data);
                Ok(data.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                self.flushed = true;
                Ok(())
            }
        }

        let mut tracker = FlushTracker {
            buf: Vec::new(),
            flushed: false,
        };
        write_message(&mut tracker, &json!({"ok": true})).unwrap();
        assert!(tracker.flushed, "write_message must flush after writing");
    }

    #[test]
    fn multiple_messages_in_sequence() {
        let msgs = vec![
            json!({"type": "status"}),
            json!({"type": "save", "url": "https://a.com"}),
            json!({"type": "save", "url": "https://b.com"}),
        ];
        let mut buf = Vec::new();
        for msg in &msgs {
            write_message(&mut buf, msg).unwrap();
        }
        let mut cursor = Cursor::new(buf);
        for expected in &msgs {
            let parsed = read_message(&mut cursor).unwrap();
            assert_eq!(&parsed, expected);
        }
        // Next read should be EOF.
        let err = read_message(&mut cursor).unwrap_err();
        assert!(matches!(err, ProtocolError::Eof));
    }
}
