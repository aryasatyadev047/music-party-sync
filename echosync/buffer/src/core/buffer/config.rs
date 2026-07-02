//! Configuration for the EchoSync Buffer Layer.
//!
//! All tunable values for buffer sizing, target delay, staleness, and
//! duplicate/loss detection live here so the Buffer Layer's runtime
//! behavior can be adjusted from a single, well-known location.

use std::time::Duration;

use super::error::BufferError;

/// Default number of packet slots the buffer is pre-sized to hold.
pub const DEFAULT_INITIAL_BUFFER_SIZE: usize = 32;

/// Default hard ceiling on the number of packets the buffer may hold
/// before `push_packet` starts reclaiming space by dropping the oldest
/// buffered packet (a buffer overflow).
pub const DEFAULT_MAX_BUFFER_SIZE: usize = 512;

/// Default target buffering delay, in milliseconds: how long a packet is
/// held before it becomes eligible for release. This is the primary
/// knob that absorbs network jitter.
pub const DEFAULT_TARGET_DELAY_MS: u64 = 60;

/// Default lower bound the adaptive algorithm will not shrink the target
/// delay below.
pub const DEFAULT_MIN_TARGET_DELAY_MS: u64 = 20;

/// Default upper bound the adaptive algorithm will not grow the target
/// delay past.
pub const DEFAULT_MAX_TARGET_DELAY_MS: u64 = 400;

/// Default maximum age, in milliseconds, a packet may sit in the buffer
/// before it is considered too stale to deliver and is dropped outright.
pub const DEFAULT_MAX_PACKET_AGE_MS: u64 = 1000;

/// Default number of recently-delivered sequence numbers retained so
/// late duplicate arrivals (of packets already handed to the consumer)
/// can still be detected and rejected.
pub const DEFAULT_DUPLICATE_CACHE_SIZE: usize = 256;

/// Default number of consecutive missing sequence numbers tolerated
/// before a gap is flagged as probable packet loss in statistics.
pub const DEFAULT_MAX_MISSING_PACKETS: u64 = 50;

/// Default step, in milliseconds, by which the adaptive algorithm nudges
/// the target delay up or down per packet arrival.
pub const DEFAULT_ADAPTIVE_STEP_MS: u64 = 5;

/// Runtime configuration for a
/// [`crate::core::buffer::buffer_manager::BufferManager`] /
/// [`crate::core::buffer::jitter_buffer::JitterBuffer`].
///
/// Constructed with [`BufferConfig::default`] for sane production
/// defaults, or built explicitly field-by-field for tests and tuning.
/// Always validate untrusted/hand-built configs with
/// [`BufferConfig::validate`] before use — both
/// [`crate::core::buffer::buffer_manager::BufferManager::new`] and
/// [`crate::core::buffer::jitter_buffer::JitterBuffer::new`] do this
/// automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferConfig {
    /// Number of packet slots the buffer is pre-sized to hold. Purely
    /// advisory (the underlying storage grows on demand up to
    /// `max_buffer_size`); kept as an explicit, documented tunable since
    /// it's a natural capacity-planning knob.
    pub initial_buffer_size: usize,

    /// Hard ceiling on the number of packets the buffer may hold before
    /// the oldest buffered packet is dropped to make room (overflow).
    pub max_buffer_size: usize,

    /// How long a packet is held before it becomes eligible for release
    /// to the consumer. Absorbs network jitter.
    pub target_delay: Duration,

    /// Lower bound the adaptive algorithm will not shrink `target_delay`
    /// below, and the floor for [`crate::core::buffer::jitter_buffer::JitterBuffer::set_target_delay`].
    pub min_target_delay: Duration,

    /// Upper bound the adaptive algorithm will not grow `target_delay`
    /// past, and the ceiling for [`crate::core::buffer::jitter_buffer::JitterBuffer::set_target_delay`].
    pub max_target_delay: Duration,

    /// Maximum age a buffered packet may reach before it is dropped as
    /// stale, regardless of `target_delay`.
    pub max_packet_age: Duration,

    /// Number of recently-delivered sequence numbers retained for late
    /// duplicate detection.
    pub duplicate_cache_size: usize,

    /// Number of consecutive missing sequence numbers tolerated before a
    /// gap is flagged as probable packet loss in statistics.
    pub max_missing_packets: u64,

    /// Whether the jitter buffer is allowed to automatically adjust
    /// `target_delay` at runtime based on observed inter-arrival jitter.
    pub adaptive_enabled: bool,

    /// Step size used by the adaptive algorithm when nudging
    /// `target_delay` up or down.
    pub adaptive_step: Duration,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            initial_buffer_size: DEFAULT_INITIAL_BUFFER_SIZE,
            max_buffer_size: DEFAULT_MAX_BUFFER_SIZE,
            target_delay: Duration::from_millis(DEFAULT_TARGET_DELAY_MS),
            min_target_delay: Duration::from_millis(DEFAULT_MIN_TARGET_DELAY_MS),
            max_target_delay: Duration::from_millis(DEFAULT_MAX_TARGET_DELAY_MS),
            max_packet_age: Duration::from_millis(DEFAULT_MAX_PACKET_AGE_MS),
            duplicate_cache_size: DEFAULT_DUPLICATE_CACHE_SIZE,
            max_missing_packets: DEFAULT_MAX_MISSING_PACKETS,
            adaptive_enabled: true,
            adaptive_step: Duration::from_millis(DEFAULT_ADAPTIVE_STEP_MS),
        }
    }
}

impl BufferConfig {
    /// Convenience constructor for tests: small buffers, short delays,
    /// and a small duplicate cache so tests run fast and deterministically.
    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self {
            initial_buffer_size: 4,
            max_buffer_size: 16,
            target_delay: Duration::from_millis(20),
            min_target_delay: Duration::from_millis(5),
            max_target_delay: Duration::from_millis(100),
            max_packet_age: Duration::from_millis(200),
            duplicate_cache_size: 32,
            max_missing_packets: 5,
            adaptive_enabled: true,
            adaptive_step: Duration::from_millis(2),
        }
    }

    /// Validates internal invariants (non-zero sizes, well-ordered delay
    /// bounds, `target_delay` inside `[min_target_delay,
    /// max_target_delay]`). Called automatically by
    /// [`crate::core::buffer::jitter_buffer::JitterBuffer::new`].
    pub fn validate(&self) -> Result<(), BufferError> {
        if self.max_buffer_size == 0 {
            return Err(BufferError::InvalidConfiguration(
                "max_buffer_size must be greater than zero".into(),
            ));
        }
        if self.initial_buffer_size > self.max_buffer_size {
            return Err(BufferError::InvalidConfiguration(
                "initial_buffer_size cannot exceed max_buffer_size".into(),
            ));
        }
        if self.min_target_delay > self.max_target_delay {
            return Err(BufferError::InvalidConfiguration(
                "min_target_delay cannot exceed max_target_delay".into(),
            ));
        }
        if self.target_delay < self.min_target_delay || self.target_delay > self.max_target_delay
        {
            return Err(BufferError::InvalidConfiguration(
                "target_delay must fall within [min_target_delay, max_target_delay]".into(),
            ));
        }
        if self.duplicate_cache_size == 0 {
            return Err(BufferError::InvalidConfiguration(
                "duplicate_cache_size must be greater than zero".into(),
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
        assert!(BufferConfig::default().validate().is_ok());
    }

    #[test]
    fn for_tests_config_validates() {
        assert!(BufferConfig::for_tests().validate().is_ok());
    }

    #[test]
    fn zero_max_buffer_size_is_rejected() {
        let mut config = BufferConfig::default();
        config.max_buffer_size = 0;
        assert_eq!(
            config.validate(),
            Err(BufferError::InvalidConfiguration(
                "max_buffer_size must be greater than zero".into()
            ))
        );
    }

    #[test]
    fn initial_larger_than_max_is_rejected() {
        let mut config = BufferConfig::default();
        config.initial_buffer_size = config.max_buffer_size + 1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn inverted_delay_bounds_are_rejected() {
        let mut config = BufferConfig::default();
        config.min_target_delay = Duration::from_millis(500);
        config.max_target_delay = Duration::from_millis(100);
        assert!(config.validate().is_err());
    }

    #[test]
    fn target_delay_outside_bounds_is_rejected() {
        let mut config = BufferConfig::default();
        config.target_delay = config.max_target_delay + Duration::from_millis(1);
        assert!(config.validate().is_err());
    }

    #[test]
    fn zero_duplicate_cache_is_rejected() {
        let mut config = BufferConfig::default();
        config.duplicate_cache_size = 0;
        assert!(config.validate().is_err());
    }
}
