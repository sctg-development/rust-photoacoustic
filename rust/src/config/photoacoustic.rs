// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Photoacoustic measurement configuration
//!
//! This module defines the structures for configuring the photoacoustic
//! measurement process in the application.

use serde::{Deserialize, Serialize};

/// Configuration for the photoacoustic measurement system.
///
/// This structure contains settings that control the photoacoustic measurement process,
/// including input sources, signal processing parameters, and analysis settings.
///
/// # Input Sources
///
/// The configuration supports two mutually exclusive input sources:
/// * `input_device` - A hardware audio device (e.g., "hw:0,0" for ALSA) "first" for the first available device
/// * `input_file` - A path to a WAV file for offline analysis
///
/// One of these must be specified, but not both simultaneously.
///
/// # Signal Processing Parameters
///
/// * `frequency` - The primary excitation frequency in Hz
/// * `bandwidth` - Filter bandwidth in Hz around the excitation frequency
/// * `frame_size` - FFT window size (power of 2 recommended)
/// * `averages` - Number of spectra to average for noise reduction
///
/// # Example
///
/// ```
/// use rust_photoacoustic::config::PhotoacousticConfig;
///
/// let pa_config = PhotoacousticConfig {
///     input_device: Some("first".to_string()),
///     input_file: None,
///     frequency: 1000.0,
///     sample_rate: 48000,
///     bandwidth: 50.0,
///     frame_size: 4096,
///     averages: 10,
///     precision: 16,
///     mock_source: false,
///     mock_correlation: 0.7,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoacousticConfig {
    /// The input device to use for data acquisition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_device: Option<String>,

    /// The input file to use for data acquisition mutually exclusive with input_device
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file: Option<String>,

    /// Enable mock data source for testing and simulation
    #[serde(default)]
    pub mock_source: bool,

    /// Correlation coefficient for mock source channels (0.0 to 1.0)
    #[serde(default = "default_mock_correlation")]
    pub mock_correlation: f32,

    /// The excitation frequency in Hz
    pub frequency: f32,

    /// Filter bandwidth in Hz
    pub bandwidth: f32,

    /// Window size for FFT analysis and frame sharing
    pub frame_size: u16,

    /// Number of spectra to average
    pub averages: u16,

    /// Sample rate of the input data (default is 48000 Hz)
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u16,

    /// Sampling precision in bits (16 bits for standard PCM)
    #[serde(default = "default_precision")]
    pub precision: u8,
}

fn default_sample_rate() -> u16 {
    44100 // Default sample rate in Hz
}

fn default_precision() -> u8 {
    16 // Default precision in bits
}

fn default_mock_correlation() -> f32 {
    0.7 // Default correlation coefficient for mock data
}

impl Default for PhotoacousticConfig {
    fn default() -> Self {
        Self {
            input_device: Some("first".to_string()), // Default to the first CPAL device
            input_file: None,                        // No file by default
            mock_source: false,                      // Mock disabled by default
            mock_correlation: default_mock_correlation(), // Default mock correlation
            frequency: 1000.0,                       // 1kHz default frequency
            bandwidth: 50.0,                         // 50Hz bandwidth
            frame_size: 4096,                        // 4K FFT window
            sample_rate: default_sample_rate(),      // Default sample rate
            averages: 10,                            // Average 10 spectra
            precision: 16,
        }
    }
}
