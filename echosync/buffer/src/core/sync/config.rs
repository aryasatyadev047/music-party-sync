//! Configuration for the EchoSync Clock Synchronization Engine.
//!
//! All tunable values for synchronization cadence, offset/drift limits,
//! and correction behavior live here so the Synchronization Engine's
//! runtime behavior can be adjusted from a single, well-known location —
//! mirroring [`crate::core::buffer::config::BufferConfig`].

use std::time::Duration;

use super::error::SyncError;

/// Default interval, in milliseconds, between synchronization passes.
pub const DEFAULT_SYNC_INTERVAL_MS: u64 = 5_000;

/// Default hard ceiling, in milliseconds, on the magnitude of an offset
/// the engine will accept. Larger offsets are rejected as
/// [`SyncError::OffsetOutOfRange`] rather than applied.
pub const DEFAULT_MAX_ALLOWED_OFFSET_MS: f64 = 2_000.0;

/// Default hard ceiling, in parts-per-million, on the magnitude of an
/// estimated drift rate the engine will accept.
pub const DEFAULT_MAX_DRIFT_PPM: f64 = 500.0;

/// Default assumed clock precision (resolution), in milliseconds.
pub const DEFAULT_CLOCK_PRECISION_MS: u64 = 1;

/// Default fraction (0.0, 1.0] of the remaining offset error corrected
/// on each [`crate::core::sync::clock_manager::ClockManager::correct_drift`]
/// step.
pub const DEFAULT_CORRECTION_RATE: f64 = 0.2;

/// Default hard ceiling, in milliseconds, on how far the applied offset
/// may move in a single correction step, regardless of `correction_rate`.
/// This is what guarantees corrections are always gradual and never an
/// abrupt time jump.
pub const DEFAULT_MAX_CORRECTION_STEP_MS: f64 = 50.0;

/// Default maximum offset error, in milliseconds, below which the clock
/// is considered synchronized.
pub const DEFAULT_RESYNC_THRESHOLD_MS: f64 = 20.0;

/// Default number of recent offset samples retained by the
/// [`crate::core::sync::drift_estimator::DriftEstimator`] for its
/// linear-regression drift estimate.
pub const DEFAULT_DRIFT_WINDOW_SAMPLES: usize = 20;

/// Runtime configuration for a
/// [`crate::core::sync::clock_manager::ClockManager`] /
/// [`crate::core::sync::synchronizer::Synchronizer`].
///
/// Constructed with [`SyncConfig::default`] for sane production
/// defaults, or built explicitly field-by-field for tests and tuning.
/// Always validate untrusted/hand-built configs with
/// [`SyncConfig::validate`] before use — both
/// [`crate::core::sync::clock_manager::ClockManager::new`] and
/// [`crate::core::sync::synchronizer::Synchronizer::new`] do this
/// automatically.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncConfig {
    /// Target interval between synchronization passes. Informational for
    /// this module (no networking/scheduling loop lives here); an
    /// external driver (e.g. the future Transport Layer) is expected to
    /// call [`crate::core::sync::synchronizer::Synchronizer::synchronize`]
    /// roughly this often.
    pub sync_interval: Duration,

    /// Hard ceiling on the magnitude of an offset the engine will
    /// accept. Offsets larger than this are rejected rather than
    /// applied, since they most likely indicate a bad sample rather than
    /// genuine clock skew.
    pub max_allowed_offset: Duration,

    /// Hard ceiling on the magnitude of an estimated drift rate, in
    /// parts-per-million, the engine will accept before flagging it as
    /// out of range.
    pub max_drift_ppm: f64,

    /// Assumed resolution of the underlying clock source. Used as a
    /// lower bound on meaningful offset/drift precision; does not affect
    /// correction math directly.
    pub clock_precision: Duration,

    /// Fraction, in `(0.0, 1.0]`, of the remaining offset error applied
    /// on each correction step. `1.0` would fully close the gap in one
    /// step (bounded by `max_correction_step`); smaller values converge
    /// more smoothly.
    pub correction_rate: f64,

    /// Hard ceiling on how far the applied offset may move in a single
    /// [`crate::core::sync::clock_manager::ClockManager::correct_drift`]
    /// call, regardless of `correction_rate`. Guarantees drift
    /// corrections are always gradual.
    pub max_correction_step: Duration,

    /// Maximum offset error magnitude below which the clock is
    /// considered synchronized
    /// ([`crate::core::sync::clock_manager::ClockManager::is_synchronized`]).
    /// Crossing back above this threshold after having been synchronized
    /// counts as a resynchronization.
    pub resync_threshold: Duration,

    /// Number of recent offset samples the drift estimator retains for
    /// its linear-regression drift estimate. Larger windows smooth out
    /// noise at the cost of slower reaction to genuine drift changes.
    pub drift_window_samples: usize,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            sync_interval: Duration::from_millis(DEFAULT_SYNC_INTERVAL_MS),
            max_allowed_offset: Duration::from_millis(DEFAULT_MAX_ALLOWED_OFFSET_MS as u64),
            max_drift_ppm: DEFAULT_MAX_DRIFT_PPM,
            clock_precision: Duration::from_millis(DEFAULT_CLOCK_PRECISION_MS),
            correction_rate: DEFAULT_CORRECTION_RATE,
            max_correction_step: Duration::from_millis(DEFAULT_MAX_CORRECTION_STEP_MS as u64),
            resync_threshold: Duration::from_millis(DEFAULT_RESYNC_THRESHOLD_MS as u64),
            drift_window_samples: DEFAULT_DRIFT_WINDOW_SAMPLES,
        }
    }
}

impl SyncConfig {
    /// Convenience constructor for tests: short intervals, tight
    /// thresholds, and a small drift window so tests run fast and
    /// deterministically.
    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self {
            sync_interval: Duration::from_millis(50),
            max_allowed_offset: Duration::from_millis(2_000),
            max_drift_ppm: 5_000.0,
            clock_precision: Duration::from_millis(1),
            correction_rate: 0.5,
            max_correction_step: Duration::from_millis(1_000),
            resync_threshold: Duration::from_millis(5),
            drift_window_samples: 5,
        }
    }

    /// Validates internal invariants (non-zero durations, a
    /// `correction_rate` inside `(0.0, 1.0]`, a `resync_threshold` no
    /// larger than `max_allowed_offset`, and a `drift_window_samples` of
    /// at least 2, the minimum needed for a linear regression). Called
    /// automatically by [`crate::core::sync::clock_manager::ClockManager::new`].
    pub fn validate(&self) -> Result<(), SyncError> {
        if self.sync_interval.is_zero() {
            return Err(SyncError::InvalidConfiguration(
                "sync_interval must be greater than zero".into(),
            ));
        }
        if self.max_allowed_offset.is_zero() {
            return Err(SyncError::InvalidConfiguration(
                "max_allowed_offset must be greater than zero".into(),
            ));
        }
        if !(self.max_drift_ppm > 0.0) {
            return Err(SyncError::InvalidConfiguration(
                "max_drift_ppm must be greater than zero".into(),
            ));
        }
        if self.clock_precision.is_zero() {
            return Err(SyncError::InvalidConfiguration(
                "clock_precision must be greater than zero".into(),
            ));
        }
        if !(self.correction_rate > 0.0) || self.correction_rate > 1.0 {
            return Err(SyncError::InvalidConfiguration(
                "correction_rate must fall within (0.0, 1.0]".into(),
            ));
        }
        if self.max_correction_step.is_zero() {
            return Err(SyncError::InvalidConfiguration(
                "max_correction_step must be greater than zero".into(),
            ));
        }
        if self.resync_threshold.is_zero() {
            return Err(SyncError::InvalidConfiguration(
                "resync_threshold must be greater than zero".into(),
            ));
        }
        if self.resync_threshold > self.max_allowed_offset {
            return Err(SyncError::InvalidConfiguration(
                "resync_threshold cannot exceed max_allowed_offset".into(),
            ));
        }
        if self.drift_window_samples < 2 {
            return Err(SyncError::InvalidConfiguration(
                "drift_window_samples must be at least 2".into(),
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
        assert!(SyncConfig::default().validate().is_ok());
    }

    #[test]
    fn for_tests_config_validates() {
        assert!(SyncConfig::for_tests().validate().is_ok());
    }

    #[test]
    fn zero_sync_interval_is_rejected() {
        let mut config = SyncConfig::default();
        config.sync_interval = Duration::ZERO;
        assert!(config.validate().is_err());
    }

    #[test]
    fn zero_max_allowed_offset_is_rejected() {
        let mut config = SyncConfig::default();
        config.max_allowed_offset = Duration::ZERO;
        assert!(config.validate().is_err());
    }

    #[test]
    fn non_positive_max_drift_ppm_is_rejected() {
        let mut config = SyncConfig::default();
        config.max_drift_ppm = 0.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn correction_rate_out_of_bounds_is_rejected() {
        let mut config = SyncConfig::default();
        config.correction_rate = 0.0;
        assert!(config.validate().is_err());

        let mut config = SyncConfig::default();
        config.correction_rate = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn resync_threshold_larger_than_max_offset_is_rejected() {
        let mut config = SyncConfig::default();
        config.resync_threshold = config.max_allowed_offset + Duration::from_millis(1);
        assert!(config.validate().is_err());
    }

    #[test]
    fn drift_window_below_two_is_rejected() {
        let mut config = SyncConfig::default();
        config.drift_window_samples = 1;
        assert!(config.validate().is_err());
    }
}
