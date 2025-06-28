// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Butterworth digital filters using SciPy-style SOS (Second-Order Sections) + filtfilt
//!
//! This module provides Butterworth filter implementations that exactly match SciPy's approach:
//! 1. Design the filter using `iirfilter_dyn` with `FilterOutputType::Sos` to get SOS coefficients
//! 2. Apply the filter using `sosfiltfilt_dyn` for zero-phase filtering
//!
//! This approach ensures:
//! - Numerical stability through SOS representation
//! - Zero-phase filtering (no delay) through forward-backward filtering
//! - Exact compatibility with SciPy's signal.filtfilt function
//!
//! # Examples
//!
//! ```no_run
//! use rust_photoacoustic::preprocessing::filter::{Filter, scipy_butter_filter::ButterBandpassFilter};
//!
//! // Create a 4th-order Butterworth bandpass filter (20-20000 Hz) at 48kHz sample rate
//! let filter = ButterBandpassFilter::new(20.0, 20000.0, 48000.0, 4);
//! let input = vec![1.0, 0.5, -0.3, 0.8, -0.2];
//! let output = filter.apply(&input);
//! ```

use super::Filter;
use anyhow::Result;
use sci_rs::signal::filter::design::{
    iirfilter_dyn, DigitalFilter, FilterBandType, FilterOutputType, FilterType, Sos,
};
use sci_rs::signal::filter::sosfiltfilt_dyn;
use serde_json::Value;
use std::sync::Mutex;

/// Butterworth bandpass filter using SOS + filtfilt
///
/// Implements a Butterworth bandpass filter using SciPy's approach:
/// - SOS (Second-Order Sections) representation for numerical stability
/// - Zero-phase filtering via forward-backward filtering (filtfilt)
///
/// # Parameters
/// - `low_freq`: Lower cutoff frequency in Hz
/// - `high_freq`: Upper cutoff frequency in Hz  
/// - `sample_rate`: Sample rate in Hz
/// - `order`: Filter order (higher = steeper roll-off)
#[derive(Debug)]
pub struct ButterBandpassFilter {
    low_freq: f64,
    high_freq: f64,
    sample_rate: f64,
    order: usize,
    sos: Mutex<Option<Vec<Sos<f64>>>>,
}

impl ButterBandpassFilter {
    /// Create a new Butterworth bandpass filter
    ///
    /// # Arguments
    /// * `low_freq` - Lower cutoff frequency in Hz
    /// * `high_freq` - Upper cutoff frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    /// * `order` - Filter order (typically 2, 4, 6, 8)
    ///
    /// # Examples
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::scipy_butter_filter::ButterBandpassFilter;
    ///
    /// // 4th-order bandpass filter from 20 Hz to 20 kHz at 48 kHz sample rate
    /// let filter = ButterBandpassFilter::new(20.0, 20000.0, 48000.0, 4);
    /// ```
    pub fn new(low_freq: f64, high_freq: f64, sample_rate: f64, order: usize) -> Self {
        Self {
            low_freq,
            high_freq,
            sample_rate,
            order,
            sos: Mutex::new(None),
        }
    }

    /// Create a new Butterworth bandpass filter with default sample rate
    ///
    /// # Arguments
    /// * `low_freq` - Lower cutoff frequency in Hz
    /// * `high_freq` - Upper cutoff frequency in Hz
    ///
    /// # Examples
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::scipy_butter_filter::ButterBandpassFilter;
    ///
    /// // Create filter that will be configured later with sample rate and order
    /// let filter = ButterBandpassFilter::new_builder(20.0, 20000.0)
    ///     .with_sample_rate(48000.0)
    ///     .with_order(4);
    /// ```
    pub fn new_builder(low_freq: f64, high_freq: f64) -> Self {
        Self {
            low_freq,
            high_freq,
            sample_rate: 48000.0, // Default sample rate
            order: 4,             // Default order
            sos: Mutex::new(None),
        }
    }

    /// Set the sample rate for the filter (builder pattern)
    pub fn with_sample_rate(mut self, sample_rate: f64) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    /// Set the filter order (builder pattern)
    pub fn with_order(mut self, order: usize) -> Self {
        self.order = order;
        self
    }

    /// Get or compute the SOS coefficients
    fn get_sos(&self) -> Result<Vec<Sos<f64>>> {
        let mut sos_guard = self.sos.lock().unwrap();

        if sos_guard.is_none() {
            // Compute normalized frequencies (SciPy uses frequencies normalized to Nyquist)
            let nyquist = self.sample_rate / 2.0;
            let low_norm = self.low_freq / nyquist;
            let high_norm = self.high_freq / nyquist;

            // Design Butterworth bandpass filter using sci-rs
            let result = iirfilter_dyn(
                self.order,                     // filter order
                vec![low_norm, high_norm],      // critical frequencies (normalized)
                None,                           // rp (not used for Butterworth)
                None,                           // rs (not used for Butterworth)
                Some(FilterBandType::Bandpass), // filter type
                Some(FilterType::Butterworth),  // analog filter type (Butterworth)
                Some(false),                    // analog = false (digital filter)
                Some(FilterOutputType::Sos),    // output as SOS
                None,                           // fs (already normalized)
            );

            // Extract SOS coefficients from the result
            match result {
                DigitalFilter::Sos(sos_filter) => {
                    *sos_guard = Some(sos_filter.sos);
                }
                _ => return Err(anyhow::anyhow!("Expected SOS output from iirfilter_dyn")),
            }
        }

        Ok(sos_guard.as_ref().unwrap().clone())
    }
}

impl Filter for ButterBandpassFilter {
    fn apply(&self, signal: &[f32]) -> Vec<f32> {
        // Convert input to f64 for sci-rs
        let signal_f64: Vec<f64> = signal.iter().map(|&x| x as f64).collect();

        // Get SOS coefficients
        let sos = match self.get_sos() {
            Ok(sos) => sos,
            Err(e) => {
                eprintln!("Error getting SOS coefficients: {}", e);
                return signal.to_vec(); // Return original signal on error
            }
        };

        // Apply zero-phase filtering using sosfiltfilt_dyn
        let filtered = sosfiltfilt_dyn(signal_f64.iter(), &sos);

        // Convert back to f32
        filtered.iter().map(|&x| x as f32).collect()
    }

    fn update_config(&mut self, parameters: &Value) -> Result<bool> {
        let mut updated = false;

        if let Some(new_low_freq) = parameters.get("low_freq").and_then(|v| v.as_f64()) {
            if new_low_freq > 0.0 && new_low_freq < self.sample_rate / 2.0 {
                self.low_freq = new_low_freq;
                updated = true;
            }
        }

        if let Some(new_high_freq) = parameters.get("high_freq").and_then(|v| v.as_f64()) {
            if new_high_freq > self.low_freq && new_high_freq < self.sample_rate / 2.0 {
                self.high_freq = new_high_freq;
                updated = true;
            }
        }

        if let Some(new_sample_rate) = parameters.get("sample_rate").and_then(|v| v.as_f64()) {
            if new_sample_rate > 0.0 {
                self.sample_rate = new_sample_rate;
                updated = true;
            }
        }

        if let Some(new_order) = parameters.get("order").and_then(|v| v.as_u64()) {
            if new_order > 0 && new_order <= 20 {
                self.order = new_order as usize;
                updated = true;
            }
        }

        // Clear cached SOS if parameters were updated
        if updated {
            *self.sos.lock().unwrap() = None;
        }

        Ok(updated)
    }
}

/// Butterworth lowpass filter using SOS + filtfilt
///
/// # Parameters
/// - `cutoff_freq`: Cutoff frequency in Hz
/// - `sample_rate`: Sample rate in Hz
/// - `order`: Filter order
#[derive(Debug)]
pub struct ButterLowpassFilter {
    cutoff_freq: f64,
    sample_rate: f64,
    order: usize,
    sos: Mutex<Option<Vec<Sos<f64>>>>,
}

impl ButterLowpassFilter {
    /// Create a new Butterworth lowpass filter
    pub fn new(cutoff_freq: f64, sample_rate: f64, order: usize) -> Self {
        Self {
            cutoff_freq,
            sample_rate,
            order,
            sos: Mutex::new(None),
        }
    }

    /// Create a new Butterworth lowpass filter with default sample rate
    ///
    /// # Arguments
    /// * `cutoff_freq` - Cutoff frequency in Hz
    ///
    /// # Examples
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::scipy_butter_filter::ButterLowpassFilter;
    ///
    /// // Create filter that will be configured later with sample rate and order
    /// let filter = ButterLowpassFilter::new_builder(1000.0)
    ///     .with_sample_rate(48000.0)
    ///     .with_order(4);
    /// ```
    pub fn new_builder(cutoff_freq: f64) -> Self {
        Self {
            cutoff_freq,
            sample_rate: 48000.0, // Default sample rate
            order: 4,             // Default order
            sos: Mutex::new(None),
        }
    }

    /// Set the sample rate for the filter (builder pattern)
    pub fn with_sample_rate(mut self, sample_rate: f64) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    /// Set the filter order (builder pattern)
    pub fn with_order(mut self, order: usize) -> Self {
        self.order = order;
        self
    }

    fn get_sos(&self) -> Result<Vec<Sos<f64>>> {
        let mut sos_guard = self.sos.lock().unwrap();

        if sos_guard.is_none() {
            let nyquist = self.sample_rate / 2.0;
            let cutoff_norm = self.cutoff_freq / nyquist;

            let result = iirfilter_dyn(
                self.order,
                vec![cutoff_norm],
                None,                          // rp (not used for Butterworth)
                None,                          // rs (not used for Butterworth)
                Some(FilterBandType::Lowpass), // filter type
                Some(FilterType::Butterworth), // analog filter type (Butterworth)
                Some(false),                   // analog = false (digital filter)
                Some(FilterOutputType::Sos),   // output as SOS
                None,                          // fs (already normalized)
            );

            match result {
                DigitalFilter::Sos(sos_filter) => {
                    *sos_guard = Some(sos_filter.sos);
                }
                _ => return Err(anyhow::anyhow!("Expected SOS output from iirfilter_dyn")),
            }
        }

        Ok(sos_guard.as_ref().unwrap().clone())
    }
}

impl Filter for ButterLowpassFilter {
    fn apply(&self, signal: &[f32]) -> Vec<f32> {
        let signal_f64: Vec<f64> = signal.iter().map(|&x| x as f64).collect();

        let sos = match self.get_sos() {
            Ok(sos) => sos,
            Err(e) => {
                eprintln!("Error getting SOS coefficients: {}", e);
                return signal.to_vec();
            }
        };

        let filtered = sosfiltfilt_dyn(signal_f64.iter(), &sos);

        filtered.iter().map(|&x| x as f32).collect()
    }

    fn update_config(&mut self, parameters: &Value) -> Result<bool> {
        let mut updated = false;

        if let Some(new_cutoff) = parameters.get("cutoff_freq").and_then(|v| v.as_f64()) {
            if new_cutoff > 0.0 && new_cutoff < self.sample_rate / 2.0 {
                self.cutoff_freq = new_cutoff;
                updated = true;
            }
        }

        if let Some(new_sample_rate) = parameters.get("sample_rate").and_then(|v| v.as_f64()) {
            if new_sample_rate > 0.0 {
                self.sample_rate = new_sample_rate;
                updated = true;
            }
        }

        if let Some(new_order) = parameters.get("order").and_then(|v| v.as_u64()) {
            if new_order > 0 && new_order <= 20 {
                self.order = new_order as usize;
                updated = true;
            }
        }

        if updated {
            *self.sos.lock().unwrap() = None;
        }

        Ok(updated)
    }
}

/// Butterworth highpass filter using SOS + filtfilt
///
/// # Parameters
/// - `cutoff_freq`: Cutoff frequency in Hz
/// - `sample_rate`: Sample rate in Hz
/// - `order`: Filter order
#[derive(Debug)]
pub struct ButterHighpassFilter {
    cutoff_freq: f64,
    sample_rate: f64,
    order: usize,
    sos: Mutex<Option<Vec<Sos<f64>>>>,
}

impl ButterHighpassFilter {
    /// Create a new Butterworth highpass filter
    pub fn new(cutoff_freq: f64, sample_rate: f64, order: usize) -> Self {
        Self {
            cutoff_freq,
            sample_rate,
            order,
            sos: Mutex::new(None),
        }
    }

    /// Create a new Butterworth highpass filter with default sample rate
    ///
    /// # Arguments
    /// * `cutoff_freq` - Cutoff frequency in Hz
    ///
    /// # Examples
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::scipy_butter_filter::ButterHighpassFilter;
    ///
    /// // Create filter that will be configured later with sample rate and order
    /// let filter = ButterHighpassFilter::new_builder(1000.0)
    ///     .with_sample_rate(48000.0)
    ///     .with_order(4);
    /// ```
    pub fn new_builder(cutoff_freq: f64) -> Self {
        Self {
            cutoff_freq,
            sample_rate: 48000.0, // Default sample rate
            order: 4,             // Default order
            sos: Mutex::new(None),
        }
    }

    /// Set the sample rate for the filter (builder pattern)
    pub fn with_sample_rate(mut self, sample_rate: f64) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    /// Set the filter order (builder pattern)
    pub fn with_order(mut self, order: usize) -> Self {
        self.order = order;
        self
    }

    fn get_sos(&self) -> Result<Vec<Sos<f64>>> {
        let mut sos_guard = self.sos.lock().unwrap();

        if sos_guard.is_none() {
            let nyquist = self.sample_rate / 2.0;
            let cutoff_norm = self.cutoff_freq / nyquist;

            let result = iirfilter_dyn(
                self.order,
                vec![cutoff_norm],
                None,                           // rp (not used for Butterworth)
                None,                           // rs (not used for Butterworth)
                Some(FilterBandType::Highpass), // filter type
                Some(FilterType::Butterworth),  // analog filter type (Butterworth)
                Some(false),                    // analog = false (digital filter)
                Some(FilterOutputType::Sos),    // output as SOS
                None,                           // fs (already normalized)
            );

            match result {
                DigitalFilter::Sos(sos_filter) => {
                    *sos_guard = Some(sos_filter.sos);
                }
                _ => return Err(anyhow::anyhow!("Expected SOS output from iirfilter_dyn")),
            }
        }

        Ok(sos_guard.as_ref().unwrap().clone())
    }
}

impl Filter for ButterHighpassFilter {
    fn apply(&self, signal: &[f32]) -> Vec<f32> {
        let signal_f64: Vec<f64> = signal.iter().map(|&x| x as f64).collect();

        let sos = match self.get_sos() {
            Ok(sos) => sos,
            Err(e) => {
                eprintln!("Error getting SOS coefficients: {}", e);
                return signal.to_vec();
            }
        };

        let filtered = sosfiltfilt_dyn(signal_f64.iter(), &sos);

        filtered.iter().map(|&x| x as f32).collect()
    }

    fn update_config(&mut self, parameters: &Value) -> Result<bool> {
        let mut updated = false;

        if let Some(new_cutoff) = parameters.get("cutoff_freq").and_then(|v| v.as_f64()) {
            if new_cutoff > 0.0 && new_cutoff < self.sample_rate / 2.0 {
                self.cutoff_freq = new_cutoff;
                updated = true;
            }
        }

        if let Some(new_sample_rate) = parameters.get("sample_rate").and_then(|v| v.as_f64()) {
            if new_sample_rate > 0.0 {
                self.sample_rate = new_sample_rate;
                updated = true;
            }
        }

        if let Some(new_order) = parameters.get("order").and_then(|v| v.as_u64()) {
            if new_order > 0 && new_order <= 20 {
                self.order = new_order as usize;
                updated = true;
            }
        }

        if updated {
            *self.sos.lock().unwrap() = None;
        }

        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    // Helper function to generate a test signal long enough for filtfilt
    fn generate_test_signal(sample_rate: f64, duration: f64, freq: f64) -> Vec<f32> {
        let samples = (sample_rate * duration) as usize;
        (0..samples)
            .map(|i| {
                let t = i as f64 / sample_rate;
                (2.0 * std::f64::consts::PI * freq * t).sin() as f32
            })
            .collect()
    }

    #[test]
    fn test_butter_bandpass_basic() {
        let filter = ButterBandpassFilter::new(1000.0, 2000.0, 8000.0, 4);
        // Generate a 0.1 second signal (800 samples) at 8kHz with mixed frequencies
        let input = generate_test_signal(8000.0, 0.1, 1500.0); // 1.5kHz tone (should pass)
        let output = filter.apply(&input);
        assert_eq!(output.len(), input.len());
        // Signal should not be completely zero after filtering
        assert!(output.iter().any(|&x| x.abs() > 1e-6));
    }

    #[test]
    fn test_butter_lowpass_basic() {
        let filter = ButterLowpassFilter::new(1000.0, 8000.0, 4);
        // Generate a 0.1 second signal (800 samples) at 8kHz with 500Hz tone (should pass)
        let input = generate_test_signal(8000.0, 0.1, 500.0);
        let output = filter.apply(&input);
        assert_eq!(output.len(), input.len());
        // Signal should not be completely zero after filtering
        assert!(output.iter().any(|&x| x.abs() > 1e-6));
    }

    #[test]
    fn test_butter_highpass_basic() {
        let filter = ButterHighpassFilter::new(1000.0, 8000.0, 4);
        // Generate a 0.1 second signal (800 samples) at 8kHz with 2kHz tone (should pass)
        let input = generate_test_signal(8000.0, 0.1, 2000.0);
        let output = filter.apply(&input);
        assert_eq!(output.len(), input.len());
        // Signal should not be completely zero after filtering
        assert!(output.iter().any(|&x| x.abs() > 1e-6));
    }

    #[test]
    fn test_config_update() {
        let mut filter = ButterBandpassFilter::new(1000.0, 2000.0, 8000.0, 4);
        let params = serde_json::json!({
            "low_freq": 500.0,
            "high_freq": 3000.0,
            "order": 6
        });
        let result = filter.update_config(&params);
        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.low_freq, 500.0);
        assert_eq!(filter.high_freq, 3000.0);
        assert_eq!(filter.order, 6);
    }
}
