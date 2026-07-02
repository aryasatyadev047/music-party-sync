//! Sequence-number bookkeeping for the jitter buffer.
//!
//! `PacketWindow` is deliberately storage-free: it never holds packets or
//! payload data, only the sequence-number metadata needed to decide
//! whether an incoming packet is admissible, and to track how many
//! packets have been delivered, reordered, lost, or duplicated. Actual
//! packet storage and reordering-by-storage lives in
//! [`crate::core::buffer::jitter_buffer::JitterBuffer`], which owns one
//! `PacketWindow` and consults it on every insert and delivery.

use std::cmp::Ordering;
use std::collections::{HashSet, VecDeque};

use super::error::BufferError;

/// Point-in-time snapshot of a [`PacketWindow`]'s counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowStatistics {
    /// Packets admitted whose sequence number matched the expected
    /// watermark exactly (arrived in order).
    pub packets_in_order: u64,
    /// Packets admitted whose sequence number was ahead of the expected
    /// watermark (arrived out of order relative to what's been
    /// delivered so far).
    pub packets_reordered: u64,
    /// Total sequence-number gap observed across all deliveries: the
    /// sum, over every call to [`PacketWindow::advance`], of how many
    /// sequence numbers were skipped because they were never delivered.
    pub packets_lost: u64,
    /// Packets rejected because their sequence number had already been
    /// admitted (still buffered) or already delivered.
    pub duplicates_detected: u64,
    /// Packets admitted with a gap larger than the configured
    /// `max_missing_packets` tolerance, flagged as probable loss.
    pub late_packets: u64,
}

/// Tracks expected sequence numbers, detects loss and duplicates, and
/// reports statistics for a single packet stream.
///
/// `PacketWindow` does not store packets; it only answers "is this
/// sequence number admissible?" ([`PacketWindow::admit`]) and "a packet
/// with this sequence number was just delivered"
/// ([`PacketWindow::advance`]).
#[derive(Debug)]
pub struct PacketWindow {
    /// The next sequence number the window expects to see delivered.
    /// `None` until the first packet is delivered.
    expected_sequence: Option<u64>,
    /// Bounded FIFO of recently-delivered sequence numbers, used to
    /// evict entries from `seen_set` once the cache is full.
    seen_cache: VecDeque<u64>,
    /// Fast membership check mirroring `seen_cache`.
    seen_set: HashSet<u64>,
    /// Maximum number of entries retained in `seen_cache`/`seen_set`.
    cache_capacity: usize,
    /// Gap size (in sequence numbers) above which an admitted packet is
    /// flagged as a probable-loss "late packet" in statistics.
    max_missing_packets: u64,
    stats: WindowStatistics,
}

impl PacketWindow {
    /// Creates a new, empty `PacketWindow`.
    pub fn new(cache_capacity: usize, max_missing_packets: u64) -> Self {
        Self {
            expected_sequence: None,
            seen_cache: VecDeque::with_capacity(cache_capacity.min(1024)),
            seen_set: HashSet::with_capacity(cache_capacity.min(1024)),
            cache_capacity,
            max_missing_packets,
            stats: WindowStatistics::default(),
        }
    }

    /// Decides whether a packet with `sequence_number` may be admitted
    /// into the jitter buffer.
    ///
    /// Returns [`BufferError::DuplicatePacket`] if this exact sequence
    /// number was already delivered (and is still in the duplicate
    /// cache), or [`BufferError::AlreadyProcessed`] if it falls behind
    /// the delivery watermark but has aged out of the duplicate cache.
    /// Does not mutate delivery state — only [`PacketWindow::advance`]
    /// does that, once the packet is actually handed to the consumer.
    pub fn admit(&mut self, sequence_number: u64) -> Result<(), BufferError> {
        if self.seen_set.contains(&sequence_number) {
            self.stats.duplicates_detected += 1;
            return Err(BufferError::DuplicatePacket { sequence_number });
        }

        if let Some(expected) = self.expected_sequence {
            match sequence_number.cmp(&expected) {
                Ordering::Less => {
                    self.stats.duplicates_detected += 1;
                    return Err(BufferError::AlreadyProcessed { sequence_number });
                }
                Ordering::Equal => {
                    self.stats.packets_in_order += 1;
                }
                Ordering::Greater => {
                    self.stats.packets_reordered += 1;
                    if sequence_number - expected > self.max_missing_packets {
                        self.stats.late_packets += 1;
                    }
                }
            }
        } else {
            self.stats.packets_in_order += 1;
        }

        Ok(())
    }

    /// Records that a packet with `sequence_number` was just delivered
    /// to the consumer, advancing the expected-sequence watermark and
    /// remembering it in the duplicate cache. Returns the number of
    /// sequence numbers that were skipped (presumed lost) to reach this
    /// delivery.
    pub fn advance(&mut self, sequence_number: u64) -> u64 {
        let lost = match self.expected_sequence {
            Some(expected) if sequence_number > expected => sequence_number - expected,
            _ => 0,
        };
        self.stats.packets_lost += lost;

        let next_expected = sequence_number.saturating_add(1);
        self.expected_sequence = Some(match self.expected_sequence {
            Some(expected) => expected.max(next_expected),
            None => next_expected,
        });

        self.remember(sequence_number);
        lost
    }

    /// Inserts `sequence_number` into the bounded duplicate cache,
    /// evicting the oldest entry if it's already at capacity.
    fn remember(&mut self, sequence_number: u64) {
        if self.cache_capacity == 0 {
            return;
        }
        if self.seen_set.insert(sequence_number) {
            self.seen_cache.push_back(sequence_number);
            if self.seen_cache.len() > self.cache_capacity {
                if let Some(oldest) = self.seen_cache.pop_front() {
                    self.seen_set.remove(&oldest);
                }
            }
        }
    }

    /// The next sequence number this window expects to see delivered, or
    /// `None` if nothing has been delivered yet.
    pub fn expected_sequence(&self) -> Option<u64> {
        self.expected_sequence
    }

    /// A snapshot of this window's counters.
    pub fn statistics(&self) -> WindowStatistics {
        self.stats
    }

    /// Resets all tracked state (watermark, duplicate cache, and
    /// counters) back to a fresh window, keeping the configured capacity
    /// and tolerance.
    pub fn reset(&mut self) {
        self.expected_sequence = None;
        self.seen_cache.clear();
        self.seen_set.clear();
        self.stats = WindowStatistics::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_packet_is_always_admitted_and_counted_in_order() {
        let mut window = PacketWindow::new(8, 3);
        assert!(window.admit(10).is_ok());
        assert_eq!(window.statistics().packets_in_order, 1);
    }

    #[test]
    fn in_order_admit_then_advance_moves_watermark_forward() {
        let mut window = PacketWindow::new(8, 3);
        window.admit(0).unwrap();
        let lost = window.advance(0);
        assert_eq!(lost, 0);
        assert_eq!(window.expected_sequence(), Some(1));

        window.admit(1).unwrap();
        window.advance(1);
        assert_eq!(window.expected_sequence(), Some(2));
    }

    #[test]
    fn out_of_order_admit_is_counted_as_reordered() {
        let mut window = PacketWindow::new(8, 3);
        window.admit(0).unwrap();
        window.advance(0);

        // Sequence 2 arrives before sequence 1: still admissible (ahead
        // of the watermark), but counted as reordered.
        window.admit(2).unwrap();
        assert_eq!(window.statistics().packets_reordered, 1);
    }

    #[test]
    fn gap_beyond_tolerance_is_flagged_late() {
        let mut window = PacketWindow::new(8, 2);
        window.admit(0).unwrap();
        window.advance(0);

        window.admit(10).unwrap();
        assert_eq!(window.statistics().late_packets, 1);
    }

    #[test]
    fn delivered_packet_reporting_as_duplicate_is_rejected() {
        let mut window = PacketWindow::new(8, 3);
        window.admit(5).unwrap();
        window.advance(5);

        let result = window.admit(5);
        assert_eq!(result, Err(BufferError::DuplicatePacket { sequence_number: 5 }));
    }

    #[test]
    fn sequence_behind_watermark_and_evicted_from_cache_is_already_processed() {
        let mut window = PacketWindow::new(2, 3);
        window.admit(0).unwrap();
        window.advance(0);
        window.admit(1).unwrap();
        window.advance(1);
        window.admit(2).unwrap();
        window.advance(2);
        // Duplicate cache capacity is 2, so sequence 0 has been evicted
        // by now but the watermark (3) still rejects it as stale.
        let result = window.admit(0);
        assert_eq!(result, Err(BufferError::AlreadyProcessed { sequence_number: 0 }));
    }

    #[test]
    fn advance_reports_lost_count_for_skipped_sequence_numbers() {
        let mut window = PacketWindow::new(8, 10);
        window.admit(0).unwrap();
        window.advance(0);

        // Sequence numbers 1..5 never arrive; sequence 5 is delivered
        // next (e.g. because it aged out and was force-delivered).
        window.admit(5).unwrap();
        let lost = window.advance(5);
        assert_eq!(lost, 4);
        assert_eq!(window.statistics().packets_lost, 4);
    }

    #[test]
    fn reset_clears_watermark_cache_and_stats() {
        let mut window = PacketWindow::new(8, 3);
        window.admit(0).unwrap();
        window.advance(0);
        window.admit(1).unwrap();

        window.reset();

        assert_eq!(window.expected_sequence(), None);
        assert_eq!(window.statistics(), WindowStatistics::default());
        // Sequence 0 is admissible again post-reset.
        assert!(window.admit(0).is_ok());
    }

    #[test]
    fn zero_capacity_cache_never_remembers_but_watermark_still_rejects() {
        let mut window = PacketWindow::new(0, 3);
        window.admit(0).unwrap();
        window.advance(0);
        // Not in the (disabled) cache, but still behind the watermark.
        assert_eq!(window.admit(0), Err(BufferError::AlreadyProcessed { sequence_number: 0 }));
    }
}
