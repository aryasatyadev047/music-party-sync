//! # EchoSync Playback Scheduler
//!
//! Decides exactly *when* each audio packet should be released for
//! playback. It does not play audio, decode Opus, or touch the network
//! — it only computes playback deadlines and hands packets back out in
//! deadline order once they're due.
//!
//! It intentionally has no knowledge of:
//! - Networking/transport, device discovery, or security — those are
//!   separate workstreams.
//! - Audio decoding or actual playback (that's the future Audio Output
//!   module's job; this crate hands it release-ready
//!   [`streaming::AudioPacket`]s in order).
//! - How packets got buffered or reordered (that's the Buffer Layer's
//!   job; this crate only consumes its public API).
//! - How this device's clock is kept in sync with the shared session
//!   clock (that's the Clock Synchronization Engine's job; this crate
//!   only reads [`buffer::Synchronizer::current_offset`]).
//!
//! ## Responsibilities
//! - Accept packets pulled from [`buffer::BufferManager::pop_packet`]
//!   ([`scheduler::PlaybackScheduler::schedule_packet`]).
//! - Compute a playback deadline from a packet's `timestamp_ms`, the
//!   configured playback latency, and the Clock Synchronization
//!   Engine's current offset ([`timeline::PlaybackTimeline`]).
//! - Hold packets in deadline order and hand them back out once due
//!   ([`scheduler::PlaybackScheduler::next_packet`],
//!   [`playback_queue::PlaybackQueue`]).
//! - Drop packets that are excessively late, and reject duplicates.
//! - Expose runtime statistics for observability
//!   ([`scheduler::PlaybackScheduler::statistics`]).
//!
//! ## Architecture
//! ```text
//!  Buffer Layer          PlaybackScheduler (async, thread-safe)         (future) Audio Output
//! ------------  pop_packet()  ┌──────────────────────────────────┐   next_packet()
//!  reordered  ─────────────────▶│  PlaybackTimeline + PlaybackQueue │──────────────▶  release-ready packets, in order
//!  packets                      └──────────────────────────────────┘
//!                                        ▲
//!                                        │ current_offset()
//!                                Clock Synchronization Engine
//! ```
//!
//! [`scheduler::PlaybackScheduler`] is the only type most callers need:
//! it wraps a single-threaded [`playback_queue::PlaybackQueue`] behind a
//! `tokio::sync::Mutex`, so multiple concurrent producer (Buffer Layer
//! consumer) and consumer (future Audio Output) tasks can share one
//! scheduling session safely. The lower-level
//! [`playback_queue::PlaybackQueue`] and [`timeline::PlaybackTimeline`]
//! types are exported too, for callers that want single-threaded,
//! lock-free access.
//!
//! ## Example
//! ```ignore
//! use buffer::{BufferManager, BufferConfig, Synchronizer, SyncConfig};
//! use scheduler::{PlaybackScheduler, SchedulerConfig};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let buffer = BufferManager::new(BufferConfig::default())?;
//! let synchronizer = Synchronizer::new(SyncConfig::default())?;
//! synchronizer.start().await?;
//!
//! let scheduler = PlaybackScheduler::new(SchedulerConfig::default(), synchronizer)?;
//! scheduler.start().await?;
//!
//! if let Some(packet) = buffer.pop_packet().await? {
//!     scheduler.schedule_packet(packet).await?;
//! }
//!
//! if let Some(ready) = scheduler.next_packet().await? {
//!     // Hand `ready` to the (future) Audio Output module.
//!     println!("release packet {}", ready.packet_id);
//! }
//!
//! scheduler.stop().await?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod error;
pub mod playback_queue;
pub mod scheduler;
pub mod statistics;
pub mod timeline;

// Re-export the primary public types at the crate root so downstream
// crates (the future Audio Output module) can write
// `scheduler::PlaybackScheduler` instead of reaching into
// `scheduler::scheduler::PlaybackScheduler`.
pub use config::SchedulerConfig;
pub use error::SchedulerError;
pub use playback_queue::{PlaybackQueue, ScheduledPacket};
pub use scheduler::PlaybackScheduler;
pub use statistics::{SchedulerStatistics, StatisticsTracker};
pub use timeline::PlaybackTimeline;
