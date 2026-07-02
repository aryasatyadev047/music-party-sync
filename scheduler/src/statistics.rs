//! Statistics tracking for the EchoSync Playback Scheduler.
//!
//! [`StatisticsTracker`] is the single-threaded accumulator owned by
//! [`crate::playback_queue::PlaybackQueue`]; [`SchedulerStatistics`] is
//! the immutable, point-in-time snapshot handed out to callers —
//! mirroring `buffer::core::sync::statistics::StatisticsTracker`.

/// Aggregated, point-in-time runtime statistics for a
/// [`crate::scheduler::PlaybackScheduler`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SchedulerStatistics {
    /// Total packets accepted by
    /// [`crate::scheduler::PlaybackScheduler::schedule_packet`] and
    /// placed into the playback queue.
    pub packets_scheduled: u64,
    /// Total packets released to the caller via
    /// [`crate::scheduler::PlaybackScheduler::next_packet`].
    pub packets_played: u64,
    /// Total packets dropped for being excessively late.
    pub packets_dropped: u64,
    /// Total scheduled packets whose deadline had already passed (by
    /// less than `max_late_threshold`) at scheduling time.
    pub late_packets: u64,
    /// Total scheduled packets whose deadline was further than
    /// `max_early_threshold` in the future at scheduling time.
    pub early_packets: u64,
    /// Running mean, in milliseconds, of the delay between a packet's
    /// deadline and the moment it was actually scheduled.
    pub average_scheduling_delay_ms: f64,
    /// Running mean, in milliseconds, of the offset between a packet's
    /// deadline and the moment it was actually released via
    /// [`crate::scheduler::PlaybackScheduler::next_packet`].
    pub average_playback_offset_ms: f64,
    /// Number of packets currently sitting in the playback queue.
    pub queue_occupancy: usize,
}

/// Single-threaded, mutable accumulator that produces
/// [`SchedulerStatistics`] snapshots. Not thread-safe on its own;
/// concurrency safety is layered on top by
/// [`crate::scheduler::PlaybackScheduler`], the same pattern used by
/// [`buffer::BufferManager`] around its internal jitter buffer.
#[derive(Debug, Clone, Copy, Default)]
pub struct StatisticsTracker {
    packets_scheduled: u64,
    packets_played: u64,
    packets_dropped: u64,
    late_packets: u64,
    early_packets: u64,
    scheduling_delay_sum_ms: f64,
    scheduling_delay_sample_count: u64,
    playback_offset_sum_ms: f64,
    playback_offset_sample_count: u64,
}

impl StatisticsTracker {
    /// Creates a fresh tracker with all counters zeroed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a packet accepted into the playback queue.
    pub fn record_scheduled(&mut self, scheduling_delay_ms: f64) {
        self.packets_scheduled += 1;
        self.scheduling_delay_sum_ms += scheduling_delay_ms;
        self.scheduling_delay_sample_count += 1;
    }

    /// Records a packet released to the caller, along with how far its
    /// release moment was from its computed deadline.
    pub fn record_played(&mut self, playback_offset_ms: f64) {
        self.packets_played += 1;
        self.playback_offset_sum_ms += playback_offset_ms;
        self.playback_offset_sample_count += 1;
    }

    /// Records a packet dropped for being excessively late.
    pub fn record_dropped(&mut self) {
        self.packets_dropped += 1;
    }

    /// Records a packet whose deadline had already passed (within
    /// tolerance) at scheduling time.
    pub fn record_late(&mut self) {
        self.late_packets += 1;
    }

    /// Records a packet whose deadline was further than the early
    /// threshold in the future at scheduling time.
    pub fn record_early(&mut self) {
        self.early_packets += 1;
    }

    /// Produces an immutable snapshot of the current counters.
    /// `queue_occupancy` is supplied by the caller since it reflects
    /// live queue state rather than an accumulated counter.
    pub fn snapshot(&self, queue_occupancy: usize) -> SchedulerStatistics {
        let average_scheduling_delay_ms = if self.scheduling_delay_sample_count > 0 {
            self.scheduling_delay_sum_ms / self.scheduling_delay_sample_count as f64
        } else {
            0.0
        };
        let average_playback_offset_ms = if self.playback_offset_sample_count > 0 {
            self.playback_offset_sum_ms / self.playback_offset_sample_count as f64
        } else {
            0.0
        };
        SchedulerStatistics {
            packets_scheduled: self.packets_scheduled,
            packets_played: self.packets_played,
            packets_dropped: self.packets_dropped,
            late_packets: self.late_packets,
            early_packets: self.early_packets,
            average_scheduling_delay_ms,
            average_playback_offset_ms,
            queue_occupancy,
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
        assert_eq!(tracker.snapshot(0), SchedulerStatistics::default());
    }

    #[test]
    fn scheduling_delay_average_accumulates() {
        let mut tracker = StatisticsTracker::new();
        tracker.record_scheduled(10.0);
        tracker.record_scheduled(30.0);

        let snapshot = tracker.snapshot(2);
        assert_eq!(snapshot.packets_scheduled, 2);
        assert_eq!(snapshot.average_scheduling_delay_ms, 20.0);
        assert_eq!(snapshot.queue_occupancy, 2);
    }

    #[test]
    fn playback_offset_average_accumulates() {
        let mut tracker = StatisticsTracker::new();
        tracker.record_played(5.0);
        tracker.record_played(15.0);

        let snapshot = tracker.snapshot(0);
        assert_eq!(snapshot.packets_played, 2);
        assert_eq!(snapshot.average_playback_offset_ms, 10.0);
    }

    #[test]
    fn counters_increment_independently() {
        let mut tracker = StatisticsTracker::new();
        tracker.record_dropped();
        tracker.record_dropped();
        tracker.record_late();
        tracker.record_early();
        tracker.record_early();
        tracker.record_early();

        let snapshot = tracker.snapshot(0);
        assert_eq!(snapshot.packets_dropped, 2);
        assert_eq!(snapshot.late_packets, 1);
        assert_eq!(snapshot.early_packets, 3);
    }

    #[test]
    fn reset_clears_every_counter() {
        let mut tracker = StatisticsTracker::new();
        tracker.record_scheduled(1.0);
        tracker.record_played(1.0);
        tracker.record_dropped();

        tracker.reset();

        assert_eq!(tracker.snapshot(0), SchedulerStatistics::default());
    }
}
