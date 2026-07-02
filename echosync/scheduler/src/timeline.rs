//! The Playback Scheduler's internal notion of "when" — translating a
//! packet's capture-time timestamp into a deadline on a monotonic
//! playback timeline.
//!
//! [`PlaybackTimeline`] deliberately has no knowledge of wall-clock time
//! (`SystemTime`) so it stays deterministic under `tokio::time::pause`
//! in tests, and no knowledge of networking, buffering, or the Clock
//! Synchronization Engine's offset/drift model — that estimation lives
//! in `buffer::core::sync`. The scheduler combines the two: the
//! Synchronization Engine says how far off this device's clock is from
//! the shared session clock, and [`PlaybackTimeline`] maps a packet's
//! `timestamp_ms` onto local elapsed time so deadlines can be compared
//! against [`tokio::time::Instant::now`].
//!
//! ## Model
//! The first packet observed after construction or [`PlaybackTimeline::reset`]
//! anchors the timeline: its `timestamp_ms` is paired with the local
//! elapsed time at that moment. Every subsequent packet's expected
//! playback time is `anchor_local + (timestamp_ms - anchor_timestamp_ms)`
//! — i.e. capture-time deltas are assumed to advance at the same rate as
//! real time, which holds for a steady audio capture rate. Packets whose
//! `timestamp_ms` precedes the anchor (arriving out of order) clamp to
//! the anchor instant rather than producing a negative offset.

use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;

use crate::error::SchedulerError;

/// Anchor pairing a packet capture timestamp with the local elapsed
/// time at which it was first observed.
#[derive(Debug, Clone, Copy)]
struct Anchor {
    timestamp_ms: u64,
    local_offset: Duration,
}

/// Maps packet capture timestamps onto a monotonic, locally-measured
/// playback timeline, and derives playback deadlines from them.
///
/// Thread-safe: the anchor is stored behind a `std::sync::Mutex` since
/// establishing/reading it never awaits, matching the non-blocking
/// requirement for scheduler internals (contrast with the `tokio::sync`
/// primitives used by [`crate::scheduler::PlaybackScheduler`] for
/// operations that do await).
#[derive(Debug)]
pub struct PlaybackTimeline {
    started_at: Instant,
    resolution: Duration,
    anchor: Mutex<Option<Anchor>>,
}

impl PlaybackTimeline {
    /// Creates a new timeline, starting its local elapsed-time clock
    /// immediately. `resolution` quantizes [`PlaybackTimeline::current_time`]
    /// reads (see [`crate::config::SchedulerConfig::timeline_resolution`]).
    pub fn new(resolution: Duration) -> Self {
        Self {
            started_at: Instant::now(),
            resolution: if resolution.is_zero() { Duration::from_millis(1) } else { resolution },
            anchor: Mutex::new(None),
        }
    }

    /// Elapsed local time since this timeline was created or last
    /// [`PlaybackTimeline::reset`], quantized to `resolution`.
    pub fn current_time(&self) -> Duration {
        self.quantize(self.started_at.elapsed())
    }

    /// The expected playback moment (as elapsed local time) for a
    /// packet captured at `packet_timestamp_ms`, per the anchoring model
    /// described in the module docs. The first call after construction
    /// or [`PlaybackTimeline::reset`] establishes the anchor.
    pub fn expected_playback_time(&self, packet_timestamp_ms: u64) -> Duration {
        let now = self.current_time();
        let mut guard = self.anchor.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        let anchor = *guard.get_or_insert(Anchor { timestamp_ms: packet_timestamp_ms, local_offset: now });

        if packet_timestamp_ms >= anchor.timestamp_ms {
            let delta_ms = packet_timestamp_ms - anchor.timestamp_ms;
            self.quantize(anchor.local_offset + Duration::from_millis(delta_ms))
        } else {
            // Out-of-order arrival relative to the anchor: clamp to the
            // anchor instant rather than going negative.
            anchor.local_offset
        }
    }

    /// The playback deadline for a packet: its
    /// [`PlaybackTimeline::expected_playback_time`] plus the configured
    /// playback latency headroom.
    pub fn calculate_deadline(&self, packet_timestamp_ms: u64, latency: Duration) -> Duration {
        self.expected_playback_time(packet_timestamp_ms) + latency
    }

    /// Clears the anchor and restarts the local elapsed-time clock from
    /// zero. Returns [`SchedulerError::OperationFailed`] only if the
    /// internal lock is poisoned by a prior panic (never in normal
    /// operation, but surfaced rather than silently ignored).
    pub fn reset(&mut self) -> Result<(), SchedulerError> {
        self.started_at = Instant::now();
        match self.anchor.lock() {
            Ok(mut guard) => {
                *guard = None;
                Ok(())
            }
            Err(_) => Err(SchedulerError::OperationFailed(
                "playback timeline anchor lock poisoned".into(),
            )),
        }
    }

    fn quantize(&self, duration: Duration) -> Duration {
        let resolution_ms = self.resolution.as_millis().max(1) as u64;
        let ms = duration.as_millis() as u64;
        Duration::from_millis((ms / resolution_ms) * resolution_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn current_time_advances_with_paused_clock() {
        let timeline = PlaybackTimeline::new(Duration::from_millis(1));
        assert_eq!(timeline.current_time(), Duration::ZERO);

        tokio::time::advance(Duration::from_millis(100)).await;
        assert_eq!(timeline.current_time(), Duration::from_millis(100));
    }

    #[tokio::test(start_paused = true)]
    async fn first_packet_anchors_expected_playback_time_to_now() {
        let timeline = PlaybackTimeline::new(Duration::from_millis(1));
        tokio::time::advance(Duration::from_millis(50)).await;

        let expected = timeline.expected_playback_time(1_000);
        assert_eq!(expected, Duration::from_millis(50));
    }

    #[tokio::test(start_paused = true)]
    async fn subsequent_packets_offset_from_the_anchor() {
        let timeline = PlaybackTimeline::new(Duration::from_millis(1));
        let _ = timeline.expected_playback_time(1_000); // anchors at t=0

        tokio::time::advance(Duration::from_millis(10)).await;
        let expected = timeline.expected_playback_time(1_040);
        assert_eq!(expected, Duration::from_millis(40));
    }

    #[tokio::test(start_paused = true)]
    async fn out_of_order_early_timestamp_clamps_to_anchor() {
        let timeline = PlaybackTimeline::new(Duration::from_millis(1));
        let anchor_time = timeline.expected_playback_time(1_000);

        let expected = timeline.expected_playback_time(900);
        assert_eq!(expected, anchor_time);
    }

    #[tokio::test(start_paused = true)]
    async fn calculate_deadline_adds_latency() {
        let timeline = PlaybackTimeline::new(Duration::from_millis(1));
        let deadline = timeline.calculate_deadline(1_000, Duration::from_millis(150));
        assert_eq!(deadline, Duration::from_millis(150));
    }

    #[tokio::test(start_paused = true)]
    async fn reset_clears_anchor_and_restarts_clock() {
        let mut timeline = PlaybackTimeline::new(Duration::from_millis(1));
        tokio::time::advance(Duration::from_millis(200)).await;
        let _ = timeline.expected_playback_time(5_000);

        timeline.reset().unwrap();
        assert_eq!(timeline.current_time(), Duration::ZERO);

        let expected = timeline.expected_playback_time(5_000);
        assert_eq!(expected, Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn resolution_quantizes_current_time_reads() {
        let timeline = PlaybackTimeline::new(Duration::from_millis(10));
        tokio::time::advance(Duration::from_millis(24)).await;
        assert_eq!(timeline.current_time(), Duration::from_millis(20));
    }

    #[tokio::test(start_paused = true)]
    async fn zero_resolution_falls_back_to_one_millisecond() {
        let timeline = PlaybackTimeline::new(Duration::ZERO);
        tokio::time::advance(Duration::from_millis(5)).await;
        assert_eq!(timeline.current_time(), Duration::from_millis(5));
    }
}
