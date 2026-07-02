//! Error types for the EchoSync Playback Scheduler.
//!
//! Every public function in this module returns `Result<T,
//! SchedulerError>` so callers (the future Audio Output module) have a
//! single, stable error type to match on — the same pattern used by
//! [`buffer::BufferError`] and [`buffer::SyncError`].

use std::fmt;

use buffer::SyncError;

/// Errors that can occur while operating the Playback Scheduler.
#[derive(Debug, Clone, PartialEq)]
pub enum SchedulerError {
    /// The supplied [`crate::config::SchedulerConfig`] failed
    /// validation. The contained string describes which invariant was
    /// violated.
    InvalidConfiguration(String),

    /// An operation that requires the [`crate::scheduler::PlaybackScheduler`]
    /// to be running was attempted before
    /// [`crate::scheduler::PlaybackScheduler::start`] was called (or
    /// after [`crate::scheduler::PlaybackScheduler::stop`]).
    NotStarted,

    /// [`crate::scheduler::PlaybackScheduler::start`] was called while
    /// already running.
    AlreadyStarted,

    /// [`crate::scheduler::PlaybackScheduler::schedule_packet`] was
    /// called with a packet whose `packet_id` is already queued and
    /// has not yet been released or cancelled.
    DuplicatePacket {
        /// The `packet_id` that was already present in the queue.
        packet_id: u64,
    },

    /// The playback queue is at [`crate::config::SchedulerConfig::queue_capacity`]
    /// and cannot accept another packet.
    QueueFull {
        /// The configured capacity that was reached.
        capacity: usize,
    },

    /// A packet's playback deadline had already passed by more than
    /// [`crate::config::SchedulerConfig::max_late_threshold`] at the
    /// time it was scheduled, so it was dropped rather than queued.
    PacketExpired {
        /// The `packet_id` of the dropped packet.
        packet_id: u64,
        /// How far past its deadline the packet was, in milliseconds.
        late_by_ms: u64,
    },

    /// [`crate::scheduler::PlaybackScheduler::cancel_packet`] was
    /// called with a `packet_id` that is not currently queued.
    PacketNotFound {
        /// The `packet_id` that could not be located.
        packet_id: u64,
    },

    /// A lower-level operation (e.g. an internal lock or a call into
    /// the Clock Synchronization Engine) failed in a way that doesn't
    /// fit a more specific variant.
    SyncError(SyncError),

    /// A scheduling computation (timeline arithmetic, deadline
    /// derivation, etc.) produced an invalid result.
    OperationFailed(String),
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchedulerError::InvalidConfiguration(reason) => {
                write!(f, "invalid scheduler configuration: {}", reason)
            }
            SchedulerError::NotStarted => write!(f, "scheduler is not started"),
            SchedulerError::AlreadyStarted => write!(f, "scheduler is already started"),
            SchedulerError::DuplicatePacket { packet_id } => {
                write!(f, "packet {} is already scheduled", packet_id)
            }
            SchedulerError::QueueFull { capacity } => {
                write!(f, "playback queue is full (capacity: {})", capacity)
            }
            SchedulerError::PacketExpired { packet_id, late_by_ms } => write!(
                f,
                "packet {} expired: {} ms past the maximum late threshold",
                packet_id, late_by_ms
            ),
            SchedulerError::PacketNotFound { packet_id } => {
                write!(f, "packet {} is not currently scheduled", packet_id)
            }
            SchedulerError::SyncError(err) => write!(f, "clock synchronization error: {}", err),
            SchedulerError::OperationFailed(reason) => {
                write!(f, "scheduler operation failed: {}", reason)
            }
        }
    }
}

impl std::error::Error for SchedulerError {}

impl From<SyncError> for SchedulerError {
    fn from(err: SyncError) -> Self {
        SchedulerError::SyncError(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages_include_relevant_fields() {
        assert!(SchedulerError::DuplicatePacket { packet_id: 42 }
            .to_string()
            .contains("42"));
        assert!(SchedulerError::QueueFull { capacity: 10 }.to_string().contains("10"));
        assert!(SchedulerError::PacketExpired { packet_id: 7, late_by_ms: 500 }
            .to_string()
            .contains("500"));
        assert!(SchedulerError::PacketNotFound { packet_id: 3 }.to_string().contains("3"));
        assert_eq!(SchedulerError::NotStarted.to_string(), "scheduler is not started");
        assert_eq!(
            SchedulerError::AlreadyStarted.to_string(),
            "scheduler is already started"
        );
    }

    #[test]
    fn errors_are_comparable_for_test_assertions() {
        assert_eq!(SchedulerError::NotStarted, SchedulerError::NotStarted);
        assert_ne!(SchedulerError::NotStarted, SchedulerError::AlreadyStarted);
    }

    #[test]
    fn sync_error_converts_via_from() {
        let err: SchedulerError = SyncError::NotStarted.into();
        assert_eq!(err, SchedulerError::SyncError(SyncError::NotStarted));
    }
}
