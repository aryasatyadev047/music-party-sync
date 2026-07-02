//! Error types for the EchoSync Streaming Engine.
//!
//! Every public function in this module returns `Result<T, StreamingError>`
//! so callers (Media Layer, Transport Layer, playback pipeline) have a
//! single, stable error type to match on.

use std::fmt;

/// Errors that can occur while operating the Streaming Engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamingError {
    /// A queue rejected a packet because it was already at capacity.
    QueueOverflow {
        /// The queue's configured maximum capacity.
        capacity: usize,
    },

    /// An operation was attempted on a queue whose channel has been
    /// closed (e.g. after the engine has fully stopped and torn down its
    /// internal channels).
    QueueClosed,

    /// A packet's encoded payload exceeded the configured maximum size.
    PacketTooLarge {
        /// The configured maximum, in bytes.
        max: usize,
        /// The actual size of the rejected payload, in bytes.
        actual: usize,
    },

    /// An operation that requires the engine to be running was attempted
    /// while it was stopped or paused.
    EngineNotRunning,

    /// `start()` was called on an engine that is already running.
    EngineAlreadyRunning,

    /// Forwarding a packet to the Transport Layer failed, even after
    /// exhausting the configured retry budget. The contained string
    /// describes the underlying cause.
    SendFailed(String),

    /// A blocking operation (e.g. waiting for space in a full queue, or
    /// waiting for a packet to arrive) exceeded the configured timeout.
    OperationTimedOut,

    /// The engine was asked to perform an operation that is invalid for
    /// its current lifecycle state (e.g. `pause()` while already stopped).
    InvalidState(String),

    /// The Transport Layer sink is unavailable (its receiving half has
    /// been dropped), so packets can no longer be handed off.
    TransportUnavailable,
}

impl fmt::Display for StreamingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamingError::QueueOverflow { capacity } => {
                write!(f, "queue overflow: capacity of {} packets exceeded", capacity)
            }
            StreamingError::QueueClosed => write!(f, "queue channel is closed"),
            StreamingError::PacketTooLarge { max, actual } => write!(
                f,
                "packet too large: max allowed {} bytes, got {} bytes",
                max, actual
            ),
            StreamingError::EngineNotRunning => {
                write!(f, "operation requires the streaming engine to be running")
            }
            StreamingError::EngineAlreadyRunning => {
                write!(f, "streaming engine is already running")
            }
            StreamingError::SendFailed(reason) => {
                write!(f, "failed to send packet to transport layer: {}", reason)
            }
            StreamingError::OperationTimedOut => write!(f, "operation timed out"),
            StreamingError::InvalidState(reason) => {
                write!(f, "invalid engine state transition: {}", reason)
            }
            StreamingError::TransportUnavailable => {
                write!(f, "transport layer sink is unavailable")
            }
        }
    }
}

impl std::error::Error for StreamingError {}
