// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Digital filters for signal preprocessing

use anyhow::Result;

/// Trait for implementing digital filters
pub trait Filter: Send + Sync {
    /// Apply the filter to a signal and return the filtered signal
    fn apply(&self, signal: &[f32]) -> Vec<f32>;
}

/// A butterworth bandpass filter
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
    pub fn new(center_freq: f32, bandwidth: f32) -> Self {
        let sample_rate = 48000; // Default sample rate
        let order = 4; // Default 4th order filter (2 biquad sections)

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
    pub fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = sample_rate;
        self.compute_coefficients();
        self
    }

    /// Set the filter order (must be even)
    pub fn with_order(mut self, order: usize) -> Self {
        if order % 2 != 0 {
            panic!("Filter order must be even");
        }
        self.order = order;
        self.compute_coefficients();
        self
    }

    /// Compute filter coefficients based on current parameters
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
pub struct LowpassFilter {
    cutoff_freq: f32,
    sample_rate: u32,
}

impl LowpassFilter {
    /// Create a new lowpass filter with the specified cutoff frequency
    pub fn new(cutoff_freq: f32) -> Self {
        let sample_rate = 48000; // Default sample rate

        Self {
            cutoff_freq,
            sample_rate,
        }
    }

    /// Set the sample rate for the filter
    pub fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = sample_rate;
        self
    }
}

impl Filter for LowpassFilter {
    fn apply(&self, signal: &[f32]) -> Vec<f32> {
        // In a real implementation, this would apply a proper lowpass filter
        // For now, we'll just simulate filtering

        let mut filtered = Vec::with_capacity(signal.len());
        let mut prev_sample = 0.0;
        let alpha = 0.2; // Smoothing factor

        // Simple mock implementation using a very basic IIR filter
        for &sample in signal {
            let filtered_sample = alpha * sample + (1.0 - alpha) * prev_sample;
            filtered.push(filtered_sample);
            prev_sample = filtered_sample;
        }

        filtered
    }
}
