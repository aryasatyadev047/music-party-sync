//! The Playback Scheduler's public, concurrency-safe entry point.
//!
//! `PlaybackScheduler` decides *when* each [`streaming::AudioPacket`]
//! pulled from the Buffer Layer should be released for playback. It
//! does not decode or play audio — it only computes deadlines (via
//! [`crate::timeline::PlaybackTimeline`], informed by the Clock
//! Synchronization Engine's clock offset) and orders packets
//! accordingly (via [`crate::playback_queue::PlaybackQueue`]), the same
//! separation of concerns [`buffer::BufferManager`] uses around its
//! internal jitter buffer.
//!
//! ## Wiring
//! ```text
//!  Buffer Layer          PlaybackScheduler::schedule_packet()      PlaybackQueue (EDF order)      PlaybackScheduler::next_packet()      (future) Audio Output
//!  pop_packet() ───────────────────────────────────────────────▶  (deadline computed via PlaybackTimeline  ──────────────────────────────────▶  release-ready packets only
//!                                                                   + Synchronizer::current_offset())
//! ```
//!
//! Cheap to clone, like [`buffer::BufferManager`] and
//! [`buffer::Synchronizer`]: cloning shares the same underlying queue,
//! timeline, and statistics state across concurrent producer/consumer
//! tasks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use tokio::sync::Mutex as AsyncMutex;
use tracing::{debug, info, warn};

use buffer::Synchronizer;
use streaming::AudioPacket;

use crate::config::SchedulerConfig;
use crate::error::SchedulerError;
use crate::playback_queue::{PlaybackQueue, ScheduledPacket};
use crate::statistics::{SchedulerStatistics, StatisticsTracker};
use crate::timeline::PlaybackTimeline;

/// The Playback Scheduler's top-level type: computes playback deadlines
/// for packets coming out of the Buffer Layer, using synchronized
/// timestamps from the Clock Synchronization Engine, and hands them
/// back out in deterministic, earliest-deadline-first order once their
/// deadline has arrived.
#[derive(Clone)]
pub struct PlaybackScheduler {
    queue: Arc<AsyncMutex<PlaybackQueue>>,
    timeline: Arc<StdMutex<PlaybackTimeline>>,
    synchronizer: Synchronizer,
    stats: Arc<StdMutex<StatisticsTracker>>,
    running: Arc<AtomicBool>,
    config: SchedulerConfig,
}

impl PlaybackScheduler {
    /// Creates a new `PlaybackScheduler`. Returns
    /// [`SchedulerError::InvalidConfiguration`] if `config` fails
    /// [`SchedulerConfig::validate`]. The scheduler is created stopped;
    /// call [`PlaybackScheduler::start`] before
    /// [`PlaybackScheduler::schedule_packet`] or
    /// [`PlaybackScheduler::next_packet`].
    ///
    /// `synchronizer` is the caller's existing, already-constructed
    /// [`buffer::Synchronizer`] handle (the Clock Synchronization
    /// Engine's public entry point) — this module never constructs one
    /// itself, so it never duplicates or bypasses that engine's state.
    pub fn new(config: SchedulerConfig, synchronizer: Synchronizer) -> Result<Self, SchedulerError> {
        config.validate()?;
        Ok(Self {
            queue: Arc::new(AsyncMutex::new(PlaybackQueue::new(config.queue_capacity))),
            timeline: Arc::new(StdMutex::new(PlaybackTimeline::new(config.timeline_resolution))),
            synchronizer,
            stats: Arc::new(StdMutex::new(StatisticsTracker::new())),
            running: Arc::new(AtomicBool::new(false)),
            config,
        })
    }

    /// The [`SchedulerConfig`] this scheduler was created with.
    pub fn config(&self) -> &SchedulerConfig {
        &self.config
    }

    /// Starts the scheduler: resets the playback timeline and allows
    /// [`PlaybackScheduler::schedule_packet`] /
    /// [`PlaybackScheduler::next_packet`] to run. Returns
    /// [`SchedulerError::AlreadyStarted`] if already running.
    pub async fn start(&self) -> Result<(), SchedulerError> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(SchedulerError::AlreadyStarted);
        }
        {
            let mut timeline = self.lock_timeline()?;
            timeline.reset()?;
        }
        info!("Scheduler Started");
        Ok(())
    }

    /// Stops the scheduler. Subsequent
    /// [`PlaybackScheduler::schedule_packet`] /
    /// [`PlaybackScheduler::next_packet`] calls fail with
    /// [`SchedulerError::NotStarted`] until [`PlaybackScheduler::start`]
    /// is called again. Queued packets and statistics are preserved.
    /// Returns [`SchedulerError::NotStarted`] if not currently running.
    pub async fn stop(&self) -> Result<(), SchedulerError> {
        if !self.running.swap(false, Ordering::SeqCst) {
            return Err(SchedulerError::NotStarted);
        }
        info!("Scheduler Stopped");
        Ok(())
    }

    /// Computes a playback deadline for `packet` (from its
    /// `timestamp_ms`, the configured `playback_latency`, and the
    /// Clock Synchronization Engine's current offset) and enqueues it in
    /// deadline order.
    ///
    /// Returns [`SchedulerError::NotStarted`] if the scheduler isn't
    /// running, [`SchedulerError::PacketExpired`] if the deadline has
    /// already passed by more than `max_late_threshold` (the packet is
    /// dropped, not queued), [`SchedulerError::DuplicatePacket`] if a
    /// packet with the same `packet_id` is already queued, or
    /// [`SchedulerError::QueueFull`] if the queue is at
    /// `queue_capacity`.
    pub async fn schedule_packet(&self, packet: AudioPacket) -> Result<(), SchedulerError> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(SchedulerError::NotStarted);
        }

        // Await the Clock Synchronization Engine's current offset
        // *before* touching any local locks, so no lock is ever held
        // across an await point.
        let offset_ms = self.synchronizer.current_offset().await?;
        let adjusted_timestamp_ms = (packet.timestamp_ms as f64 + offset_ms).max(0.0) as u64;

        let (deadline, now) = {
            let timeline = self.lock_timeline()?;
            let deadline = timeline.calculate_deadline(adjusted_timestamp_ms, self.config.playback_latency);
            let now = timeline.current_time();
            (deadline, now)
        };

        let diff_ms = deadline.as_millis() as i64 - now.as_millis() as i64;
        let packet_id = packet.packet_id;

        if diff_ms < 0 {
            let late_by_ms = (-diff_ms) as u64;
            if late_by_ms > self.config.max_late_threshold.as_millis() as u64 {
                self.record_dropped();
                warn!(packet_id, late_by_ms, "Packet Dropped");
                return Err(SchedulerError::PacketExpired { packet_id, late_by_ms });
            }
            self.record_late();
            debug!(packet_id, late_by_ms, "Late Packet");
        } else if diff_ms as u64 > self.config.max_early_threshold.as_millis() as u64 {
            self.record_early();
        }

        let item = ScheduledPacket { packet, deadline, scheduled_at: now };

        {
            let mut queue = self.queue.lock().await;
            if queue.is_full() {
                warn!(packet_id, "Queue Overflow");
            }
            queue.enqueue(item)?;
        }

        self.record_scheduled(diff_ms.unsigned_abs() as f64);
        debug!(packet_id, ?deadline, "Packet Scheduled");
        Ok(())
    }

    /// Returns the next packet whose deadline has arrived, removing it
    /// from the queue. Returns `Ok(None)` — not an error — if the queue
    /// is empty or the packet at the head of the queue isn't due yet
    /// (an "early" wait, not an underflow from the caller's point of
    /// view). Returns [`SchedulerError::NotStarted`] if the scheduler
    /// isn't running.
    pub async fn next_packet(&self) -> Result<Option<AudioPacket>, SchedulerError> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(SchedulerError::NotStarted);
        }

        let now = {
            let timeline = self.lock_timeline()?;
            timeline.current_time()
        };

        let mut queue = self.queue.lock().await;
        let due = match queue.peek()? {
            Some(head) => head.deadline <= now,
            None => {
                debug!("Queue Underflow");
                false
            }
        };

        if !due {
            return Ok(None);
        }

        let item = match queue.dequeue()? {
            Some(item) => item,
            None => return Ok(None),
        };
        drop(queue);

        let playback_offset_ms = now.as_millis() as i64 - item.deadline.as_millis() as i64;
        self.record_played(playback_offset_ms as f64);
        info!(packet_id = item.packet.packet_id, "Packet Released");
        Ok(Some(item.packet))
    }

    /// Removes a specific packet from the queue before it's released.
    /// Returns `Ok(true)` if a matching packet was found and removed,
    /// `Ok(false)` if no packet with that `packet_id` was queued.
    pub async fn cancel_packet(&self, packet_id: u64) -> Result<bool, SchedulerError> {
        let mut queue = self.queue.lock().await;
        queue.cancel(packet_id)
    }

    /// Removes every currently queued packet. Statistics accumulated so
    /// far are preserved; use [`PlaybackScheduler::statistics`]
    /// beforehand if they're still needed.
    pub async fn clear(&self) -> Result<(), SchedulerError> {
        let mut queue = self.queue.lock().await;
        queue.clear()
    }

    /// A snapshot of the scheduler's runtime statistics, including
    /// live queue occupancy.
    pub async fn statistics(&self) -> Result<SchedulerStatistics, SchedulerError> {
        let occupancy = {
            let queue = self.queue.lock().await;
            queue.size()
        };
        let stats = self.stats.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        Ok(stats.snapshot(occupancy))
    }

    fn lock_timeline(&self) -> Result<std::sync::MutexGuard<'_, PlaybackTimeline>, SchedulerError> {
        self.timeline
            .lock()
            .map_err(|_| SchedulerError::OperationFailed("playback timeline lock poisoned".into()))
    }

    fn record_dropped(&self) {
        let mut stats = self.stats.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        stats.record_dropped();
    }

    fn record_late(&self) {
        let mut stats = self.stats.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        stats.record_late();
    }

    fn record_early(&self) {
        let mut stats = self.stats.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        stats.record_early();
    }

    fn record_scheduled(&self, scheduling_delay_ms: f64) {
        let mut stats = self.stats.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        stats.record_scheduled(scheduling_delay_ms);
    }

    fn record_played(&self, playback_offset_ms: f64) {
        let mut stats = self.stats.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        stats.record_played(playback_offset_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buffer::SyncConfig;
    use std::time::Duration;
    use streaming::PacketFlags;

    fn make_packet(packet_id: u64, sequence_number: u64, timestamp_ms: u64) -> AudioPacket {
        let mut packet = AudioPacket::new(
            packet_id,
            sequence_number,
            vec![0u8; 8],
            PacketFlags::new(),
            "device-a",
            "session-1",
            4000,
        )
        .unwrap();
        packet.timestamp_ms = timestamp_ms;
        packet
    }

    fn sync_test_config() -> SyncConfig {
        let mut config = SyncConfig::default();
        config.sync_interval = Duration::from_millis(50);
        config.max_allowed_offset = Duration::from_millis(2_000);
        config.max_drift_ppm = 5_000.0;
        config.clock_precision = Duration::from_millis(1);
        config.correction_rate = 0.5;
        config.max_correction_step = Duration::from_millis(1_000);
        config.resync_threshold = Duration::from_millis(5);
        config.drift_window_samples = 5;
        config
    }

    async fn synchronizer() -> Synchronizer {
        let sync = Synchronizer::new(sync_test_config()).unwrap();
        sync.start().await.unwrap();
        sync
    }

    async fn scheduler() -> PlaybackScheduler {
        PlaybackScheduler::new(SchedulerConfig::for_tests(), synchronizer().await).unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn schedule_before_start_fails() {
        let sched = scheduler().await;
        let result = sched.schedule_packet(make_packet(1, 0, 0)).await;
        assert_eq!(result, Err(SchedulerError::NotStarted));
    }

    #[tokio::test(start_paused = true)]
    async fn next_packet_before_start_fails() {
        let sched = scheduler().await;
        assert_eq!(sched.next_packet().await, Err(SchedulerError::NotStarted));
    }

    #[tokio::test(start_paused = true)]
    async fn starting_twice_fails() {
        let sched = scheduler().await;
        sched.start().await.unwrap();
        assert_eq!(sched.start().await, Err(SchedulerError::AlreadyStarted));
    }

    #[tokio::test(start_paused = true)]
    async fn stopping_without_starting_fails() {
        let sched = scheduler().await;
        assert_eq!(sched.stop().await, Err(SchedulerError::NotStarted));
    }

    #[tokio::test(start_paused = true)]
    async fn schedule_then_next_packet_releases_once_deadline_arrives() {
        let sched = scheduler().await;
        sched.start().await.unwrap();

        // Anchors the timeline at t=0; deadline = 0 + playback_latency (20ms).
        sched.schedule_packet(make_packet(1, 0, 0)).await.unwrap();
        assert_eq!(sched.next_packet().await.unwrap(), None);

        tokio::time::advance(Duration::from_millis(21)).await;
        let released = sched.next_packet().await.unwrap().unwrap();
        assert_eq!(released.packet_id, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn packets_release_in_deadline_order_not_schedule_order() {
        let sched = scheduler().await;
        sched.start().await.unwrap();

        sched.schedule_packet(make_packet(1, 0, 0)).await.unwrap(); // deadline 20ms
        tokio::time::advance(Duration::from_millis(5)).await;
        sched.schedule_packet(make_packet(2, 1, 0)).await.unwrap(); // anchored, deadline still ~20ms via anchor model
        tokio::time::advance(Duration::from_millis(30)).await;

        let first = sched.next_packet().await.unwrap().unwrap();
        let second = sched.next_packet().await.unwrap().unwrap();
        // Both packets share timestamp_ms=0, so they tie on deadline and
        // break the tie by sequence_number.
        assert_eq!([first.packet_id, second.packet_id], [1, 2]);
    }

    #[tokio::test(start_paused = true)]
    async fn duplicate_scheduling_is_rejected() {
        let sched = scheduler().await;
        sched.start().await.unwrap();

        sched.schedule_packet(make_packet(1, 0, 0)).await.unwrap();
        let result = sched.schedule_packet(make_packet(1, 0, 0)).await;
        assert_eq!(result, Err(SchedulerError::DuplicatePacket { packet_id: 1 }));
    }

    #[tokio::test(start_paused = true)]
    async fn excessively_late_packets_are_dropped() {
        let sched = scheduler().await;
        sched.start().await.unwrap();

        // Anchor the timeline far in the past relative to a later packet.
        sched.schedule_packet(make_packet(1, 0, 0)).await.unwrap();
        tokio::time::advance(Duration::from_secs(10)).await;

        // This packet's deadline (0 + 20ms) is now ~10s in the past —
        // far beyond max_late_threshold (50ms).
        let result = sched.schedule_packet(make_packet(2, 1, 0)).await;
        assert!(matches!(result, Err(SchedulerError::PacketExpired { packet_id: 2, .. })));

        let stats = sched.statistics().await.unwrap();
        assert_eq!(stats.packets_dropped, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn queue_full_is_rejected() {
        let mut config = SchedulerConfig::for_tests();
        config.queue_capacity = 1;
        let sched = PlaybackScheduler::new(config, synchronizer().await).unwrap();
        sched.start().await.unwrap();

        sched.schedule_packet(make_packet(1, 0, 0)).await.unwrap();
        let result = sched.schedule_packet(make_packet(2, 1, 0)).await;
        assert_eq!(result, Err(SchedulerError::QueueFull { capacity: 1 }));
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_packet_prevents_release() {
        let sched = scheduler().await;
        sched.start().await.unwrap();

        sched.schedule_packet(make_packet(1, 0, 0)).await.unwrap();
        assert!(sched.cancel_packet(1).await.unwrap());
        assert!(!sched.cancel_packet(1).await.unwrap());

        tokio::time::advance(Duration::from_millis(50)).await;
        assert_eq!(sched.next_packet().await.unwrap(), None);
    }

    #[tokio::test(start_paused = true)]
    async fn clear_empties_the_queue() {
        let sched = scheduler().await;
        sched.start().await.unwrap();

        sched.schedule_packet(make_packet(1, 0, 0)).await.unwrap();
        sched.schedule_packet(make_packet(2, 1, 0)).await.unwrap();
        sched.clear().await.unwrap();

        tokio::time::advance(Duration::from_millis(50)).await;
        assert_eq!(sched.next_packet().await.unwrap(), None);
    }

    #[tokio::test(start_paused = true)]
    async fn statistics_report_scheduled_and_played_counts() {
        let sched = scheduler().await;
        sched.start().await.unwrap();

        sched.schedule_packet(make_packet(1, 0, 0)).await.unwrap();
        tokio::time::advance(Duration::from_millis(21)).await;
        sched.next_packet().await.unwrap();

        let stats = sched.statistics().await.unwrap();
        assert_eq!(stats.packets_scheduled, 1);
        assert_eq!(stats.packets_played, 1);
        assert_eq!(stats.queue_occupancy, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn stop_preserves_queue_and_statistics() {
        let sched = scheduler().await;
        sched.start().await.unwrap();
        sched.schedule_packet(make_packet(1, 0, 0)).await.unwrap();
        sched.stop().await.unwrap();

        let stats = sched.statistics().await.unwrap();
        assert_eq!(stats.queue_occupancy, 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_scheduling_all_lands_without_data_loss() {
        let mut config = SchedulerConfig::for_tests();
        config.queue_capacity = 2000;
        let sched = PlaybackScheduler::new(config, synchronizer().await).unwrap();
        sched.start().await.unwrap();

        let mut handles = Vec::new();
        for producer in 0..4u64 {
            let sched = sched.clone();
            handles.push(tokio::spawn(async move {
                for i in 0..250u64 {
                    let packet_id = producer * 250 + i;
                    sched.schedule_packet(make_packet(packet_id, packet_id, 0)).await.unwrap();
                }
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }

        let stats = sched.statistics().await.unwrap();
        assert_eq!(stats.packets_scheduled, 1000);
        assert_eq!(stats.queue_occupancy, 1000);
    }

    #[tokio::test(start_paused = true)]
    async fn stress_thousands_of_packets_schedule_and_release_cleanly() {
        let mut config = SchedulerConfig::for_tests();
        config.queue_capacity = 5000;
        config.max_early_threshold = Duration::from_secs(10_000);
        let sched = PlaybackScheduler::new(config, synchronizer().await).unwrap();
        sched.start().await.unwrap();

        for i in 0..3000u64 {
            sched.schedule_packet(make_packet(i, i, i)).await.unwrap();
        }

        tokio::time::advance(Duration::from_secs(10)).await;

        let mut released = Vec::with_capacity(3000);
        while let Some(packet) = sched.next_packet().await.unwrap() {
            released.push(packet.sequence_number);
        }

        assert_eq!(released, (0..3000u64).collect::<Vec<u64>>());
        let stats = sched.statistics().await.unwrap();
        assert_eq!(stats.packets_played, 3000);
    }

    #[tokio::test(start_paused = true)]
    async fn stress_one_hundred_thousand_packets_schedule_cleanly() {
        const PACKET_COUNT: u64 = 100_001;
        let mut config = SchedulerConfig::for_tests();
        config.queue_capacity = PACKET_COUNT as usize;
        config.max_early_threshold = Duration::from_secs(10_000);
        let sched = PlaybackScheduler::new(config, synchronizer().await).unwrap();
        sched.start().await.unwrap();

        for i in 0..PACKET_COUNT {
            sched.schedule_packet(make_packet(i, i, i)).await.unwrap();
        }

        let stats = sched.statistics().await.unwrap();
        assert_eq!(stats.packets_scheduled, PACKET_COUNT);
        assert_eq!(stats.queue_occupancy, PACKET_COUNT as usize);
        assert_eq!(stats.packets_dropped, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn long_running_scheduling_keeps_statistics_consistent() {
        let mut config = SchedulerConfig::for_tests();
        config.queue_capacity = 1000;
        let sched = PlaybackScheduler::new(config, synchronizer().await).unwrap();
        sched.start().await.unwrap();

        for i in 0..500u64 {
            sched.schedule_packet(make_packet(i, i, i)).await.unwrap();
            tokio::time::advance(Duration::from_millis(1)).await;
            let _ = sched.next_packet().await.unwrap();
        }

        let stats = sched.statistics().await.unwrap();
        assert_eq!(stats.packets_scheduled, 500);
        assert!(stats.packets_played + stats.queue_occupancy as u64 <= 500);
    }
}
