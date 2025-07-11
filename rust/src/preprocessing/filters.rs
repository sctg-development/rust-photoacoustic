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
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::{Filter, BandpassFilter};
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
    order: usize,                                       // Filter order (must be even)
    biquad_coeffs: Vec<BiquadCoeffs>,                   // Coefficients for each biquad section
    biquad_states: std::sync::RwLock<Vec<BiquadState>>, // State variables for each biquad section
}

/// Coefficients for a single biquad section
#[derive(Clone, Debug)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32, // Feedforward coefficients
    a1: f32,
    a2: f32, // Feedback coefficients (a0 normalized to 1)
}

/// State variables for a single biquad section (Direct Form II Transposed)
#[derive(Clone, Debug)]
struct BiquadState {
    z1: f32, // First delay element
    z2: f32, // Second delay element
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
        let order = 2; // Default 2nd order filter (1 biquad section)

        let mut filter = Self {
            center_freq,
            bandwidth,
            sample_rate,
            order,
            biquad_coeffs: Vec::new(),
            biquad_states: std::sync::RwLock::new(Vec::new()),
        };

        filter.compute_coefficients();
        filter
    }

    /// Reset the filter's internal state
    ///
    /// Clears all delay elements and state variables, allowing the filter
    /// to start processing from a clean state. This is useful when processing
    /// discontinuous signals or when you want to avoid transients from previous processing.
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::{Filter, BandpassFilter};
    ///
    /// let filter = BandpassFilter::new(1000.0, 200.0);
    /// let signal1 = vec![1.0, 0.5, -0.3];
    /// let _output1 = filter.apply(&signal1);
    ///
    /// // Reset state before processing new signal
    /// filter.reset_state();
    /// let signal2 = vec![0.8, -0.2, 0.4];
    /// let _output2 = filter.apply(&signal2);
    /// ```
    pub fn reset_state(&self) {
        let mut states = self.biquad_states.write().unwrap();
        for state in states.iter_mut() {
            state.z1 = 0.0;
            state.z2 = 0.0;
        }
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

    /// Update the filter configuration with new parameters (hot-reload support)
    ///
    /// This method allows dynamic updating of filter parameters without recreating
    /// the filter instance. Supported parameters:
    /// - `center_freq`: Center frequency in Hz
    /// - `bandwidth`: Filter bandwidth in Hz  
    /// - `sample_rate`: Sample rate in Hz
    /// - `order`: Filter order (must be even)
    ///
    /// ### Arguments
    ///
    /// * `parameters` - JSON object containing the new parameters
    ///
    /// ### Returns
    ///
    /// * `Ok(true)` - Configuration updated successfully
    /// * `Ok(false)` - No supported parameters found in input
    /// * `Err(anyhow::Error)` - Configuration update failed
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::BandpassFilter;
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
    ///     "sample_rate": 44100
    /// }));
    /// assert!(result.is_ok());
    /// assert!(result.unwrap());
    /// ```
    pub fn update_config(&mut self, parameters: &serde_json::Value) -> anyhow::Result<bool> {
        let mut updated = false;

        // Update center frequency if provided
        if let Some(center_freq) = parameters.get("center_freq") {
            if let Some(freq) = center_freq.as_f64() {
                if freq > 0.0 && freq < (self.sample_rate as f64 / 2.0) {
                    self.center_freq = freq as f32;
                    updated = true;
                } else {
                    anyhow::bail!(
                        "center_freq must be positive and less than Nyquist frequency ({})",
                        self.sample_rate / 2
                    );
                }
            } else {
                anyhow::bail!("center_freq must be a number");
            }
        }

        // Update bandwidth if provided
        if let Some(bandwidth) = parameters.get("bandwidth") {
            if let Some(bw) = bandwidth.as_f64() {
                if bw > 0.0 {
                    self.bandwidth = bw as f32;
                    updated = true;
                } else {
                    anyhow::bail!("bandwidth must be positive");
                }
            } else {
                anyhow::bail!("bandwidth must be a number");
            }
        }

        // Update sample rate if provided
        if let Some(sample_rate) = parameters.get("sample_rate") {
            if let Some(sr) = sample_rate.as_u64() {
                if sr > 0 && sr <= u32::MAX as u64 {
                    self.sample_rate = sr as u32;
                    updated = true;
                } else {
                    anyhow::bail!("sample_rate must be a positive integer within u32 range");
                }
            } else {
                anyhow::bail!("sample_rate must be an integer");
            }
        }

        // Update order if provided
        if let Some(order) = parameters.get("order") {
            if let Some(ord) = order.as_u64() {
                if ord > 0 && ord % 2 == 0 && ord <= usize::MAX as u64 {
                    self.order = ord as usize;
                    updated = true;
                } else {
                    anyhow::bail!("order must be a positive even integer");
                }
            } else {
                anyhow::bail!("order must be an integer");
            }
        }

        // Recompute coefficients if any parameter was updated
        if updated {
            self.compute_coefficients();
        }

        Ok(updated)
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
        // Clear existing coefficients and states
        self.biquad_coeffs.clear();
        self.biquad_states.write().unwrap().clear();

        let fs = self.sample_rate as f32;
        let fc = self.center_freq;
        let bw = self.bandwidth;

        // Number of biquad sections
        let n_sections = self.order / 2;

        // For Butterworth bandpass filter, we'll create cascaded sections
        // Each section is a 2nd-order bandpass with slightly different Q factors
        // to achieve the overall Butterworth response

        for k in 0..n_sections {
            // Calculate Q factor for this section to achieve Butterworth response
            // For higher order filters, distribute Q values appropriately
            let section_q = if n_sections == 1 {
                fc / bw // Standard Q for single section
            } else {
                // For multiple sections, use modified Q to maintain overall response
                let butterworth_q_factor = 1.0
                    / (2.0
                        * (std::f32::consts::PI * (2.0 * k as f32 + 1.0)
                            / (4.0 * n_sections as f32))
                            .sin());
                (fc / bw) * butterworth_q_factor
            };

            // Calculate biquad coefficients using the standard bandpass formula
            let w0 = 2.0 * std::f32::consts::PI * fc / fs;
            let alpha = w0.sin() / (2.0 * section_q);

            // Bandpass filter coefficients
            let b0 = alpha;
            let b1 = 0.0;
            let b2 = -alpha;
            let a0 = 1.0 + alpha;
            let a1 = -2.0 * w0.cos();
            let a2 = 1.0 - alpha;

            // Normalize by a0 and store
            self.biquad_coeffs.push(BiquadCoeffs {
                b0: b0 / a0,
                b1: b1 / a0,
                b2: b2 / a0,
                a1: a1 / a0,
                a2: a2 / a0,
            });

            // Initialize state variables for this section
            self.biquad_states
                .write()
                .unwrap()
                .push(BiquadState { z1: 0.0, z2: 0.0 });
        }

        // Apply gain correction for multiple sections
        if n_sections > 1 {
            let gain_correction = (n_sections as f32).sqrt();
            for coeffs in &mut self.biquad_coeffs {
                coeffs.b0 *= gain_correction;
                coeffs.b2 *= gain_correction;
            }
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
        if self.biquad_coeffs.is_empty() {
            // Return the original signal if no coefficients are available
            return signal.to_vec();
        }

        // Acquire write lock on states
        let mut states = self.biquad_states.write().unwrap();

        // Process each sample through the cascade of biquad sections
        for &x in signal {
            let mut y = x;

            // Apply each biquad section in cascade
            for (section, coeffs) in self.biquad_coeffs.iter().enumerate() {
                // Direct Form II Transposed biquad implementation
                let state = &mut states[section];

                // Calculate output
                let y_out = coeffs.b0 * y + state.z1;

                // Update state variables
                state.z1 = coeffs.b1 * y - coeffs.a1 * y_out + state.z2;
                state.z2 = coeffs.b2 * y - coeffs.a2 * y_out;

                // Output of this section becomes input to the next section
                y = y_out;
            }

            filtered.push(y);
        }

        filtered
    }

    fn update_config(&mut self, parameters: &serde_json::Value) -> anyhow::Result<bool> {
        // Delegate to the concrete implementation's update_config method
        self.update_config(parameters)
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

    /// Update the filter configuration with new parameters (hot-reload support)
    ///
    /// This method allows dynamic updating of filter parameters without recreating
    /// the filter instance. Supported parameters:
    /// - `cutoff_freq`: Cutoff frequency in Hz
    /// - `sample_rate`: Sample rate in Hz
    /// - `order`: Filter order (must be positive)
    ///
    /// ### Arguments
    ///
    /// * `parameters` - JSON object containing the new parameters
    ///
    /// ### Returns
    ///
    /// * `Ok(true)` - Configuration updated successfully
    /// * `Ok(false)` - No supported parameters found in input
    /// * `Err(anyhow::Error)` - Configuration update failed
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::LowpassFilter;
    /// use serde_json::json;
    ///
    /// let mut filter = LowpassFilter::new(1000.0);
    ///
    /// // Update cutoff frequency
    /// let result = filter.update_config(&json!({"cutoff_freq": 1500.0}));
    /// assert!(result.is_ok());
    /// assert!(result.unwrap());
    ///
    /// // Update multiple parameters
    /// let result = filter.update_config(&json!({
    ///     "cutoff_freq": 2000.0,
    ///     "sample_rate": 44100,
    ///     "order": 3
    /// }));
    /// assert!(result.is_ok());
    /// assert!(result.unwrap());
    /// ```
    pub fn update_config(&mut self, parameters: &serde_json::Value) -> anyhow::Result<bool> {
        let mut updated = false;

        // Update cutoff frequency if provided
        if let Some(cutoff_freq) = parameters.get("cutoff_freq") {
            if let Some(freq) = cutoff_freq.as_f64() {
                if freq > 0.0 && freq < (self.sample_rate as f64 / 2.0) {
                    self.cutoff_freq = freq as f32;
                    updated = true;
                } else {
                    anyhow::bail!(
                        "cutoff_freq must be positive and less than Nyquist frequency ({})",
                        self.sample_rate / 2
                    );
                }
            } else {
                anyhow::bail!("cutoff_freq must be a number");
            }
        }

        // Update sample rate if provided
        if let Some(sample_rate) = parameters.get("sample_rate") {
            if let Some(sr) = sample_rate.as_u64() {
                if sr > 0 && sr <= u32::MAX as u64 {
                    self.sample_rate = sr as u32;
                    updated = true;
                } else {
                    anyhow::bail!("sample_rate must be a positive integer within u32 range");
                }
            } else {
                anyhow::bail!("sample_rate must be an integer");
            }
        }

        // Update order if provided
        if let Some(order) = parameters.get("order") {
            if let Some(ord) = order.as_u64() {
                if ord > 0 && ord <= usize::MAX as u64 {
                    self.order = ord as usize;
                    updated = true;
                } else {
                    anyhow::bail!("order must be a positive integer");
                }
            } else {
                anyhow::bail!("order must be an integer");
            }
        }

        Ok(updated)
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

    fn update_config(&mut self, parameters: &serde_json::Value) -> anyhow::Result<bool> {
        // Delegate to the concrete implementation's update_config method
        self.update_config(parameters)
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

    /// Update the filter configuration with new parameters (hot-reload support)
    ///
    /// This method allows dynamic updating of filter parameters without recreating
    /// the filter instance. Supported parameters:
    /// - `cutoff_freq`: Cutoff frequency in Hz
    /// - `sample_rate`: Sample rate in Hz
    /// - `order`: Filter order (must be positive)
    ///
    /// ### Arguments
    ///
    /// * `parameters` - JSON object containing the new parameters
    ///
    /// ### Returns
    ///
    /// * `Ok(true)` - Configuration updated successfully
    /// * `Ok(false)` - No supported parameters found in input
    /// * `Err(anyhow::Error)` - Configuration update failed
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::filters::HighpassFilter;
    /// use serde_json::json;
    ///
    /// let mut filter = HighpassFilter::new(100.0);
    ///
    /// // Update cutoff frequency
    /// let result = filter.update_config(&json!({"cutoff_freq": 150.0}));
    /// assert!(result.is_ok());
    /// assert!(result.unwrap());
    ///
    /// // Update multiple parameters
    /// let result = filter.update_config(&json!({
    ///     "cutoff_freq": 200.0,
    ///     "sample_rate": 44100,
    ///     "order": 3
    /// }));
    /// assert!(result.is_ok());
    /// assert!(result.unwrap());
    /// ```
    pub fn update_config(&mut self, parameters: &serde_json::Value) -> anyhow::Result<bool> {
        let mut updated = false;

        // Update cutoff frequency if provided
        if let Some(cutoff_freq) = parameters.get("cutoff_freq") {
            if let Some(freq) = cutoff_freq.as_f64() {
                if freq > 0.0 && freq < (self.sample_rate as f64 / 2.0) {
                    self.cutoff_freq = freq as f32;
                    updated = true;
                } else {
                    anyhow::bail!(
                        "cutoff_freq must be positive and less than Nyquist frequency ({})",
                        self.sample_rate / 2
                    );
                }
            } else {
                anyhow::bail!("cutoff_freq must be a number");
            }
        }

        // Update sample rate if provided
        if let Some(sample_rate) = parameters.get("sample_rate") {
            if let Some(sr) = sample_rate.as_u64() {
                if sr > 0 && sr <= u32::MAX as u64 {
                    self.sample_rate = sr as u32;
                    updated = true;
                } else {
                    anyhow::bail!("sample_rate must be a positive integer within u32 range");
                }
            } else {
                anyhow::bail!("sample_rate must be an integer");
            }
        }

        // Update order if provided
        if let Some(order) = parameters.get("order") {
            if let Some(ord) = order.as_u64() {
                if ord > 0 && ord <= usize::MAX as u64 {
                    self.order = ord as usize;
                    updated = true;
                } else {
                    anyhow::bail!("order must be a positive integer");
                }
            } else {
                anyhow::bail!("order must be an integer");
            }
        }

        Ok(updated)
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
        // Each stage: H(z) = (1 - z^-1) / (1 - α*z^-1)

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

    fn update_config(&mut self, parameters: &serde_json::Value) -> anyhow::Result<bool> {
        // Delegate to the concrete implementation's update_config method
        self.update_config(parameters)
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
        assert_eq!(filter.order, 2);
        assert!(!filter.biquad_coeffs.is_empty());
        assert!(!filter.biquad_states.read().unwrap().is_empty());
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
        // Should have 3 sections (6/2)
        assert_eq!(filter.biquad_coeffs.len(), 3); // 3 biquad sections
        assert_eq!(filter.biquad_states.read().unwrap().len(), 3); // 3 state variables
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

    // ==================== Hot-reload (update_config) tests ====================

    #[test]
    fn test_bandpass_filter_update_config_center_freq() {
        use serde_json::json;

        let mut filter = BandpassFilter::new(1000.0, 200.0);

        let params = json!({"center_freq": 1500.0});
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.center_freq, 1500.0);
    }

    #[test]
    fn test_bandpass_filter_update_config_bandwidth() {
        use serde_json::json;

        let mut filter = BandpassFilter::new(1000.0, 200.0);

        let params = json!({"bandwidth": 300.0});
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.bandwidth, 300.0);
    }

    #[test]
    fn test_bandpass_filter_update_config_sample_rate() {
        use serde_json::json;

        let mut filter = BandpassFilter::new(1000.0, 200.0);

        let params = json!({"sample_rate": 44100});
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.sample_rate, 44100);
    }

    #[test]
    fn test_bandpass_filter_update_config_order() {
        use serde_json::json;

        let mut filter = BandpassFilter::new(1000.0, 200.0);

        let params = json!({"order": 6});
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.order, 6);
    }

    #[test]
    fn test_bandpass_filter_update_config_multiple_params() {
        use serde_json::json;

        let mut filter = BandpassFilter::new(1000.0, 200.0);

        let params = json!({
            "center_freq": 2000.0,
            "bandwidth": 400.0,
            "sample_rate": 44100,
            "order": 4
        });
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.center_freq, 2000.0);
        assert_eq!(filter.bandwidth, 400.0);
        assert_eq!(filter.sample_rate, 44100);
        assert_eq!(filter.order, 4);
    }

    #[test]
    fn test_bandpass_filter_update_config_no_params() {
        use serde_json::json;

        let mut filter = BandpassFilter::new(1000.0, 200.0);

        let params = json!({});
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false for no updates
    }

    #[test]
    fn test_bandpass_filter_update_config_invalid_center_freq() {
        use serde_json::json;

        let mut filter = BandpassFilter::new(1000.0, 200.0);

        // Test negative frequency
        let params = json!({"center_freq": -100.0});
        let result = filter.update_config(&params);
        assert!(result.is_err());

        // Test frequency above Nyquist
        let params = json!({"center_freq": 50000.0}); // Above 48000/2 = 24000
        let result = filter.update_config(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_bandpass_filter_update_config_invalid_order() {
        use serde_json::json;

        let mut filter = BandpassFilter::new(1000.0, 200.0);

        // Test odd order
        let params = json!({"order": 5});
        let result = filter.update_config(&params);
        assert!(result.is_err());

        // Test zero order
        let params = json!({"order": 0});
        let result = filter.update_config(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_lowpass_filter_update_config_cutoff_freq() {
        use serde_json::json;

        let mut filter = LowpassFilter::new(1000.0);

        let params = json!({"cutoff_freq": 1500.0});
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.cutoff_freq, 1500.0);
    }

    #[test]
    fn test_lowpass_filter_update_config_sample_rate() {
        use serde_json::json;

        let mut filter = LowpassFilter::new(1000.0);

        let params = json!({"sample_rate": 44100});
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.sample_rate, 44100);
    }

    #[test]
    fn test_lowpass_filter_update_config_order() {
        use serde_json::json;

        let mut filter = LowpassFilter::new(1000.0);

        let params = json!({"order": 3});
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.order, 3);
    }

    #[test]
    fn test_lowpass_filter_update_config_multiple_params() {
        use serde_json::json;

        let mut filter = LowpassFilter::new(1000.0);

        let params = json!({
            "cutoff_freq": 2000.0,
            "sample_rate": 44100,
            "order": 4
        });
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.cutoff_freq, 2000.0);
        assert_eq!(filter.sample_rate, 44100);
        assert_eq!(filter.order, 4);
    }

    #[test]
    fn test_lowpass_filter_update_config_invalid_cutoff_freq() {
        use serde_json::json;

        let mut filter = LowpassFilter::new(1000.0);

        // Test negative frequency
        let params = json!({"cutoff_freq": -100.0});
        let result = filter.update_config(&params);
        assert!(result.is_err());

        // Test frequency above Nyquist
        let params = json!({"cutoff_freq": 50000.0}); // Above 48000/2 = 24000
        let result = filter.update_config(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_lowpass_filter_update_config_invalid_order() {
        use serde_json::json;

        let mut filter = LowpassFilter::new(1000.0);

        // Test zero order
        let params = json!({"order": 0});
        let result = filter.update_config(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_highpass_filter_update_config_cutoff_freq() {
        use serde_json::json;

        let mut filter = HighpassFilter::new(100.0);

        let params = json!({"cutoff_freq": 150.0});
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.cutoff_freq, 150.0);
    }

    #[test]
    fn test_highpass_filter_update_config_sample_rate() {
        use serde_json::json;

        let mut filter = HighpassFilter::new(100.0);

        let params = json!({"sample_rate": 44100});
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.sample_rate, 44100);
    }

    #[test]
    fn test_highpass_filter_update_config_order() {
        use serde_json::json;

        let mut filter = HighpassFilter::new(100.0);

        let params = json!({"order": 2});
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.order, 2);
    }

    #[test]
    fn test_highpass_filter_update_config_multiple_params() {
        use serde_json::json;

        let mut filter = HighpassFilter::new(100.0);

        let params = json!({
            "cutoff_freq": 200.0,
            "sample_rate": 44100,
            "order": 3
        });
        let result = filter.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.cutoff_freq, 200.0);
        assert_eq!(filter.sample_rate, 44100);
        assert_eq!(filter.order, 3);
    }

    #[test]
    fn test_highpass_filter_update_config_invalid_cutoff_freq() {
        use serde_json::json;

        let mut filter = HighpassFilter::new(100.0);

        // Test negative frequency
        let params = json!({"cutoff_freq": -50.0});
        let result = filter.update_config(&params);
        assert!(result.is_err());

        // Test frequency above Nyquist
        let params = json!({"cutoff_freq": 50000.0}); // Above 48000/2 = 24000
        let result = filter.update_config(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_highpass_filter_update_config_invalid_order() {
        use serde_json::json;

        let mut filter = HighpassFilter::new(100.0);

        // Test zero order
        let params = json!({"order": 0});
        let result = filter.update_config(&params);
        assert!(result.is_err());
    }
}
