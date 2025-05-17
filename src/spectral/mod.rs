// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
//!
//! Spectral analysis module
//!
//! This module handles spectral analysis of the signal,
//! particularly Fast Fourier Transform (FFT) processing.

mod fft;

pub use fft::SpectralAnalyzer;

/// Create a new spectral analyzer with the given window size and averaging
pub fn create_spectral_analyzer(window_size: usize, averages: usize) -> Box<dyn SpectralAnalyzer> {
    Box::new(fft::FFTAnalyzer::new(window_size, averages))
}
