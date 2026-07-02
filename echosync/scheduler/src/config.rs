//! Configuration for the EchoSync Playback Scheduler.
//!
//! All tunable values for release timing, tolerance windows, and queue
//! sizing live here so the scheduler's runtime behavior can be adjusted
//! from a single, well-known location — mirroring
//! [`buffer::BufferConfig`] and [`buffer::SyncConfig`].

use std::time::Duration;

use crate::error::SchedulerError;

/// Default playback latency, in milliseconds: how far in the future
/// (relative to a packet's expected playback time) release is targeted,
/// giving downstream decode/output stages headroom.
pub const DEFAULT_PLAYBACK_LATENCY_MS: u64 = 150;

/// Default maximum lateness, in milliseconds, tolerated before a packet
/// is dropped instead of scheduled.
pub const DEFAULT_MAX_LATE_THRESHOLD_MS: u64 = 200;

/// Default maximum earliness, in milliseconds, before a packet is
/// counted as "early" in statistics (it is still scheduled and queued
/// either way).
pub const DEFAULT_MAX_EARLY_THRESHOLD_MS: u64 = 500;

/// Default interval, in milliseconds, at which an external driver (the
/// future Audio Output module) is expected to poll
/// [`crate::scheduler::PlaybackScheduler::next_packet`].
pub const DEFAULT_SCHEDULING_INTERVAL_MS: u64 = 10;

/// Default maximum number of packets the playback queue will hold at
/// once.
pub const DEFAULT_QUEUE_CAPACITY: usize = 512;

/// Default granularity, in milliseconds, at which
/// [`crate::timeline::PlaybackTimeline`] quantizes elapsed time reads.
pub const DEFAULT_TIMELINE_RESOLUTION_MS: u64 = 1;

/// Runtime configuration for a
/// [`crate::scheduler::PlaybackScheduler`].
///
/// Constructed with [`SchedulerConfig::default`] for sane production
/// defaults, or built explicitly field-by-field for tests and tuning.
/// Always validate untrusted/hand-built configs with
/// [`SchedulerConfig::validate`] before use —
/// [`crate::scheduler::PlaybackScheduler::new`] does this automatically.
#[derive(Debug, Clone, PartialEq)]
pub struct SchedulerConfig {
    /// How far in the future (relative to a packet's expected playback
    /// time on the shared timeline) release is targeted. Gives
    /// downstream decode/output stages headroom to do their work before
    /// the audio actually needs to sound.
    pub playback_latency: Duration,

    /// Maximum amount of time a packet's deadline may already have
    /// passed, at the moment it is scheduled, before it is dropped
    /// instead of queued.
    pub max_late_threshold: Duration,

    /// Maximum amount of time a packet's deadline may sit in the future,
    /// at the moment it is scheduled, before it is counted as "early" in
    /// statistics. The packet is queued regardless; this only affects
    /// bookkeeping.
    pub max_early_threshold: Duration,

    /// Target interval between polls of
    /// [`crate::scheduler::PlaybackScheduler::next_packet`] by an
    /// external driver. Informational for this module (no internal
    /// polling loop lives here, matching the pull-based pattern used by
    /// [`buffer::BufferManager::pop_packet`]).
    pub scheduling_interval: Duration,

    /// Maximum number of packets the playback queue will hold at once.
    /// [`crate::scheduler::PlaybackScheduler::schedule_packet`] returns
    /// [`SchedulerError::QueueFull`] once this is reached.
    pub queue_capacity: usize,

    /// Granularity at which [`crate::timeline::PlaybackTimeline`]
    /// quantizes elapsed-time reads. Smaller values give finer-grained
    /// deadlines at the cost of more frequent recomputation.
    pub timeline_resolution: Duration,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            playback_latency: Duration::from_millis(DEFAULT_PLAYBACK_LATENCY_MS),
            max_late_threshold: Duration::from_millis(DEFAULT_MAX_LATE_THRESHOLD_MS),
            max_early_threshold: Duration::from_millis(DEFAULT_MAX_EARLY_THRESHOLD_MS),
            scheduling_interval: Duration::from_millis(DEFAULT_SCHEDULING_INTERVAL_MS),
            queue_capacity: DEFAULT_QUEUE_CAPACITY,
            timeline_resolution: Duration::from_millis(DEFAULT_TIMELINE_RESOLUTION_MS),
        }
    }
}

impl SchedulerConfig {
    /// Convenience constructor for tests: short latencies/thresholds and
    /// a small queue so tests run fast and deterministically.
    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self {
            playback_latency: Duration::from_millis(20),
            max_late_threshold: Duration::from_millis(50),
            max_early_threshold: Duration::from_millis(100),
            scheduling_interval: Duration::from_millis(5),
            queue_capacity: 16,
            timeline_resolution: Duration::from_millis(1),
        }
    }

    /// Validates internal invariants (non-zero durations and a non-zero
    /// queue capacity). Called automatically by
    /// [`crate::scheduler::PlaybackScheduler::new`].
    pub fn validate(&self) -> Result<(), SchedulerError> {
        if self.max_late_threshold.is_zero() {
            return Err(SchedulerError::InvalidConfiguration(
                "max_late_threshold must be greater than zero".into(),
            ));
        }
        if self.max_early_threshold.is_zero() {
            return Err(SchedulerError::InvalidConfiguration(
                "max_early_threshold must be greater than zero".into(),
            ));
        }
        if self.scheduling_interval.is_zero() {
            return Err(SchedulerError::InvalidConfiguration(
                "scheduling_interval must be greater than zero".into(),
            ));
        }
        if self.queue_capacity == 0 {
            return Err(SchedulerError::InvalidConfiguration(
                "queue_capacity must be greater than zero".into(),
            ));
        }
        if self.timeline_resolution.is_zero() {
            return Err(SchedulerError::InvalidConfiguration(
                "timeline_resolution must be greater than zero".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_validates() {
        assert!(SchedulerConfig::default().validate().is_ok());
    }

    #[test]
    fn for_tests_config_validates() {
        assert!(SchedulerConfig::for_tests().validate().is_ok());
    }

    #[test]
    fn zero_max_late_threshold_is_rejected() {
        let mut config = SchedulerConfig::default();
        config.max_late_threshold = Duration::ZERO;
        assert!(config.validate().is_err());
    }

    #[test]
    fn zero_max_early_threshold_is_rejected() {
        let mut config = SchedulerConfig::default();
        config.max_early_threshold = Duration::ZERO;
        assert!(config.validate().is_err());
    }

    #[test]
    fn zero_scheduling_interval_is_rejected() {
        let mut config = SchedulerConfig::default();
        config.scheduling_interval = Duration::ZERO;
        assert!(config.validate().is_err());
    }

    #[test]
    fn zero_queue_capacity_is_rejected() {
        let mut config = SchedulerConfig::default();
        config.queue_capacity = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn zero_timeline_resolution_is_rejected() {
        let mut config = SchedulerConfig::default();
        config.timeline_resolution = Duration::ZERO;
        assert!(config.validate().is_err());
    }
}
