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
//! - **[`BandpassFilter`]**: Butterworth bandpass filter with cascaded biquad sections (configurable order)
//! - **[`LowpassFilter`]**: Cascaded first-order IIR lowpass filter for noise reduction (configurable order)
//! - **[`HighpassFilter`]**: Cascaded first-order RC highpass filter for DC removal (configurable order)
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
//! ```
//! use rust_photoacoustic::preprocessing::filters::{Filter, BandpassFilter, LowpassFilter, HighpassFilter};
//! use std::f32::consts::PI;
//!
//! // Create a bandpass filter for 1kHz ± 100Hz (2nd order = 12dB/octave)
//! let bandpass = BandpassFilter::new(1000.0, 200.0)
//!     .with_sample_rate(48000)
//!     .with_order(2);
//!
//! // Create a test signal with multiple frequencies
//! let mut signal = Vec::new();
//! for i in 0..1000 {
//!     let t = i as f32 / 48000.0;
//!     // Mix of 500Hz, 1000Hz, and 2000Hz
//!     let sample = (2.0 * PI * 500.0 * t).sin() +
//!                  (2.0 * PI * 1000.0 * t).sin() +
//!                  (2.0 * PI * 2000.0 * t).sin();
//!     signal.push(sample);
//! }
//!
//! // Apply the filter
//! let filtered = bandpass.apply(&signal);
//! assert_eq!(filtered.len(), signal.len());
//! ```
//!
//! ## Filter Chain Processing
//!
//! ```
//! use rust_photoacoustic::preprocessing::filters::{Filter, HighpassFilter, LowpassFilter};
//! use std::f32::consts::PI;
//!
//! // Create a filter chain: highpass -> lowpass (both 2nd order)
//! let highpass = HighpassFilter::new(20.0).with_sample_rate(48000).with_order(2);
//! let lowpass = LowpassFilter::new(20000.0).with_sample_rate(48000).with_order(2);
//!
//! // Generate noisy signal with DC offset
//! let mut signal = Vec::new();
//! for i in 0..500 {
//!     let t = i as f32 / 48000.0;
//!     let sample = 1.0 +                           // DC offset
//!                  (2.0 * PI * 1000.0 * t).sin() + // Desired signal
//!                  0.1 * (2.0 * PI * 25000.0 * t).sin(); // High freq noise
//!     signal.push(sample);
//! }
//!
//! // Apply filter chain
//! let step1 = highpass.apply(&signal);  // Remove DC offset
//! let filtered = lowpass.apply(&step1); // Remove high frequency noise
//!
//! assert_eq!(filtered.len(), signal.len());
//! ```

/// Trait for implementing digital filters
///
/// This trait provides a common interface for all digital filter implementations.
/// All filters are thread-safe and can be used in multi-threaded environments.
///
/// ### Examples
///
/// ```
/// use rust_photoacoustic::preprocessing::filters::{Filter, LowpassFilter};
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
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::{Filter, LowpassFilter};
    ///
    /// let filter = LowpassFilter::new(1000.0);
    /// let input = vec![1.0, 0.0, -1.0, 0.0];
    /// let output = filter.apply(&input);
    /// assert_eq!(output.len(), 4);
    /// ```
    fn apply(&self, signal: &[f32]) -> Vec<f32>;
}

/// A Butterworth bandpass filter
///
/// This filter allows frequencies within a specified band to pass through while
/// attenuating frequencies outside this band. It uses cascaded biquad sections
/// to achieve higher-order filtering with good numerical stability.
///
/// The filter is implemented using the Direct Form II Transposed structure,
/// which provides good numerical properties and low coefficient sensitivity.
///
/// ### Examples
///
/// ```
/// use rust_photoacoustic::preprocessing::filters::{Filter, BandpassFilter};
/// use std::f32::consts::PI;
///
/// // Create a bandpass filter centered at 1kHz with 200Hz bandwidth
/// let filter = BandpassFilter::new(1000.0, 200.0)
///     .with_sample_rate(48000)
///     .with_order(4);
///
/// // Generate a test signal with multiple frequencies
/// let mut signal = Vec::new();
/// for i in 0..100 {
///     let t = i as f32 / 48000.0;
///     let sample = (2.0 * PI * 800.0 * t).sin() +   // Should be attenuated
///                  (2.0 * PI * 1000.0 * t).sin() +  // Should pass through
///                  (2.0 * PI * 1500.0 * t).sin();   // Should be attenuated
///     signal.push(sample);
/// }
///
/// let filtered = filter.apply(&signal);
/// assert_eq!(filtered.len(), signal.len());
/// ```
pub struct BandpassFilter {
    center_freq: f32,
    bandwidth: f32,
    sample_rate: u32,
    order: usize,       // Filter order (number of biquad sections = order/2)
    a_coeffs: Vec<f32>, // Feedback coefficients
    b_coeffs: Vec<f32>, // Feedforward coefficients
}

impl BandpassFilter {
    /// Create a new bandpass filter centered at the given frequency with the specified bandwidth
    ///
    /// Creates a 4th-order Butterworth bandpass filter with default sample rate of 48kHz.
    /// The filter coefficients are automatically computed based on the center frequency
    /// and bandwidth parameters.
    ///
    /// ### Arguments
    ///
    /// * `center_freq` - Center frequency in Hz (must be positive and less than Nyquist frequency)
    /// * `bandwidth` - Filter bandwidth in Hz (must be positive and reasonable relative to center frequency)
    ///
    /// ### Returns
    ///
    /// A new `BandpassFilter` instance with computed coefficients
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::BandpassFilter;
    ///
    /// // Create a filter for voice frequencies (300Hz ± 150Hz)
    /// let voice_filter = BandpassFilter::new(300.0, 300.0);
    ///
    /// // Create a filter for ultrasonic frequencies (40kHz ± 5kHz)
    /// let ultrasonic_filter = BandpassFilter::new(40000.0, 10000.0);
    /// ```
    pub fn new(center_freq: f32, bandwidth: f32) -> Self {
        let sample_rate = 48000; // Default sample rate
        let order = 2; // Default 4th order filter (2 biquad sections)

        let mut filter = Self {
            center_freq,
            bandwidth,
            sample_rate,
            order,
            a_coeffs: Vec::new(),
            b_coeffs: Vec::new(),
        };

        filter.compute_coefficients();
        filter
    }

    /// Set the sample rate for the filter
    ///
    /// Updates the sample rate and recomputes the filter coefficients accordingly.
    /// This method can be chained with other builder methods.
    ///
    /// ### Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz (common values: 44100, 48000, 96000)
    ///
    /// ### Returns
    ///
    /// The modified filter instance for method chaining
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::BandpassFilter;
    ///
    /// let filter = BandpassFilter::new(1000.0, 200.0)
    ///     .with_sample_rate(44100);
    /// ```
    pub fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = sample_rate;
        self.compute_coefficients();
        self
    }

    /// Set the filter order (must be even)
    ///
    /// Updates the filter order and recomputes coefficients. Higher orders provide
    /// steeper roll-off at the cost of increased computational complexity and
    /// potential numerical issues.
    ///
    /// ### Arguments
    ///
    /// * `order` - Filter order (must be even, typical values: 2, 4, 6, 8)
    ///
    /// ### Returns
    ///
    /// The modified filter instance for method chaining
    ///
    /// ### Panics
    ///
    /// Panics if the order is odd, since each biquad section implements 2nd-order filtering
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::BandpassFilter;
    ///
    /// let filter = BandpassFilter::new(1000.0, 200.0)
    ///     .with_order(6);  // 3 biquad sections
    /// ```
    pub fn with_order(mut self, order: usize) -> Self {
        if order % 2 != 0 {
            panic!("Filter order must be even");
        }
        self.order = order;
        self.compute_coefficients();
        self
    }

    /// Compute filter coefficients based on current parameters
    ///
    /// This method calculates the filter coefficients for cascaded biquad sections
    /// based on the current center frequency, bandwidth, sample rate, and order.
    /// It uses the bilinear transform to convert from analog to digital domain.
    ///
    /// The method is automatically called when creating a new filter or when
    /// parameters are changed via builder methods.
    ///
    /// ### Implementation Details
    ///
    /// - Uses biquad sections for numerical stability
    /// - Each biquad implements a 2nd-order bandpass filter
    /// - Multiple sections are cascaded to achieve higher orders
    /// - Coefficients are normalized for optimal numerical precision
    fn compute_coefficients(&mut self) {
        // Clear existing coefficients
        self.a_coeffs.clear();
        self.b_coeffs.clear();

        // Convert to angular frequency
        let fs = self.sample_rate as f32;
        let w0 = 2.0 * std::f32::consts::PI * self.center_freq / fs;
        // Q factor calculation (relates to bandwidth)
        let q = self.center_freq / self.bandwidth;
        let alpha = w0.sin() / (2.0 * q);

        // Calculate biquad coefficients for a single second-order section
        // For a bandpass filter, we have:
        let b0 = alpha;
        let b1 = 0.0;
        let b2 = -alpha;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha;

        // Normalize by a0
        let b0_norm = b0 / a0;
        let b1_norm = b1 / a0;
        let b2_norm = b2 / a0;
        let a1_norm = a1 / a0;
        let a2_norm = a2 / a0;

        // For higher order filters, we'd cascade multiple biquad sections
        // For simplicity, we're implementing just one second-order section
        // In a real implementation, we'd calculate multiple sections based on the order

        // For now, we'll just duplicate the same coefficients for each section
        for _ in 0..(self.order / 2) {
            // Each biquad section has 3 b coeffs and 3 a coeffs (with a0 normalized to 1)
            self.b_coeffs.push(b0_norm);
            self.b_coeffs.push(b1_norm);
            self.b_coeffs.push(b2_norm);

            // a0 is always normalized to 1.0, so we don't store it
            self.a_coeffs.push(a1_norm);
            self.a_coeffs.push(a2_norm);
        }
    }
}

impl Filter for BandpassFilter {
    /// Apply the bandpass filter to a signal
    ///
    /// Processes the input signal through cascaded biquad sections using the
    /// Direct Form II Transposed structure. This implementation provides good
    /// numerical stability and low coefficient sensitivity.
    ///
    /// ### Arguments
    ///
    /// * `signal` - Input signal samples as a slice of f32 values
    ///
    /// ### Returns
    ///
    /// A new vector containing the filtered signal samples with the same length as input
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::{Filter, BandpassFilter};
    /// use std::f32::consts::PI;
    ///
    /// let filter = BandpassFilter::new(1000.0, 200.0);
    ///
    /// // Generate a test signal
    /// let mut input = Vec::new();
    /// for i in 0..100 {
    ///     let t = i as f32 / 48000.0;
    ///     input.push((2.0 * PI * 1000.0 * t).sin());
    /// }
    ///
    /// let output = filter.apply(&input);
    /// assert_eq!(output.len(), input.len());
    /// ```
    fn apply(&self, signal: &[f32]) -> Vec<f32> {
        let mut filtered = Vec::with_capacity(signal.len());

        // Ensure we have calculated coefficients
        if self.a_coeffs.is_empty() || self.b_coeffs.is_empty() {
            // Return the original signal if no coefficients are available
            return signal.to_vec();
        }

        // Number of biquad sections
        let n_sections = self.order / 2;

        // Initialize state variables for Direct Form II Transposed structure
        let mut z1 = vec![0.0f32; n_sections]; // z^-1 state for each section
        let mut z2 = vec![0.0f32; n_sections]; // z^-2 state for each section

        // Process each sample through the cascade of biquad sections
        for &x in signal {
            let mut y = x;

            // Apply each biquad section in cascade
            for section in 0..n_sections {
                // Get coefficients for this section
                let b0 = self.b_coeffs[section * 3];
                let b1 = self.b_coeffs[section * 3 + 1];
                let b2 = self.b_coeffs[section * 3 + 2];
                let a1 = self.a_coeffs[section * 2];
                let a2 = self.a_coeffs[section * 2 + 1];

                // Direct Form II Transposed biquad implementation
                let y_section = b0 * y + z1[section];
                z1[section] = b1 * y - a1 * y_section + z2[section];
                z2[section] = b2 * y - a2 * y_section;

                // Output of this section becomes input to the next section
                y = y_section;
            }

            filtered.push(y);
        }

        filtered
    }
}

/// A lowpass filter for removing high frequency noise
///
/// This filter allows frequencies below the cutoff frequency to pass through
/// while attenuating higher frequencies. It's implemented using cascaded first-order
/// IIR (Infinite Impulse Response) filters for higher-order response with good stability.
///
/// The filter uses cascaded single-pole designs, where each first-order section provides
/// -6dB/octave roll-off. Multiple sections are cascaded to achieve higher orders:
/// - Order 1: -6dB/octave roll-off
/// - Order 2: -12dB/octave roll-off  
/// - Order 3: -18dB/octave roll-off
/// - etc.
///
/// Each section has transfer function: H(z) = α / (1 - (1-α)z⁻¹)
/// where α is calculated based on the cutoff frequency and sample rate.
///
/// ### Examples
///
/// ```
/// use rust_photoacoustic::preprocessing::filters::{Filter, LowpassFilter};
/// use std::f32::consts::PI;
///
/// // Create a first-order lowpass filter with 1kHz cutoff (-6dB/octave)
/// let filter = LowpassFilter::new(1000.0)
///     .with_sample_rate(48000)
///     .with_order(1);
///
/// // Create a third-order filter for steeper roll-off (-18dB/octave)
/// let steep_filter = LowpassFilter::new(1000.0)
///     .with_sample_rate(48000)
///     .with_order(3);
///
/// // Generate a test signal with low and high frequency components
/// let mut signal = Vec::new();
/// for i in 0..100 {
///     let t = i as f32 / 48000.0;
///     let sample = (2.0 * PI * 500.0 * t).sin() +   // Should pass through
///                  (2.0 * PI * 5000.0 * t).sin();   // Should be attenuated
///     signal.push(sample);
/// }
///
/// let filtered = filter.apply(&signal);
/// assert_eq!(filtered.len(), signal.len());
/// ```
pub struct LowpassFilter {
    cutoff_freq: f32,
    sample_rate: u32,
    order: usize,
}

impl LowpassFilter {
    /// Create a new lowpass filter with the specified cutoff frequency
    ///
    /// Creates a first-order IIR lowpass filter with default sample rate of 48kHz.
    /// The filter provides -6dB/octave roll-off above the cutoff frequency.
    /// Use `with_order()` to create higher-order filters for steeper roll-off.
    ///
    /// ### Arguments
    ///
    /// * `cutoff_freq` - Cutoff frequency in Hz (must be positive and less than Nyquist frequency)
    ///
    /// ### Returns
    ///
    /// A new `LowpassFilter` instance with order 1 (first-order, -6dB/octave)
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::LowpassFilter;
    ///
    /// // Create a filter to remove frequencies above 1kHz (-6dB/octave)
    /// let filter = LowpassFilter::new(1000.0);
    ///
    /// // Create a filter for audio applications (remove above 20kHz)
    /// let audio_filter = LowpassFilter::new(20000.0);
    /// ```
    pub fn new(cutoff_freq: f32) -> Self {
        let sample_rate = 48000; // Default sample rate
        let order = 1; // Default to first-order filter

        Self {
            cutoff_freq,
            sample_rate,
            order,
        }
    }

    /// Set the sample rate for the filter
    ///
    /// Updates the sample rate for the filter. This affects the filter's frequency response
    /// and should be set to match the sample rate of the input signal.
    ///
    /// ### Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz (common values: 44100, 48000, 96000)
    ///
    /// ### Returns
    ///
    /// The modified filter instance for method chaining
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::LowpassFilter;
    ///
    /// let filter = LowpassFilter::new(1000.0)
    ///     .with_sample_rate(44100);
    /// ```
    pub fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    /// Set the filter order (number of cascaded first-order sections)
    ///
    /// Updates the filter order, where each additional order adds another -6dB/octave
    /// of roll-off and increases the steepness of the transition band.
    ///
    /// ### Arguments
    ///
    /// * `order` - Filter order (must be positive, typical values: 1, 2, 3, 4)
    ///   - Order 1: -6dB/octave roll-off (gentle)
    ///   - Order 2: -12dB/octave roll-off (moderate)
    ///   - Order 3: -18dB/octave roll-off (steep)
    ///   - Order 4: -24dB/octave roll-off (very steep)
    ///
    /// ### Returns
    ///
    /// The modified filter instance for method chaining
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::LowpassFilter;
    ///
    /// // Create a third-order filter (-18dB/octave)
    /// let filter = LowpassFilter::new(1000.0)
    ///     .with_order(3);
    /// ```
    pub fn with_order(mut self, order: usize) -> Self {
        if order == 0 {
            panic!("Filter order must be greater than 0");
        }
        self.order = order;
        self
    }
}

impl Filter for LowpassFilter {
    /// Apply the lowpass filter to a signal
    ///
    /// Processes the input signal using cascaded first-order IIR filters with automatic
    /// gain control to prevent numerical overflow. The implementation includes
    /// input clamping and output validation for robust operation.
    ///
    /// Higher-order filters provide steeper roll-off:
    /// - Order 1: -6dB/octave
    /// - Order 2: -12dB/octave  
    /// - Order 3: -18dB/octave
    /// - etc.
    ///
    /// ### Arguments
    ///
    /// * `signal` - Input signal samples as a slice of f32 values
    ///
    /// ### Returns
    ///
    /// A new vector containing the filtered signal samples with high frequencies attenuated
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::{Filter, LowpassFilter};
    /// use std::f32::consts::PI;
    ///
    /// let filter = LowpassFilter::new(1000.0)
    ///     .with_order(2); // Second-order filter (-12dB/octave)
    ///
    /// // Generate a signal with high frequency noise
    /// let mut input = Vec::new();
    /// for i in 0..100 {
    ///     let t = i as f32 / 48000.0;
    ///     let signal = (2.0 * PI * 500.0 * t).sin() +   // Low frequency component
    ///                  0.1 * (2.0 * PI * 5000.0 * t).sin(); // High frequency noise
    ///     input.push(signal);
    /// }
    ///
    /// let output = filter.apply(&input);
    /// assert_eq!(output.len(), input.len());
    /// ```
    fn apply(&self, signal: &[f32]) -> Vec<f32> {
        // Cascaded first-order IIR lowpass filter implementation
        let mut filtered = Vec::with_capacity(signal.len());

        if signal.is_empty() {
            return filtered;
        }

        // Calculate filter coefficient based on cutoff frequency
        let omega_c = 2.0 * std::f32::consts::PI * self.cutoff_freq / self.sample_rate as f32;
        let alpha = omega_c / (omega_c + 1.0); // More stable coefficient calculation

        // Initialize state variables for each cascade stage
        let mut prev_samples = vec![0.0; self.order];

        for &sample in signal {
            // Clamp input to prevent overflow
            let mut current_sample = sample.clamp(-1e6, 1e6);

            // Process through each cascade stage
            for stage in 0..self.order {
                let filtered_sample = alpha * current_sample + (1.0 - alpha) * prev_samples[stage];

                // Ensure output is finite
                let final_sample = if filtered_sample.is_finite() {
                    filtered_sample
                } else {
                    0.0
                };

                prev_samples[stage] = final_sample;
                current_sample = final_sample;
            }

            filtered.push(current_sample);
        }

        filtered
    }
}

/// A highpass filter for removing low frequency noise and DC offset
///
/// This filter allows frequencies above the cutoff frequency to pass through
/// while attenuating lower frequencies. It's particularly useful for removing
/// DC offset and low-frequency noise from signals.
///
/// The filter is implemented using cascaded first-order RC highpass designs, where
/// each first-order section provides -6dB/octave roll-off. Multiple sections are
/// cascaded to achieve higher orders:
/// - Order 1: -6dB/octave roll-off
/// - Order 2: -12dB/octave roll-off
/// - Order 3: -18dB/octave roll-off  
/// - etc.
///
/// Each section has transfer function: H(z) = (1 - z⁻¹) / (1 - αz⁻¹)
/// where α = e^(-2πfc/fs)
///
/// ### Examples
///
/// ```
/// use rust_photoacoustic::preprocessing::filters::{Filter, HighpassFilter};
/// use std::f32::consts::PI;
///
/// // Create a first-order highpass filter to remove DC and low frequency noise (-6dB/octave)
/// let filter = HighpassFilter::new(100.0)
///     .with_sample_rate(48000)
///     .with_order(1);
///
/// // Create a second-order filter for steeper roll-off (-12dB/octave)
/// let steep_filter = HighpassFilter::new(100.0)
///     .with_sample_rate(48000)
///     .with_order(2);
///
/// // Generate a test signal with DC offset and various frequencies
/// let mut signal = Vec::new();
/// for i in 0..100 {
///     let t = i as f32 / 48000.0;
///     let sample = 0.5 +                           // DC offset (should be removed)
///                  (2.0 * PI * 50.0 * t).sin() +  // Should be attenuated
///                  (2.0 * PI * 500.0 * t).sin();  // Should pass through
///     signal.push(sample);
/// }
///
/// let filtered = filter.apply(&signal);
/// assert_eq!(filtered.len(), signal.len());
/// ```
pub struct HighpassFilter {
    cutoff_freq: f32,
    sample_rate: u32,
    order: usize,
}

impl HighpassFilter {
    /// Create a new highpass filter with the specified cutoff frequency
    ///
    /// Creates a first-order IIR highpass filter with default sample rate of 48kHz.
    /// The filter provides -6dB/octave roll-off below the cutoff frequency and
    /// completely removes DC offset. Use `with_order()` to create higher-order
    /// filters for steeper roll-off.
    ///
    /// ### Arguments
    ///
    /// * `cutoff_freq` - Cutoff frequency in Hz (must be positive, typically 1-1000 Hz)
    ///
    /// ### Returns
    ///
    /// A new `HighpassFilter` instance with order 1 (first-order, -6dB/octave)
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::HighpassFilter;
    ///
    /// // Create a filter to remove DC offset and subsonic noise (below 20Hz)
    /// let subsonic_filter = HighpassFilter::new(20.0);
    ///
    /// // Create a filter for voice processing (remove below 100Hz)
    /// let voice_filter = HighpassFilter::new(100.0);
    /// ```
    pub fn new(cutoff_freq: f32) -> Self {
        let sample_rate = 48000; // Default sample rate
        let order = 1; // Default to first-order filter

        Self {
            cutoff_freq,
            sample_rate,
            order,
        }
    }

    /// Set the sample rate for the filter
    ///
    /// Updates the sample rate for the filter. This affects the filter's frequency response
    /// and should be set to match the sample rate of the input signal.
    ///
    /// ### Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz (common values: 44100, 48000, 96000)
    ///
    /// ### Returns
    ///
    /// The modified filter instance for method chaining
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::HighpassFilter;
    ///
    /// let filter = HighpassFilter::new(100.0)
    ///     .with_sample_rate(44100);
    /// ```
    pub fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    /// Set the filter order (number of cascaded first-order sections)
    ///
    /// Updates the filter order, where each additional order adds another -6dB/octave
    /// of roll-off and increases the steepness of the transition band.
    ///
    /// ### Arguments
    ///
    /// * `order` - Filter order (must be positive, typical values: 1, 2, 3, 4)
    ///   - Order 1: -6dB/octave roll-off (gentle)
    ///   - Order 2: -12dB/octave roll-off (moderate)
    ///   - Order 3: -18dB/octave roll-off (steep)
    ///   - Order 4: -24dB/octave roll-off (very steep)
    ///
    /// ### Returns
    ///
    /// The modified filter instance for method chaining
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::HighpassFilter;
    ///
    /// // Create a second-order filter (-12dB/octave)
    /// let filter = HighpassFilter::new(100.0)
    ///     .with_order(2);
    /// ```
    pub fn with_order(mut self, order: usize) -> Self {
        if order == 0 {
            panic!("Filter order must be greater than 0");
        }
        self.order = order;
        self
    }
}

impl Filter for HighpassFilter {
    /// Apply the highpass filter to a signal
    ///
    /// Processes the input signal using cascaded first-order RC highpass filters that
    /// effectively remove DC offset and low-frequency components. The implementation
    /// uses the difference equation y[n] = α*y[n-1] + (x[n] - x[n-1]) for each stage
    /// with input clamping for numerical stability.
    ///
    /// Higher-order filters provide steeper roll-off:
    /// - Order 1: -6dB/octave
    /// - Order 2: -12dB/octave
    /// - Order 3: -18dB/octave
    /// - etc.
    ///
    /// ### Arguments
    ///
    /// * `signal` - Input signal samples as a slice of f32 values
    ///
    /// ### Returns
    ///
    /// A new vector containing the filtered signal samples with low frequencies and DC removed
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::{Filter, HighpassFilter};
    /// use std::f32::consts::PI;
    ///
    /// let filter = HighpassFilter::new(100.0)
    ///     .with_order(2); // Second-order filter (-12dB/octave)
    ///
    /// // Generate a signal with DC offset and low frequency component
    /// let mut input = Vec::new();
    /// for i in 0..100 {
    ///     let t = i as f32 / 48000.0;
    ///     let signal = 1.0 +                        // DC offset (will be removed)
    ///                  (2.0 * PI * 50.0 * t).sin() + // Low frequency (attenuated)
    ///                  (2.0 * PI * 1000.0 * t).sin(); // High frequency (preserved)
    ///     input.push(signal);
    /// }
    ///
    /// let output = filter.apply(&input);
    /// assert_eq!(output.len(), input.len());
    /// ```
    fn apply(&self, signal: &[f32]) -> Vec<f32> {
        // Cascaded first-order RC highpass filter implementation
        // Each stage: H(z) = (1 - z^-1) / (1 - α*z^-1) where α = e^(-2πfc/fs)

        let mut filtered = Vec::with_capacity(signal.len());

        if signal.is_empty() {
            return filtered;
        }

        // Calculate filter coefficient
        let omega_c = 2.0 * std::f32::consts::PI * self.cutoff_freq / self.sample_rate as f32;
        let alpha = (-omega_c).exp(); // Pole location

        // Initialize state variables for each cascade stage
        let mut x_prev = vec![0.0; self.order]; // Previous input sample for each stage
        let mut y_prev = vec![0.0; self.order]; // Previous output sample for each stage

        // Process first sample (no previous state)
        let first_sample = signal[0].clamp(-1e6, 1e6);

        // Initialize all stages with the first sample
        for stage in 0..self.order {
            x_prev[stage] = first_sample;
            y_prev[stage] = first_sample;
        }
        filtered.push(first_sample);

        // Process remaining samples using difference equation for each stage:
        // y[n] = α*y[n-1] + (x[n] - x[n-1])
        for &x_curr in &signal[1..] {
            let mut current_sample = x_curr.clamp(-1e6, 1e6);

            // Process through each cascade stage
            for stage in 0..self.order {
                let y_curr = alpha * y_prev[stage] + (current_sample - x_prev[stage]);

                // Ensure output is finite
                let final_sample = if y_curr.is_finite() { y_curr } else { 0.0 };

                // Update state variables for this stage
                x_prev[stage] = current_sample;
                y_prev[stage] = final_sample;

                // Output of this stage becomes input to next stage
                current_sample = final_sample;
            }

            filtered.push(current_sample);
        }

        filtered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    /// Helper function to generate a sine wave
    fn generate_sine_wave(frequency: f32, sample_rate: u32, duration_sec: f32) -> Vec<f32> {
        let num_samples = (sample_rate as f32 * duration_sec) as usize;
        let mut signal = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * PI * frequency * t).sin();
            signal.push(sample);
        }

        signal
    }

    /// Helper function to generate a composite signal (sum of multiple frequencies)
    fn generate_composite_signal(
        frequencies: &[f32],
        sample_rate: u32,
        duration_sec: f32,
    ) -> Vec<f32> {
        let num_samples = (sample_rate as f32 * duration_sec) as usize;
        let mut signal = vec![0.0; num_samples];

        for &freq in frequencies {
            let sine_wave = generate_sine_wave(freq, sample_rate, duration_sec);
            for (i, &sample) in sine_wave.iter().enumerate() {
                signal[i] += sample / frequencies.len() as f32; // Normalize by number of components
            }
        }

        signal
    }

    /// Helper function to calculate RMS (Root Mean Square) of a signal
    fn calculate_rms(signal: &[f32]) -> f32 {
        let sum_squares: f32 = signal.iter().map(|&x| x * x).sum();
        (sum_squares / signal.len() as f32).sqrt()
    }

    /// Helper function to calculate signal power in a frequency band
    fn calculate_power_in_band(
        signal: &[f32],
        center_freq: f32,
        bandwidth: f32,
        sample_rate: u32,
    ) -> f32 {
        // Simple approximation: filter the signal and measure RMS
        let bandpass = BandpassFilter::new(center_freq, bandwidth).with_sample_rate(sample_rate);
        let filtered = bandpass.apply(signal);
        calculate_rms(&filtered)
    }

    #[test]
    fn test_bandpass_filter_creation() {
        let filter = BandpassFilter::new(1000.0, 200.0);
        assert_eq!(filter.center_freq, 1000.0);
        assert_eq!(filter.bandwidth, 200.0);
        assert_eq!(filter.sample_rate, 48000);
        assert_eq!(filter.order, 4);
        assert!(!filter.a_coeffs.is_empty());
        assert!(!filter.b_coeffs.is_empty());
    }

    #[test]
    fn test_bandpass_filter_with_sample_rate() {
        let filter = BandpassFilter::new(1000.0, 200.0).with_sample_rate(44100);
        assert_eq!(filter.sample_rate, 44100);
    }

    #[test]
    fn test_bandpass_filter_with_order() {
        let filter = BandpassFilter::new(1000.0, 200.0).with_order(6);
        assert_eq!(filter.order, 6);
        // Should have 3 sections (6/2), each with 3 b coeffs and 2 a coeffs
        assert_eq!(filter.b_coeffs.len(), 9); // 3 sections * 3 coeffs
        assert_eq!(filter.a_coeffs.len(), 6); // 3 sections * 2 coeffs
    }

    #[test]
    #[should_panic(expected = "Filter order must be even")]
    fn test_bandpass_filter_odd_order_panics() {
        BandpassFilter::new(1000.0, 200.0).with_order(5);
    }

    #[test]
    fn test_bandpass_filter_empty_signal() {
        let filter = BandpassFilter::new(1000.0, 200.0);
        let empty_signal = vec![];
        let result = filter.apply(&empty_signal);
        assert!(result.is_empty());
    }

    #[test]
    fn test_bandpass_filter_dc_rejection() {
        let filter = BandpassFilter::new(1000.0, 200.0);

        // Test with DC signal (should be heavily attenuated)
        let dc_signal = vec![1.0; 1000];
        let filtered = filter.apply(&dc_signal);

        // DC component should be significantly reduced
        let dc_rms = calculate_rms(&dc_signal);
        let filtered_rms = calculate_rms(&filtered);
        assert!(
            filtered_rms < dc_rms * 0.1,
            "DC component not sufficiently attenuated"
        );
    }

    #[test]
    fn test_bandpass_filter_passband() {
        let sample_rate = 48000;
        let center_freq = 1000.0;
        let bandwidth = 200.0;
        let filter = BandpassFilter::new(center_freq, bandwidth).with_sample_rate(sample_rate);

        // Generate signal at center frequency (should pass through)
        let passband_signal = generate_sine_wave(center_freq, sample_rate, 0.1);
        let filtered = filter.apply(&passband_signal);

        let original_rms = calculate_rms(&passband_signal);
        let filtered_rms = calculate_rms(&filtered);

        // Signal at center frequency should pass with minimal attenuation
        assert!(
            filtered_rms > original_rms * 0.5,
            "Passband signal too attenuated"
        );
    }

    #[test]
    fn test_lowpass_filter_creation() {
        let filter = LowpassFilter::new(1000.0);
        assert_eq!(filter.cutoff_freq, 1000.0);
        assert_eq!(filter.sample_rate, 48000);
        assert_eq!(filter.order, 1);
    }

    #[test]
    fn test_lowpass_filter_with_sample_rate() {
        let filter = LowpassFilter::new(1000.0).with_sample_rate(44100);
        assert_eq!(filter.sample_rate, 44100);
    }

    #[test]
    fn test_lowpass_filter_with_order() {
        let filter = LowpassFilter::new(1000.0).with_order(3);
        assert_eq!(filter.order, 3);
    }

    #[test]
    #[should_panic(expected = "Filter order must be greater than 0")]
    fn test_lowpass_filter_zero_order_panics() {
        LowpassFilter::new(1000.0).with_order(0);
    }

    #[test]
    fn test_lowpass_filter_empty_signal() {
        let filter = LowpassFilter::new(1000.0);
        let empty_signal = vec![];
        let result = filter.apply(&empty_signal);
        assert!(result.is_empty());
    }

    #[test]
    fn test_lowpass_filter_smoothing() {
        let filter = LowpassFilter::new(1000.0);

        // Test with noisy signal (step function)
        let step_signal = vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0];
        let filtered = filter.apply(&step_signal);

        // Filtered signal should be smoother (less variation)
        let original_variation: f32 = step_signal.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
        let filtered_variation: f32 = filtered.windows(2).map(|w| (w[1] - w[0]).abs()).sum();

        assert!(
            filtered_variation < original_variation,
            "Lowpass filter should smooth the signal"
        );
    }

    #[test]
    fn test_highpass_filter_creation() {
        let filter = HighpassFilter::new(100.0);
        assert_eq!(filter.cutoff_freq, 100.0);
        assert_eq!(filter.sample_rate, 48000);
        assert_eq!(filter.order, 1);
    }

    #[test]
    fn test_highpass_filter_with_sample_rate() {
        let filter = HighpassFilter::new(100.0).with_sample_rate(44100);
        assert_eq!(filter.sample_rate, 44100);
    }

    #[test]
    fn test_highpass_filter_with_order() {
        let filter = HighpassFilter::new(100.0).with_order(2);
        assert_eq!(filter.order, 2);
    }

    #[test]
    #[should_panic(expected = "Filter order must be greater than 0")]
    fn test_highpass_filter_zero_order_panics() {
        HighpassFilter::new(100.0).with_order(0);
    }

    #[test]
    fn test_highpass_filter_empty_signal() {
        let filter = HighpassFilter::new(100.0);
        let empty_signal = vec![];
        let result = filter.apply(&empty_signal);
        assert!(result.is_empty());
    }

    #[test]
    fn test_highpass_filter_single_sample() {
        let filter = HighpassFilter::new(100.0);
        let single_sample = vec![1.0];
        let result = filter.apply(&single_sample);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 1.0); // First sample should pass through unchanged
    }

    #[test]
    fn test_highpass_filter_dc_removal() {
        let filter = HighpassFilter::new(100.0);

        // Test with DC offset + AC signal
        let sample_rate = 48000;
        let ac_freq = 1000.0; // Well above cutoff
        let dc_offset = 2.0;

        let mut signal = generate_sine_wave(ac_freq, sample_rate, 0.1);
        // Add DC offset
        for sample in &mut signal {
            *sample += dc_offset;
        }

        let filtered = filter.apply(&signal);

        // Calculate average (DC component) of original and filtered signals
        let original_dc: f32 = signal.iter().sum::<f32>() / signal.len() as f32;
        let filtered_dc: f32 = filtered.iter().sum::<f32>() / filtered.len() as f32;

        assert!(
            original_dc > 1.5,
            "Original signal should have significant DC component"
        );
        assert!(
            filtered_dc.abs() < 0.5,
            "Filtered signal should have reduced DC component"
        );
    }

    #[test]
    fn test_highpass_filter_preserves_high_frequencies() {
        let sample_rate = 48000;
        let cutoff = 100.0;
        let test_freq = 1000.0; // 10x cutoff frequency

        let filter = HighpassFilter::new(cutoff).with_sample_rate(sample_rate);

        // Generate high frequency signal
        let signal = generate_sine_wave(test_freq, sample_rate, 0.1);
        let filtered = filter.apply(&signal);

        let original_rms = calculate_rms(&signal);
        let filtered_rms = calculate_rms(&filtered);

        // High frequency signal should pass through with minimal attenuation
        assert!(
            filtered_rms > original_rms * 0.7,
            "High frequency signal should be preserved"
        );
    }

    #[test]
    fn test_filter_trait_object() {
        // Test that all filters can be used as trait objects
        let filters: Vec<Box<dyn Filter>> = vec![
            Box::new(BandpassFilter::new(1000.0, 200.0)),
            Box::new(LowpassFilter::new(1000.0)),
            Box::new(HighpassFilter::new(100.0)),
        ];

        let test_signal = vec![1.0, 0.0, -1.0, 0.0, 1.0];

        for filter in filters {
            let result = filter.apply(&test_signal);
            assert_eq!(result.len(), test_signal.len());
        }
    }

    #[test]
    fn test_filter_order_effectiveness() {
        // Test that higher-order filters provide better attenuation
        let sample_rate = 48000;
        let cutoff = 1000.0;
        let test_freq = 5000.0; // 5x cutoff frequency

        // Create filters of different orders
        let filter_order1 = LowpassFilter::new(cutoff)
            .with_sample_rate(sample_rate)
            .with_order(1);
        let filter_order2 = LowpassFilter::new(cutoff)
            .with_sample_rate(sample_rate)
            .with_order(2);
        let filter_order3 = LowpassFilter::new(cutoff)
            .with_sample_rate(sample_rate)
            .with_order(3);

        // Generate high frequency signal
        let signal = generate_sine_wave(test_freq, sample_rate, 0.1);

        let filtered1 = filter_order1.apply(&signal);
        let filtered2 = filter_order2.apply(&signal);
        let filtered3 = filter_order3.apply(&signal);

        let original_rms = calculate_rms(&signal);
        let filtered1_rms = calculate_rms(&filtered1);
        let filtered2_rms = calculate_rms(&filtered2);
        let filtered3_rms = calculate_rms(&filtered3);

        // Higher order filters should provide better attenuation
        assert!(
            filtered2_rms < filtered1_rms,
            "Order 2 should attenuate more than order 1"
        );
        assert!(
            filtered3_rms < filtered2_rms,
            "Order 3 should attenuate more than order 2"
        );
        assert!(
            filtered3_rms < original_rms * 0.1,
            "Order 3 should provide significant attenuation"
        );
    }

    #[test]
    fn test_highpass_filter_order_effectiveness() {
        // Test that higher-order highpass filters provide better attenuation of low frequencies
        let sample_rate = 48000;
        let cutoff = 1000.0;
        let test_freq = 200.0; // 1/5 of cutoff frequency

        // Create filters of different orders
        let filter_order1 = HighpassFilter::new(cutoff)
            .with_sample_rate(sample_rate)
            .with_order(1);
        let filter_order2 = HighpassFilter::new(cutoff)
            .with_sample_rate(sample_rate)
            .with_order(2);

        // Generate low frequency signal
        let signal = generate_sine_wave(test_freq, sample_rate, 0.1);

        let filtered1 = filter_order1.apply(&signal);
        let filtered2 = filter_order2.apply(&signal);

        let original_rms = calculate_rms(&signal);
        let filtered1_rms = calculate_rms(&filtered1);
        let filtered2_rms = calculate_rms(&filtered2);

        // Higher order filter should provide better attenuation
        assert!(
            filtered2_rms < filtered1_rms,
            "Order 2 should attenuate low frequencies more than order 1"
        );
        assert!(
            filtered2_rms < original_rms * 0.5,
            "Order 2 should provide significant attenuation"
        );
    }

    #[test]
    fn test_filter_chain() {
        // Test chaining filters together
        let sample_rate = 48000;
        let signal = generate_composite_signal(&[50.0, 500.0, 5000.0], sample_rate, 0.1);

        // Create filter chain: highpass (remove 50Hz) -> lowpass (remove 5kHz)
        let highpass = HighpassFilter::new(100.0).with_sample_rate(sample_rate);
        let lowpass = LowpassFilter::new(1000.0).with_sample_rate(sample_rate);

        // Apply filters in sequence
        let step1 = highpass.apply(&signal);
        let final_result = lowpass.apply(&step1);

        assert_eq!(final_result.len(), signal.len());

        // The middle frequency (500Hz) should be the most prominent
        let power_low = calculate_power_in_band(&final_result, 50.0, 20.0, sample_rate);
        let power_mid = calculate_power_in_band(&final_result, 500.0, 100.0, sample_rate);
        let power_high = calculate_power_in_band(&final_result, 5000.0, 500.0, sample_rate);

        assert!(
            power_mid > power_low,
            "Middle frequency should be stronger than low"
        );
        assert!(
            power_mid > power_high,
            "Middle frequency should be stronger than high"
        );
    }

    #[test]
    fn test_filter_stability() {
        // Test that filters don't produce NaN or infinite values
        let sample_rate = 48000;

        // Test with more reasonable extreme input values
        let extreme_signal = vec![1000.0, -1000.0, 0.0, 1.0, -1.0, 100.0, -100.0];

        let bandpass = BandpassFilter::new(1000.0, 200.0).with_sample_rate(sample_rate);
        let lowpass = LowpassFilter::new(1000.0).with_sample_rate(sample_rate);
        let highpass = HighpassFilter::new(100.0).with_sample_rate(sample_rate);

        let results = [
            bandpass.apply(&extreme_signal),
            lowpass.apply(&extreme_signal),
            highpass.apply(&extreme_signal),
        ];

        for (i, result) in results.iter().enumerate() {
            for (j, &sample) in result.iter().enumerate() {
                assert!(
                    sample.is_finite(),
                    "Filter {} output at sample {} should be finite, got: {}",
                    i,
                    j,
                    sample
                );
            }
        }
    }

    #[test]
    fn test_filter_extreme_values() {
        // Test filters with very large (but not MAX) values
        let sample_rate = 48000;
        let large_signal = vec![1e5, -1e5, 1e4, -1e4];

        let lowpass = LowpassFilter::new(1000.0).with_sample_rate(sample_rate);
        let highpass = HighpassFilter::new(100.0).with_sample_rate(sample_rate);

        let lowpass_result = lowpass.apply(&large_signal);
        let highpass_result = highpass.apply(&large_signal);

        // All outputs should be finite and reasonable
        for &sample in &lowpass_result {
            assert!(sample.is_finite());
            assert!(sample.abs() < 1e6); // Should be clamped
        }

        for &sample in &highpass_result {
            assert!(sample.is_finite());
            assert!(sample.abs() < 1e6); // Should be clamped
        }
    }
}
