// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Mock audio source module
//!
//! This module provides a mock audio source that generates synthetic photoacoustic signals
//! using the NoiseGenerator for testing and simulation purposes.

use super::AudioSource;
use crate::config::PhotoacousticConfig;
use crate::utility::noise_generator::NoiseGenerator;
use anyhow::Result;
use log::debug;
use std::time::{Duration, Instant};

/// Mock audio source that generates synthetic photoacoustic signals with controlled correlation
pub struct MockSource {
    generator: NoiseGenerator,
    sample_rate: u32,
    frame_size: usize,
    config: PhotoacousticConfig,
    // Mock signal parameters
    noise_amplitude: f32,
    pulse_width: f32,
    min_pulse_amplitude: f32,
    max_pulse_amplitude: f32,
    correlation: f32,
    // Timing control for real-time simulation
    last_frame_time: Option<Instant>,
    frame_duration: Duration,
    real_time_mode: bool,
}

impl MockSource {
    /// Create a new MockSource using the provided PhotoacousticConfig
    ///
    /// # Arguments
    ///
    /// * `config` - PhotoacousticConfig containing frequency, sample_rate, and precision settings
    ///
    /// # Returns
    ///
    /// A new MockSource instance configured for synthetic photoacoustic signal generation
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::acquisition::MockSource;
    /// use rust_photoacoustic::config::PhotoacousticConfig;
    ///
    /// let config = PhotoacousticConfig::default();
    /// let mock_source = MockSource::new(config)?;
    /// ```
    pub fn new(config: PhotoacousticConfig) -> Result<Self> {
        let generator = NoiseGenerator::new_from_system_time();
        let sample_rate = config.sample_rate as u32;
        let frame_size = config.window_size as usize;

        let correlation = config.mock_correlation.clamp(0.0, 1.0); // Ensure correlation is within valid range

        // Calculate frame duration for real-time simulation
        let frame_duration = Duration::from_secs_f64(frame_size as f64 / sample_rate as f64);

        debug!("Creating MockSource with config:");
        debug!("  Sample rate: {} Hz", sample_rate);
        debug!("  Frequency: {} Hz", config.frequency);
        debug!("  Precision: {} bits", config.precision);
        debug!("  Frame size: {} samples per channel", frame_size);
        debug!(
            "  Frame duration: {:.1}ms",
            frame_duration.as_secs_f64() * 1000.0
        );
        debug!("  Expected FPS: {:.1}", 1.0 / frame_duration.as_secs_f64());
        debug!("  Correlation: {}", correlation);

        Ok(Self {
            generator,
            sample_rate,
            frame_size,
            config,
            // Default mock signal parameters - can be made configurable later
            noise_amplitude: 0.3,     // 30% noise level
            pulse_width: 0.04,        // 40ms pulse width
            min_pulse_amplitude: 0.8, // Minimum 80% pulse amplitude
            max_pulse_amplitude: 1.0, // Maximum 100% pulse amplitude
            correlation,
            last_frame_time: None,
            frame_duration,
            real_time_mode: true, // Enable real-time simulation by default
        })
    }

    /// Create a new MockSource with custom signal parameters
    ///
    /// # Arguments
    ///
    /// * `config` - PhotoacousticConfig containing frequency, sample_rate, and precision settings
    /// * `correlation` - Correlation coefficient between channels [-1.0, 1.0]
    /// * `noise_amplitude` - Background noise amplitude [0.0, 1.0]
    /// * `pulse_width` - Width of each pulse in seconds
    /// * `min_pulse_amplitude` - Minimum pulse amplitude [0.0, 1.0]
    /// * `max_pulse_amplitude` - Maximum pulse amplitude [0.0, 1.0]
    pub fn with_signal_params(
        config: PhotoacousticConfig,
        correlation: f32,
        noise_amplitude: f32,
        pulse_width: f32,
        min_pulse_amplitude: f32,
        max_pulse_amplitude: f32,
    ) -> Result<Self> {
        let mut config = config.clone();
        config.mock_correlation = correlation.clamp(0.0, 1.0); // Ensure correlation is within valid range
        let mut mock_source = Self::new(config)?;
        mock_source.noise_amplitude = noise_amplitude;
        mock_source.pulse_width = pulse_width;
        mock_source.min_pulse_amplitude = min_pulse_amplitude;
        mock_source.max_pulse_amplitude = max_pulse_amplitude;
        Ok(mock_source)
    }

    /// Update the correlation coefficient between channels
    pub fn set_correlation(&mut self, correlation: f32) {
        self.correlation = correlation.clamp(-1.0, 1.0);
    }

    /// Update the noise amplitude
    pub fn set_noise_amplitude(&mut self, amplitude: f32) {
        self.noise_amplitude = amplitude.clamp(0.0, 1.0);
    }

    /// Update the pulse width
    pub fn set_pulse_width(&mut self, width: f32) {
        self.pulse_width = width.max(0.0);
    }

    /// Enable or disable real-time simulation
    pub fn set_real_time_mode(&mut self, enabled: bool) {
        self.real_time_mode = enabled;
        if !enabled {
            self.last_frame_time = None;
        }
    }
}

impl AudioSource for MockSource {
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)> {
        // Real-time timing simulation
        if self.real_time_mode {
            let now = Instant::now();

            if let Some(last_time) = self.last_frame_time {
                let elapsed = now.duration_since(last_time);
                if elapsed < self.frame_duration {
                    let sleep_duration = self.frame_duration - elapsed;
                    // debug!(
                    //     "Mock timing: sleeping for {:.1}ms to maintain real-time generation",
                    //     sleep_duration.as_secs_f64() * 1000.0
                    // );
                    std::thread::sleep(sleep_duration);
                }
            }

            self.last_frame_time = Some(Instant::now());
        }

        // Generate correlated stereo mock photoacoustic signal
        let samples = self.generator.generate_mock_photoacoustic_correlated(
            self.frame_size as u32,
            self.sample_rate,
            self.noise_amplitude,
            self.config.frequency,
            self.pulse_width,
            self.min_pulse_amplitude,
            self.max_pulse_amplitude,
            self.correlation,
        );

        // Convert interleaved i16 samples to separate f32 channels
        let mut channel_a = Vec::with_capacity(self.frame_size);
        let mut channel_b = Vec::with_capacity(self.frame_size);

        for chunk in samples.chunks_exact(2) {
            let left = chunk[0] as f32 / i16::MAX as f32;
            let right = chunk[1] as f32 / i16::MAX as f32;
            channel_a.push(left);
            channel_b.push(right);
        }

        Ok((channel_a, channel_b))
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PhotoacousticConfig;

    #[test]
    fn test_mock_source_creation() {
        let mut config = PhotoacousticConfig::default();
        config.window_size = 1024;
        config.mock_correlation = 0.7;
        let mock_source = MockSource::new(config);
        assert!(mock_source.is_ok());
    }

    #[test]
    fn test_mock_source_read_frame() {
        let mut config = PhotoacousticConfig::default();
        config.window_size = 512;
        config.mock_correlation = 0.5;
        let mut mock_source = MockSource::new(config).unwrap();

        let result = mock_source.read_frame();
        assert!(result.is_ok());

        let (channel_a, channel_b) = result.unwrap();
        assert_eq!(channel_a.len(), 512);
        assert_eq!(channel_b.len(), 512);

        // Check that samples are in valid range [-1.0, 1.0]
        for sample in &channel_a {
            assert!(sample >= &-1.0 && sample <= &1.0);
        }
        for sample in &channel_b {
            assert!(sample >= &-1.0 && sample <= &1.0);
        }
    }

    #[test]
    fn test_mock_source_sample_rate() {
        let mut config = PhotoacousticConfig::default();
        config.window_size = 1024;
        config.mock_correlation = 0.7;
        let mock_source = MockSource::new(config.clone()).unwrap();
        assert_eq!(mock_source.sample_rate(), config.sample_rate as u32);
    }

    #[test]
    fn test_mock_source_parameter_updates() {
        let mut config = PhotoacousticConfig::default();
        config.window_size = 1024;
        config.mock_correlation = 0.7;
        let mut mock_source = MockSource::new(config).unwrap();

        mock_source.set_correlation(0.9);
        assert_eq!(mock_source.correlation, 0.9);

        mock_source.set_noise_amplitude(0.5);
        assert_eq!(mock_source.noise_amplitude, 0.5);

        mock_source.set_pulse_width(0.02);
        assert_eq!(mock_source.pulse_width, 0.02);
    }

    #[test]
    fn test_mock_source_parameter_clamping() {
        let mut config = PhotoacousticConfig::default();
        config.window_size = 1024;
        config.mock_correlation = 0.7;
        let mut mock_source = MockSource::new(config).unwrap();

        // Test correlation clamping
        mock_source.set_correlation(2.0);
        assert_eq!(mock_source.correlation, 1.0);

        mock_source.set_correlation(-2.0);
        assert_eq!(mock_source.correlation, -1.0);

        // Test noise amplitude clamping
        mock_source.set_noise_amplitude(2.0);
        assert_eq!(mock_source.noise_amplitude, 1.0);

        mock_source.set_noise_amplitude(-0.5);
        assert_eq!(mock_source.noise_amplitude, 0.0);
    }
}
