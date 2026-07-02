//! The Playback Scheduler's single-threaded ordering core: a
//! deadline-ordered (earliest-deadline-first) queue of packets awaiting
//! release.
//!
//! [`PlaybackQueue`] is the single-threaded analogue of
//! `buffer::core::buffer::jitter_buffer::JitterBuffer`: it owns the
//! actual storage and ordering logic, and is wrapped in a
//! `tokio::sync::Mutex` by [`crate::scheduler::PlaybackScheduler`] so
//! multiple producer/consumer tasks can share one queue safely without
//! this type needing to know anything about async or locking itself.

use std::collections::{BinaryHeap, HashSet};
use std::time::Duration;

use streaming::AudioPacket;

use crate::error::SchedulerError;

/// A packet paired with its computed playback deadline, as held inside
/// [`PlaybackQueue`].
#[derive(Debug, Clone)]
pub struct ScheduledPacket {
    /// The underlying audio packet, unmodified.
    pub packet: AudioPacket,
    /// The playback timeline moment (see
    /// [`crate::timeline::PlaybackTimeline`]) at which this packet
    /// should be released.
    pub deadline: Duration,
    /// The playback timeline moment at which this packet was scheduled
    /// (enqueued), used for scheduling-delay statistics.
    pub scheduled_at: Duration,
}

impl PartialEq for ScheduledPacket {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline && self.packet.packet_id == other.packet.packet_id
    }
}
impl Eq for ScheduledPacket {}

impl PartialOrd for ScheduledPacket {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledPacket {
    /// Orders earliest-deadline-first when used as a max-heap key: the
    /// packet with the *smallest* deadline compares as *greatest*, so
    /// [`std::collections::BinaryHeap::pop`] returns it first. Ties
    /// break on `sequence_number` then `packet_id` for a fully
    /// deterministic playback order.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .deadline
            .cmp(&self.deadline)
            .then_with(|| other.packet.sequence_number.cmp(&self.packet.sequence_number))
            .then_with(|| other.packet.packet_id.cmp(&self.packet.packet_id))
    }
}

/// Single-threaded, deadline-ordered playback queue. Not thread-safe on
/// its own; concurrency safety is layered on top by
/// [`crate::scheduler::PlaybackScheduler`].
#[derive(Debug)]
pub struct PlaybackQueue {
    heap: BinaryHeap<ScheduledPacket>,
    ids: HashSet<u64>,
    capacity: usize,
}

impl PlaybackQueue {
    /// Creates an empty queue that holds at most `capacity` packets.
    pub fn new(capacity: usize) -> Self {
        Self { heap: BinaryHeap::new(), ids: HashSet::new(), capacity: capacity.max(1) }
    }

    /// Inserts a packet in deadline order. Returns
    /// [`SchedulerError::QueueFull`] if the queue is already at
    /// capacity, or [`SchedulerError::DuplicatePacket`] if a packet with
    /// the same `packet_id` is already queued.
    pub fn enqueue(&mut self, item: ScheduledPacket) -> Result<(), SchedulerError> {
        if self.ids.contains(&item.packet.packet_id) {
            return Err(SchedulerError::DuplicatePacket { packet_id: item.packet.packet_id });
        }
        if self.ids.len() >= self.capacity {
            return Err(SchedulerError::QueueFull { capacity: self.capacity });
        }
        self.ids.insert(item.packet.packet_id);
        self.heap.push(item);
        Ok(())
    }

    /// Removes and returns the packet with the earliest deadline, if
    /// any.
    pub fn dequeue(&mut self) -> Result<Option<ScheduledPacket>, SchedulerError> {
        match self.heap.pop() {
            Some(item) => {
                self.ids.remove(&item.packet.packet_id);
                Ok(Some(item))
            }
            None => Ok(None),
        }
    }

    /// Returns a clone of the packet with the earliest deadline, without
    /// removing it.
    pub fn peek(&self) -> Result<Option<ScheduledPacket>, SchedulerError> {
        Ok(self.heap.peek().cloned())
    }

    /// Removes a specific packet by `packet_id`, wherever it sits in
    /// deadline order. Returns `Ok(true)` if a matching packet was
    /// found and removed, `Ok(false)` otherwise.
    pub fn cancel(&mut self, packet_id: u64) -> Result<bool, SchedulerError> {
        if !self.ids.remove(&packet_id) {
            return Ok(false);
        }
        let remaining: Vec<ScheduledPacket> =
            self.heap.drain().filter(|item| item.packet.packet_id != packet_id).collect();
        self.heap = remaining.into_iter().collect();
        Ok(true)
    }

    /// Number of packets currently queued.
    pub fn size(&self) -> usize {
        self.heap.len()
    }

    /// `true` if no packets are currently queued.
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// `true` if the queue is at capacity.
    pub fn is_full(&self) -> bool {
        self.ids.len() >= self.capacity
    }

    /// Removes every queued packet.
    pub fn clear(&mut self) -> Result<(), SchedulerError> {
        self.heap.clear();
        self.ids.clear();
        Ok(())
    }

    /// `true` if a packet with this `packet_id` is currently queued.
    pub fn contains(&self, packet_id: u64) -> bool {
        self.ids.contains(&packet_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use streaming::PacketFlags;

    fn packet(packet_id: u64, sequence_number: u64) -> AudioPacket {
        AudioPacket::new(
            packet_id,
            sequence_number,
            vec![0u8; 8],
            PacketFlags::new(),
            "device-a",
            "session-1",
            4000,
        )
        .unwrap()
    }

    fn item(packet_id: u64, sequence_number: u64, deadline_ms: u64) -> ScheduledPacket {
        ScheduledPacket {
            packet: packet(packet_id, sequence_number),
            deadline: Duration::from_millis(deadline_ms),
            scheduled_at: Duration::ZERO,
        }
    }

    #[test]
    fn enqueue_then_dequeue_round_trips() {
        let mut queue = PlaybackQueue::new(10);
        queue.enqueue(item(1, 0, 100)).unwrap();

        let popped = queue.dequeue().unwrap().unwrap();
        assert_eq!(popped.packet.packet_id, 1);
        assert!(queue.is_empty());
    }

    #[test]
    fn dequeue_returns_earliest_deadline_first() {
        let mut queue = PlaybackQueue::new(10);
        queue.enqueue(item(1, 0, 300)).unwrap();
        queue.enqueue(item(2, 1, 100)).unwrap();
        queue.enqueue(item(3, 2, 200)).unwrap();

        let mut order = Vec::new();
        while let Some(p) = queue.dequeue().unwrap() {
            order.push(p.packet.packet_id);
        }
        assert_eq!(order, vec![2, 3, 1]);
    }

    #[test]
    fn equal_deadlines_break_ties_by_sequence_number() {
        let mut queue = PlaybackQueue::new(10);
        queue.enqueue(item(1, 5, 100)).unwrap();
        queue.enqueue(item(2, 2, 100)).unwrap();
        queue.enqueue(item(3, 8, 100)).unwrap();

        let mut order = Vec::new();
        while let Some(p) = queue.dequeue().unwrap() {
            order.push(p.packet.sequence_number);
        }
        assert_eq!(order, vec![2, 5, 8]);
    }

    #[test]
    fn peek_does_not_remove() {
        let mut queue = PlaybackQueue::new(10);
        queue.enqueue(item(1, 0, 100)).unwrap();

        let peeked = queue.peek().unwrap().unwrap();
        assert_eq!(peeked.packet.packet_id, 1);
        assert_eq!(queue.size(), 1);
    }

    #[test]
    fn peek_and_dequeue_on_empty_queue_return_none() {
        let mut queue = PlaybackQueue::new(10);
        assert_eq!(queue.peek().unwrap(), None);
        assert_eq!(queue.dequeue().unwrap(), None);
    }

    #[test]
    fn duplicate_packet_id_is_rejected() {
        let mut queue = PlaybackQueue::new(10);
        queue.enqueue(item(1, 0, 100)).unwrap();
        let result = queue.enqueue(item(1, 1, 200));
        assert_eq!(result, Err(SchedulerError::DuplicatePacket { packet_id: 1 }));
    }

    #[test]
    fn queue_full_is_rejected() {
        let mut queue = PlaybackQueue::new(2);
        queue.enqueue(item(1, 0, 100)).unwrap();
        queue.enqueue(item(2, 1, 200)).unwrap();

        let result = queue.enqueue(item(3, 2, 300));
        assert_eq!(result, Err(SchedulerError::QueueFull { capacity: 2 }));
    }

    #[test]
    fn cancel_removes_a_specific_packet_regardless_of_position() {
        let mut queue = PlaybackQueue::new(10);
        queue.enqueue(item(1, 0, 300)).unwrap();
        queue.enqueue(item(2, 1, 100)).unwrap();
        queue.enqueue(item(3, 2, 200)).unwrap();

        assert!(queue.cancel(2).unwrap());
        assert!(!queue.contains(2));
        assert_eq!(queue.size(), 2);

        let mut order = Vec::new();
        while let Some(p) = queue.dequeue().unwrap() {
            order.push(p.packet.packet_id);
        }
        assert_eq!(order, vec![3, 1]);
    }

    #[test]
    fn cancel_unknown_packet_returns_false() {
        let mut queue = PlaybackQueue::new(10);
        assert!(!queue.cancel(999).unwrap());
    }

    #[test]
    fn clear_empties_the_queue() {
        let mut queue = PlaybackQueue::new(10);
        queue.enqueue(item(1, 0, 100)).unwrap();
        queue.enqueue(item(2, 1, 200)).unwrap();

        queue.clear().unwrap();

        assert!(queue.is_empty());
        assert!(!queue.contains(1));
        assert_eq!(queue.dequeue().unwrap(), None);
    }

    #[test]
    fn is_full_reflects_capacity() {
        let mut queue = PlaybackQueue::new(1);
        assert!(!queue.is_full());
        queue.enqueue(item(1, 0, 100)).unwrap();
        assert!(queue.is_full());
    }

    #[test]
    fn stress_many_packets_dequeue_in_deadline_order() {
        let mut queue = PlaybackQueue::new(5000);
        for i in 0..2000u64 {
            // Insert in a scrambled order but with unique deadlines.
            let deadline = (2000 - i) as u64;
            queue.enqueue(item(i, i, deadline)).unwrap();
        }

        let mut last_deadline = 0u64;
        let mut count = 0;
        while let Some(p) = queue.dequeue().unwrap() {
            assert!(p.deadline.as_millis() as u64 >= last_deadline);
            last_deadline = p.deadline.as_millis() as u64;
            count += 1;
        }
        assert_eq!(count, 2000);
    }
}
