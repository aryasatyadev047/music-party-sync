//! The single-threaded core of the Buffer Layer.
//!
//! `JitterBuffer` owns packet storage (a [`BTreeMap`] keyed by sequence
//! number, which keeps buffered packets in playback order for free) and
//! delegates all sequence-number bookkeeping to a
//! [`PacketWindow`](super::packet_window::PacketWindow). It has no
//! internal locking or `async` methods — concurrent, multi-producer /
//! multi-consumer access is layered on top by
//! [`crate::core::buffer::buffer_manager::BufferManager`], which wraps a
//! `JitterBuffer` in a `tokio::sync::Mutex`.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use tracing::{debug, trace, warn};

use streaming::AudioPacket;

use super::config::BufferConfig;
use super::error::BufferError;
use super::packet_window::{PacketWindow, WindowStatistics};

/// A buffered packet plus the local time it was accepted, used to judge
/// both target-delay release timing and staleness eviction.
#[derive(Debug, Clone)]
struct BufferedEntry {
    packet: AudioPacket,
    arrival: Instant,
}

/// Point-in-time snapshot of a [`JitterBuffer`]'s counters.
#[derive(Debug, Clone, Copy, Default)]
pub struct JitterBufferStatistics {
    /// Total packets ever accepted into the buffer.
    pub packets_buffered: u64,
    /// Total packets ever released to the consumer via
    /// [`JitterBuffer::next_packet`] or [`JitterBuffer::flush`].
    pub packets_delivered: u64,
    /// Total packets dropped without delivery, due to overflow or
    /// exceeding `max_packet_age`.
    pub packets_dropped: u64,
    /// The highest number of packets simultaneously held in the buffer.
    pub max_occupancy: usize,
    /// Running RFC 3550-style estimate of inter-arrival jitter, in
    /// milliseconds.
    pub average_jitter_ms: f64,
    /// Sequence-tracking statistics from the underlying
    /// [`PacketWindow`].
    pub window: WindowStatistics,
}

/// Absorbs network jitter and delivers packets to the consumer in
/// playback order.
///
/// Packets are held for at least `target_delay` before becoming eligible
/// for release via [`JitterBuffer::next_packet`], which smooths out
/// arrival-time variance. `target_delay` can be adjusted manually
/// ([`JitterBuffer::set_target_delay`]) or automatically, based on
/// observed jitter, when `config.adaptive_enabled` is set.
pub struct JitterBuffer {
    config: BufferConfig,
    entries: BTreeMap<u64, BufferedEntry>,
    window: PacketWindow,
    target_delay: Duration,
    last_packet_timestamp_ms: Option<u64>,
    last_arrival: Option<Instant>,
    jitter_estimate_ms: f64,
    packets_buffered_total: u64,
    packets_delivered_total: u64,
    packets_dropped_total: u64,
    max_occupancy: usize,
}

impl JitterBuffer {
    /// Creates a new, empty `JitterBuffer`. Returns
    /// [`BufferError::InvalidConfiguration`] if `config` fails
    /// [`BufferConfig::validate`].
    pub fn new(config: BufferConfig) -> Result<Self, BufferError> {
        config.validate()?;
        let window = PacketWindow::new(config.duplicate_cache_size, config.max_missing_packets);
        let target_delay = config.target_delay;

        Ok(Self {
            config,
            entries: BTreeMap::new(),
            window,
            target_delay,
            last_packet_timestamp_ms: None,
            last_arrival: None,
            jitter_estimate_ms: 0.0,
            packets_buffered_total: 0,
            packets_delivered_total: 0,
            packets_dropped_total: 0,
            max_occupancy: 0,
        })
    }

    /// Accepts a packet from the Streaming Engine, storing it in
    /// sequence order.
    ///
    /// Rejects duplicates ([`BufferError::DuplicatePacket`]) and
    /// already-processed packets ([`BufferError::AlreadyProcessed`]).
    /// If the buffer is at `max_buffer_size`, the oldest buffered packet
    /// is dropped to make room (logged as a buffer overflow) rather than
    /// rejecting the new arrival outright, since holding on to the
    /// stalest packet is rarely useful for real-time playback.
    pub fn insert_packet(&mut self, packet: AudioPacket) -> Result<(), BufferError> {
        let sequence_number = packet.sequence_number;

        if self.entries.contains_key(&sequence_number) {
            warn!(sequence_number, "Duplicate Packet");
            return Err(BufferError::DuplicatePacket { sequence_number });
        }

        self.window.admit(sequence_number)?;

        if let Some(expected) = self.window.expected_sequence() {
            if sequence_number != expected {
                debug!(sequence_number, expected, "Packet Reordered");
            }
        }

        if self.entries.len() >= self.config.max_buffer_size {
            if let Some((&oldest_sequence, _)) = self.entries.iter().next() {
                self.entries.remove(&oldest_sequence);
                self.packets_dropped_total += 1;
                warn!(
                    dropped_sequence = oldest_sequence,
                    capacity = self.config.max_buffer_size,
                    "Buffer Overflow"
                );
            }
        }

        let now = Instant::now();
        self.update_jitter_estimate(&packet, now);

        self.entries.insert(sequence_number, BufferedEntry { packet, arrival: now });
        self.packets_buffered_total += 1;
        if self.entries.len() > self.max_occupancy {
            self.max_occupancy = self.entries.len();
        }

        if self.config.adaptive_enabled {
            self.adapt_target_delay();
        }

        debug!(sequence_number, buffered = self.entries.len(), "Packet Buffered");
        Ok(())
    }

    /// Updates the running RFC 3550-style jitter estimate:
    /// `J += (|D| - J) / 16`, where `D` is the difference between the
    /// gap in arrival times and the gap in the packets' own capture
    /// timestamps.
    fn update_jitter_estimate(&mut self, packet: &AudioPacket, now: Instant) {
        if let (Some(last_ts), Some(last_arrival)) =
            (self.last_packet_timestamp_ms, self.last_arrival)
        {
            let arrival_gap_ms = now.saturating_duration_since(last_arrival).as_secs_f64() * 1000.0;
            let send_gap_ms = packet.timestamp_ms as f64 - last_ts as f64;
            let d = (arrival_gap_ms - send_gap_ms).abs();
            self.jitter_estimate_ms += (d - self.jitter_estimate_ms) / 16.0;
        }
        self.last_packet_timestamp_ms = Some(packet.timestamp_ms);
        self.last_arrival = Some(now);
    }

    /// Nudges `target_delay` toward a value with enough headroom to
    /// absorb the currently observed jitter, moving at most
    /// `config.adaptive_step` per packet arrival so playback delay
    /// doesn't jump abruptly.
    fn adapt_target_delay(&mut self) {
        let jitter_headroom = Duration::from_secs_f64((self.jitter_estimate_ms.max(0.0) * 4.0) / 1000.0);
        let candidate = self
            .config
            .target_delay
            .saturating_add(jitter_headroom)
            .clamp(self.config.min_target_delay, self.config.max_target_delay);

        let step = self.config.adaptive_step;
        let stepped = if candidate > self.target_delay {
            (self.target_delay + step).min(candidate)
        } else if candidate < self.target_delay {
            self.target_delay.saturating_sub(step).max(candidate)
        } else {
            self.target_delay
        };
        let new_delay = stepped.clamp(self.config.min_target_delay, self.config.max_target_delay);

        if new_delay != self.target_delay {
            trace!(
                previous_ms = self.target_delay.as_millis() as u64,
                new_ms = new_delay.as_millis() as u64,
                "Adaptive target delay adjusted"
            );
            self.target_delay = new_delay;
        }
    }

    /// Drops any buffered packets that have exceeded `max_packet_age`,
    /// counting each as dropped rather than delivered.
    fn evict_stale(&mut self) {
        let max_age = self.config.max_packet_age;
        let stale_sequences: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.arrival.elapsed() > max_age)
            .map(|(&sequence_number, _)| sequence_number)
            .collect();

        for sequence_number in stale_sequences {
            self.entries.remove(&sequence_number);
            self.packets_dropped_total += 1;
            warn!(sequence_number, max_age_ms = max_age.as_millis() as u64, "Packet Dropped");
        }
    }

    /// Returns the next packet, in sequence order, if it has sat in the
    /// buffer for at least `target_delay`. Returns `Ok(None)` (not an
    /// error) if the buffer is empty or the earliest packet isn't ready
    /// yet — the caller (typically polling on an interval) should treat
    /// that as "nothing to deliver right now."
    pub fn next_packet(&mut self) -> Result<Option<AudioPacket>, BufferError> {
        self.evict_stale();

        let ready_sequence = match self.entries.iter().next() {
            Some((&sequence_number, entry)) if entry.arrival.elapsed() >= self.target_delay => {
                Some(sequence_number)
            }
            Some(_) => None,
            None => {
                trace!("Buffer Underflow");
                None
            }
        };

        let Some(sequence_number) = ready_sequence else {
            return Ok(None);
        };

        let entry = self
            .entries
            .remove(&sequence_number)
            .expect("sequence_number was just observed as the map's first key");
        let lost = self.window.advance(sequence_number);
        if lost > 0 {
            warn!(sequence_number, lost, "Packet Dropped");
        }
        self.packets_delivered_total += 1;
        debug!(sequence_number, remaining = self.entries.len(), "Packet Released");
        Ok(Some(entry.packet))
    }

    /// Drains every buffered packet immediately, in sequence order,
    /// ignoring `target_delay`. Used for shutdown/handoff paths where
    /// the caller wants everything that's currently buffered right now.
    pub fn flush(&mut self) -> Vec<AudioPacket> {
        let sequences: Vec<u64> = self.entries.keys().copied().collect();
        let mut drained = Vec::with_capacity(sequences.len());

        for sequence_number in sequences {
            if let Some(entry) = self.entries.remove(&sequence_number) {
                self.window.advance(sequence_number);
                self.packets_delivered_total += 1;
                drained.push(entry.packet);
            }
        }

        debug!(count = drained.len(), "Packet Released via flush");
        drained
    }

    /// Clears all buffered packets and sequence-tracking state, and
    /// restores `target_delay` to `config.target_delay`. Configuration
    /// itself is preserved.
    pub fn reset(&mut self) {
        self.entries.clear();
        self.window.reset();
        self.target_delay = self.config.target_delay;
        self.last_packet_timestamp_ms = None;
        self.last_arrival = None;
        self.jitter_estimate_ms = 0.0;
        debug!("Buffer Reset");
    }

    /// Sets the target buffering delay. Returns
    /// [`BufferError::InvalidTargetDelay`] if `delay` falls outside
    /// `[config.min_target_delay, config.max_target_delay]`.
    pub fn set_target_delay(&mut self, delay: Duration) -> Result<(), BufferError> {
        if delay < self.config.min_target_delay || delay > self.config.max_target_delay {
            return Err(BufferError::InvalidTargetDelay {
                requested_ms: delay.as_millis() as u64,
                min_ms: self.config.min_target_delay.as_millis() as u64,
                max_ms: self.config.max_target_delay.as_millis() as u64,
            });
        }
        self.target_delay = delay;
        Ok(())
    }

    /// The currently active target buffering delay.
    pub fn current_delay(&self) -> Duration {
        self.target_delay
    }

    /// Number of packets currently held in the buffer.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` if no packets are currently buffered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// A clone of the earliest (lowest sequence number) buffered packet,
    /// without removing it.
    pub fn peek(&self) -> Option<&AudioPacket> {
        self.entries.values().next().map(|entry| &entry.packet)
    }

    /// A snapshot of this buffer's runtime counters.
    pub fn statistics(&self) -> JitterBufferStatistics {
        JitterBufferStatistics {
            packets_buffered: self.packets_buffered_total,
            packets_delivered: self.packets_delivered_total,
            packets_dropped: self.packets_dropped_total,
            max_occupancy: self.max_occupancy,
            average_jitter_ms: self.jitter_estimate_ms,
            window: self.window.statistics(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        // Staleness eviction is a separate concern (covered by
        // `stale_packet_beyond_max_age_is_dropped_on_next_packet`); give
        // it plenty of headroom here so pure ordering/throughput tests
        // that push many packets before draining aren't racing the
        // wall clock against `max_packet_age`.
        config.max_packet_age = Duration::from_secs(3600);
        config
    }

    #[test]
    fn insert_then_next_packet_round_trips_in_order() {
        let mut buffer = JitterBuffer::new(zero_delay_config()).unwrap();
        buffer.insert_packet(make_packet(0)).unwrap();

        let packet = buffer.next_packet().unwrap();
        assert_eq!(packet.unwrap().sequence_number, 0);
    }

    #[test]
    fn packets_are_delivered_in_sequence_order_despite_out_of_order_arrival() {
        let mut buffer = JitterBuffer::new(zero_delay_config()).unwrap();
        buffer.insert_packet(make_packet(2)).unwrap();
        buffer.insert_packet(make_packet(0)).unwrap();
        buffer.insert_packet(make_packet(1)).unwrap();

        let first = buffer.next_packet().unwrap().unwrap();
        let second = buffer.next_packet().unwrap().unwrap();
        let third = buffer.next_packet().unwrap().unwrap();
        assert_eq!([first.sequence_number, second.sequence_number, third.sequence_number], [0, 1, 2]);
    }

    #[test]
    fn duplicate_packet_is_rejected() {
        let mut buffer = JitterBuffer::new(zero_delay_config()).unwrap();
        buffer.insert_packet(make_packet(0)).unwrap();
        let result = buffer.insert_packet(make_packet(0));
        assert_eq!(result, Err(BufferError::DuplicatePacket { sequence_number: 0 }));
    }

    #[test]
    fn already_delivered_packet_arriving_again_is_rejected() {
        let mut buffer = JitterBuffer::new(zero_delay_config()).unwrap();
        buffer.insert_packet(make_packet(0)).unwrap();
        buffer.next_packet().unwrap();

        let result = buffer.insert_packet(make_packet(0));
        assert_eq!(result, Err(BufferError::DuplicatePacket { sequence_number: 0 }));
    }

    #[test]
    fn missing_packet_does_not_block_delivery_of_the_next_available_one() {
        let mut buffer = JitterBuffer::new(zero_delay_config()).unwrap();
        // Sequence 0 never arrives; sequence 1 should still be
        // deliverable once its own target delay has elapsed.
        buffer.insert_packet(make_packet(1)).unwrap();

        let packet = buffer.next_packet().unwrap().unwrap();
        assert_eq!(packet.sequence_number, 1);
    }

    #[test]
    fn next_packet_returns_none_before_target_delay_elapses() {
        let mut config = BufferConfig::for_tests();
        config.target_delay = Duration::from_millis(200);
        config.min_target_delay = Duration::from_millis(200);
        config.max_target_delay = Duration::from_millis(200);
        config.adaptive_enabled = false;

        let mut buffer = JitterBuffer::new(config).unwrap();
        buffer.insert_packet(make_packet(0)).unwrap();

        // Buffer Underflow: nothing ready yet, but this is Ok(None), not
        // an error.
        assert_eq!(buffer.next_packet().unwrap(), None);
    }

    #[test]
    fn overflow_drops_oldest_packet_to_make_room() {
        let mut config = zero_delay_config();
        config.max_buffer_size = 2;
        config.initial_buffer_size = 2;
        let mut buffer = JitterBuffer::new(config).unwrap();

        buffer.insert_packet(make_packet(0)).unwrap();
        buffer.insert_packet(make_packet(1)).unwrap();
        buffer.insert_packet(make_packet(2)).unwrap();

        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.statistics().packets_dropped, 1);
        // Sequence 0 was the oldest and should have been evicted.
        assert!(buffer.peek().unwrap().sequence_number >= 1);
    }

    #[test]
    fn stale_packet_beyond_max_age_is_dropped_on_next_packet() {
        let mut config = BufferConfig::for_tests();
        config.max_packet_age = Duration::from_millis(1);
        config.target_delay = Duration::from_millis(500);
        config.min_target_delay = Duration::from_millis(500);
        config.max_target_delay = Duration::from_millis(500);
        config.adaptive_enabled = false;

        let mut buffer = JitterBuffer::new(config).unwrap();
        buffer.insert_packet(make_packet(0)).unwrap();
        std::thread::sleep(Duration::from_millis(20));

        // The packet is older than max_packet_age, so it's evicted as
        // stale even though target_delay hasn't been reached.
        assert_eq!(buffer.next_packet().unwrap(), None);
        assert_eq!(buffer.statistics().packets_dropped, 1);
    }

    #[test]
    fn flush_drains_everything_immediately_in_order() {
        let mut config = BufferConfig::for_tests();
        config.target_delay = Duration::from_secs(10);
        config.min_target_delay = Duration::from_millis(1);
        config.max_target_delay = Duration::from_secs(20);
        config.adaptive_enabled = false;
        let mut buffer = JitterBuffer::new(config).unwrap();

        buffer.insert_packet(make_packet(2)).unwrap();
        buffer.insert_packet(make_packet(0)).unwrap();
        buffer.insert_packet(make_packet(1)).unwrap();

        let drained = buffer.flush();
        let sequences: Vec<u64> = drained.iter().map(|p| p.sequence_number).collect();
        assert_eq!(sequences, vec![0, 1, 2]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn reset_clears_buffered_packets_and_restores_configured_delay() {
        let mut buffer = JitterBuffer::new(zero_delay_config()).unwrap();
        buffer.insert_packet(make_packet(0)).unwrap();
        buffer.set_target_delay(Duration::from_millis(0)).unwrap();

        buffer.reset();

        assert!(buffer.is_empty());
        assert_eq!(buffer.current_delay(), buffer.config.target_delay);
    }

    #[test]
    fn set_target_delay_rejects_values_outside_configured_bounds() {
        let mut buffer = JitterBuffer::new(BufferConfig::for_tests()).unwrap();
        let too_low = Duration::from_millis(0);
        let result = buffer.set_target_delay(too_low);
        assert!(matches!(result, Err(BufferError::InvalidTargetDelay { .. })));
    }

    #[test]
    fn set_target_delay_within_bounds_succeeds() {
        let mut buffer = JitterBuffer::new(BufferConfig::for_tests()).unwrap();
        let config = BufferConfig::for_tests();
        let mid = (config.min_target_delay + config.max_target_delay) / 2;
        assert!(buffer.set_target_delay(mid).is_ok());
        assert_eq!(buffer.current_delay(), mid);
    }

    #[test]
    fn adaptive_delay_grows_when_jitter_is_high() {
        let mut config = BufferConfig::for_tests();
        config.target_delay = Duration::from_millis(10);
        config.min_target_delay = Duration::from_millis(5);
        config.max_target_delay = Duration::from_millis(100);
        config.adaptive_step = Duration::from_millis(20);
        config.adaptive_enabled = true;
        let mut buffer = JitterBuffer::new(config).unwrap();

        let initial_delay = buffer.current_delay();

        // Feed packets whose capture timestamps are evenly spaced but
        // whose *arrival* spacing is wildly uneven, to synthesize high
        // jitter.
        let mut packet = make_packet(0);
        packet.timestamp_ms = 0;
        buffer.insert_packet(packet).unwrap();

        std::thread::sleep(Duration::from_millis(5));
        let mut packet = make_packet(1);
        packet.timestamp_ms = 20;
        buffer.insert_packet(packet).unwrap();

        std::thread::sleep(Duration::from_millis(60));
        let mut packet = make_packet(2);
        packet.timestamp_ms = 40;
        buffer.insert_packet(packet).unwrap();

        assert!(buffer.statistics().average_jitter_ms > 0.0);
        assert!(buffer.current_delay() >= initial_delay);
    }

    #[test]
    fn statistics_reflect_buffered_and_delivered_counts() {
        let mut buffer = JitterBuffer::new(zero_delay_config()).unwrap();
        buffer.insert_packet(make_packet(0)).unwrap();
        buffer.insert_packet(make_packet(1)).unwrap();
        buffer.next_packet().unwrap();

        let stats = buffer.statistics();
        assert_eq!(stats.packets_buffered, 2);
        assert_eq!(stats.packets_delivered, 1);
        assert_eq!(stats.max_occupancy, 2);
    }

    #[test]
    fn stress_thousands_of_out_of_order_packets_deliver_in_sequence() {
        let mut config = zero_delay_config();
        config.max_buffer_size = 5000;
        config.duplicate_cache_size = 5000;
        let mut buffer = JitterBuffer::new(config).unwrap();

        let mut sequences: Vec<u64> = (0..4000).collect();
        // Deterministic shuffle: reverse-ish interleave, no external RNG
        // dependency needed.
        sequences.sort_by_key(|&s| (s * 2654435761u64) % 4000);

        for &sequence_number in &sequences {
            buffer.insert_packet(make_packet(sequence_number)).unwrap();
        }

        let mut delivered = Vec::with_capacity(4000);
        while let Some(packet) = buffer.next_packet().unwrap() {
            delivered.push(packet.sequence_number);
        }

        assert_eq!(delivered, (0..4000).collect::<Vec<u64>>());
        assert_eq!(buffer.statistics().packets_delivered, 4000);
    }
}
