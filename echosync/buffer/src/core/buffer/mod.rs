//! # EchoSync Buffer Layer
//!
//! Sits between the Streaming Engine and the (future) Synchronization
//! Engine. It intentionally has no knowledge of:
//! - Clock/playback synchronization across devices — that's the
//!   Synchronization Engine's job.
//! - Networking/transport (QUIC, discovery, etc.).
//! - Audio decoding or playback.
//!
//! ## Responsibilities
//! - Accept packets from the Streaming Engine
//!   ([`BufferManager::push_packet`]).
//! - Absorb network jitter by holding packets for a configurable target
//!   delay ([`config::BufferConfig::target_delay`]), adjusted
//!   automatically as arrival jitter changes.
//! - Reorder out-of-order packets and maintain sequence continuity
//!   ([`packet_window::PacketWindow`]).
//! - Detect and reject duplicate or already-processed packets.
//! - Deliver packets in playback order to the consumer
//!   ([`BufferManager::pop_packet`]).
//! - Expose runtime statistics for observability
//!   ([`BufferManager::statistics`]).
//!
//! ## Architecture
//! ```text
//!  Streaming Engine        BufferManager (async, thread-safe)        Synchronization Engine
//! ------------  push_packet()   ┌───────────────────────────┐   pop_packet()
//!  dequeue() ──────────────────▶│  JitterBuffer + PacketWindow │──────────────────▶  (future, out of scope)
//!                                └───────────────────────────┘
//! ```
//!
//! [`BufferManager`] is the only type most callers need: it wraps a
//! single-threaded [`jitter_buffer::JitterBuffer`] (packet storage,
//! target-delay release timing, adaptive delay) behind a
//! `tokio::sync::Mutex`, so multiple concurrent producer and consumer
//! tasks can share one buffer safely. The lower-level
//! [`jitter_buffer::JitterBuffer`] and [`packet_window::PacketWindow`]
//! types are exported too, for callers that want single-threaded, lock-
//! free access (e.g. inside an already-synchronized worker loop).
//!
//! ## Example
//! ```ignore
//! use buffer::core::buffer::{BufferManager, BufferConfig};
//!
//! # async fn run(packet: streaming::AudioPacket) -> Result<(), Box<dyn std::error::Error>> {
//! let manager = BufferManager::new(BufferConfig::default())?;
//! manager.push_packet(packet).await?;
//!
//! if let Some(next) = manager.pop_packet().await? {
//!     println!("ready for playback: sequence {}", next.sequence_number);
//! }
//! # Ok(())
//! # }
//! ```

pub mod buffer_manager;
pub mod config;
pub mod error;
pub mod jitter_buffer;
pub mod packet_window;

// Re-export the primary public types at the module root so downstream
// crates (the Synchronization Engine, once it exists) can write
// `buffer::BufferManager` instead of reaching into
// `buffer::core::buffer::buffer_manager::BufferManager`.
pub use buffer_manager::{BufferManager, BufferStatistics};
pub use config::BufferConfig;
pub use error::BufferError;
pub use jitter_buffer::{JitterBuffer, JitterBufferStatistics};
pub use packet_window::{PacketWindow, WindowStatistics};
