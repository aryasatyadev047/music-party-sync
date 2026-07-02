//! Error types for the EchoSync Buffer Layer.
//!
//! Every public function in this module returns `Result<T, BufferError>`
//! so callers (Streaming Engine glue code, and eventually the
//! Synchronization Engine) have a single, stable error type to match on.

use std::fmt;

/// Errors that can occur while operating the Buffer Layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferError {
    /// A packet was rejected because the buffer is already at its
    /// configured maximum capacity and no room could be reclaimed.
    BufferFull {
        /// The buffer's configured maximum capacity, in packets.
        capacity: usize,
    },

    /// An operation that requires at least one buffered packet was
    /// attempted while the buffer was empty.
    BufferEmpty,

    /// A packet with this sequence number is already sitting in the
    /// buffer, awaiting release.
    DuplicatePacket {
        /// The sequence number of the rejected duplicate.
        sequence_number: u64,
    },

    /// A packet with this sequence number was already delivered (or its
    /// slot was already skipped as lost) earlier in the stream, so it is
    /// rejected as stale rather than re-buffered.
    AlreadyProcessed {
        /// The sequence number of the rejected, already-processed packet.
        sequence_number: u64,
    },

    /// A packet was dropped because it sat in the buffer longer than the
    /// configured maximum packet age.
    PacketTooOld {
        /// The sequence number of the dropped packet.
        sequence_number: u64,
        /// The configured maximum packet age, in milliseconds.
        max_age_ms: u64,
    },

    /// The supplied [`crate::core::buffer::config::BufferConfig`] failed
    /// validation. The contained string describes which invariant was
    /// violated.
    InvalidConfiguration(String),

    /// [`crate::core::buffer::jitter_buffer::JitterBuffer::set_target_delay`]
    /// was called with a value outside the configured
    /// `[min_target_delay, max_target_delay]` range.
    InvalidTargetDelay {
        /// The requested target delay, in milliseconds.
        requested_ms: u64,
        /// The configured minimum target delay, in milliseconds.
        min_ms: u64,
        /// The configured maximum target delay, in milliseconds.
        max_ms: u64,
    },

    /// A lower-level operation (e.g. an internal lock or channel) failed
    /// in a way that doesn't fit a more specific variant. The contained
    /// string describes the underlying cause.
    OperationFailed(String),
}

impl fmt::Display for BufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BufferError::BufferFull { capacity } => {
                write!(f, "buffer full: capacity of {} packets exceeded", capacity)
            }
            BufferError::BufferEmpty => write!(f, "buffer is empty"),
            BufferError::DuplicatePacket { sequence_number } => {
                write!(f, "duplicate packet: sequence {} is already buffered", sequence_number)
            }
            BufferError::AlreadyProcessed { sequence_number } => write!(
                f,
                "already processed: sequence {} was already delivered or skipped",
                sequence_number
            ),
            BufferError::PacketTooOld { sequence_number, max_age_ms } => write!(
                f,
                "packet too old: sequence {} exceeded max age of {} ms",
                sequence_number, max_age_ms
            ),
            BufferError::InvalidConfiguration(reason) => {
                write!(f, "invalid buffer configuration: {}", reason)
            }
            BufferError::InvalidTargetDelay { requested_ms, min_ms, max_ms } => write!(
                f,
                "invalid target delay: {} ms is outside allowed range [{}, {}] ms",
                requested_ms, min_ms, max_ms
            ),
            BufferError::OperationFailed(reason) => {
                write!(f, "buffer operation failed: {}", reason)
            }
        }
    }
}

impl std::error::Error for BufferError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages_include_relevant_fields() {
        assert!(BufferError::BufferFull { capacity: 10 }.to_string().contains("10"));
        assert!(BufferError::DuplicatePacket { sequence_number: 7 }
            .to_string()
            .contains('7'));
        assert!(BufferError::AlreadyProcessed { sequence_number: 3 }
            .to_string()
            .contains('3'));
        assert!(BufferError::PacketTooOld { sequence_number: 9, max_age_ms: 500 }
            .to_string()
            .contains("500"));
        assert!(BufferError::InvalidTargetDelay { requested_ms: 1, min_ms: 20, max_ms: 400 }
            .to_string()
            .contains('1'));
        assert_eq!(BufferError::BufferEmpty.to_string(), "buffer is empty");
    }
}
