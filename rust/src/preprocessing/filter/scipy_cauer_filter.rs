// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Cauer (elliptic) digital filters using SciPy-style SOS (Second-Order Sections) + filtfilt
//!
//! This module provides Cauer (elliptic) filter implementations that exactly match SciPy's approach:
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
//! use rust_photoacoustic::preprocessing::filter::{Filter, scipy_cauer_filter::CauerBandpassFilter};
//!
//! // Create a 4th-order Cauer (elliptic) bandpass filter (20-20000 Hz) at 48kHz sample rate
//! let filter = CauerBandpassFilter::new(20.0, 20000.0, 48000.0, 4, 1.0, 60.0);
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

/// Cauer (elliptic) bandpass filter using SOS + filtfilt
///
/// Implements a Cauer (elliptic) bandpass filter using SciPy's approach:
/// - SOS (Second-Order Sections) representation for numerical stability
/// - Zero-phase filtering via forward-backward filtering (filtfilt)
///
/// # Parameters
/// - `low_freq`: Lower cutoff frequency in Hz
/// - `high_freq`: Upper cutoff frequency in Hz  
/// - `sample_rate`: Sample rate in Hz
/// - `order`: Filter order (higher = steeper roll-off)
#[derive(Debug)]
pub struct CauerBandpassFilter {
    low_freq: f64,
    high_freq: f64,
    sample_rate: f64,
    order: usize,
    ripple: f64,      // Passband ripple in dB (rp)
    attenuation: f64, // Stopband attenuation in dB (rs)
    sos: Mutex<Option<Vec<Sos<f64>>>>,
}

impl CauerBandpassFilter {
    /// Create a new Cauer (elliptic) bandpass filter
    ///
    /// # Arguments
    /// * `low_freq` - Lower cutoff frequency in Hz
    /// * `high_freq` - Upper cutoff frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    /// * `order` - Filter order (typically 2, 4, 6, 8)
    /// * `ripple` - Maximum ripple in the passband, in dB (typically 0.1 to 3.0)
    /// * `attenuation` - Minimum attenuation in the stopband, in dB (typically 40 to 80)
    ///
    /// # Examples
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::scipy_cauer_filter::CauerBandpassFilter;
    ///
    /// // 4th-order bandpass filter from 20 Hz to 20 kHz at 48 kHz sample rate
    /// // with 1 dB passband ripple and 60 dB stopband attenuation
    /// let filter = CauerBandpassFilter::new(20.0, 20000.0, 48000.0, 4, 1.0, 60.0);
    /// ```
    pub fn new(
        low_freq: f64,
        high_freq: f64,
        sample_rate: f64,
        order: usize,
        ripple: f64,
        attenuation: f64,
    ) -> Self {
        Self {
            low_freq,
            high_freq,
            sample_rate,
            order,
            ripple,
            attenuation,
            sos: Mutex::new(None),
        }
    }

    /// Create a new Cauer (elliptic) bandpass filter with default sample rate and ripple parameters
    ///
    /// # Arguments
    /// * `low_freq` - Lower cutoff frequency in Hz
    /// * `high_freq` - Upper cutoff frequency in Hz
    ///
    /// # Examples
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::scipy_cauer_filter::CauerBandpassFilter;
    ///
    /// // Create filter that will be configured later with sample rate and order
    /// let filter = CauerBandpassFilter::new_builder(20.0, 20000.0)
    ///     .with_sample_rate(48000.0)
    ///     .with_order(4)
    ///     .with_ripple(1.0)
    ///     .with_attenuation(60.0);
    /// ```
    pub fn new_builder(low_freq: f64, high_freq: f64) -> Self {
        Self {
            low_freq,
            high_freq,
            sample_rate: 48000.0, // Default sample rate
            order: 4,             // Default order
            ripple: 1.0,          // Default 1 dB passband ripple
            attenuation: 60.0,    // Default 60 dB stopband attenuation
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

    /// Set the passband ripple (builder pattern)
    pub fn with_ripple(mut self, ripple: f64) -> Self {
        self.ripple = ripple;
        self
    }

    /// Set the stopband attenuation (builder pattern)
    pub fn with_attenuation(mut self, attenuation: f64) -> Self {
        self.attenuation = attenuation;
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

            // Design Cauer (elliptic) bandpass filter using sci-rs
            let result = iirfilter_dyn(
                self.order,                      // filter order
                vec![low_norm, high_norm],       // critical frequencies (normalized)
                Some(self.ripple),               // rp (passband ripple in dB)
                Some(self.attenuation),          // rs (stopband attenuation in dB)
                Some(FilterBandType::Bandpass),  // filter type
                Some(FilterType::CauerElliptic), // analog filter type (Cauer (elliptic))
                Some(false),                     // analog = false (digital filter)
                Some(FilterOutputType::Sos),     // output as SOS
                None,                            // fs (already normalized)
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

impl Filter for CauerBandpassFilter {
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

        if let Some(new_ripple) = parameters.get("ripple").and_then(|v| v.as_f64()) {
            if new_ripple > 0.0 && new_ripple <= 10.0 {
                // Reasonable range for passband ripple
                self.ripple = new_ripple;
                updated = true;
            }
        }

        if let Some(new_attenuation) = parameters.get("attenuation").and_then(|v| v.as_f64()) {
            if new_attenuation > 0.0 && new_attenuation <= 120.0 {
                // Reasonable range for stopband attenuation
                self.attenuation = new_attenuation;
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

/// Cauer (elliptic) lowpass filter using SOS + filtfilt
///
/// # Parameters
/// - `cutoff_freq`: Cutoff frequency in Hz
/// - `sample_rate`: Sample rate in Hz
/// - `order`: Filter order
/// - `ripple`: Passband ripple in dB
/// - `attenuation`: Stopband attenuation in dB
#[derive(Debug)]
pub struct CauerLowpassFilter {
    cutoff_freq: f64,
    sample_rate: f64,
    order: usize,
    ripple: f64,      // Passband ripple in dB (rp)
    attenuation: f64, // Stopband attenuation in dB (rs)
    sos: Mutex<Option<Vec<Sos<f64>>>>,
}

impl CauerLowpassFilter {
    /// Create a new Cauer (elliptic) lowpass filter
    ///
    /// # Arguments
    /// * `cutoff_freq` - Cutoff frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    /// * `order` - Filter order
    /// * `ripple` - Maximum ripple in the passband, in dB (typically 0.1 to 3.0)
    /// * `attenuation` - Minimum attenuation in the stopband, in dB (typically 40 to 80)
    pub fn new(
        cutoff_freq: f64,
        sample_rate: f64,
        order: usize,
        ripple: f64,
        attenuation: f64,
    ) -> Self {
        Self {
            cutoff_freq,
            sample_rate,
            order,
            ripple,
            attenuation,
            sos: Mutex::new(None),
        }
    }

    /// Create a new Cauer (elliptic) lowpass filter with default sample rate and ripple parameters
    ///
    /// # Arguments
    /// * `cutoff_freq` - Cutoff frequency in Hz
    ///
    /// # Examples
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::scipy_cauer_filter::CauerLowpassFilter;
    ///
    /// // Create filter that will be configured later with sample rate and order
    /// let filter = CauerLowpassFilter::new_builder(1000.0)
    ///     .with_sample_rate(48000.0)
    ///     .with_order(4)
    ///     .with_ripple(1.0)
    ///     .with_attenuation(60.0);
    /// ```
    pub fn new_builder(cutoff_freq: f64) -> Self {
        Self {
            cutoff_freq,
            sample_rate: 48000.0, // Default sample rate
            order: 4,             // Default order
            ripple: 1.0,          // Default 1 dB passband ripple
            attenuation: 60.0,    // Default 60 dB stopband attenuation
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

    /// Set the passband ripple (builder pattern)
    pub fn with_ripple(mut self, ripple: f64) -> Self {
        self.ripple = ripple;
        self
    }

    /// Set the stopband attenuation (builder pattern)
    pub fn with_attenuation(mut self, attenuation: f64) -> Self {
        self.attenuation = attenuation;
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
                Some(self.ripple),               // rp (passband ripple in dB)
                Some(self.attenuation),          // rs (stopband attenuation in dB)
                Some(FilterBandType::Lowpass),   // filter type
                Some(FilterType::CauerElliptic), // analog filter type (Cauer (elliptic))
                Some(false),                     // analog = false (digital filter)
                Some(FilterOutputType::Sos),     // output as SOS
                None,                            // fs (already normalized)
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

impl Filter for CauerLowpassFilter {
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

        if let Some(new_ripple) = parameters.get("ripple").and_then(|v| v.as_f64()) {
            if new_ripple > 0.0 && new_ripple <= 10.0 {
                // Reasonable range for passband ripple
                self.ripple = new_ripple;
                updated = true;
            }
        }

        if let Some(new_attenuation) = parameters.get("attenuation").and_then(|v| v.as_f64()) {
            if new_attenuation > 0.0 && new_attenuation <= 120.0 {
                // Reasonable range for stopband attenuation
                self.attenuation = new_attenuation;
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

/// Cauer (elliptic) highpass filter using SOS + filtfilt
///
/// # Parameters
/// - `cutoff_freq`: Cutoff frequency in Hz
/// - `sample_rate`: Sample rate in Hz
/// - `order`: Filter order
/// - `ripple`: Passband ripple in dB
/// - `attenuation`: Stopband attenuation in dB
#[derive(Debug)]
pub struct CauerHighpassFilter {
    cutoff_freq: f64,
    sample_rate: f64,
    order: usize,
    ripple: f64,      // Passband ripple in dB (rp)
    attenuation: f64, // Stopband attenuation in dB (rs)
    sos: Mutex<Option<Vec<Sos<f64>>>>,
}

impl CauerHighpassFilter {
    /// Create a new Cauer (elliptic) highpass filter
    ///
    /// # Arguments
    /// * `cutoff_freq` - Cutoff frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    /// * `order` - Filter order
    /// * `ripple` - Maximum ripple in the passband, in dB (typically 0.1 to 3.0)
    /// * `attenuation` - Minimum attenuation in the stopband, in dB (typically 40 to 80)
    pub fn new(
        cutoff_freq: f64,
        sample_rate: f64,
        order: usize,
        ripple: f64,
        attenuation: f64,
    ) -> Self {
        Self {
            cutoff_freq,
            sample_rate,
            order,
            ripple,
            attenuation,
            sos: Mutex::new(None),
        }
    }

    /// Create a new Cauer (elliptic) highpass filter with default sample rate and ripple parameters
    ///
    /// # Arguments
    /// * `cutoff_freq` - Cutoff frequency in Hz
    ///
    /// # Examples
    /// ```no_run
    /// use rust_photoacoustic::preprocessing::filter::scipy_cauer_filter::CauerHighpassFilter;
    ///
    /// // Create filter that will be configured later with sample rate and order
    /// let filter = CauerHighpassFilter::new_builder(1000.0)
    ///     .with_sample_rate(48000.0)
    ///     .with_order(4)
    ///     .with_ripple(1.0)
    ///     .with_attenuation(60.0);
    /// ```
    pub fn new_builder(cutoff_freq: f64) -> Self {
        Self {
            cutoff_freq,
            sample_rate: 48000.0, // Default sample rate
            order: 4,             // Default order
            ripple: 1.0,          // Default 1 dB passband ripple
            attenuation: 60.0,    // Default 60 dB stopband attenuation
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

    /// Set the passband ripple (builder pattern)
    pub fn with_ripple(mut self, ripple: f64) -> Self {
        self.ripple = ripple;
        self
    }

    /// Set the stopband attenuation (builder pattern)
    pub fn with_attenuation(mut self, attenuation: f64) -> Self {
        self.attenuation = attenuation;
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
                Some(self.ripple),               // rp (passband ripple in dB)
                Some(self.attenuation),          // rs (stopband attenuation in dB)
                Some(FilterBandType::Highpass),  // filter type
                Some(FilterType::CauerElliptic), // analog filter type (Cauer (elliptic))
                Some(false),                     // analog = false (digital filter)
                Some(FilterOutputType::Sos),     // output as SOS
                None,                            // fs (already normalized)
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

impl Filter for CauerHighpassFilter {
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

        if let Some(new_ripple) = parameters.get("ripple").and_then(|v| v.as_f64()) {
            if new_ripple > 0.0 && new_ripple <= 10.0 {
                // Reasonable range for passband ripple
                self.ripple = new_ripple;
                updated = true;
            }
        }

        if let Some(new_attenuation) = parameters.get("attenuation").and_then(|v| v.as_f64()) {
            if new_attenuation > 0.0 && new_attenuation <= 120.0 {
                // Reasonable range for stopband attenuation
                self.attenuation = new_attenuation;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[ignore] // Not yet implemented by sci-rs
    #[test]
    fn test_cauer_bandpass_basic() {
        let filter = CauerBandpassFilter::new(1000.0, 2000.0, 8000.0, 4, 1.0, 60.0);
        let input = vec![1.0, 0.0, -1.0, 0.0, 1.0, 0.0, -1.0, 0.0];
        let output = filter.apply(&input);
        assert_eq!(output.len(), input.len());
    }

    #[ignore] // Not yet implemented by sci-rs
    #[test]
    fn test_cauer_lowpass_basic() {
        let filter = CauerLowpassFilter::new(1000.0, 8000.0, 4, 1.0, 60.0);
        let input = vec![1.0, 0.0, -1.0, 0.0, 1.0, 0.0, -1.0, 0.0];
        let output = filter.apply(&input);
        assert_eq!(output.len(), input.len());
    }

    #[ignore] // Not yet implemented by sci-rs
    #[test]
    fn test_cauer_highpass_basic() {
        let filter = CauerHighpassFilter::new(1000.0, 8000.0, 4, 1.0, 60.0);
        let input = vec![1.0, 0.0, -1.0, 0.0, 1.0, 0.0, -1.0, 0.0];
        let output = filter.apply(&input);
        assert_eq!(output.len(), input.len());
    }

    #[test]
    fn test_config_update() {
        let mut filter = CauerBandpassFilter::new(1000.0, 2000.0, 8000.0, 4, 1.0, 60.0);
        let params = serde_json::json!({
            "low_freq": 500.0,
            "high_freq": 3000.0,
            "order": 6,
            "ripple": 2.0,
            "attenuation": 80.0
        });
        let result = filter.update_config(&params);
        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(filter.low_freq, 500.0);
        assert_eq!(filter.high_freq, 3000.0);
        assert_eq!(filter.order, 6);
        assert_eq!(filter.ripple, 2.0);
        assert_eq!(filter.attenuation, 80.0);
    }
}
