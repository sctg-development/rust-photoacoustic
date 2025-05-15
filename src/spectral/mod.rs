//! Spectral analysis module
//!
//! This module handles spectral analysis of the signal,
//! particularly Fast Fourier Transform (FFT) processing.

mod fft;

pub use fft::SpectralAnalyzer;

use anyhow::Result;

/// Create a new spectral analyzer with the given window size and averaging
pub fn create_spectral_analyzer(window_size: usize, averages: usize) -> Box<dyn SpectralAnalyzer> {
    Box::new(fft::FFTAnalyzer::new(window_size, averages))
}
