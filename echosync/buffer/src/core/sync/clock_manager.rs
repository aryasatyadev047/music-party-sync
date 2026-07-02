//! The single-threaded core of the Clock Synchronization Engine.
//!
//! `ClockManager` owns the local monotonic clock, the currently applied
//! host-clock offset, drift estimation, and synchronization statistics.
//! It has no internal locking or `async` methods — concurrent,
//! multi-reader / multi-writer access is layered on top by
//! [`crate::core::sync::synchronizer::Synchronizer`], which wraps a
//! `ClockManager` in a `tokio::sync::RwLock`. This mirrors the
//! [`crate::core::buffer::jitter_buffer::JitterBuffer`] /
//! [`crate::core::buffer::buffer_manager::BufferManager`] split in the
//! Buffer Layer.
//!
//! ## Clock model
//! - **Local Clock**: this device's own monotonic clock, exposed by
//!   [`ClockManager::local_time`]. Measured as elapsed time since the
//!   `ClockManager` was created (or last [`ClockManager::reset`]).
//! - **Host Clock**: the reference/master device's clock, as estimated
//!   from this device's perspective. There is no networking in this
//!   module — callers obtain host timestamps out-of-band (e.g. from a
//!   future Transport Layer sync message) and hand them to
//!   [`ClockManager::calculate_offset`].
//! - **Playback Clock**: the common timeline all devices render audio
//!   against. Exposed by [`ClockManager::host_time`], which is simply
//!   `local_time() + current applied offset` — the local clock projected
//!   onto the host's timeline. The future Playback Scheduler consumes
//!   this value directly.

use std::time::{Duration, Instant};

use tracing::{debug, info, trace, warn};

use super::config::SyncConfig;
use super::drift_estimator::DriftEstimator;
use super::error::SyncError;
use super::statistics::{StatisticsTracker, SyncStatistics};

/// Converts a [`Duration`] to milliseconds as `f64`, preserving
/// sub-millisecond precision.
pub fn duration_to_millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// Converts a non-negative millisecond value to a [`Duration`]. Returns
/// [`SyncError::ClockError`] if `millis` is negative or not finite.
pub fn millis_to_duration(millis: f64) -> Result<Duration, SyncError> {
    if !millis.is_finite() || millis < 0.0 {
        return Err(SyncError::ClockError(format!(
            "cannot convert {} ms to a Duration: value must be finite and non-negative",
            millis
        )));
    }
    Ok(Duration::from_secs_f64(millis / 1000.0))
}

/// Owns the local monotonic clock, the applied host-clock offset, drift
/// estimation, and synchronization statistics for a single device.
///
/// See the [module docs](self) for the clock model. All methods are
/// synchronous and take `&mut self` (or `&self` for pure reads); wrap in
/// [`crate::core::sync::synchronizer::Synchronizer`] for safe concurrent
/// access from multiple Tokio tasks.
pub struct ClockManager {
    config: SyncConfig,
    /// Reference instant `local_time()` is measured from.
    epoch: Instant,
    /// The offset currently applied to derive `host_time()` from
    /// `local_time()`, in milliseconds. Moves gradually toward
    /// `target_offset_ms` via [`ClockManager::correct_drift`].
    current_offset_ms: f64,
    /// The most recent raw offset measurement from
    /// [`ClockManager::calculate_offset`] /
    /// [`ClockManager::apply_offset`], in milliseconds. What
    /// `current_offset_ms` is gradually corrected toward.
    target_offset_ms: f64,
    /// The most recently estimated drift rate, in parts-per-million.
    current_drift_ppm: f64,
    /// Whether at least one offset has ever been applied.
    has_synced: bool,
    drift_estimator: DriftEstimator,
    stats: StatisticsTracker,
}

impl ClockManager {
    /// Creates a new `ClockManager` with its local clock epoch starting
    /// now. Returns [`SyncError::InvalidConfiguration`] if `config` fails
    /// [`SyncConfig::validate`].
    pub fn new(config: SyncConfig) -> Result<Self, SyncError> {
        config.validate()?;
        let drift_estimator = DriftEstimator::new(config.drift_window_samples);

        Ok(Self {
            config,
            epoch: Instant::now(),
            current_offset_ms: 0.0,
            target_offset_ms: 0.0,
            current_drift_ppm: 0.0,
            has_synced: false,
            drift_estimator,
            stats: StatisticsTracker::new(),
        })
    }

    /// Returns this device's local monotonic time: elapsed time since
    /// this `ClockManager` was created or last [`ClockManager::reset`].
    pub fn local_time(&self) -> Result<Duration, SyncError> {
        Ok(self.epoch.elapsed())
    }

    /// Returns the current estimate of the shared playback timeline:
    /// `local_time()` projected onto the host's clock via the currently
    /// applied offset. Returns [`SyncError::ClockError`] if the result
    /// would be negative (the offset would push the timeline before the
    /// `ClockManager`'s epoch).
    pub fn host_time(&self) -> Result<Duration, SyncError> {
        let local_ms = duration_to_millis(self.local_time()?);
        let playback_ms = local_ms + self.current_offset_ms;
        millis_to_duration(playback_ms).map_err(|_| {
            SyncError::ClockError(format!(
                "playback time would be negative: local={:.3}ms + offset={:.3}ms",
                local_ms, self.current_offset_ms
            ))
        })
    }

    /// Computes the raw offset, in milliseconds, between a host-clock
    /// timestamp and the local-clock timestamp it was paired with
    /// (positive means the host clock leads the local clock). Does not
    /// apply the offset; call [`ClockManager::apply_offset`] with the
    /// result to do that. Returns [`SyncError::OffsetOutOfRange`] if the
    /// magnitude exceeds `config.max_allowed_offset`.
    pub fn calculate_offset(
        &self,
        host_timestamp: Duration,
        local_timestamp: Duration,
    ) -> Result<f64, SyncError> {
        let offset_ms = duration_to_millis(host_timestamp) - duration_to_millis(local_timestamp);
        let max_offset_ms = duration_to_millis(self.config.max_allowed_offset);

        trace!(offset_ms, "Offset Calculated");

        if offset_ms.abs() > max_offset_ms {
            return Err(SyncError::OffsetOutOfRange { offset_ms, max_offset_ms });
        }
        Ok(offset_ms)
    }

    /// Sets the correction target to `offset_ms`, records it for drift
    /// estimation and statistics, and — on the very first call —
    /// initializes the applied offset directly, since there is no prior
    /// state to jump away from. Every subsequent call only moves the
    /// *target*; [`ClockManager::correct_drift`] is what gradually moves
    /// the applied offset toward it, so playback timing never jumps.
    ///
    /// Returns [`SyncError::OffsetOutOfRange`] if `offset_ms` exceeds
    /// `config.max_allowed_offset`.
    pub fn apply_offset(&mut self, offset_ms: f64) -> Result<(), SyncError> {
        let max_offset_ms = duration_to_millis(self.config.max_allowed_offset);
        if offset_ms.abs() > max_offset_ms {
            return Err(SyncError::OffsetOutOfRange { offset_ms, max_offset_ms });
        }

        self.target_offset_ms = offset_ms;
        if !self.has_synced {
            // Nothing to converge away from yet: seed the applied offset
            // directly so the very first sync doesn't wait for repeated
            // correct_drift() calls to catch up from zero.
            self.current_offset_ms = offset_ms;
            self.has_synced = true;
        }

        let elapsed_secs = self.local_time()?.as_secs_f64();
        self.drift_estimator.record_sample(elapsed_secs, offset_ms);

        self.stats.record_offset(offset_ms);
        self.stats.record_accuracy((self.target_offset_ms - self.current_offset_ms).abs());

        debug!(offset_ms, "Clock Updated");
        Ok(())
    }

    /// Re-estimates the drift rate from recently recorded offset samples
    /// and stores it as [`ClockManager::current_drift`]. Returns
    /// [`SyncError::DriftOutOfRange`] if the magnitude exceeds
    /// `config.max_drift_ppm` (the estimate is still recorded before the
    /// error is returned, so statistics stay accurate).
    pub fn estimate_drift(&mut self) -> Result<f64, SyncError> {
        let drift_ppm = self.drift_estimator.estimate_ppm();
        self.current_drift_ppm = drift_ppm;
        self.stats.record_drift(drift_ppm);

        debug!(drift_ppm, "Drift Estimated");

        if drift_ppm.abs() > self.config.max_drift_ppm {
            return Err(SyncError::DriftOutOfRange {
                drift_ppm,
                max_drift_ppm: self.config.max_drift_ppm,
            });
        }
        Ok(drift_ppm)
    }

    /// Applies one gradual correction step, nudging the applied offset a
    /// bounded amount toward the current target offset. The step size is
    /// `min(|target - current| * correction_rate, max_correction_step)`,
    /// so the applied offset — and therefore [`ClockManager::host_time`]
    /// — never jumps abruptly, regardless of how large the target offset
    /// is. Returns the signed step actually applied, in milliseconds
    /// (`0.0` if already converged).
    pub fn correct_drift(&mut self) -> Result<f64, SyncError> {
        let delta_ms = self.target_offset_ms - self.current_offset_ms;
        if delta_ms == 0.0 {
            return Ok(0.0);
        }

        let max_step_ms = duration_to_millis(self.config.max_correction_step);
        let raw_step_ms = delta_ms * self.config.correction_rate;
        let step_ms = raw_step_ms.clamp(-max_step_ms, max_step_ms);

        let was_synchronized = self.is_synchronized();
        self.current_offset_ms += step_ms;
        self.stats.record_correction();

        let accuracy_ms = (self.target_offset_ms - self.current_offset_ms).abs();
        self.stats.record_accuracy(accuracy_ms);

        if was_synchronized && !self.is_synchronized() {
            self.stats.record_resync();
        }

        debug!(step_ms, current_offset_ms = self.current_offset_ms, "Drift Corrected");
        Ok(step_ms)
    }

    /// Returns whether the applied offset is currently within
    /// `config.resync_threshold` of the target offset. `false` before
    /// any offset has ever been applied.
    pub fn is_synchronized(&self) -> bool {
        if !self.has_synced {
            return false;
        }
        let resync_threshold_ms = duration_to_millis(self.config.resync_threshold);
        (self.target_offset_ms - self.current_offset_ms).abs() <= resync_threshold_ms
    }

    /// The currently applied offset, in milliseconds.
    pub fn current_offset(&self) -> f64 {
        self.current_offset_ms
    }

    /// The most recently estimated drift rate, in parts-per-million.
    pub fn current_drift(&self) -> f64 {
        self.current_drift_ppm
    }

    /// Records that a synchronization pass completed successfully.
    pub fn record_success(&mut self) -> Result<(), SyncError> {
        self.stats.record_success();
        info!("Synchronization Success");
        Ok(())
    }

    /// Records that a synchronization pass failed.
    pub fn record_failure(&mut self) -> Result<(), SyncError> {
        self.stats.record_failure();
        warn!("Synchronization Failure");
        Ok(())
    }

    /// Returns a snapshot of current synchronization statistics.
    pub fn statistics(&self) -> Result<SyncStatistics, SyncError> {
        Ok(self.stats.snapshot())
    }

    /// Resets the clock manager to its initial state: a fresh local
    /// epoch, zeroed offset/drift, cleared drift-estimation history, and
    /// zeroed statistics.
    pub fn reset(&mut self) -> Result<(), SyncError> {
        self.epoch = Instant::now();
        self.current_offset_ms = 0.0;
        self.target_offset_ms = 0.0;
        self.current_drift_ppm = 0.0;
        self.has_synced = false;
        self.drift_estimator.reset();
        self.stats.reset();
        info!("Clock Reset");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> ClockManager {
        ClockManager::new(SyncConfig::for_tests()).unwrap()
    }

    #[test]
    fn new_manager_starts_unsynchronized_at_zero_offset() {
        let clock = manager();
        assert_eq!(clock.current_offset(), 0.0);
        assert_eq!(clock.current_drift(), 0.0);
        assert!(!clock.is_synchronized());
    }

    #[test]
    fn local_time_is_monotonic() {
        let clock = manager();
        let first = clock.local_time().unwrap();
        std::thread::sleep(Duration::from_millis(5));
        let second = clock.local_time().unwrap();
        assert!(second >= first);
    }

    #[test]
    fn host_time_reflects_applied_offset() {
        let mut clock = manager();
        clock.apply_offset(100.0).unwrap();
        let local_ms = duration_to_millis(clock.local_time().unwrap());
        let host_ms = duration_to_millis(clock.host_time().unwrap());
        // First apply_offset seeds current_offset_ms directly, so
        // host_time should lead local_time by ~100ms.
        assert!((host_ms - local_ms - 100.0).abs() < 5.0);
    }

    #[test]
    fn host_time_errors_on_negative_result() {
        let mut clock = manager();
        // Force a large negative offset that would push host_time below
        // zero at the very start of the epoch.
        clock.apply_offset(-1_500.0).unwrap();
        assert!(matches!(clock.host_time(), Err(SyncError::ClockError(_))));
    }

    #[test]
    fn calculate_offset_computes_signed_difference() {
        let clock = manager();
        let offset = clock
            .calculate_offset(Duration::from_millis(150), Duration::from_millis(100))
            .unwrap();
        assert!((offset - 50.0).abs() < 1e-9);

        let offset = clock
            .calculate_offset(Duration::from_millis(100), Duration::from_millis(150))
            .unwrap();
        assert!((offset - (-50.0)).abs() < 1e-9);
    }

    #[test]
    fn calculate_offset_rejects_excessive_magnitude() {
        let clock = manager();
        let result = clock.calculate_offset(Duration::from_secs(10), Duration::ZERO);
        assert!(matches!(result, Err(SyncError::OffsetOutOfRange { .. })));
    }

    #[test]
    fn apply_offset_rejects_excessive_magnitude() {
        let mut clock = manager();
        let result = clock.apply_offset(1_000_000.0);
        assert!(matches!(result, Err(SyncError::OffsetOutOfRange { .. })));
    }

    #[test]
    fn first_apply_offset_seeds_current_offset_directly() {
        let mut clock = manager();
        clock.apply_offset(75.0).unwrap();
        assert_eq!(clock.current_offset(), 75.0);
        assert!(clock.is_synchronized());
    }

    #[test]
    fn correct_drift_moves_gradually_not_abruptly() {
        let mut config = SyncConfig::for_tests();
        config.correction_rate = 0.1;
        config.max_correction_step = Duration::from_millis(1_000);
        let mut clock = ClockManager::new(config).unwrap();

        // Seed a baseline offset, then feed in a larger target without
        // re-seeding (simulate a second, different measurement) by
        // manipulating target directly through apply_offset again.
        clock.apply_offset(0.0).unwrap();
        clock.target_offset_ms = 1000.0; // simulate a fresh raw measurement

        let step = clock.correct_drift().unwrap();
        // 10% of the 1000ms gap = 100ms, far less than an abrupt jump.
        assert!((step - 100.0).abs() < 1e-9);
        assert_eq!(clock.current_offset(), 100.0);
    }

    #[test]
    fn correct_drift_never_exceeds_max_correction_step() {
        let mut config = SyncConfig::for_tests();
        config.correction_rate = 1.0;
        config.max_correction_step = Duration::from_millis(10);
        let mut clock = ClockManager::new(config).unwrap();

        clock.apply_offset(0.0).unwrap();
        clock.target_offset_ms = 5_000.0;

        let step = clock.correct_drift().unwrap();
        assert!(step <= 10.0);
    }

    #[test]
    fn repeated_correction_converges_to_target() {
        let mut config = SyncConfig::for_tests();
        config.correction_rate = 0.5;
        config.max_correction_step = Duration::from_millis(1_000);
        config.resync_threshold = Duration::from_millis(1);
        let mut clock = ClockManager::new(config).unwrap();

        clock.apply_offset(0.0).unwrap();
        clock.target_offset_ms = 200.0;

        for _ in 0..50 {
            clock.correct_drift().unwrap();
        }

        assert!((clock.current_offset() - 200.0).abs() < 1.0);
        assert!(clock.is_synchronized());
    }

    #[test]
    fn correct_drift_is_a_no_op_once_converged() {
        let mut clock = manager();
        clock.apply_offset(42.0).unwrap();
        let step = clock.correct_drift().unwrap();
        assert_eq!(step, 0.0);
    }

    #[test]
    fn estimate_drift_flags_excessive_drift() {
        let mut config = SyncConfig::for_tests();
        config.max_drift_ppm = 10.0;
        config.drift_window_samples = 2;
        let mut clock = ClockManager::new(config).unwrap();

        clock.apply_offset(0.0).unwrap();
        // Force a second sample with a huge jump in a tiny time window
        // by writing directly into the estimator via apply_offset after
        // sleeping briefly.
        std::thread::sleep(Duration::from_millis(2));
        clock.apply_offset(500.0).unwrap();

        let result = clock.estimate_drift();
        assert!(matches!(result, Err(SyncError::DriftOutOfRange { .. })));
    }

    #[test]
    fn reset_clears_offset_drift_and_statistics() {
        let mut clock = manager();
        clock.apply_offset(123.0).unwrap();
        clock.correct_drift().unwrap();
        clock.estimate_drift().unwrap();
        clock.record_success().unwrap();

        clock.reset().unwrap();

        assert_eq!(clock.current_offset(), 0.0);
        assert_eq!(clock.current_drift(), 0.0);
        assert!(!clock.is_synchronized());
        let stats = clock.statistics().unwrap();
        assert_eq!(stats, SyncStatistics::default());
    }

    #[test]
    fn statistics_reflect_successes_and_failures() {
        let mut clock = manager();
        clock.record_success().unwrap();
        clock.record_success().unwrap();
        clock.record_failure().unwrap();

        let stats = clock.statistics().unwrap();
        assert_eq!(stats.successful_syncs, 2);
        assert_eq!(stats.failed_syncs, 1);
    }

    #[test]
    fn millis_to_duration_rejects_negative_values() {
        assert!(millis_to_duration(-1.0).is_err());
        assert!(millis_to_duration(0.0).is_ok());
        assert!(millis_to_duration(50.0).is_ok());
    }

    #[test]
    fn duration_millis_roundtrip_is_precise() {
        let original = Duration::from_millis(1234);
        let ms = duration_to_millis(original);
        let roundtrip = millis_to_duration(ms).unwrap();
        assert!((duration_to_millis(roundtrip) - duration_to_millis(original)).abs() < 1e-6);
    }
}
