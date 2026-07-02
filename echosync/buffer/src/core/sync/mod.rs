//! # EchoSync Clock Synchronization Engine
//!
//! Maintains a common playback timeline across all connected devices. It
//! intentionally has no knowledge of:
//! - Networking/transport (QUIC, discovery, etc.) — host-clock
//!   timestamps are handed in by the caller, not fetched over the wire.
//! - Audio decoding, buffering, or playback.
//! - Device discovery or security/encryption.
//!
//! ## Responsibilities
//! - Estimate the offset between this device's local clock and the
//!   host (reference) device's clock
//!   ([`clock_manager::ClockManager::calculate_offset`]).
//! - Track clock drift over time via linear regression over recent
//!   offset samples ([`drift_estimator::DriftEstimator`]).
//! - Apply corrections gradually, never as an abrupt time jump
//!   ([`clock_manager::ClockManager::correct_drift`]), so long-running
//!   sessions stay smooth.
//! - Expose synchronization statistics for observability
//!   ([`synchronizer::Synchronizer::statistics`]).
//!
//! ## Architecture
//! ```text
//!  (future) Transport Layer         Synchronizer (async, thread-safe)        (future) Playback Scheduler
//! ------------  synchronize(host_ts, local_ts)  ┌───────────────────────────┐   playback_time() / current_offset()
//!  sync message ─────────────────────────────────▶│  ClockManager + DriftEstimator │──────────────────▶  consumes synchronized timestamps
//!                                                  └───────────────────────────┘
//! ```
//!
//! [`synchronizer::Synchronizer`] is the only type most callers need: it
//! wraps a single-threaded [`clock_manager::ClockManager`] (offset
//! tracking, drift estimation, gradual correction) behind a
//! `tokio::sync::RwLock`, so multiple concurrent reader and writer tasks
//! can share one synchronization session safely. The lower-level
//! [`clock_manager::ClockManager`] and [`drift_estimator::DriftEstimator`]
//! types are exported too, for callers that want single-threaded,
//! lock-free access (e.g. inside an already-synchronized worker loop).
//!
//! ## Example
//! ```ignore
//! use buffer::core::sync::{Synchronizer, SyncConfig};
//! use std::time::Duration;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let synchronizer = Synchronizer::new(SyncConfig::default())?;
//! synchronizer.start().await?;
//!
//! // `host_timestamp` would come from a Transport Layer sync message;
//! // `local_timestamp` from `synchronizer`'s own clock at receipt time.
//! synchronizer.synchronize(Duration::from_millis(1050), Duration::from_millis(1000)).await?;
//!
//! let playback_time = synchronizer.playback_time().await?;
//! println!("shared timeline is at {:?}", playback_time);
//!
//! synchronizer.stop().await?;
//! # Ok(())
//! # }
//! ```

pub mod clock_manager;
pub mod config;
pub mod drift_estimator;
pub mod error;
pub mod statistics;
pub mod synchronizer;

// Re-export the primary public types at the module root so downstream
// crates (the future Playback Scheduler) can write `sync::Synchronizer`
// instead of reaching into
// `sync::synchronizer::Synchronizer`.
pub use clock_manager::{duration_to_millis, millis_to_duration, ClockManager};
pub use config::SyncConfig;
pub use drift_estimator::DriftEstimator;
pub use error::SyncError;
pub use statistics::{StatisticsTracker, SyncStatistics};
pub use synchronizer::Synchronizer;
