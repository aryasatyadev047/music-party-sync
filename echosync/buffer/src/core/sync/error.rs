//! Error types for the EchoSync Clock Synchronization Engine.
//!
//! Every public function in this module returns `Result<T, SyncError>` so
//! callers (the future Playback Scheduler, and eventually the Transport
//! Layer's sync-message handler) have a single, stable error type to
//! match on.

use std::fmt;

/// Errors that can occur while operating the Clock Synchronization
/// Engine.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncError {
    /// The supplied [`crate::core::sync::config::SyncConfig`] failed
    /// validation. The contained string describes which invariant was
    /// violated.
    InvalidConfiguration(String),

    /// An operation that requires the [`crate::core::sync::synchronizer::Synchronizer`]
    /// to be running was attempted before [`crate::core::sync::synchronizer::Synchronizer::start`]
    /// was called (or after [`crate::core::sync::synchronizer::Synchronizer::stop`]).
    NotStarted,

    /// [`crate::core::sync::synchronizer::Synchronizer::start`] was
    /// called while already running.
    AlreadyStarted,

    /// A calculated or applied offset exceeded the configured maximum
    /// allowed offset.
    OffsetOutOfRange {
        /// The offset that was rejected, in milliseconds.
        offset_ms: f64,
        /// The configured maximum allowed offset, in milliseconds.
        max_offset_ms: f64,
    },

    /// An estimated drift rate exceeded the configured maximum allowed
    /// drift.
    DriftOutOfRange {
        /// The drift rate that was rejected, in parts-per-million.
        drift_ppm: f64,
        /// The configured maximum allowed drift, in parts-per-million.
        max_drift_ppm: f64,
    },

    /// A clock computation (timestamp conversion, playback-time
    /// derivation, etc.) produced an invalid result, such as a negative
    /// playback timestamp.
    ClockError(String),

    /// A synchronization pass failed for a reason not captured by a more
    /// specific variant. The contained string describes the cause.
    SynchronizationFailed(String),

    /// A lower-level operation (e.g. an internal lock) failed in a way
    /// that doesn't fit a more specific variant. The contained string
    /// describes the underlying cause.
    OperationFailed(String),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncError::InvalidConfiguration(reason) => {
                write!(f, "invalid sync configuration: {}", reason)
            }
            SyncError::NotStarted => {
                write!(f, "synchronizer is not started")
            }
            SyncError::AlreadyStarted => {
                write!(f, "synchronizer is already started")
            }
            SyncError::OffsetOutOfRange { offset_ms, max_offset_ms } => write!(
                f,
                "offset out of range: {:.3} ms exceeds maximum allowed offset of {:.3} ms",
                offset_ms, max_offset_ms
            ),
            SyncError::DriftOutOfRange { drift_ppm, max_drift_ppm } => write!(
                f,
                "drift out of range: {:.3} ppm exceeds maximum allowed drift of {:.3} ppm",
                drift_ppm, max_drift_ppm
            ),
            SyncError::ClockError(reason) => write!(f, "clock error: {}", reason),
            SyncError::SynchronizationFailed(reason) => {
                write!(f, "synchronization failed: {}", reason)
            }
            SyncError::OperationFailed(reason) => {
                write!(f, "sync operation failed: {}", reason)
            }
        }
    }
}

impl std::error::Error for SyncError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages_include_relevant_fields() {
        assert!(SyncError::OffsetOutOfRange { offset_ms: 2500.0, max_offset_ms: 2000.0 }
            .to_string()
            .contains("2500"));
        assert!(SyncError::DriftOutOfRange { drift_ppm: 750.0, max_drift_ppm: 500.0 }
            .to_string()
            .contains("750"));
        assert_eq!(SyncError::NotStarted.to_string(), "synchronizer is not started");
        assert_eq!(
            SyncError::AlreadyStarted.to_string(),
            "synchronizer is already started"
        );
        assert!(SyncError::ClockError("negative playback time".into())
            .to_string()
            .contains("negative playback time"));
    }

    #[test]
    fn errors_are_comparable_for_test_assertions() {
        assert_eq!(SyncError::NotStarted, SyncError::NotStarted);
        assert_ne!(SyncError::NotStarted, SyncError::AlreadyStarted);
    }
}
