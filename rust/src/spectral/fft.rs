// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Fast Fourier Transform (FFT) implementation for spectral analysis
//!
//! This module provides functionality for analyzing signals in the frequency domain
//! using Fast Fourier Transform (FFT). It includes:
//!
//! - A trait-based approach for spectral analysis with the `SpectralAnalyzer` trait
//! - An FFT-based implementation `FFTAnalyzer` using the rustfft library
//! - Support for different window functions to reduce spectral leakage
//! - Capability for spectral averaging to improve signal-to-noise ratio
//! - Amplitude and phase extraction from frequency-domain signals
//!
//! # Example
//!
//! ```
//! use rust_photoacoustic::spectral::fft::{FFTAnalyzer, SpectralAnalyzer, WindowFunction};
//!
//! // Create a test signal (a sine wave at 1000 Hz)
//! let sample_rate = 44100;
//! let frequency = 1000.0;
//! let duration = 1.0; // seconds
//! let num_samples = (sample_rate as f32 * duration) as usize;
//!
//! let mut signal = vec![0.0; num_samples];
//! for i in 0..num_samples {
//!     let t = i as f32 / sample_rate as f32;
//!     signal[i] = (2.0 * std::f32::consts::PI * frequency * t).sin();
//! }
//!
//! // Create an FFT analyzer with a window size of 4096 and 3 averages
//! let mut analyzer = FFTAnalyzer::new(4096, 3);
//!
//! // Analyze the signal
//! let spectrum = analyzer.analyze(&signal, sample_rate).unwrap();
//!
//! // Get the amplitude at the expected frequency
//! let amplitude = analyzer.get_amplitude_at(frequency).unwrap();
//! println!("Amplitude at {} Hz: {}", frequency, amplitude);
//!
//! // Print the first few frequency bins
//! for i in 0..10 {
//!     println!("Frequency: {:.2} Hz, Amplitude: {:.6}",
//!              spectrum.frequencies[i],
//!              spectrum.amplitudes[i]);
//! }
//! ```
//!
//! # Spectral Analysis Process
//!
//! 1. Window the time-domain signal to reduce spectral leakage
//! 2. Compute the FFT of the windowed signal
//! 3. Average multiple FFTs if enabled (to reduce noise)
//! 4. Extract amplitude and phase information
//! 5. Return the results as a `SpectrumData` structure

use anyhow::Result;
use rustfft::{num_complex::Complex32, FftPlanner};

/// Trait for implementing spectral analysis algorithms
///
/// This trait defines the interface for all spectral analyzers in the system.
/// It allows for different implementations (FFT, wavelet, etc.) to be used
/// interchangeably in signal processing pipelines.
///
/// All implementing types must be thread-safe (`Send + Sync`) to allow
/// for parallel processing of multiple signals.
pub trait SpectralAnalyzer: Send + Sync {
    /// Analyze the given signal and extract spectral components
    ///
    /// This method transforms the time-domain signal into the frequency domain
    /// and extracts the amplitude and phase information for each frequency component.
    ///
    /// ### Parameters
    ///
    /// * `signal` - The time-domain signal as a slice of f32 samples
    /// * `sample_rate` - The sample rate of the signal in Hz
    ///
    /// ### Returns
    ///
    /// A `Result` containing the `SpectrumData` with frequency, amplitude, and phase
    /// information, or an error if the analysis failed.
    ///
    /// ### Errors
    ///
    /// Implementations may return errors if:
    /// - The signal is too short for the analysis window
    /// - The sample rate is invalid (e.g., zero)
    /// - Internal processing errors occur
    fn analyze(&mut self, signal: &[f32], sample_rate: u32) -> Result<SpectrumData>;

    /// Get the amplitude of the component at the specified frequency
    ///
    /// This method retrieves the amplitude of a specific frequency component
    /// from the most recently analyzed spectrum. It's useful for tracking
    /// specific frequencies of interest over time.
    ///
    /// ### Parameters
    ///
    /// * `frequency` - The frequency in Hz to get the amplitude for
    ///
    /// ### Returns
    ///
    /// A `Result` containing the amplitude at the specified frequency,
    /// or an error if the frequency is out of range or no spectrum has been analyzed.
    ///
    /// ### Errors
    ///
    /// This method will return an error if:
    /// - No prior analysis has been performed (`analyze()` must be called first)
    /// - The requested frequency is outside the analyzable range (beyond Nyquist)
    fn get_amplitude_at(&self, frequency: f32) -> Result<f32>;
}

/// Data resulting from spectral analysis
///
/// This structure contains the complete results of a spectral analysis operation,
/// including the frequencies analyzed, their corresponding amplitudes and phases,
/// and the sample rate of the original signal.
///
/// The vectors `frequencies`, `amplitudes`, and `phases` all have the same length,
/// with each index representing data for the same frequency component.
///
/// ### Example
///
/// ```
/// use rust_photoacoustic::spectral::fft::{FFTAnalyzer, SpectralAnalyzer, SpectrumData};
///
/// // Create a simple analyzer and analyze a signal
/// let mut analyzer = FFTAnalyzer::new(1024, 1);
/// let signal = vec![0.0; 1024]; // Just zeros for this example
/// let result = analyzer.analyze(&signal, 44100).unwrap();
///
/// // Working with spectrum data
/// for i in 0..10 {
///     println!("At {:.2} Hz: Amplitude = {:.6}, Phase = {:.6} rad",
///              result.frequencies[i],
///              result.amplitudes[i],
///              result.phases[i]);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SpectrumData {
    /// Vector of frequency values in Hz
    ///
    /// Each element corresponds to a frequency bin in the spectrum,
    /// starting from 0 Hz (DC) and increasing linearly up to the
    /// Nyquist frequency (sample_rate / 2).
    pub frequencies: Vec<f32>,

    /// Vector of amplitude values
    ///
    /// Each element represents the amplitude (magnitude) of the corresponding
    /// frequency component. For a real signal, these are normalized such that
    /// a pure sine wave with amplitude 1.0 will have a spectral peak of 1.0.
    pub amplitudes: Vec<f32>,

    /// Vector of phase values in radians
    ///
    /// Each element represents the phase angle (in radians) of the corresponding
    /// frequency component. The range is typically from -π to π.
    pub phases: Vec<f32>,

    /// Sample rate of the original signal in Hz
    ///
    /// This is preserved to allow conversion between bin indices and actual
    /// frequencies, and to determine the frequency resolution of the analysis.
    pub sample_rate: u32,
}

/// FFT-based spectral analyzer
///
/// This analyzer uses the Fast Fourier Transform (FFT) algorithm to convert
/// time-domain signals into the frequency domain for spectral analysis.
/// It supports various window functions to reduce spectral leakage and
/// can perform averaging across multiple FFT frames to improve the
/// signal-to-noise ratio.
///
/// ### Features
///
/// - Configurable FFT window size for different frequency resolutions
/// - Support for multiple window functions (Rectangular, Hann, Blackman)
/// - Spectral averaging to reduce noise
/// - Caching of analysis results for frequency-specific queries
///
/// ### Performance Considerations
///
/// - Larger window sizes provide better frequency resolution but worse time resolution
/// - The window size should be a power of 2 for optimal FFT performance
/// - More averages improve SNR but increase the required computation and latency
///
/// ### Example
///
/// ```
/// use rust_photoacoustic::spectral::fft::{FFTAnalyzer, SpectralAnalyzer, WindowFunction};
///
/// // Create an analyzer with a 2048-point FFT and 4x averaging
/// let mut analyzer = FFTAnalyzer::new(2048, 4);
///
/// // Process a signal (assuming signal and sample_rate are defined)
/// let signal = vec![0.0f32; 2048];
/// let sample_rate = 44100;
/// let spectrum = analyzer.analyze(&signal, sample_rate).unwrap();
///
/// // Check the amplitude at a specific frequency
/// let freq_of_interest = 1000.0; // Hz
/// let amplitude = analyzer.get_amplitude_at(freq_of_interest).unwrap();
/// println!("Amplitude at {} Hz: {}", freq_of_interest, amplitude);
/// ```
pub struct FFTAnalyzer {
    /// Size of the FFT window in samples
    ///
    /// For best performance, this should be a power of 2.
    /// Larger windows provide better frequency resolution but worse time resolution.
    frame_size: usize,

    /// Number of FFT frames to average
    ///
    /// Higher values improve signal-to-noise ratio but increase computation time.
    /// A value of 1 means no averaging is performed.
    averages: usize,

    /// Window function to apply before FFT computation
    ///
    /// Window functions reduce spectral leakage by tapering the signal
    /// at the edges of each analysis frame.
    window_function: WindowFunction,

    /// Cache of the most recent spectrum analysis result
    ///
    /// This allows the `get_amplitude_at` method to work without
    /// recomputing the entire spectrum.
    spectrum_data: Option<SpectrumData>,

    /// Storage for previous FFT outputs for averaging
    ///
    /// This vector stores the complex FFT outputs from previous frames
    /// to enable spectral averaging.
    previous_spectra: Vec<Vec<Complex32>>,
}

impl FFTAnalyzer {
    /// Create a new FFT analyzer with the given window size and averaging
    ///
    /// This constructor creates an FFT analyzer with the specified parameters
    /// and defaults to using a Hann window function, which provides a good
    /// balance between spectral resolution and leakage reduction.
    ///
    /// ### Parameters
    ///
    /// * `frame_size` - The size of the FFT window in samples. For best performance,
    ///   this should be a power of 2 (e.g., 1024, 2048, 4096).
    /// * `averages` - The number of FFT frames to average. Higher values reduce noise
    ///   but increase processing latency. Set to 1 for no averaging.
    ///
    /// ### Returns
    ///
    /// A new `FFTAnalyzer` instance configured with the specified parameters
    /// and ready to analyze signals.
    ///
    /// ### Example
    ///
    /// ```
    /// use rust_photoacoustic::spectral::fft::FFTAnalyzer;
    ///
    /// // Create an FFT analyzer with a 4096-point FFT and 3x averaging
    /// let analyzer = FFTAnalyzer::new(4096, 3);
    /// ```
    pub fn new(frame_size: usize, averages: usize) -> Self {
        Self {
            frame_size,
            averages,
            window_function: WindowFunction::Hann, // Default to Hann window
            spectrum_data: None,
            previous_spectra: Vec::with_capacity(averages),
        }
    }

    /// Apply window function to the input signal
    ///
    /// This method applies a window function to the input signal to reduce
    /// spectral leakage. Windowing tapers the signal at the edges of the
    /// analysis frame, which helps to reduce artificial frequencies that
    /// would otherwise be introduced by the abrupt truncation of the signal.
    ///
    /// ### Parameters
    ///
    /// * `signal` - The input signal to apply the window function to
    ///
    /// ### Returns
    ///
    /// A new vector containing the windowed signal
    ///
    /// ### Window Functions
    ///
    /// - **Rectangular**: No windowing (flat window), maximum frequency resolution
    ///   but highest spectral leakage
    /// - **Hann**: Cosine-based window with good balance between resolution and leakage
    /// - **Blackman**: More aggressive tapering, better leakage suppression but
    ///   worse frequency resolution
    ///
    /// ### Example
    ///
    /// ```
    /// use rust_photoacoustic::spectral::fft::FFTAnalyzer;
    /// let analyzer = FFTAnalyzer::new(1024, 1);
    /// let signal = vec![1.0f32; 1024];
    /// let windowed_signal = analyzer.apply_window(&signal);
    /// // The windowed signal will have tapered edges
    /// assert!(windowed_signal[0] < signal[0]);
    /// assert!(windowed_signal[signal.len() - 1] < signal[signal.len() - 1]);
    /// ```
    pub fn apply_window(&self, signal: &[f32]) -> Vec<f32> {
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
    ///
    /// This method applies the Fast Fourier Transform algorithm to convert
    /// a time-domain signal into the frequency domain. It uses the rustfft
    /// library for efficient FFT computation.
    ///
    /// ### Parameters
    ///
    /// * `signal` - The time-domain signal to transform
    ///
    /// ### Returns
    ///
    /// A vector of complex numbers representing the frequency-domain signal
    ///
    /// ### Implementation Details
    ///
    /// The method:
    /// 1. Converts the real input signal to complex numbers with zero imaginary part
    /// 2. Creates an FFT plan using the rustfft library
    /// 3. Executes the FFT in-place on the complex array
    /// 4. Returns the transformed data for further processing
    ///
    /// The output contains both positive and negative frequency components,
    /// with the DC (0 Hz) component at index 0 and the Nyquist frequency
    /// component at index N/2 (where N is the signal length).
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
    ///
    /// This method transforms the raw FFT output (complex numbers) into a more
    /// useful format containing frequencies, amplitudes, and phases. It handles
    /// normalization, extracts only the meaningful part of the spectrum (up to
    /// the Nyquist frequency), and organizes the data into a structured format.
    ///
    /// ### Parameters
    ///
    /// * `fft_output` - The complex FFT output from `compute_fft()`
    /// * `sample_rate` - The sample rate of the original signal in Hz
    ///
    /// ### Returns
    ///
    /// A `SpectrumData` structure containing:
    /// - Frequencies in Hz
    /// - Normalized amplitudes
    /// - Phases in radians
    ///
    /// ### Implementation Details
    ///
    /// - Only the first half of the FFT result is used (up to the Nyquist frequency)
    /// - Amplitudes are normalized by dividing by the FFT size and multiplying by 2
    ///   (the factor of 2 accounts for the energy in the negative frequencies)
    /// - The frequency resolution is determined by the sample rate and FFT size
    /// - Phase information is preserved from the complex FFT output
    fn fft_to_spectrum(&self, fft_output: &[Complex32], sample_rate: u32) -> SpectrumData {
        let n = fft_output.len();
        let _nyquist = sample_rate as f32 / 2.0; // Unused but kept for clarity
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
    /// Analyze the given signal and extract spectral components
    ///
    /// This implementation follows these steps to analyze a signal:
    /// 1. Validates that the signal is long enough for the configured window size
    /// 2. Applies the selected window function to reduce spectral leakage
    /// 3. Computes the FFT of the windowed signal
    /// 4. Stores and averages multiple FFT frames if averaging is enabled
    /// 5. Converts the complex FFT result to amplitude and phase information
    /// 6. Returns the formatted spectrum data
    ///
    /// The analysis maintains a history of previous FFT frames for averaging,
    /// which helps reduce noise and improve the reliability of the spectral
    /// estimates, particularly useful for signals with low SNR.
    ///
    /// ### Parameters
    ///
    /// * `signal` - The time-domain signal as a slice of f32 samples
    /// * `sample_rate` - The sample rate of the signal in Hz
    ///
    /// ### Returns
    ///
    /// A `Result` containing the `SpectrumData` with frequency, amplitude, and phase
    /// information, or an error if the analysis failed.
    ///
    /// ### Errors
    ///
    /// Returns an error if the signal is shorter than the configured window size.
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::spectral::fft::{FFTAnalyzer, SpectralAnalyzer};
    /// let mut analyzer = FFTAnalyzer::new(1024, 1);
    /// let sample_rate = 44100;
    /// let signal = vec![0.0f32; 1024];
    /// let spectrum = analyzer.analyze(&signal, sample_rate).unwrap();
    /// println!("Number of frequency bins: {}", spectrum.frequencies.len());
    /// ```
    fn analyze(&mut self, signal: &[f32], sample_rate: u32) -> Result<SpectrumData> {
        // Check if signal is long enough
        if signal.len() < self.frame_size {
            return Err(anyhow::anyhow!(
                "Signal too short: {} samples (need {})",
                signal.len(),
                self.frame_size
            ));
        }

        // Apply window function
        let windowed = self.apply_window(&signal[0..self.frame_size]);

        // Compute FFT
        let fft_result = self.compute_fft(&windowed);

        // Add to previous spectra for averaging
        self.previous_spectra.push(fft_result.clone());
        if self.previous_spectra.len() > self.averages {
            self.previous_spectra.remove(0);
        }

        // Average spectra
        let mut avg_spectrum = vec![Complex32::new(0.0, 0.0); self.frame_size];

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

    /// Get the amplitude of the component at the specified frequency
    ///
    /// This method retrieves the amplitude at a specific frequency from the
    /// most recent spectrum analysis. It identifies the closest frequency bin
    /// to the requested frequency and returns the corresponding amplitude value.
    ///
    /// ### Parameters
    ///
    /// * `frequency` - The frequency in Hz to get the amplitude for
    ///
    /// ### Returns
    ///
    /// A `Result` containing the amplitude at the specified frequency,
    /// or an error if the frequency is out of range or no spectrum has been analyzed.
    ///
    /// ### Errors
    ///
    /// Returns an error if:
    /// - No spectrum analysis has been performed yet (call `analyze()` first)
    /// - The requested frequency is outside the analyzed range (beyond Nyquist frequency)
    ///
    /// ### Example
    ///
    /// ```
    /// use rust_photoacoustic::spectral::fft::{FFTAnalyzer, SpectralAnalyzer};
    /// let mut analyzer = FFTAnalyzer::new(1024, 1);
    /// let sample_rate = 44100;
    /// let signal = vec![0.0f32; 1024];
    /// let _ = analyzer.analyze(&signal, sample_rate);
    /// // Get amplitude at 1000 Hz
    /// match analyzer.get_amplitude_at(1000.0) {
    ///     Ok(amplitude) => println!("Amplitude at 1000 Hz: {}", amplitude),
    ///     Err(e) => eprintln!("Error: {}", e),
    /// }
    /// ```
    fn get_amplitude_at(&self, frequency: f32) -> Result<f32> {
        let spectrum = self
            .spectrum_data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No spectrum data available. Call analyze() first."))?;

        // Find the closest frequency bin
        let df = spectrum.sample_rate as f32 / (self.frame_size as f32);
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
///
/// Window functions are applied to time-domain signals before FFT analysis
/// to reduce spectral leakage. Each window function has different characteristics
/// in terms of frequency resolution, amplitude accuracy, and leakage suppression.
///
/// ### Window Function Characteristics
///
/// - **Rectangular**: No windowing, provides best frequency resolution but worst
///   spectral leakage. Useful when the signal contains exactly an integer number
///   of cycles within the analysis window.
///
/// - **Hann**: A cosine-based window that provides a good balance between frequency
///   resolution and leakage suppression. It has good frequency resolution and
///   moderate amplitude accuracy. This is often a good default choice.
///
/// - **Blackman**: Provides excellent leakage suppression but reduced frequency
///   resolution compared to other windows. Useful when analyzing signals with
///   components that have very different amplitudes.
///
/// ### Example
///
/// ```
/// use rust_photoacoustic::spectral::fft::{FFTAnalyzer, WindowFunction};
///
/// // Create an analyzer with a specific window function
/// let mut analyzer = FFTAnalyzer::new(2048, 1);
/// // You can access the window functions directly from the enum
/// println!("Available window functions: Rectangular, Hann, Blackman");
/// ```
#[derive(Debug, Clone, Copy)]
pub enum WindowFunction {
    /// Rectangular window (no windowing)
    Rectangular,
    /// Hann window (cosine-based)
    Hann,
    /// Blackman window (enhanced leakage suppression)
    Blackman,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_sine(amplitude: f32, freq: f32, sample_rate: u32, num_samples: usize) -> Vec<f32> {
        let mut signal = vec![0.0f32; num_samples];
        for n in 0..num_samples {
            let t = n as f32 / sample_rate as f32;
            signal[n] = amplitude * (2.0 * std::f32::consts::PI * freq * t).sin();
        }
        signal
    }

    #[test]
    fn test_apply_window_hann_tapers_edges() {
        let analyzer = FFTAnalyzer::new(1024, 1);
        let signal = vec![1.0f32; 1024];
        let windowed = analyzer.apply_window(&signal);
        assert!(windowed[0].abs() < 1.0);
        assert!(windowed[windowed.len() - 1].abs() < 1.0);
        // Middle should stay close to 1.0 but slightly less than original for Hann
        assert!(windowed[windowed.len() / 2] > 0.7);
    }

    #[test]
    fn test_fft_of_sine_has_expected_amplitude() {
        let mut analyzer = FFTAnalyzer::new(1024, 1);
        // Use a rectangular window for an unmodified amplitude
        analyzer.window_function = WindowFunction::Rectangular;
        let sample_rate = 1024;
        let freq = 10.0; // integer bin when sample_rate == frame_size
        let signal = create_sine(1.0, freq, sample_rate, analyzer.frame_size);

        let spectrum = analyzer.analyze(&signal, sample_rate).unwrap();
        // Amplitude near 1.0 at the expected frequency
        let amp = analyzer.get_amplitude_at(freq).unwrap();
        assert!((amp - 1.0).abs() < 1e-2, "amplitude mismatch: {}", amp);
        // Ensure the peak is significantly larger than nearby bins
        let df = sample_rate as f32 / analyzer.frame_size as f32;
        let bin_idx = (freq / df).round() as usize;
        let left = if bin_idx > 0 {
            spectrum.amplitudes[bin_idx - 1]
        } else {
            0.0
        };
        let right = if bin_idx + 1 < spectrum.amplitudes.len() {
            spectrum.amplitudes[bin_idx + 1]
        } else {
            0.0
        };
        assert!(spectrum.amplitudes[bin_idx] > left * 5.0);
        assert!(spectrum.amplitudes[bin_idx] > right * 5.0);
    }

    #[test]
    fn test_analyze_too_short_signal_errors() {
        let mut analyzer = FFTAnalyzer::new(512, 1);
        let signal = vec![0.0f32; 128];
        let res = analyzer.analyze(&signal, 44100);
        assert!(res.is_err());
    }

    #[test]
    fn test_get_amplitude_before_analyze_errors() {
        let analyzer = FFTAnalyzer::new(512, 1);
        let res = analyzer.get_amplitude_at(1000.0);
        assert!(res.is_err());
    }

    #[test]
    fn test_averaging_keeps_amplitudes_stable() {
        let mut analyzer = FFTAnalyzer::new(1024, 4);
        // Use a rectangular window so amplitude remains consistent
        analyzer.window_function = WindowFunction::Rectangular;
        let sample_rate = 1024;
        let freq = 20.0;
        // feed the analyzer 4 identical frames, amplitude should remain near 1.0
        for _ in 0..4 {
            let signal = create_sine(1.0, freq, sample_rate, analyzer.frame_size);
            let _ = analyzer.analyze(&signal, sample_rate).unwrap();
        }
        let amp = analyzer.get_amplitude_at(freq).unwrap();
        assert!((amp - 1.0).abs() < 2e-2);
    }
}
