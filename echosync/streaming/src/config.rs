//! Configuration for the EchoSync Streaming Engine.
//!
//! All tunable values for packet queueing, timeouts, retries, and worker
//! concurrency live here so the engine's runtime behavior can be adjusted
//! from a single, well-known location.

use std::time::Duration;

/// Default maximum number of packets that may sit in a single queue
/// (outbound or inbound) before `enqueue` reports overflow.
pub const DEFAULT_QUEUE_CAPACITY: usize = 512;

/// Default time an operation (e.g. a blocking dequeue used by
/// `receive_packet`'s waiters) will wait before giving up.
pub const DEFAULT_PACKET_TIMEOUT_MS: u64 = 200;

/// Default maximum size, in bytes, of a single packet's encoded Opus
/// payload. This intentionally matches the Media Layer's
/// `MAX_PACKET_SIZE_BYTES` (4000 bytes) so packets that pass the codec
/// boundary are never rejected here.
pub const DEFAULT_MAX_PACKET_SIZE_BYTES: usize = 4000;

/// Default number of times the engine will retry handing a packet to the
/// Transport Layer before giving up and reporting a send failure.
pub const DEFAULT_MAX_RETRY_COUNT: u32 = 3;

/// Default number of background worker tasks draining the outbound queue
/// and forwarding packets to the Transport Layer.
pub const DEFAULT_WORKER_COUNT: usize = 2;

/// Default delay between retry attempts when a send to the Transport
/// Layer fails.
pub const DEFAULT_RETRY_BACKOFF_MS: u64 = 20;

/// Runtime configuration for a [`crate::engine::StreamingEngine`].
///
/// Constructed with [`StreamingConfig::default`] for sane production
/// defaults, or built explicitly field-by-field for tests and tuning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamingConfig {
    /// Maximum number of packets allowed to sit in the outbound queue
    /// (Media Layer -> Transport Layer) at once.
    pub outbound_queue_capacity: usize,

    /// Maximum number of packets allowed to sit in the inbound queue
    /// (Transport Layer -> playback pipeline) at once.
    pub inbound_queue_capacity: usize,

    /// How long a packet send/receive operation may take before it is
    /// considered timed out.
    pub packet_timeout: Duration,

    /// Maximum allowed size, in bytes, of a single packet's encoded
    /// payload. Packets larger than this are rejected at ingestion.
    pub max_packet_size_bytes: usize,

    /// Number of retry attempts for a failed send to the Transport Layer.
    pub max_retry_count: u32,

    /// Delay between retry attempts.
    pub retry_backoff: Duration,

    /// Number of background worker tasks draining the outbound queue.
    pub worker_count: usize,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            outbound_queue_capacity: DEFAULT_QUEUE_CAPACITY,
            inbound_queue_capacity: DEFAULT_QUEUE_CAPACITY,
            packet_timeout: Duration::from_millis(DEFAULT_PACKET_TIMEOUT_MS),
            max_packet_size_bytes: DEFAULT_MAX_PACKET_SIZE_BYTES,
            max_retry_count: DEFAULT_MAX_RETRY_COUNT,
            retry_backoff: Duration::from_millis(DEFAULT_RETRY_BACKOFF_MS),
            worker_count: DEFAULT_WORKER_COUNT,
        }
    }
}

impl StreamingConfig {
    /// Convenience constructor for tests: small queues, no retries, a
    /// single worker, and short timeouts so tests run fast and
    /// deterministically.
    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self {
            outbound_queue_capacity: 8,
            inbound_queue_capacity: 8,
            packet_timeout: Duration::from_millis(50),
            max_packet_size_bytes: DEFAULT_MAX_PACKET_SIZE_BYTES,
            max_retry_count: 1,
            retry_backoff: Duration::from_millis(1),
            worker_count: 1,
        }
    }
}
