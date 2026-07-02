//! Clock drift estimation for the EchoSync Clock Synchronization Engine.
//!
//! [`DriftEstimator`] keeps a bounded, sliding window of `(elapsed
//! seconds, offset milliseconds)` samples and fits a simple linear
//! regression through them. The slope of that fit is the clock's drift
//! rate — how fast the measured offset is growing or shrinking over time
//! — reported in parts-per-million (ppm), the standard unit for clock
//! skew (e.g. "this device's clock runs 50 ppm fast" means it gains 50
//! microseconds every second).

use std::collections::VecDeque;

/// Single-threaded accumulator that estimates clock drift from a
/// sliding window of offset samples. Not thread-safe on its own;
/// concurrency safety is layered on top by
/// [`crate::core::sync::clock_manager::ClockManager`] and
/// [`crate::core::sync::synchronizer::Synchronizer`].
#[derive(Debug, Clone)]
pub struct DriftEstimator {
    window_size: usize,
    samples: VecDeque<(f64, f64)>,
}

impl DriftEstimator {
    /// Creates a new, empty `DriftEstimator` retaining at most
    /// `window_size` samples. `window_size` is clamped to a minimum of 2
    /// (the minimum needed for a linear regression).
    pub fn new(window_size: usize) -> Self {
        Self { window_size: window_size.max(2), samples: VecDeque::with_capacity(window_size) }
    }

    /// Records a new `(elapsed_secs, offset_ms)` sample: `elapsed_secs`
    /// is time since some fixed reference point (typically the owning
    /// [`crate::core::sync::clock_manager::ClockManager`]'s epoch), and
    /// `offset_ms` is the raw measured offset at that time. Evicts the
    /// oldest sample once `window_size` is exceeded.
    pub fn record_sample(&mut self, elapsed_secs: f64, offset_ms: f64) {
        if self.samples.len() >= self.window_size {
            self.samples.pop_front();
        }
        self.samples.push_back((elapsed_secs, offset_ms));
    }

    /// Number of samples currently held in the window.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Estimates the current drift rate, in parts-per-million, by
    /// fitting an ordinary-least-squares line through the samples in the
    /// window and converting its slope (milliseconds of offset drift per
    /// second of elapsed time) to ppm.
    ///
    /// Returns `0.0` if fewer than two samples have been recorded, or if
    /// the samples don't have enough time spread to fit a line (all
    /// `elapsed_secs` values identical).
    pub fn estimate_ppm(&self) -> f64 {
        let n = self.samples.len();
        if n < 2 {
            return 0.0;
        }

        let n_f = n as f64;
        let sum_x: f64 = self.samples.iter().map(|(x, _)| x).sum();
        let sum_y: f64 = self.samples.iter().map(|(_, y)| y).sum();
        let sum_xy: f64 = self.samples.iter().map(|(x, y)| x * y).sum();
        let sum_xx: f64 = self.samples.iter().map(|(x, _)| x * x).sum();

        let denominator = n_f * sum_xx - sum_x * sum_x;
        if denominator.abs() < f64::EPSILON {
            return 0.0;
        }

        // Slope of the best-fit line, in milliseconds of offset per
        // second of elapsed time.
        let slope_ms_per_sec = (n_f * sum_xy - sum_x * sum_y) / denominator;

        // Convert ms/s to ppm: (ms/s) / 1000 gives a dimensionless
        // seconds-per-second ratio; multiplying by 1e6 expresses that
        // ratio in parts-per-million.
        slope_ms_per_sec * 1000.0
    }

    /// Clears every recorded sample.
    pub fn reset(&mut self) {
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_estimator_reports_zero_drift() {
        let estimator = DriftEstimator::new(10);
        assert_eq!(estimator.estimate_ppm(), 0.0);
    }

    #[test]
    fn single_sample_reports_zero_drift() {
        let mut estimator = DriftEstimator::new(10);
        estimator.record_sample(0.0, 5.0);
        assert_eq!(estimator.estimate_ppm(), 0.0);
    }

    #[test]
    fn constant_offset_reports_zero_drift() {
        let mut estimator = DriftEstimator::new(10);
        for t in 0..10 {
            estimator.record_sample(t as f64, 42.0);
        }
        assert!(estimator.estimate_ppm().abs() < 1e-6);
    }

    #[test]
    fn linearly_growing_offset_reports_expected_drift() {
        // Offset grows by 1 ms every second => 1ms/s == 1000 ppm.
        let mut estimator = DriftEstimator::new(10);
        for t in 0..10 {
            estimator.record_sample(t as f64, t as f64);
        }
        let ppm = estimator.estimate_ppm();
        assert!((ppm - 1000.0).abs() < 1e-6, "expected ~1000 ppm, got {ppm}");
    }

    #[test]
    fn shrinking_offset_reports_negative_drift() {
        // Offset shrinks by 2 ms every second => -2000 ppm.
        let mut estimator = DriftEstimator::new(10);
        for t in 0..10 {
            estimator.record_sample(t as f64, 100.0 - 2.0 * t as f64);
        }
        let ppm = estimator.estimate_ppm();
        assert!((ppm - (-2000.0)).abs() < 1e-6, "expected ~-2000 ppm, got {ppm}");
    }

    #[test]
    fn window_evicts_oldest_sample_once_full() {
        let mut estimator = DriftEstimator::new(3);
        estimator.record_sample(0.0, 0.0);
        estimator.record_sample(1.0, 0.0);
        estimator.record_sample(2.0, 0.0);
        assert_eq!(estimator.sample_count(), 3);

        estimator.record_sample(3.0, 0.0);
        assert_eq!(estimator.sample_count(), 3);
    }

    #[test]
    fn window_size_is_clamped_to_a_minimum_of_two() {
        let estimator = DriftEstimator::new(0);
        assert_eq!(estimator.window_size, 2);
    }

    #[test]
    fn reset_clears_all_samples() {
        let mut estimator = DriftEstimator::new(10);
        estimator.record_sample(0.0, 1.0);
        estimator.record_sample(1.0, 2.0);
        estimator.reset();
        assert_eq!(estimator.sample_count(), 0);
        assert_eq!(estimator.estimate_ppm(), 0.0);
    }
}
