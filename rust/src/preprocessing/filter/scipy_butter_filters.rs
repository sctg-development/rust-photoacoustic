// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! SciPy-style Butterworth filter implementations using sci-rs
//!
//! This module provides Butterworth filter implementations that use the sci-rs library
//! to match the behavior of SciPy's signal.butter functions. These filters offer
//! the classic Butterworth response with no ripple in the passband.

use super::Filter;
use anyhow::{bail, Result};
use sci_rs::signal::filter::design::{butter_dyn, DigitalFilter, FilterBandType, FilterOutputType};
use std::sync::RwLock;

/// Butterworth bandpass filter using sci-rs implementation
///
/// This filter provides SciPy-compatible Butterworth bandpass filtering using
/// the sci-rs signal processing library. Butterworth filters are characterized
/// by a maximally flat frequency response in the passband.
///
/// ### Examples
///
/// ```no_run
/// use rust_photoacoustic::preprocessing::filter::{Filter, scipy_butter_filters::ButterBandpassFilter};
///
/// let filter = ButterBandpassFilter::new(1000.0, 200.0, 48000)
///     .with_order(4);
///
/// let input = vec![1.0, 0.5, -0.3, 0.8, -0.2];
/// let output = filter.apply(&input);
/// assert_eq!(output.len(), input.len());
/// ```
pub struct ButterBandpassFilter {
    center_freq: f32,
    bandwidth: f32,
    sample_rate: u32,
    order: usize,
    coefficients: RwLock<Option<(Vec<f64>, Vec<f64>)>>, // (b, a) coefficients
    filter_state: RwLock<Option<Vec<f64>>>,             // Filter state
}

impl ButterBandpassFilter {
    /// Create a new Butterworth bandpass filter
    ///
    /// ### Arguments
    ///
    /// * `center_freq` - Center frequency in Hz
    /// * `bandwidth` - Filter bandwidth in Hz
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// ### Returns
    ///
    /// A new `ButterBandpassFilter` instance
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::scipy_butter_filters::ButterBandpassFilter;
    ///
    /// let filter = ButterBandpassFilter::new(1000.0, 200.0, 48000);
    /// ```
    pub fn new(center_freq: f32, bandwidth: f32, sample_rate: u32) -> Self {
        let mut filter = Self {
            center_freq,
            bandwidth,
            sample_rate,
            order: 4, // Default order
            coefficients: RwLock::new(None),
            filter_state: RwLock::new(None),
        };

        filter.compute_coefficients().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to compute filter coefficients: {}", e);
        });

        filter
    }

    /// Set the filter order
    ///
    /// ### Arguments
    ///
    /// * `order` - Filter order (must be positive, typical values: 2, 4, 6, 8)
    ///
    /// ### Returns
    ///
    /// The modified filter instance for method chaining
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::scipy_butter_filters::ButterBandpassFilter;
    ///
    /// let filter = ButterBandpassFilter::new(1000.0, 200.0, 48000)
    ///     .with_order(6);
    /// ```
    pub fn with_order(mut self, order: usize) -> Self {
        if order == 0 {
            panic!("Filter order must be greater than 0");
        }
        self.order = order;
        self.compute_coefficients().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to compute filter coefficients: {}", e);
        });
        self
    }

    /// Compute Butterworth filter coefficients using sci-rs
    fn compute_coefficients(&mut self) -> Result<()> {
        let nyquist = self.sample_rate as f64 / 2.0;
        let low_freq = (self.center_freq - self.bandwidth / 2.0) as f64 / nyquist;
        let high_freq = (self.center_freq + self.bandwidth / 2.0) as f64 / nyquist;

        if low_freq <= 0.0 || high_freq >= 1.0 {
            bail!("Filter frequencies out of valid range");
        }

        let critical_freqs = vec![low_freq, high_freq];

        // Use sci-rs to design the Butterworth bandpass filter
        let filter = butter_dyn(
            self.order,
            critical_freqs,            // Vec<f64>, not &Vec<f64>
            Some(FilterBandType::Bandpass), // FilterBandType enum
            Some(false),               // analog: Option<bool>
            Some(FilterOutputType::Ba), // output type
            None,                      // fs: already normalized to Nyquist
        );

        // Extract coefficients from the DigitalFilter
        match filter {
            DigitalFilter::Ba(ba_filter) => {
                *self.coefficients.write().unwrap() = Some((ba_filter.b, ba_filter.a));
                *self.filter_state.write().unwrap() = None; // Reset state
                Ok(())
            }
            _ => bail!("Expected Ba filter output format"),
        }
    }

    /// Reset the filter's internal state
    pub fn reset_state(&self) {
        *self.filter_state.write().unwrap() = None;
    }
}

impl Filter for ButterBandpassFilter {
    fn apply(&self, signal: &[f32]) -> Vec<f32> {
        let coeffs_guard = self.coefficients.read().unwrap();
        let Some((ref b, ref a)) = *coeffs_guard else {
            // Return original signal if no coefficients available
            return signal.to_vec();
        };

        // Convert input to f64 for processing
        let input_f64: Vec<f64> = signal.iter().map(|&x| x as f64).collect();

        // Apply the filter
        let filtered = self.apply_iir_filter(&input_f64, b, a);

        // Convert back to f32
        filtered.iter().map(|&x| x as f32).collect()
    }

    fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
        let mut updated = false;

        if let Some(center_freq) = parameters.get("center_freq") {
            if let Some(freq) = center_freq.as_f64() {
                if freq > 0.0 && freq < (self.sample_rate as f64 / 2.0) {
                    self.center_freq = freq as f32;
                    updated = true;
                } else {
                    bail!("center_freq must be positive and less than Nyquist frequency");
                }
            } else {
                bail!("center_freq must be a number");
            }
        }

        if let Some(bandwidth) = parameters.get("bandwidth") {
            if let Some(bw) = bandwidth.as_f64() {
                if bw > 0.0 {
                    self.bandwidth = bw as f32;
                    updated = true;
                } else {
                    bail!("bandwidth must be positive");
                }
            } else {
                bail!("bandwidth must be a number");
            }
        }

        if let Some(sample_rate) = parameters.get("sample_rate") {
            if let Some(sr) = sample_rate.as_u64() {
                if sr > 0 && sr <= u32::MAX as u64 {
                    self.sample_rate = sr as u32;
                    updated = true;
                } else {
                    bail!("sample_rate must be a positive integer within u32 range");
                }
            } else {
                bail!("sample_rate must be an integer");
            }
        }

        if let Some(order) = parameters.get("order") {
            if let Some(ord) = order.as_u64() {
                if ord > 0 && ord <= usize::MAX as u64 {
                    self.order = ord as usize;
                    updated = true;
                } else {
                    bail!("order must be a positive integer");
                }
            } else {
                bail!("order must be an integer");
            }
        }

        if updated {
            self.compute_coefficients()?;
        }

        Ok(updated)
    }
}

impl ButterBandpassFilter {
    /// Apply IIR filter with given coefficients
    /// This is a Direct Form II implementation
    fn apply_iir_filter(&self, input: &[f64], b: &[f64], a: &[f64]) -> Vec<f64> {
        let mut output = vec![0.0; input.len()];
        let n_b = b.len();
        let n_a = a.len();
        let max_len = n_b.max(n_a);

        // Initialize state if needed
        let mut state_guard = self.filter_state.write().unwrap();
        if state_guard.is_none() {
            *state_guard = Some(vec![0.0; max_len]);
        }
        let state = state_guard.as_mut().unwrap();

        for (i, &x) in input.iter().enumerate() {
            let mut y = 0.0;

            // Apply feedforward (numerator) coefficients
            for (j, &b_coeff) in b.iter().enumerate() {
                if i >= j {
                    y += b_coeff * input[i - j];
                } else if j - i - 1 < state.len() {
                    y += b_coeff * state[j - i - 1];
                }
            }

            // Apply feedback (denominator) coefficients (skip a[0] which should be 1.0)
            for (j, &a_coeff) in a.iter().skip(1).enumerate() {
                let idx = j + 1;
                if i >= idx {
                    y -= a_coeff * output[i - idx];
                } else if idx - i - 1 < state.len() {
                    y -= a_coeff * state[idx - i - 1];
                }
            }

            // Normalize by a[0] if it's not 1.0
            if !a.is_empty() && a[0] != 0.0 && a[0] != 1.0 {
                y /= a[0];
            }

            output[i] = y;

            // Update state
            if i < max_len {
                for j in ((i + 1)..max_len).rev() {
                    if j < state.len() {
                        state[j] = state[j - 1];
                    }
                }
                if !state.is_empty() {
                    state[0] = x;
                }
            }
        }

        output
    }
}

/// Butterworth lowpass filter using sci-rs implementation
///
/// This filter provides SciPy-compatible Butterworth lowpass filtering.
/// Butterworth filters have a maximally flat passband response.
///
/// ### Examples
///
/// ```no_run
/// use rust_photoacoustic::preprocessing::filter::{Filter, scipy_butter_filters::ButterLowpassFilter};
///
/// let filter = ButterLowpassFilter::new(1000.0, 48000)
///     .with_order(4);
///
/// let input = vec![1.0, 0.5, -0.3, 0.8, -0.2];
/// let output = filter.apply(&input);
/// assert_eq!(output.len(), input.len());
/// ```
pub struct ButterLowpassFilter {
    cutoff_freq: f32,
    sample_rate: u32,
    order: usize,
    coefficients: RwLock<Option<(Vec<f64>, Vec<f64>)>>,
    filter_state: RwLock<Option<Vec<f64>>>,
}

impl ButterLowpassFilter {
    /// Create a new Butterworth lowpass filter
    ///
    /// ### Arguments
    ///
    /// * `cutoff_freq` - Cutoff frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// ### Returns
    ///
    /// A new `ButterLowpassFilter` instance
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::scipy_butter_filters::ButterLowpassFilter;
    ///
    /// let filter = ButterLowpassFilter::new(1000.0, 48000);
    /// ```
    pub fn new(cutoff_freq: f32, sample_rate: u32) -> Self {
        let mut filter = Self {
            cutoff_freq,
            sample_rate,
            order: 4,
            coefficients: RwLock::new(None),
            filter_state: RwLock::new(None),
        };

        filter.compute_coefficients().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to compute filter coefficients: {}", e);
        });

        filter
    }

    /// Set the filter order
    pub fn with_order(mut self, order: usize) -> Self {
        if order == 0 {
            panic!("Filter order must be greater than 0");
        }
        self.order = order;
        self.compute_coefficients().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to compute filter coefficients: {}", e);
        });
        self
    }

    fn compute_coefficients(&mut self) -> Result<()> {
        let nyquist = self.sample_rate as f64 / 2.0;
        let normalized_freq = self.cutoff_freq as f64 / nyquist;

        if normalized_freq <= 0.0 || normalized_freq >= 1.0 {
            bail!("Filter frequency out of valid range");
        }

        let critical_freqs = vec![normalized_freq];

        let filter = butter_dyn(
            self.order,
            critical_freqs,                 // Vec<f64>
            Some(FilterBandType::Lowpass),  // FilterBandType enum
            Some(false),                    // analog: Option<bool>
            Some(FilterOutputType::Ba),     // output type
            None,                           // fs: already normalized
        );

        // Extract coefficients from the DigitalFilter
        match filter {
            DigitalFilter::Ba(ba_filter) => {
                *self.coefficients.write().unwrap() = Some((ba_filter.b, ba_filter.a));
                *self.filter_state.write().unwrap() = None;
                Ok(())
            }
            _ => bail!("Expected Ba filter output format"),
        }
    }

    /// Reset the filter's internal state
    pub fn reset_state(&self) {
        *self.filter_state.write().unwrap() = None;
    }
}

impl Filter for ButterLowpassFilter {
    fn apply(&self, signal: &[f32]) -> Vec<f32> {
        let coeffs_guard = self.coefficients.read().unwrap();
        let Some((ref b, ref a)) = *coeffs_guard else {
            return signal.to_vec();
        };

        let input_f64: Vec<f64> = signal.iter().map(|&x| x as f64).collect();
        let filtered = self.apply_iir_filter(&input_f64, b, a);
        filtered.iter().map(|&x| x as f32).collect()
    }

    fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
        let mut updated = false;

        if let Some(cutoff_freq) = parameters.get("cutoff_freq") {
            if let Some(freq) = cutoff_freq.as_f64() {
                if freq > 0.0 && freq < (self.sample_rate as f64 / 2.0) {
                    self.cutoff_freq = freq as f32;
                    updated = true;
                } else {
                    bail!("cutoff_freq must be positive and less than Nyquist frequency");
                }
            } else {
                bail!("cutoff_freq must be a number");
            }
        }

        if let Some(sample_rate) = parameters.get("sample_rate") {
            if let Some(sr) = sample_rate.as_u64() {
                if sr > 0 && sr <= u32::MAX as u64 {
                    self.sample_rate = sr as u32;
                    updated = true;
                } else {
                    bail!("sample_rate must be a positive integer within u32 range");
                }
            } else {
                bail!("sample_rate must be an integer");
            }
        }

        if let Some(order) = parameters.get("order") {
            if let Some(ord) = order.as_u64() {
                if ord > 0 && ord <= usize::MAX as u64 {
                    self.order = ord as usize;
                    updated = true;
                } else {
                    bail!("order must be a positive integer");
                }
            } else {
                bail!("order must be an integer");
            }
        }

        if updated {
            self.compute_coefficients()?;
        }

        Ok(updated)
    }
}

impl ButterLowpassFilter {
    fn apply_iir_filter(&self, input: &[f64], b: &[f64], a: &[f64]) -> Vec<f64> {
        let mut output = vec![0.0; input.len()];
        let n_b = b.len();
        let n_a = a.len();
        let max_len = n_b.max(n_a);

        let mut state_guard = self.filter_state.write().unwrap();
        if state_guard.is_none() {
            *state_guard = Some(vec![0.0; max_len]);
        }
        let state = state_guard.as_mut().unwrap();

        for (i, &x) in input.iter().enumerate() {
            let mut y = 0.0;

            // Apply feedforward coefficients
            for (j, &b_coeff) in b.iter().enumerate() {
                if i >= j {
                    y += b_coeff * input[i - j];
                } else if j - i - 1 < state.len() {
                    y += b_coeff * state[j - i - 1];
                }
            }

            // Apply feedback coefficients
            for (j, &a_coeff) in a.iter().skip(1).enumerate() {
                let idx = j + 1;
                if i >= idx {
                    y -= a_coeff * output[i - idx];
                } else if idx - i - 1 < state.len() {
                    y -= a_coeff * state[idx - i - 1];
                }
            }

            if !a.is_empty() && a[0] != 0.0 && a[0] != 1.0 {
                y /= a[0];
            }

            output[i] = y;

            // Update state
            if i < max_len {
                for j in ((i + 1)..max_len).rev() {
                    if j < state.len() {
                        state[j] = state[j - 1];
                    }
                }
                if !state.is_empty() {
                    state[0] = x;
                }
            }
        }

        output
    }
}

/// Butterworth highpass filter using sci-rs implementation
///
/// This filter provides SciPy-compatible Butterworth highpass filtering.
/// Butterworth filters have a maximally flat passband response.
///
/// ### Examples
///
/// ```no_run
/// use rust_photoacoustic::preprocessing::filter::{Filter, scipy_butter_filters::ButterHighpassFilter};
///
/// let filter = ButterHighpassFilter::new(100.0, 48000)
///     .with_order(4);
///
/// let input = vec![1.0, 0.5, -0.3, 0.8, -0.2];
/// let output = filter.apply(&input);
/// assert_eq!(output.len(), input.len());
/// ```
pub struct ButterHighpassFilter {
    cutoff_freq: f32,
    sample_rate: u32,
    order: usize,
    coefficients: RwLock<Option<(Vec<f64>, Vec<f64>)>>,
    filter_state: RwLock<Option<Vec<f64>>>,
}

impl ButterHighpassFilter {
    /// Create a new Butterworth highpass filter
    ///
    /// ### Arguments
    ///
    /// * `cutoff_freq` - Cutoff frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// ### Returns
    ///
    /// A new `ButterHighpassFilter` instance
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::scipy_butter_filters::ButterHighpassFilter;
    ///
    /// let filter = ButterHighpassFilter::new(100.0, 48000);
    /// ```
    pub fn new(cutoff_freq: f32, sample_rate: u32) -> Self {
        let mut filter = Self {
            cutoff_freq,
            sample_rate,
            order: 4,
            coefficients: RwLock::new(None),
            filter_state: RwLock::new(None),
        };

        filter.compute_coefficients().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to compute filter coefficients: {}", e);
        });

        filter
    }

    /// Set the filter order
    pub fn with_order(mut self, order: usize) -> Self {
        if order == 0 {
            panic!("Filter order must be greater than 0");
        }
        self.order = order;
        self.compute_coefficients().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to compute filter coefficients: {}", e);
        });
        self
    }

    fn compute_coefficients(&mut self) -> Result<()> {
        let nyquist = self.sample_rate as f64 / 2.0;
        let normalized_freq = self.cutoff_freq as f64 / nyquist;

        if normalized_freq <= 0.0 || normalized_freq >= 1.0 {
            bail!("Filter frequency out of valid range");
        }

        let critical_freqs = vec![normalized_freq];

        let filter = butter_dyn(
            self.order,
            critical_freqs,                 // Vec<f64>
            Some(FilterBandType::Highpass), // FilterBandType enum
            Some(false),                    // analog: Option<bool>
            Some(FilterOutputType::Ba),     // output type
            None,                           // fs: already normalized
        );

        // Extract coefficients from the DigitalFilter
        match filter {
            DigitalFilter::Ba(ba_filter) => {
                *self.coefficients.write().unwrap() = Some((ba_filter.b, ba_filter.a));
                *self.filter_state.write().unwrap() = None;
                Ok(())
            }
            _ => bail!("Expected Ba filter output format"),
        }
    }

    /// Reset the filter's internal state
    pub fn reset_state(&self) {
        *self.filter_state.write().unwrap() = None;
    }
}

impl Filter for ButterHighpassFilter {
    fn apply(&self, signal: &[f32]) -> Vec<f32> {
        let coeffs_guard = self.coefficients.read().unwrap();
        let Some((ref b, ref a)) = *coeffs_guard else {
            return signal.to_vec();
        };

        let input_f64: Vec<f64> = signal.iter().map(|&x| x as f64).collect();
        let filtered = self.apply_iir_filter(&input_f64, b, a);
        filtered.iter().map(|&x| x as f32).collect()
    }

    fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
        let mut updated = false;

        if let Some(cutoff_freq) = parameters.get("cutoff_freq") {
            if let Some(freq) = cutoff_freq.as_f64() {
                if freq > 0.0 && freq < (self.sample_rate as f64 / 2.0) {
                    self.cutoff_freq = freq as f32;
                    updated = true;
                } else {
                    bail!("cutoff_freq must be positive and less than Nyquist frequency");
                }
            } else {
                bail!("cutoff_freq must be a number");
            }
        }

        if let Some(sample_rate) = parameters.get("sample_rate") {
            if let Some(sr) = sample_rate.as_u64() {
                if sr > 0 && sr <= u32::MAX as u64 {
                    self.sample_rate = sr as u32;
                    updated = true;
                } else {
                    bail!("sample_rate must be a positive integer within u32 range");
                }
            } else {
                bail!("sample_rate must be an integer");
            }
        }

        if let Some(order) = parameters.get("order") {
            if let Some(ord) = order.as_u64() {
                if ord > 0 && ord <= usize::MAX as u64 {
                    self.order = ord as usize;
                    updated = true;
                } else {
                    bail!("order must be a positive integer");
                }
            } else {
                bail!("order must be an integer");
            }
        }

        if updated {
            self.compute_coefficients()?;
        }

        Ok(updated)
    }
}

impl ButterHighpassFilter {
    fn apply_iir_filter(&self, input: &[f64], b: &[f64], a: &[f64]) -> Vec<f64> {
        let mut output = vec![0.0; input.len()];
        let n_b = b.len();
        let n_a = a.len();
        let max_len = n_b.max(n_a);

        let mut state_guard = self.filter_state.write().unwrap();
        if state_guard.is_none() {
            *state_guard = Some(vec![0.0; max_len]);
        }
        let state = state_guard.as_mut().unwrap();

        for (i, &x) in input.iter().enumerate() {
            let mut y = 0.0;

            // Apply feedforward coefficients
            for (j, &b_coeff) in b.iter().enumerate() {
                if i >= j {
                    y += b_coeff * input[i - j];
                } else if j - i - 1 < state.len() {
                    y += b_coeff * state[j - i - 1];
                }
            }

            // Apply feedback coefficients
            for (j, &a_coeff) in a.iter().skip(1).enumerate() {
                let idx = j + 1;
                if i >= idx {
                    y -= a_coeff * output[i - idx];
                } else if idx - i - 1 < state.len() {
                    y -= a_coeff * state[idx - i - 1];
                }
            }

            if !a.is_empty() && a[0] != 0.0 && a[0] != 1.0 {
                y /= a[0];
            }

            output[i] = y;

            // Update state
            if i < max_len {
                for j in ((i + 1)..max_len).rev() {
                    if j < state.len() {
                        state[j] = state[j - 1];
                    }
                }
                if !state.is_empty() {
                    state[0] = x;
                }
            }
        }

        output
    }
}
