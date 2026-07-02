//! The Buffer Layer's public, concurrency-safe entry point.
//!
//! `BufferManager` wraps a single-threaded [`JitterBuffer`] in a
//! `tokio::sync::Mutex` so multiple producer tasks (accepting packets
//! from the Streaming Engine) and multiple consumer tasks (pulling
//! packets for the future Synchronization Engine) can share one buffer
//! safely. Every method is either non-blocking (`size`, `is_empty`) or
//! `async` and yields at the `await` point rather than blocking a
//! worker thread, so it composes cleanly with Tokio's cooperative
//! scheduler.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex as AsyncMutex;
use tracing::{debug, info};

use streaming::AudioPacket;

use super::config::BufferConfig;
use super::error::BufferError;
use super::jitter_buffer::JitterBuffer;

/// Aggregated, point-in-time runtime statistics for a [`BufferManager`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BufferStatistics {
    /// Number of packets currently held in the buffer.
    pub current_buffer_size: usize,
    /// The buffer's currently active target delay, in milliseconds.
    pub average_delay_ms: f64,
    /// Total sequence-number gap observed across all deliveries
    /// (presumed lost packets).
    pub packet_loss_count: u64,
    /// Total packets rejected as duplicates or already-processed.
    pub duplicate_count: u64,
    /// Total packets admitted with a gap larger than the configured
    /// missing-packet tolerance.
    pub late_packet_count: u64,
    /// Total packets dropped without delivery (overflow or staleness).
    pub dropped_packet_count: u64,
    /// The highest number of packets simultaneously held in the buffer.
    pub max_buffer_occupancy: usize,
    /// Running estimate of inter-arrival jitter, in milliseconds.
    pub average_packet_jitter_ms: f64,
    /// Total packets delivered to the consumer.
    pub packets_delivered: u64,
    /// Total packets ever accepted into the buffer.
    pub packets_buffered: u64,
}

/// The Buffer Layer's top-level type: absorbs network jitter, reorders
/// packets, detects duplicates and loss, and hands packets to the
/// (future) Synchronization Engine in playback order.
///
/// Cheap to clone: cloning a `BufferManager` clones its internal `Arc`
/// handles, so all clones share the same underlying buffer state. This
/// makes it safe to hand a handle to multiple concurrent producer and
/// consumer tasks — the same pattern used by
/// [`streaming::StreamingEngine`] and [`streaming::PacketQueue`].
#[derive(Clone)]
pub struct BufferManager {
    inner: Arc<AsyncMutex<JitterBuffer>>,
    /// Approximate current buffer length, updated after every
    /// operation that changes it, so [`BufferManager::size`] and
    /// [`BufferManager::is_empty`] can answer without an `await`.
    approx_len: Arc<AtomicUsize>,
    config: BufferConfig,
}

impl BufferManager {
    /// Creates a new `BufferManager`. Returns
    /// [`BufferError::InvalidConfiguration`] if `config` fails
    /// [`BufferConfig::validate`].
    pub fn new(config: BufferConfig) -> Result<Self, BufferError> {
        config.validate()?;
        let jitter_buffer = JitterBuffer::new(config.clone())?;

        Ok(Self {
            inner: Arc::new(AsyncMutex::new(jitter_buffer)),
            approx_len: Arc::new(AtomicUsize::new(0)),
            config,
        })
    }

    /// Accepts a packet from the Streaming Engine and buffers it in
    /// sequence order. See [`JitterBuffer::insert_packet`] for the exact
    /// duplicate/overflow behavior.
    pub async fn push_packet(&self, packet: AudioPacket) -> Result<(), BufferError> {
        let mut buffer = self.inner.lock().await;
        let result = buffer.insert_packet(packet);
        self.approx_len.store(buffer.len(), Ordering::Relaxed);
        result
    }

    /// Removes and returns the next packet in playback order, if one has
    /// sat in the buffer for at least the current target delay.
    /// `Ok(None)` means the buffer is empty or nothing is ready yet — a
    /// buffer underflow from the consumer's point of view, not an error.
    pub async fn pop_packet(&self) -> Result<Option<AudioPacket>, BufferError> {
        let mut buffer = self.inner.lock().await;
        let packet = buffer.next_packet()?;
        self.approx_len.store(buffer.len(), Ordering::Relaxed);
        Ok(packet)
    }

    /// Returns a clone of the next packet in playback order without
    /// removing it from the buffer.
    pub async fn peek_packet(&self) -> Result<Option<AudioPacket>, BufferError> {
        let buffer = self.inner.lock().await;
        Ok(buffer.peek().cloned())
    }

    /// Immediately drains every buffered packet, discarding them (not
    /// handing them to any consumer). Use [`BufferManager::pop_packet`]
    /// in a loop first if the drained packets still need delivering.
    pub async fn clear(&self) -> Result<(), BufferError> {
        let mut buffer = self.inner.lock().await;
        let drained = buffer.flush();
        self.approx_len.store(0, Ordering::Relaxed);
        debug!(cleared = drained.len(), "Buffer cleared");
        Ok(())
    }

    /// Approximate number of packets currently buffered. Lock-free;
    /// reflects the state as of the most recent completed operation.
    pub fn size(&self) -> usize {
        self.approx_len.load(Ordering::Relaxed)
    }

    /// `true` if no packets are currently buffered, per the same
    /// approximate counter as [`BufferManager::size`].
    pub fn is_empty(&self) -> bool {
        self.size() == 0
    }

    /// Clears all buffered packets and sequence-tracking state, and
    /// restores the target delay to the configured default.
    pub async fn reset(&self) -> Result<(), BufferError> {
        let mut buffer = self.inner.lock().await;
        buffer.reset();
        self.approx_len.store(0, Ordering::Relaxed);
        info!("Buffer Reset");
        Ok(())
    }

    /// A snapshot of the buffer's runtime statistics.
    pub async fn statistics(&self) -> BufferStatistics {
        let buffer = self.inner.lock().await;
        let stats = buffer.statistics();

        BufferStatistics {
            current_buffer_size: buffer.len(),
            average_delay_ms: buffer.current_delay().as_secs_f64() * 1000.0,
            packet_loss_count: stats.window.packets_lost,
            duplicate_count: stats.window.duplicates_detected,
            late_packet_count: stats.window.late_packets,
            dropped_packet_count: stats.packets_dropped,
            max_buffer_occupancy: stats.max_occupancy,
            average_packet_jitter_ms: stats.average_jitter_ms,
            packets_delivered: stats.packets_delivered,
            packets_buffered: stats.packets_buffered,
        }
    }

    /// Manually sets the target buffering delay, bypassing adaptive
    /// adjustment for subsequent packets until it changes it again.
    /// Returns [`BufferError::InvalidTargetDelay`] if `delay` falls
    /// outside the configured `[min_target_delay, max_target_delay]`.
    pub async fn set_target_delay(&self, delay: Duration) -> Result<(), BufferError> {
        let mut buffer = self.inner.lock().await;
        buffer.set_target_delay(delay)
    }

    /// The currently active target buffering delay.
    pub async fn current_delay(&self) -> Duration {
        let buffer = self.inner.lock().await;
        buffer.current_delay()
    }

    /// The configuration this `BufferManager` was constructed with.
    pub fn config(&self) -> &BufferConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use streaming::PacketFlags;

    fn make_packet(sequence_number: u64) -> AudioPacket {
        AudioPacket::new(
            sequence_number,
            sequence_number,
            vec![0u8; 16],
            PacketFlags::new(),
            "device-a",
            "session-1",
            4000,
        )
        .expect("small packet fits within the default max size")
    }

    fn zero_delay_config() -> BufferConfig {
        let mut config = BufferConfig::for_tests();
        config.target_delay = Duration::ZERO;
        config.min_target_delay = Duration::ZERO;
        config.adaptive_enabled = false;
        // Staleness eviction is exercised separately in jitter_buffer's
        // own tests; give it plenty of headroom here so tests that push
        // many packets before draining aren't racing the wall clock
        // against `max_packet_age`.
        config.max_packet_age = Duration::from_secs(3600);
        config
    }

    #[tokio::test]
    async fn push_then_pop_round_trips_a_single_packet() {
        let manager = BufferManager::new(zero_delay_config()).unwrap();
        manager.push_packet(make_packet(0)).await.unwrap();

        let packet = manager.pop_packet().await.unwrap().unwrap();
        assert_eq!(packet.sequence_number, 0);
        assert!(manager.is_empty());
    }

    #[tokio::test]
    async fn peek_does_not_remove_the_packet() {
        let manager = BufferManager::new(zero_delay_config()).unwrap();
        manager.push_packet(make_packet(0)).await.unwrap();

        let peeked = manager.peek_packet().await.unwrap().unwrap();
        assert_eq!(peeked.sequence_number, 0);
        assert_eq!(manager.size(), 1);

        let popped = manager.pop_packet().await.unwrap().unwrap();
        assert_eq!(popped.sequence_number, 0);
    }

    #[tokio::test]
    async fn pop_on_empty_buffer_returns_none_not_error() {
        let manager = BufferManager::new(zero_delay_config()).unwrap();
        assert_eq!(manager.pop_packet().await.unwrap(), None);
    }

    #[tokio::test]
    async fn out_of_order_pushes_still_pop_in_sequence_order() {
        let manager = BufferManager::new(zero_delay_config()).unwrap();
        manager.push_packet(make_packet(2)).await.unwrap();
        manager.push_packet(make_packet(0)).await.unwrap();
        manager.push_packet(make_packet(1)).await.unwrap();

        let mut sequences = Vec::new();
        while let Some(packet) = manager.pop_packet().await.unwrap() {
            sequences.push(packet.sequence_number);
        }
        assert_eq!(sequences, vec![0, 1, 2]);
    }

    #[tokio::test]
    async fn duplicate_push_is_rejected() {
        let manager = BufferManager::new(zero_delay_config()).unwrap();
        manager.push_packet(make_packet(0)).await.unwrap();
        let result = manager.push_packet(make_packet(0)).await;
        assert_eq!(result, Err(BufferError::DuplicatePacket { sequence_number: 0 }));
    }

    #[tokio::test]
    async fn missing_packet_is_skipped_without_blocking_later_delivery() {
        let manager = BufferManager::new(zero_delay_config()).unwrap();
        manager.push_packet(make_packet(0)).await.unwrap();
        manager.push_packet(make_packet(2)).await.unwrap();
        // Sequence 1 never arrives.

        let first = manager.pop_packet().await.unwrap().unwrap();
        let second = manager.pop_packet().await.unwrap().unwrap();
        assert_eq!([first.sequence_number, second.sequence_number], [0, 2]);

        let stats = manager.statistics().await;
        assert_eq!(stats.packet_loss_count, 1);
    }

    #[tokio::test]
    async fn overflow_beyond_max_buffer_size_drops_the_oldest_packet() {
        let mut config = zero_delay_config();
        config.max_buffer_size = 2;
        config.initial_buffer_size = 2;
        let manager = BufferManager::new(config).unwrap();

        manager.push_packet(make_packet(0)).await.unwrap();
        manager.push_packet(make_packet(1)).await.unwrap();
        manager.push_packet(make_packet(2)).await.unwrap();

        assert_eq!(manager.size(), 2);
        let stats = manager.statistics().await;
        assert_eq!(stats.dropped_packet_count, 1);
    }

    #[tokio::test]
    async fn clear_empties_the_buffer() {
        let manager = BufferManager::new(zero_delay_config()).unwrap();
        manager.push_packet(make_packet(0)).await.unwrap();
        manager.push_packet(make_packet(1)).await.unwrap();

        manager.clear().await.unwrap();

        assert!(manager.is_empty());
        assert_eq!(manager.pop_packet().await.unwrap(), None);
    }

    #[tokio::test]
    async fn reset_restores_a_fresh_buffer_and_allows_reusing_old_sequence_numbers() {
        let manager = BufferManager::new(zero_delay_config()).unwrap();
        manager.push_packet(make_packet(0)).await.unwrap();
        manager.pop_packet().await.unwrap();

        manager.reset().await.unwrap();

        assert!(manager.is_empty());
        // Sequence 0 was already delivered pre-reset; post-reset it's
        // admissible again since the watermark was cleared.
        assert!(manager.push_packet(make_packet(0)).await.is_ok());
    }

    #[tokio::test]
    async fn statistics_report_buffered_and_delivered_counts() {
        let manager = BufferManager::new(zero_delay_config()).unwrap();
        manager.push_packet(make_packet(0)).await.unwrap();
        manager.push_packet(make_packet(1)).await.unwrap();
        manager.pop_packet().await.unwrap();

        let stats = manager.statistics().await;
        assert_eq!(stats.packets_buffered, 2);
        assert_eq!(stats.packets_delivered, 1);
        assert_eq!(stats.current_buffer_size, 1);
    }

    #[tokio::test]
    async fn set_target_delay_out_of_range_is_rejected() {
        let manager = BufferManager::new(BufferConfig::for_tests()).unwrap();
        let too_high = BufferConfig::for_tests().max_target_delay + Duration::from_millis(1);
        let result = manager.set_target_delay(too_high).await;
        assert!(matches!(result, Err(BufferError::InvalidTargetDelay { .. })));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_producers_all_land_without_data_loss() {
        let mut config = zero_delay_config();
        config.max_buffer_size = 2000;
        config.duplicate_cache_size = 2000;
        let manager = BufferManager::new(config).unwrap();

        let mut handles = Vec::new();
        for producer in 0..4u64 {
            let manager = manager.clone();
            handles.push(tokio::spawn(async move {
                for i in 0..250u64 {
                    let sequence_number = producer * 250 + i;
                    manager.push_packet(make_packet(sequence_number)).await.unwrap();
                }
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }

        assert_eq!(manager.size(), 1000);
        let stats = manager.statistics().await;
        assert_eq!(stats.packets_buffered, 1000);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_consumers_never_double_deliver_a_packet() {
        let mut config = zero_delay_config();
        config.max_buffer_size = 2000;
        config.duplicate_cache_size = 2000;
        let manager = BufferManager::new(config).unwrap();

        for sequence_number in 0..1000u64 {
            manager.push_packet(make_packet(sequence_number)).await.unwrap();
        }

        let mut handles = Vec::new();
        for _ in 0..4 {
            let manager = manager.clone();
            handles.push(tokio::spawn(async move {
                let mut delivered = Vec::new();
                while let Some(packet) = manager.pop_packet().await.unwrap() {
                    delivered.push(packet.sequence_number);
                }
                delivered
            }));
        }

        let mut all_delivered = Vec::new();
        for handle in handles {
            all_delivered.extend(handle.await.unwrap());
        }

        all_delivered.sort_unstable();
        assert_eq!(all_delivered, (0..1000u64).collect::<Vec<u64>>());
    }

    #[tokio::test]
    async fn stress_thousands_of_packets_push_and_pop_cleanly() {
        let mut config = zero_delay_config();
        config.max_buffer_size = 10_000;
        config.duplicate_cache_size = 10_000;
        let manager = BufferManager::new(config).unwrap();

        for sequence_number in 0..5000u64 {
            manager.push_packet(make_packet(sequence_number)).await.unwrap();
        }

        let mut delivered = Vec::with_capacity(5000);
        while let Some(packet) = manager.pop_packet().await.unwrap() {
            delivered.push(packet.sequence_number);
        }

        assert_eq!(delivered, (0..5000u64).collect::<Vec<u64>>());
        let stats = manager.statistics().await;
        assert_eq!(stats.packets_delivered, 5000);
        assert_eq!(stats.dropped_packet_count, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn stress_one_hundred_thousand_packets_push_and_pop_cleanly() {
        const PACKET_COUNT: u64 = 100_001;
        let mut config = zero_delay_config();
        config.max_buffer_size = PACKET_COUNT as usize;
        config.initial_buffer_size = PACKET_COUNT as usize;
        config.duplicate_cache_size = PACKET_COUNT as usize;
        let manager = BufferManager::new(config).unwrap();

        for sequence_number in 0..PACKET_COUNT {
            manager.push_packet(make_packet(sequence_number)).await.unwrap();
        }

        for expected_sequence in 0..PACKET_COUNT {
            let packet = manager.pop_packet().await.unwrap().unwrap();
            assert_eq!(packet.sequence_number, expected_sequence);
        }

        let stats = manager.statistics().await;
        assert_eq!(stats.packets_buffered, PACKET_COUNT);
        assert_eq!(stats.packets_delivered, PACKET_COUNT);
        assert_eq!(stats.dropped_packet_count, 0);
        assert!(manager.is_empty());
    }
}
