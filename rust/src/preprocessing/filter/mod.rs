// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Digital filters for signal preprocessing
//!
//! This module provides various digital filter implementations for processing audio signals
//! in photoacoustic applications. All filters implement the [`Filter`] trait and support
//! real-time processing with configurable parameters.
//!
//! # Filter Types
//!
//! ## Standard Implementation Filters
//!
//! - **[`standard_filters::BandpassFilter`]**: Butterworth bandpass filter with cascaded biquad sections
//! - **[`standard_filters::LowpassFilter`]**: Cascaded first-order IIR lowpass filter for noise reduction
//! - **[`standard_filters::HighpassFilter`]**: Cascaded first-order RC highpass filter for DC removal
//!
//! ## SciPy-style Digital Filters (SOS + filtfilt)
//!
//! - **[`scipy_butter_filter::ButterBandpassFilter`]**: Butterworth bandpass filter using SOS + filtfilt
//! - **[`scipy_butter_filter::ButterLowpassFilter`]**: Butterworth lowpass filter using SOS + filtfilt  
//! - **[`scipy_butter_filter::ButterHighpassFilter`]**: Butterworth highpass filter using SOS + filtfilt
//!
//! All filters support configurable order which controls the steepness of the roll-off:
//! - Order 2: -12dB/octave roll-off (moderate)  
//! - Order 4: -24dB/octave roll-off (very steep)
//!
//! # Performance Characteristics
//!
//! All filters are designed for real-time audio processing with:
//! - Low computational overhead
//! - Good numerical stability
//! - Thread-safe operation
//! - Configurable sample rates
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```no_run
//! use rust_photoacoustic::preprocessing::filter::{Filter, standard_filters::BandpassFilter};
//!
//! // Create a bandpass filter centered at 1kHz with 200Hz bandwidth
//! let filter = BandpassFilter::new(1000.0, 200.0)
//!     .with_sample_rate(48000)
//!     .with_order(4);
//!
//! let input = vec![1.0, 0.5, -0.3, 0.8, -0.2];
//! let output = filter.apply(&input);
//! ```

pub mod scipy_butter_filter;
pub mod scipy_cauer_filter;
pub mod scipy_cheby_filter;
pub mod standard_filters;

/// Trait for implementing digital filters
///
/// This trait provides a common interface for all digital filter implementations.
/// All filters are thread-safe and can be used in multi-threaded environments.
///
/// ### Examples
///
/// ```no_run
/// use rust_photoacoustic::preprocessing::filter::{Filter, standard_filters::LowpassFilter};
///
/// let filter = LowpassFilter::new(1000.0);
/// let input = vec![1.0, 0.5, -0.3, 0.8, -0.2];
/// let output = filter.apply(&input);
/// assert_eq!(output.len(), input.len());
/// ```
pub trait Filter: Send + Sync {
    /// Apply the filter to a signal and return the filtered signal
    ///
    /// ### Arguments
    ///
    /// * `signal` - Input signal samples as a slice of f32 values
    ///
    /// ### Returns
    ///
    /// A new vector containing the filtered signal samples
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::{Filter, standard_filters::LowpassFilter};
    ///
    /// let filter = LowpassFilter::new(1000.0);
    /// let input = vec![1.0, 0.0, -1.0, 0.0];
    /// let output = filter.apply(&input);
    /// assert_eq!(output.len(), 4);
    /// ```
    fn apply(&self, signal: &[f32]) -> Vec<f32>;

    /// Update filter configuration with new parameters
    ///
    /// This method allows dynamic reconfiguration of filter parameters without
    /// recreating the filter instance. This enables hot-reload capabilities for
    /// real-time parameter adjustments during audio processing.
    ///
    /// ### Arguments
    ///
    /// * `parameters` - JSON object containing the parameters to update
    ///
    /// ### Returns
    ///
    /// * `Ok(true)` - Parameters were successfully updated
    /// * `Ok(false)` - No updates were made (no matching parameters found)
    /// * `Err(anyhow::Error)` - Invalid parameter values or update failed
    ///
    /// ### Supported Parameters
    ///
    /// The specific parameters supported depend on the filter type:
    /// - **BandpassFilter**: `center_freq`, `bandwidth`, `sample_rate`, `order`
    /// - **LowpassFilter**: `cutoff_freq`, `sample_rate`, `order`
    /// - **HighpassFilter**: `cutoff_freq`, `sample_rate`, `order`
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::{Filter, standard_filters::BandpassFilter};
    /// use serde_json::json;
    ///
    /// let mut filter = BandpassFilter::new(1000.0, 200.0);
    ///
    /// // Update center frequency
    /// let result = filter.update_config(&json!({"center_freq": 1500.0}));
    /// assert!(result.is_ok());
    /// assert!(result.unwrap());
    ///
    /// // Update multiple parameters
    /// let result = filter.update_config(&json!({
    ///     "center_freq": 2000.0,
    ///     "bandwidth": 300.0,
    ///     "order": 4
    /// }));
    /// assert!(result.is_ok());
    /// ```
    fn update_config(&mut self, parameters: &serde_json::Value) -> anyhow::Result<bool>;
}

// Re-export commonly used filters for backward compatibility
pub use scipy_butter_filter::{ButterBandpassFilter, ButterHighpassFilter, ButterLowpassFilter};
pub use scipy_cauer_filter::{CauerBandpassFilter, CauerHighpassFilter, CauerLowpassFilter};
pub use scipy_cheby_filter::{ChebyBandpassFilter, ChebyHighpassFilter, ChebyLowpassFilter};
pub use standard_filters::{BandpassFilter, HighpassFilter, LowpassFilter};
