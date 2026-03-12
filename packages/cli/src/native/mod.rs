//! Chrome native messaging transport layer (Spec 17).
//!
//! This module provides the reusable protocol framing and typed message
//! contracts for Chrome's native messaging host interface. The actual
//! command loop and save-pipeline dispatch belong to Spec 18.
//!
//! - [`protocol`] — length-prefixed JSON read/write over `Read`/`Write`
//! - [`messages`] — `IncomingMessage` / `OutgoingMessage` enums and conversion helpers

pub mod messages;
pub mod protocol;

pub use messages::{IncomingMessage, MessageError, OutgoingMessage};
pub use protocol::{read_message, write_message, ProtocolError};
