// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
//!
//! # Spectral Analysis Module
//!
//! This module provides tools for analyzing signals in the frequency domain,
//! particularly using Fast Fourier Transform (FFT) processing. It enables the
//! extraction of frequency components, amplitude measurements, and phase information
//! from time-domain signals.
//!
//! ## Features
//!
//! - Trait-based API for flexible spectral analysis implementations
//! - FFT-based analysis with configurable parameters
//! - Window functions to reduce spectral leakage
//! - Spectral averaging for improved signal-to-noise ratio
//! - Frequency-specific amplitude extraction
//!
//! ## Architecture
//!
//! The module uses a trait-based design pattern:
//!
//! - `SpectralAnalyzer` trait defines the interface for all analyzers
//! - `FFTAnalyzer` provides a concrete implementation using FFT
//! - Factory function `create_spectral_analyzer()` instantiates a suitable analyzer
//!
//! This design allows for easy extension with alternative spectral analysis methods
//! while maintaining a consistent API for application code.
//!
//! ## Usage
//!
//! ```
//! use rust_photoacoustic::spectral;
//!
//! // Create an analyzer with 2048-point FFT and 4x averaging
//! let mut analyzer = spectral::create_spectral_analyzer(2048, 4);
//!
//! // Generate a simple test signal (replace with your actual signal)
//! let sample_rate = 44100;
//! let signal = vec![0.0f32; 4096]; // Sample signal
//!
//! // Analyze the signal
//! let spectrum = analyzer.analyze(&signal, sample_rate).unwrap();
//!
//! // Extract information from the spectrum
//! println!("Number of frequency bins: {}", spectrum.frequencies.len());
//! println!("Frequency resolution: {:.2} Hz",
//!          spectrum.frequencies[1] - spectrum.frequencies[0]);
//! ```

// Make the fft module public for documentation examples
pub mod fft;

// Re-export key types and functions for public use at the top level
pub use fft::SpectralAnalyzer;

/// Create a new spectral analyzer with the given window size and averaging
///
/// This factory function creates and returns a new spectral analyzer that
/// implements the `SpectralAnalyzer` trait. It abstracts away the specific
/// implementation details, allowing the calling code to work with any
/// compatible analyzer.
///
/// # Parameters
///
/// * `window_size` - The size of the analysis window in samples. For FFT
///   analysis, this should ideally be a power of 2 (e.g., 1024, 2048, 4096)
///   for optimal performance.
///
/// * `averages` - The number of consecutive analysis frames to average.
///   Higher values improve the signal-to-noise ratio but increase latency
///   and computational cost. Set to 1 for no averaging.
///
/// # Returns
///
/// A boxed trait object implementing the `SpectralAnalyzer` trait
///
/// # Example
///
/// ```
/// use rust_photoacoustic::spectral;
///
/// // Create an analyzer with a 4096-point window and 5x averaging
/// let analyzer = spectral::create_spectral_analyzer(4096, 5);
///
/// // Now you can use this analyzer with any compatible signal
/// ```
pub fn create_spectral_analyzer(window_size: usize, averages: usize) -> Box<dyn SpectralAnalyzer> {
    Box::new(fft::FFTAnalyzer::new(window_size, averages))
}
