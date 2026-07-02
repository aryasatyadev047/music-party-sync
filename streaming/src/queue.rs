//! A thread-safe, async, FIFO packet queue.
//!
//! Backed by a bounded `tokio::sync::mpsc` channel, which gives us:
//! - FIFO ordering (guaranteed by `mpsc`).
//! - Non-blocking overflow detection via `try_send`.
//! - Cheap multi-producer support out of the box (`Sender` is `Clone`).
//!
//! `mpsc::Receiver` only supports a single consumer, so to allow multiple
//! *consumer tasks* to `dequeue` concurrently (as required by the engine's
//! worker pool), the receiving half is shared behind a `tokio::sync::Mutex`.
//! Contention is limited to the brief moment a task pulls the next packet
//! off the channel, so this remains a low-overhead, fair FIFO queue
//! without requiring a fully lock-free MPMC data structure.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tracing::warn;

use crate::error::StreamingError;
use crate::packet::AudioPacket;

/// A bounded, thread-safe FIFO queue of [`AudioPacket`]s.
///
/// Cheap to clone: cloning a `PacketQueue` clones the underlying channel
/// handles (`Arc`-backed), so all clones observe the same queue.
#[derive(Clone)]
pub struct PacketQueue {
    sender: mpsc::Sender<AudioPacket>,
    receiver: Arc<AsyncMutex<mpsc::Receiver<AudioPacket>>>,
    capacity: usize,
    /// Approximate current queue length, tracked independently of the
    /// channel's internal state so `len()`/`status()` calls never need to
    /// lock the receiver.
    length: Arc<AtomicUsize>,
}

impl PacketQueue {
    /// Creates a new queue bounded to `capacity` packets.
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(capacity.max(1));

        Self {
            sender,
            receiver: Arc::new(AsyncMutex::new(receiver)),
            capacity,
            length: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Attempts to push `packet` onto the back of the queue without
    /// blocking.
    ///
    /// Returns [`StreamingError::QueueOverflow`] if the queue is at
    /// capacity, and [`StreamingError::QueueClosed`] if every receiver has
    /// been dropped (i.e. the engine has torn down this queue).
    pub fn enqueue(&self, packet: AudioPacket) -> Result<(), StreamingError> {
        match self.sender.try_send(packet) {
            Ok(()) => {
                self.length.fetch_add(1, Ordering::AcqRel);
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!(capacity = self.capacity, "queue overflow: packet dropped");
                Err(StreamingError::QueueOverflow {
                    capacity: self.capacity,
                })
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(StreamingError::QueueClosed),
        }
    }

    /// Pops the next packet off the front of the queue, waiting
    /// asynchronously if the queue is currently empty.
    ///
    /// This method is cancel-safe: it is suitable for use inside
    /// `tokio::select!` without risk of losing a packet that was already
    /// pulled off the channel.
    ///
    /// Returns [`StreamingError::QueueClosed`] once the queue has been
    /// shut down and drained.
    pub async fn dequeue(&self) -> Result<AudioPacket, StreamingError> {
        let mut receiver = self.receiver.lock().await;
        match receiver.recv().await {
            Some(packet) => {
                self.length.fetch_sub(1, Ordering::AcqRel);
                Ok(packet)
            }
            None => Err(StreamingError::QueueClosed),
        }
    }

    /// Removes every packet currently buffered in the queue without
    /// processing them, leaving the queue open for further use.
    pub async fn clear(&self) {
        let mut receiver = self.receiver.lock().await;
        let mut drained = 0_usize;
        while receiver.try_recv().is_ok() {
            drained += 1;
        }
        if drained > 0 {
            self.length.fetch_sub(drained, Ordering::AcqRel);
        }
    }

    /// Returns the approximate number of packets currently buffered.
    ///
    /// This is a best-effort snapshot: in the presence of concurrent
    /// producers/consumers the true length may change immediately after
    /// this call returns.
    pub fn len(&self) -> usize {
        self.length.load(Ordering::Acquire)
    }

    /// Returns `true` if the queue currently holds no packets.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the queue's configured maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::PacketFlags;

    fn make_packet(sequence_number: u64) -> AudioPacket {
        AudioPacket::new(
            sequence_number,
            sequence_number,
            vec![0_u8; 4],
            PacketFlags::new(),
            "sender",
            "session",
            4000,
        )
        .expect("test packet should construct")
    }

    #[tokio::test]
    async fn enqueue_then_dequeue_preserves_fifo_order() {
        let queue = PacketQueue::new(8);

        for i in 0..5 {
            queue.enqueue(make_packet(i)).expect("enqueue should succeed");
        }

        for i in 0..5 {
            let packet = queue.dequeue().await.expect("dequeue should succeed");
            assert_eq!(packet.sequence_number, i);
        }
    }

    #[tokio::test]
    async fn enqueue_reports_overflow_at_capacity() {
        let queue = PacketQueue::new(2);

        queue.enqueue(make_packet(0)).unwrap();
        queue.enqueue(make_packet(1)).unwrap();

        let result = queue.enqueue(make_packet(2));
        assert_eq!(result, Err(StreamingError::QueueOverflow { capacity: 2 }));
    }

    #[tokio::test]
    async fn len_tracks_enqueue_and_dequeue() {
        let queue = PacketQueue::new(4);
        assert_eq!(queue.len(), 0);

        queue.enqueue(make_packet(0)).unwrap();
        queue.enqueue(make_packet(1)).unwrap();
        assert_eq!(queue.len(), 2);

        queue.dequeue().await.unwrap();
        assert_eq!(queue.len(), 1);
    }

    #[tokio::test]
    async fn clear_drains_all_buffered_packets() {
        let queue = PacketQueue::new(4);
        queue.enqueue(make_packet(0)).unwrap();
        queue.enqueue(make_packet(1)).unwrap();
        queue.enqueue(make_packet(2)).unwrap();

        queue.clear().await;

        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[tokio::test]
    async fn concurrent_producers_all_packets_delivered() {
        let queue = PacketQueue::new(64);
        let mut handles = Vec::new();

        for producer_id in 0..8_u64 {
            let queue = queue.clone();
            handles.push(tokio::spawn(async move {
                for i in 0..8_u64 {
                    let seq = producer_id * 8 + i;
                    queue.enqueue(make_packet(seq)).expect("enqueue should succeed");
                }
            }));
        }

        for handle in handles {
            handle.await.expect("producer task should not panic");
        }

        let mut received = Vec::new();
        for _ in 0..64 {
            let packet = queue.dequeue().await.expect("dequeue should succeed");
            received.push(packet.sequence_number);
        }

        received.sort_unstable();
        let expected: Vec<u64> = (0..64).collect();
        assert_eq!(received, expected);
    }

    #[tokio::test]
    async fn concurrent_consumers_each_packet_delivered_exactly_once() {
        let queue = PacketQueue::new(64);
        for i in 0..64_u64 {
            queue.enqueue(make_packet(i)).unwrap();
        }

        let results = Arc::new(AsyncMutex::new(Vec::new()));
        let mut handles = Vec::new();

        for _ in 0..8 {
            let queue = queue.clone();
            let results = Arc::clone(&results);
            handles.push(tokio::spawn(async move {
                for _ in 0..8 {
                    let packet = queue.dequeue().await.expect("dequeue should succeed");
                    results.lock().await.push(packet.sequence_number);
                }
            }));
        }

        for handle in handles {
            handle.await.expect("consumer task should not panic");
        }

        let mut received = results.lock().await.clone();
        received.sort_unstable();
        let expected: Vec<u64> = (0..64).collect();
        assert_eq!(received, expected);
    }
}
