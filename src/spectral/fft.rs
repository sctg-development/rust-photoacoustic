// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! FFT implementation for spectral analysis

use anyhow::Result;
use rustfft::{num_complex::Complex32, FftPlanner};

/// Trait for implementing spectral analysis
pub trait SpectralAnalyzer: Send + Sync {
    /// Analyze the given signal and extract spectral components
    fn analyze(&mut self, signal: &[f32], sample_rate: u32) -> Result<SpectrumData>;

    /// Get the amplitude of the component at the specified frequency
    fn get_amplitude_at(&self, frequency: f32) -> Result<f32>;
}

/// Data resulting from spectral analysis
#[derive(Debug, Clone)]
pub struct SpectrumData {
    pub frequencies: Vec<f32>,
    pub amplitudes: Vec<f32>,
    pub phases: Vec<f32>,
    pub sample_rate: u32,
}

/// FFT-based spectral analyzer
pub struct FFTAnalyzer {
    window_size: usize,
    averages: usize,
    window_function: WindowFunction,
    spectrum_data: Option<SpectrumData>,
    previous_spectra: Vec<Vec<Complex32>>,
}

impl FFTAnalyzer {
    /// Create a new FFT analyzer with the given window size and averaging
    pub fn new(window_size: usize, averages: usize) -> Self {
        Self {
            window_size,
            averages,
            window_function: WindowFunction::Hann,
            spectrum_data: None,
            previous_spectra: Vec::with_capacity(averages),
        }
    }

    /// Apply window function to the input signal
    fn apply_window(&self, signal: &[f32]) -> Vec<f32> {
        let mut windowed = Vec::with_capacity(signal.len());

        for (i, &sample) in signal.iter().enumerate() {
            let window_factor = match self.window_function {
                WindowFunction::Rectangular => 1.0,
                WindowFunction::Hann => {
                    0.5 * (1.0
                        - (2.0 * std::f32::consts::PI * i as f32 / (signal.len() - 1) as f32).cos())
                }
                WindowFunction::Blackman => {
                    let a0 = 0.42;
                    let a1 = 0.5;
                    let a2 = 0.08;
                    let x = i as f32 / (signal.len() - 1) as f32;
                    a0 - a1 * (2.0 * std::f32::consts::PI * x).cos()
                        + a2 * (4.0 * std::f32::consts::PI * x).cos()
                }
            };

            windowed.push(sample * window_factor);
        }

        windowed
    }

    /// Compute FFT of the input signal
    fn compute_fft(&mut self, signal: &[f32]) -> Vec<Complex32> {
        // Convert input to complex numbers
        let mut complex_input: Vec<Complex32> =
            signal.iter().map(|&x| Complex32::new(x, 0.0)).collect();

        // Create FFT planner and get FFT algorithm
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(signal.len());

        // Execute FFT in-place
        fft.process(&mut complex_input);

        complex_input
    }

    /// Convert FFT output to meaningful spectrum data
    fn fft_to_spectrum(&self, fft_output: &[Complex32], sample_rate: u32) -> SpectrumData {
        let n = fft_output.len();
        let nyquist = sample_rate as f32 / 2.0;
        let df = sample_rate as f32 / n as f32;

        // We only need the first half of the spectrum (up to Nyquist frequency)
        let useful_bins = n / 2;

        let mut frequencies = Vec::with_capacity(useful_bins);
        let mut amplitudes = Vec::with_capacity(useful_bins);
        let mut phases = Vec::with_capacity(useful_bins);

        for i in 0..useful_bins {
            let frequency = i as f32 * df;
            let complex_val = fft_output[i];

            // Calculate amplitude (normalized by window size)
            let amplitude = (complex_val.norm() / n as f32) * 2.0; // Multiply by 2 to account for negative frequencies
            let phase = complex_val.arg();

            frequencies.push(frequency);
            amplitudes.push(amplitude);
            phases.push(phase);
        }

        SpectrumData {
            frequencies,
            amplitudes,
            phases,
            sample_rate,
        }
    }
}

impl SpectralAnalyzer for FFTAnalyzer {
    fn analyze(&mut self, signal: &[f32], sample_rate: u32) -> Result<SpectrumData> {
        // Check if signal is long enough
        if signal.len() < self.window_size {
            return Err(anyhow::anyhow!(
                "Signal too short: {} samples (need {})",
                signal.len(),
                self.window_size
            ));
        }

        // Apply window function
        let windowed = self.apply_window(&signal[0..self.window_size]);

        // Compute FFT
        let fft_result = self.compute_fft(&windowed);

        // Add to previous spectra for averaging
        self.previous_spectra.push(fft_result.clone());
        if self.previous_spectra.len() > self.averages {
            self.previous_spectra.remove(0);
        }

        // Average spectra
        let mut avg_spectrum = vec![Complex32::new(0.0, 0.0); self.window_size];

        for spectrum in &self.previous_spectra {
            for (i, &complex_val) in spectrum.iter().enumerate() {
                avg_spectrum[i] += complex_val;
            }
        }

        for complex_val in &mut avg_spectrum {
            *complex_val /= Complex32::new(self.previous_spectra.len() as f32, 0.0);
        }

        // Convert to spectrum data
        let spectrum = self.fft_to_spectrum(&avg_spectrum, sample_rate);

        // Store for later reference
        self.spectrum_data = Some(spectrum.clone());

        Ok(spectrum)
    }

    fn get_amplitude_at(&self, frequency: f32) -> Result<f32> {
        let spectrum = self
            .spectrum_data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No spectrum data available. Call analyze() first."))?;

        // Find the closest frequency bin
        let df = spectrum.sample_rate as f32 / (self.window_size as f32);
        let bin = (frequency / df).round() as usize;

        if bin >= spectrum.frequencies.len() {
            return Err(anyhow::anyhow!(
                "Frequency {} Hz is outside the analyzed spectrum",
                frequency
            ));
        }

        Ok(spectrum.amplitudes[bin])
    }
}

/// Available window functions for spectral analysis
#[derive(Debug, Clone, Copy)]
pub enum WindowFunction {
    Rectangular,
    Hann,
    Blackman,
}
