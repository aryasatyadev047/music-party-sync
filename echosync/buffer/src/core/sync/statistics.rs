//! Statistics tracking for the EchoSync Clock Synchronization Engine.
//!
//! [`StatisticsTracker`] is the single-threaded accumulator owned by
//! [`crate::core::sync::clock_manager::ClockManager`]; [`SyncStatistics`]
//! is the immutable, point-in-time snapshot handed out to callers —
//! mirroring [`crate::core::buffer::jitter_buffer::JitterBufferStatistics`].

/// Aggregated, point-in-time runtime statistics for a
/// [`crate::core::sync::clock_manager::ClockManager`] /
/// [`crate::core::sync::synchronizer::Synchronizer`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SyncStatistics {
    /// The most recently applied offset, in milliseconds. Positive means
    /// the host clock leads the local clock.
    pub current_offset_ms: f64,
    /// Running mean of every offset ever applied via
    /// [`crate::core::sync::clock_manager::ClockManager::apply_offset`].
    pub average_offset_ms: f64,
    /// The largest offset magnitude ever applied.
    pub max_offset_ms: f64,
    /// The most recently estimated drift rate, in parts-per-million.
    pub current_drift_ppm: f64,
    /// Running mean of every drift estimate ever produced.
    pub average_drift_ppm: f64,
    /// Synchronization accuracy: the absolute distance, in milliseconds,
    /// between the current applied offset and the last measured target
    /// offset. Smaller is better; `0.0` means the clock has fully
    /// converged.
    pub sync_accuracy_ms: f64,
    /// Total number of gradual correction steps applied via
    /// [`crate::core::sync::clock_manager::ClockManager::correct_drift`].
    pub correction_count: u64,
    /// Total number of times the clock transitioned from synchronized to
    /// unsynchronized and required a fresh convergence.
    pub resync_count: u64,
    /// Total number of synchronization passes that completed without
    /// error.
    pub successful_syncs: u64,
    /// Total number of synchronization passes that failed (offset or
    /// drift out of range, or another synchronization error).
    pub failed_syncs: u64,
}

/// Single-threaded, mutable accumulator that produces [`SyncStatistics`]
/// snapshots. Not thread-safe on its own; concurrency safety is layered
/// on top by [`crate::core::sync::synchronizer::Synchronizer`], the same
/// pattern used by [`crate::core::buffer::buffer_manager::BufferManager`]
/// around [`crate::core::buffer::jitter_buffer::JitterBuffer`].
#[derive(Debug, Clone, Copy, Default)]
pub struct StatisticsTracker {
    current_offset_ms: f64,
    offset_sum_ms: f64,
    offset_sample_count: u64,
    max_offset_ms: f64,
    current_drift_ppm: f64,
    drift_sum_ppm: f64,
    drift_sample_count: u64,
    sync_accuracy_ms: f64,
    correction_count: u64,
    resync_count: u64,
    successful_syncs: u64,
    failed_syncs: u64,
}

impl StatisticsTracker {
    /// Creates a fresh tracker with all counters zeroed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a newly applied offset sample, updating current, running
    /// average, and maximum-magnitude offset statistics.
    pub fn record_offset(&mut self, offset_ms: f64) {
        self.current_offset_ms = offset_ms;
        self.offset_sum_ms += offset_ms;
        self.offset_sample_count += 1;
        if offset_ms.abs() > self.max_offset_ms {
            self.max_offset_ms = offset_ms.abs();
        }
    }

    /// Records a newly estimated drift rate sample, updating current and
    /// running average drift statistics.
    pub fn record_drift(&mut self, drift_ppm: f64) {
        self.current_drift_ppm = drift_ppm;
        self.drift_sum_ppm += drift_ppm;
        self.drift_sample_count += 1;
    }

    /// Updates the current synchronization accuracy: the absolute
    /// distance, in milliseconds, between the applied offset and the
    /// last measured target offset.
    pub fn record_accuracy(&mut self, accuracy_ms: f64) {
        self.sync_accuracy_ms = accuracy_ms;
    }

    /// Records that a gradual correction step was applied.
    pub fn record_correction(&mut self) {
        self.correction_count += 1;
    }

    /// Records that the clock transitioned from synchronized to
    /// unsynchronized and a fresh resynchronization began.
    pub fn record_resync(&mut self) {
        self.resync_count += 1;
    }

    /// Records a synchronization pass that completed without error.
    pub fn record_success(&mut self) {
        self.successful_syncs += 1;
    }

    /// Records a synchronization pass that failed.
    pub fn record_failure(&mut self) {
        self.failed_syncs += 1;
    }

    /// Produces an immutable snapshot of the current counters.
    pub fn snapshot(&self) -> SyncStatistics {
        let average_offset_ms = if self.offset_sample_count > 0 {
            self.offset_sum_ms / self.offset_sample_count as f64
        } else {
            0.0
        };
        let average_drift_ppm = if self.drift_sample_count > 0 {
            self.drift_sum_ppm / self.drift_sample_count as f64
        } else {
            0.0
        };
        SyncStatistics {
            current_offset_ms: self.current_offset_ms,
            average_offset_ms,
            max_offset_ms: self.max_offset_ms,
            current_drift_ppm: self.current_drift_ppm,
            average_drift_ppm,
            sync_accuracy_ms: self.sync_accuracy_ms,
            correction_count: self.correction_count,
            resync_count: self.resync_count,
            successful_syncs: self.successful_syncs,
            failed_syncs: self.failed_syncs,
        }
    }

    /// Resets every counter back to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_tracker_snapshots_to_all_zeros() {
        let tracker = StatisticsTracker::new();
        assert_eq!(tracker.snapshot(), SyncStatistics::default());
    }

    #[test]
    fn offset_statistics_track_current_average_and_max() {
        let mut tracker = StatisticsTracker::new();
        tracker.record_offset(10.0);
        tracker.record_offset(-30.0);
        tracker.record_offset(20.0);

        let snapshot = tracker.snapshot();
        assert_eq!(snapshot.current_offset_ms, 20.0);
        assert!((snapshot.average_offset_ms - 0.0).abs() < 1e-9);
        assert_eq!(snapshot.max_offset_ms, 30.0);
    }

    #[test]
    fn drift_statistics_track_current_and_average() {
        let mut tracker = StatisticsTracker::new();
        tracker.record_drift(100.0);
        tracker.record_drift(200.0);

        let snapshot = tracker.snapshot();
        assert_eq!(snapshot.current_drift_ppm, 200.0);
        assert_eq!(snapshot.average_drift_ppm, 150.0);
    }

    #[test]
    fn counters_increment_independently() {
        let mut tracker = StatisticsTracker::new();
        tracker.record_correction();
        tracker.record_correction();
        tracker.record_resync();
        tracker.record_success();
        tracker.record_success();
        tracker.record_success();
        tracker.record_failure();

        let snapshot = tracker.snapshot();
        assert_eq!(snapshot.correction_count, 2);
        assert_eq!(snapshot.resync_count, 1);
        assert_eq!(snapshot.successful_syncs, 3);
        assert_eq!(snapshot.failed_syncs, 1);
    }

    #[test]
    fn reset_clears_every_counter() {
        let mut tracker = StatisticsTracker::new();
        tracker.record_offset(50.0);
        tracker.record_drift(10.0);
        tracker.record_correction();
        tracker.record_success();

        tracker.reset();

        assert_eq!(tracker.snapshot(), SyncStatistics::default());
    }
}
