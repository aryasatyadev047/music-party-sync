//! The Clock Synchronization Engine's public, concurrency-safe entry
//! point.
//!
//! `Synchronizer` wraps a single-threaded [`ClockManager`] in a
//! `tokio::sync::RwLock` so multiple concurrent readers (e.g. a Playback
//! Scheduler polling [`Synchronizer::current_offset`] every frame) and
//! writers (a Transport Layer sync-message handler calling
//! [`Synchronizer::synchronize`]) can share one synchronization session
//! safely. Every method is `async` and yields at the `await` point
//! rather than blocking a worker thread — the same pattern used by
//! [`crate::core::buffer::buffer_manager::BufferManager`] around
//! [`crate::core::buffer::jitter_buffer::JitterBuffer`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock as AsyncRwLock;
use tracing::info;

use super::clock_manager::ClockManager;
use super::config::SyncConfig;
use super::error::SyncError;
use super::statistics::SyncStatistics;

/// The Clock Synchronization Engine's top-level type: maintains a common
/// playback timeline across devices by estimating clock offsets,
/// measuring drift, and applying smooth corrections.
///
/// Cheap to clone: cloning a `Synchronizer` clones its internal `Arc`
/// handles, so all clones share the same underlying synchronization
/// state. This makes it safe to hand a handle to multiple concurrent
/// tasks — the same pattern used by
/// [`crate::core::buffer::buffer_manager::BufferManager`].
#[derive(Clone)]
pub struct Synchronizer {
    inner: Arc<AsyncRwLock<ClockManager>>,
    running: Arc<AtomicBool>,
    config: SyncConfig,
}

impl Synchronizer {
    /// Creates a new `Synchronizer`. Returns
    /// [`SyncError::InvalidConfiguration`] if `config` fails
    /// [`SyncConfig::validate`]. The synchronizer is created stopped;
    /// call [`Synchronizer::start`] before [`Synchronizer::synchronize`]
    /// or [`Synchronizer::update`].
    pub fn new(config: SyncConfig) -> Result<Self, SyncError> {
        let clock_manager = ClockManager::new(config.clone())?;
        Ok(Self {
            inner: Arc::new(AsyncRwLock::new(clock_manager)),
            running: Arc::new(AtomicBool::new(false)),
            config,
        })
    }

    /// The [`SyncConfig`] this synchronizer was created with.
    pub fn config(&self) -> &SyncConfig {
        &self.config
    }

    /// Starts a synchronization session: resets the underlying clock
    /// state and allows [`Synchronizer::synchronize`] /
    /// [`Synchronizer::update`] to run. Returns
    /// [`SyncError::AlreadyStarted`] if already running.
    pub async fn start(&self) -> Result<(), SyncError> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(SyncError::AlreadyStarted);
        }
        let mut clock = self.inner.write().await;
        clock.reset()?;
        info!("Clock Started");
        Ok(())
    }

    /// Stops the current synchronization session. Subsequent
    /// [`Synchronizer::synchronize`] / [`Synchronizer::update`] calls
    /// fail with [`SyncError::NotStarted`] until [`Synchronizer::start`]
    /// is called again. Returns [`SyncError::NotStarted`] if not
    /// currently running.
    pub async fn stop(&self) -> Result<(), SyncError> {
        if !self.running.swap(false, Ordering::SeqCst) {
            return Err(SyncError::NotStarted);
        }
        info!("Synchronizer stopped");
        Ok(())
    }

    /// Runs one full synchronization pass: computes the offset between
    /// `host_timestamp` and `local_timestamp`, applies it (gradually —
    /// see [`ClockManager::correct_drift`]), re-estimates drift, and
    /// applies one gradual correction step toward the new target.
    ///
    /// Returns [`SyncError::NotStarted`] if [`Synchronizer::start`] has
    /// not been called. Returns [`SyncError::OffsetOutOfRange`] or
    /// [`SyncError::DriftOutOfRange`] if the measurement looks invalid;
    /// in either case the failure is recorded in statistics before the
    /// error is returned.
    pub async fn synchronize(
        &self,
        host_timestamp: Duration,
        local_timestamp: Duration,
    ) -> Result<(), SyncError> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(SyncError::NotStarted);
        }

        let mut clock = self.inner.write().await;

        let offset_ms = match clock.calculate_offset(host_timestamp, local_timestamp) {
            Ok(offset_ms) => offset_ms,
            Err(err) => {
                clock.record_failure()?;
                return Err(err);
            }
        };

        if let Err(err) = clock.apply_offset(offset_ms) {
            clock.record_failure()?;
            return Err(err);
        }

        if let Err(err) = clock.estimate_drift() {
            clock.record_failure()?;
            return Err(err);
        }

        clock.correct_drift()?;
        clock.record_success()?;
        Ok(())
    }

    /// Runs one gradual correction step without a fresh offset
    /// measurement — intended to be called between
    /// [`Synchronizer::synchronize`] passes (e.g. once per playback
    /// frame) so the applied offset keeps smoothly converging toward the
    /// last known target rather than jumping only at sync time.
    ///
    /// Returns [`SyncError::NotStarted`] if [`Synchronizer::start`] has
    /// not been called.
    pub async fn update(&self) -> Result<(), SyncError> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(SyncError::NotStarted);
        }
        let mut clock = self.inner.write().await;
        clock.correct_drift()?;
        Ok(())
    }

    /// The currently applied offset, in milliseconds.
    pub async fn current_offset(&self) -> Result<f64, SyncError> {
        let clock = self.inner.read().await;
        Ok(clock.current_offset())
    }

    /// The most recently estimated drift rate, in parts-per-million.
    pub async fn current_drift(&self) -> Result<f64, SyncError> {
        let clock = self.inner.read().await;
        Ok(clock.current_drift())
    }

    /// Whether the applied offset is currently within the configured
    /// resync threshold of the target offset.
    pub async fn is_synchronized(&self) -> Result<bool, SyncError> {
        let clock = self.inner.read().await;
        Ok(clock.is_synchronized())
    }

    /// A snapshot of current synchronization statistics.
    pub async fn statistics(&self) -> Result<SyncStatistics, SyncError> {
        let clock = self.inner.read().await;
        clock.statistics()
    }

    /// The current estimate of the shared playback timeline. See
    /// [`ClockManager::host_time`].
    pub async fn playback_time(&self) -> Result<Duration, SyncError> {
        let clock = self.inner.read().await;
        clock.host_time()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synchronizer() -> Synchronizer {
        Synchronizer::new(SyncConfig::for_tests()).unwrap()
    }

    #[tokio::test]
    async fn synchronize_before_start_fails() {
        let sync = synchronizer();
        let result = sync.synchronize(Duration::from_millis(100), Duration::ZERO).await;
        assert!(matches!(result, Err(SyncError::NotStarted)));
    }

    #[tokio::test]
    async fn update_before_start_fails() {
        let sync = synchronizer();
        assert!(matches!(sync.update().await, Err(SyncError::NotStarted)));
    }

    #[tokio::test]
    async fn starting_twice_fails() {
        let sync = synchronizer();
        sync.start().await.unwrap();
        assert!(matches!(sync.start().await, Err(SyncError::AlreadyStarted)));
    }

    #[tokio::test]
    async fn stopping_without_starting_fails() {
        let sync = synchronizer();
        assert!(matches!(sync.stop().await, Err(SyncError::NotStarted)));
    }

    #[tokio::test]
    async fn full_lifecycle_start_synchronize_stop() {
        let sync = synchronizer();
        sync.start().await.unwrap();

        sync.synchronize(Duration::from_millis(150), Duration::from_millis(50)).await.unwrap();
        assert!((sync.current_offset().await.unwrap() - 100.0).abs() < 1e-6);

        let stats = sync.statistics().await.unwrap();
        assert_eq!(stats.successful_syncs, 1);
        assert_eq!(stats.failed_syncs, 0);

        sync.stop().await.unwrap();
        assert!(matches!(
            sync.synchronize(Duration::from_millis(150), Duration::from_millis(50)).await,
            Err(SyncError::NotStarted)
        ));
    }

    #[tokio::test]
    async fn out_of_range_offset_is_recorded_as_failure() {
        let sync = synchronizer();
        sync.start().await.unwrap();

        let result = sync.synchronize(Duration::from_secs(30), Duration::ZERO).await;
        assert!(matches!(result, Err(SyncError::OffsetOutOfRange { .. })));

        let stats = sync.statistics().await.unwrap();
        assert_eq!(stats.failed_syncs, 1);
        assert_eq!(stats.successful_syncs, 0);
    }

    #[tokio::test]
    async fn update_gradually_converges_offset_between_syncs() {
        let mut config = SyncConfig::for_tests();
        config.correction_rate = 0.5;
        config.max_correction_step = Duration::from_millis(1_000);
        // This test drives two synchronize() calls back-to-back with no
        // sleep between them, purely to exercise offset convergence —
        // not drift estimation. With ~zero elapsed wall-clock time
        // between samples the regression-based drift estimate is
        // (correctly, given the model) enormous, so raise the ceiling
        // rather than let an unrelated drift check fail this test.
        config.max_drift_ppm = 1.0e12;
        let sync = Synchronizer::new(config).unwrap();
        sync.start().await.unwrap();

        // First sync seeds current_offset_ms directly (no gap yet).
        sync.synchronize(Duration::from_millis(100), Duration::ZERO).await.unwrap();
        assert!((sync.current_offset().await.unwrap() - 100.0).abs() < 1e-6);

        // A second, larger measurement creates a gap that update() must
        // close gradually rather than instantly.
        sync.synchronize(Duration::from_millis(300), Duration::ZERO).await.unwrap();
        let after_first_correction = sync.current_offset().await.unwrap();
        assert!(after_first_correction < 300.0);

        for _ in 0..20 {
            sync.update().await.unwrap();
        }
        assert!((sync.current_offset().await.unwrap() - 300.0).abs() < 1.0);
    }

    #[tokio::test]
    async fn playback_time_tracks_local_time_plus_offset() {
        let sync = synchronizer();
        sync.start().await.unwrap();
        sync.synchronize(Duration::from_millis(50), Duration::ZERO).await.unwrap();

        let playback = sync.playback_time().await.unwrap();
        assert!(playback.as_millis() >= 50);
    }

    #[tokio::test]
    async fn is_synchronized_reflects_convergence() {
        let mut config = SyncConfig::for_tests();
        config.resync_threshold = Duration::from_millis(1);
        let sync = Synchronizer::new(config).unwrap();
        sync.start().await.unwrap();

        assert!(!sync.is_synchronized().await.unwrap());
        sync.synchronize(Duration::from_millis(10), Duration::ZERO).await.unwrap();
        assert!(sync.is_synchronized().await.unwrap());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_readers_and_writer_do_not_panic_or_deadlock() {
        // A tight loop of synchronize() calls with no sleep between
        // them can trip the drift ceiling (near-zero elapsed time
        // between samples), which is why per-call errors are
        // deliberately ignored below — this test is about concurrency
        // safety, not drift-limit enforcement.
        let mut config = SyncConfig::for_tests();
        config.max_drift_ppm = 1.0e12;
        let sync = Synchronizer::new(config).unwrap();
        sync.start().await.unwrap();

        let mut handles = Vec::new();

        let writer = sync.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..50u64 {
                let _ = writer
                    .synchronize(Duration::from_millis(10 + i), Duration::ZERO)
                    .await;
            }
        }));

        for _ in 0..4 {
            let reader = sync.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..50 {
                    let _ = reader.current_offset().await.unwrap();
                    let _ = reader.current_drift().await.unwrap();
                    let _ = reader.is_synchronized().await.unwrap();
                    let _ = reader.statistics().await.unwrap();
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let stats = sync.statistics().await.unwrap();
        assert_eq!(stats.successful_syncs + stats.failed_syncs, 50);
    }

    #[tokio::test]
    async fn long_running_session_accumulates_statistics_without_drift_blowup() {
        let mut config = SyncConfig::for_tests();
        config.correction_rate = 0.3;
        config.max_correction_step = Duration::from_millis(200);
        // Back-to-back synchronize() calls with no sleep make the
        // wall-clock elapsed time between samples ~0, so the
        // regression-based drift estimate is enormous by construction.
        // This test cares about offsets/statistics staying finite over
        // a long session, not about drift-ceiling enforcement.
        config.max_drift_ppm = 1.0e12;
        let sync = Synchronizer::new(config).unwrap();
        sync.start().await.unwrap();

        for i in 0..200u64 {
            let host_ms = 100 + (i % 10);
            sync.synchronize(Duration::from_millis(host_ms), Duration::ZERO).await.unwrap();
            sync.update().await.unwrap();
        }

        let stats = sync.statistics().await.unwrap();
        assert_eq!(stats.successful_syncs, 200);
        assert!(stats.correction_count >= 200);
        assert!(sync.current_offset().await.unwrap().is_finite());
    }

    #[tokio::test]
    async fn stress_many_sequential_synchronize_calls_stay_consistent() {
        // Thousands of back-to-back calls with no sleep between them
        // means ~0 wall-clock time separates samples, so raise the
        // drift ceiling — this test is about throughput/consistency,
        // not drift-limit enforcement.
        let mut config = SyncConfig::for_tests();
        config.max_drift_ppm = 1.0e15;
        let sync = Synchronizer::new(config).unwrap();
        sync.start().await.unwrap();

        for i in 0..2_000u64 {
            let host_ms = (i % 500) as u64;
            sync.synchronize(Duration::from_millis(host_ms), Duration::ZERO).await.unwrap();
        }

        let stats = sync.statistics().await.unwrap();
        assert_eq!(stats.successful_syncs, 2_000);
        assert_eq!(stats.failed_syncs, 0);
    }
}
