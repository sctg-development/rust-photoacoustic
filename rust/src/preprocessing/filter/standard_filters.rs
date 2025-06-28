// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Standard filter implementations using custom algorithms
//!
//! This module provides the original filter implementations that were
//! developed specifically for this project. These filters use custom
//! algorithms with optimized numerical stability and performance.

use super::Filter;
use std::sync::RwLock;

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
/// ```no_run
/// use rust_photoacoustic::preprocessing::filter::{Filter, standard_filters::BandpassFilter};
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
    order: usize,                            // Filter order (must be even)
    biquad_coeffs: Vec<BiquadCoeffs>,        // Coefficients for each biquad section
    biquad_states: RwLock<Vec<BiquadState>>, // State variables for each biquad section
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::standard_filters::BandpassFilter;
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
            biquad_states: RwLock::new(Vec::new()),
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::{Filter, standard_filters::BandpassFilter};
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::standard_filters::BandpassFilter;
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::standard_filters::BandpassFilter;
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::standard_filters::BandpassFilter;
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::{Filter, standard_filters::BandpassFilter};
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
/// ```no_run
/// use rust_photoacoustic::preprocessing::filter::{Filter, standard_filters::LowpassFilter};
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::standard_filters::LowpassFilter;
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::standard_filters::LowpassFilter;
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::standard_filters::LowpassFilter;
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::standard_filters::LowpassFilter;
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::{Filter, standard_filters::LowpassFilter};
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
/// ```no_run
/// use rust_photoacoustic::preprocessing::filter::{Filter, standard_filters::HighpassFilter};
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::standard_filters::HighpassFilter;
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::standard_filters::HighpassFilter;
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::standard_filters::HighpassFilter;
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::standard_filters::HighpassFilter;
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
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::{Filter, standard_filters::HighpassFilter};
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
