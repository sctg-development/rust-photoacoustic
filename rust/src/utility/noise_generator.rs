// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Photoacoustic Signal Generator
//!
//! This module provides implementations for generating:
//!
//! 1. Gaussian white noise - for basic testing and calibration
//! 2. Mock photoacoustic signals - simulated signals with periodic pulses overlaid on white noise
//!
//! These signal generators are commonly used in photoacoustic signal processing for:
//!
//! - Testing and calibration of signal processing algorithms
//! - Simulating background noise in photoacoustic signals
//! - Generating synthetic photoacoustic responses with known parameters
//! - Evaluating filter performance and signal-to-noise ratio
//! - Creating test signals with controlled noise characteristics
//!
//! ## Features
//!
//! * Fast XORShift pseudo-random number generation
//! * Box-Muller transform for Gaussian distribution
//! * Support for mono and stereo noise generation
//! * Configurable amplitude scaling
//! * Correlated stereo noise generation with adjustable correlation coefficient
//! * Mock photoacoustic signal generation with:
//!   * Configurable pulse frequency
//!   * Adjustable pulse width
//!   * Random pulse amplitude within a specified range
//!   * Background white noise with controllable amplitude
//!   * Support for mono, stereo, and correlated stereo signals
//!
//! ## White Noise Examples
//!
//! ```rust
//! use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
//!
//! // Create a noise generator with system time as seed
//! let mut generator = NoiseGenerator::new_from_system_time();
//!
//! // Generate 1 second of mono noise at 48kHz with 50% amplitude
//! let mono_samples = generator.generate_mono(48000, 0.5);
//!
//! // Generate stereo noise with correlation coefficient of 0.7
//! let stereo_correlated = generator.generate_correlated_stereo(48000, 0.5, 0.7);
//! ```
//!
//! ## Mock Photoacoustic Signal Examples
//!
//! ```rust
//! use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
//!
//! // Create a noise generator with system time as seed
//! let mut generator = NoiseGenerator::new_from_system_time();
//!
//! // Generate 1 second of mono mock photoacoustic signal at 48kHz
//! // with 30% noise amplitude, 2kHz pulse frequency, 40ms pulse width,
//! // and pulse amplitude between 80% and 100%
//! let mono_mock = generator.generate_mock_photoacoustic_mono(
//!     48000,    // num_samples (1 second at 48kHz)
//!     48000,    // sample_rate
//!     0.3,      // noise_amplitude
//!     2000.0,   // pulse_frequency (Hz)
//!     0.04,     // pulse_width (seconds)
//!     0.8,      // min_pulse_amplitude
//!     1.0       // max_pulse_amplitude
//! );
//!
//! // Generate correlated stereo mock photoacoustic signal
//! let correlated_mock = generator.generate_mock_photoacoustic_correlated(
//!     48000,    // num_samples
//!     48000,    // sample_rate
//!     0.3,      // noise_amplitude
//!     2000.0,   // pulse_frequency
//!     0.04,     // pulse_width
//!     0.8,      // min_pulse_amplitude
//!     1.0,      // max_pulse_amplitude
//!     0.7       // correlation coefficient
//! );
//! ```

use std::time::SystemTime;

/// Random number generator using XORShift algorithm for generating noise samples.
///
/// This struct implements a fast and lightweight pseudo-random number generator
/// based on the XORShift algorithm. It's suitable for generating noise samples
/// but should not be used for cryptographic purposes.
///
/// The generator maintains an internal state that evolves with each random
/// number generated, producing a sequence of pseudo-random values.
///
/// # Examples
///
/// ```
/// use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
///
/// // Create a generator with a specific seed
/// let mut generator = NoiseGenerator::new(12345);
///
/// // Generate a random float between -1.0 and 1.0
/// let random_value = generator.random_float();
///
/// // Generate a random value from a Gaussian distribution
/// let gaussian_value = generator.random_gaussian();
/// ```
pub struct NoiseGenerator {
    /// Internal state of the XORShift random number generator.
    /// This value evolves with each random number generation.
    rng_state: u32,
}

impl NoiseGenerator {
    /// Creates a new noise generator with a given seed.
    ///
    /// The seed determines the initial state of the random number generator,
    /// and thus the entire sequence of random numbers that will be generated.
    /// Using the same seed will produce the same sequence of random numbers.
    ///
    /// # Arguments
    ///
    /// * `seed` - A 32-bit unsigned integer used to initialize the generator state
    ///
    /// # Returns
    ///
    /// A new `NoiseGenerator` instance initialized with the specified seed
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
    ///
    /// // Create a generator with seed 12345
    /// let generator = NoiseGenerator::new(12345);
    /// ```
    pub fn new(seed: u32) -> Self {
        Self { rng_state: seed }
    }

    /// Creates a new noise generator with a seed derived from the system time.
    ///
    /// This constructor uses the current system time in milliseconds since the Unix epoch
    /// as the seed value. This provides a different seed each time the generator is created,
    /// which is useful for applications that need different noise patterns on each run.
    ///
    /// # Returns
    ///
    /// A new `NoiseGenerator` instance initialized with a time-based seed
    ///
    /// # Panics
    ///
    /// Panics if the system time is before the Unix epoch (extremely unlikely)
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
    ///
    /// // Create a generator with a seed from the current time
    /// let generator = NoiseGenerator::new_from_system_time();
    /// ```
    pub fn new_from_system_time() -> Self {
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u32;
        Self::new(seed)
    }

    /// Generates a random floating-point number between -1.0 and 1.0.
    ///
    /// This method uses the XORShift algorithm to update the internal state
    /// and produce a pseudo-random number. The resulting 32-bit value is
    /// normalized to the range [-1.0, 1.0].
    ///
    /// # Returns
    ///
    /// A random f32 value in the range [-1.0, 1.0]
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
    ///
    /// let mut generator = NoiseGenerator::new(12345);
    /// let random_value = generator.random_float();
    /// assert!(random_value >= -1.0 && random_value <= 1.0);
    /// ```
    pub fn random_float(&mut self) -> f32 {
        // XOR Shift algorithm for pseudo-random numbers
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;

        // Convert to float between -1.0 and 1.0
        (self.rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Generates a random value from a standard Gaussian (normal) distribution.
    ///
    /// This method uses the Box-Muller transform to convert uniformly distributed
    /// random numbers into normally distributed random numbers. The resulting
    /// distribution has a mean of 0 and a standard deviation of 1.
    ///
    /// # Returns
    ///
    /// A random f32 value from a standard Gaussian distribution
    ///
    /// # Mathematical Background
    ///
    /// The Box-Muller transform converts uniform random variables to normally
    /// distributed random variables using the formula:
    /// ```text
    /// z = sqrt(-2 * ln(u1)) * cos(2 * π * u2)
    /// ```
    /// where u1 and u2 are uniform random variables in the range (0,1).
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
    ///
    /// let mut generator = NoiseGenerator::new(12345);
    /// let gaussian_value = generator.random_gaussian();
    /// ```
    pub fn random_gaussian(&mut self) -> f32 {
        // Use Box-Muller transform to generate gaussian distributed values
        let u1 = (self.random_float() + 1.0) / 2.0; // remap to (0,1)
        let u2 = (self.random_float() + 1.0) / 2.0;

        // Avoid ln(0)
        let u1 = if u1 < 0.0001 { 0.0001 } else { u1 };

        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
    }

    /// Generates a buffer of mono (single channel) Gaussian white noise.
    ///
    /// This method creates a vector of 16-bit integer samples representing
    /// Gaussian white noise with the specified amplitude. The samples are
    /// suitable for use in audio applications or signal processing.
    ///
    /// # Arguments
    ///
    /// * `num_samples` - The number of samples to generate
    /// * `amplitude` - The amplitude scaling factor in the range [0.0, 1.0]
    ///
    /// # Returns
    ///
    /// A vector of i16 samples containing the generated noise
    ///
    /// # Sample Values
    ///
    /// The output samples are scaled to utilize the full i16 range [-32768, 32767],
    /// with the amplitude parameter controlling the overall level. An amplitude of 1.0
    /// will generate noise that uses the full available range.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
    ///
    /// let mut generator = NoiseGenerator::new(12345);
    ///
    /// // Generate 1 second of mono noise at 48kHz with 50% amplitude
    /// let samples = generator.generate_mono(48000, 0.5);
    /// assert_eq!(samples.len(), 48000);
    /// ```
    pub fn generate_mono(&mut self, num_samples: u32, amplitude: f32) -> Vec<i16> {
        let mut samples = Vec::with_capacity(num_samples as usize);

        for _ in 0..num_samples {
            let sample = self.random_gaussian() * amplitude;
            let value = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            samples.push(value);
        }

        samples
    }

    /// Generates a buffer of stereo (two channel) Gaussian white noise with independent channels.
    ///
    /// This method creates a vector of 16-bit integer samples representing
    /// two channels of independent Gaussian white noise. The samples are
    /// interleaved in the output vector (L,R,L,R,...).
    ///
    /// # Arguments
    ///
    /// * `num_samples` - The number of samples to generate per channel
    /// * `amplitude` - The amplitude scaling factor in the range [0.0, 1.0]
    ///
    /// # Returns
    ///
    /// A vector of i16 samples containing interleaved stereo noise samples.
    /// The length of the vector will be 2 * num_samples.
    ///
    /// # Interleaving
    ///
    /// The samples are interleaved in the standard audio format:
    /// [left_0, right_0, left_1, right_1, ...].
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
    ///
    /// let mut generator = NoiseGenerator::new(12345);
    ///
    /// // Generate 1 second of stereo noise at 44.1kHz with 70% amplitude
    /// let samples = generator.generate_stereo(44100, 0.7);
    /// assert_eq!(samples.len(), 88200); // 2 channels * 44100 samples
    /// ```
    pub fn generate_stereo(&mut self, num_samples: u32, amplitude: f32) -> Vec<i16> {
        let mut samples = Vec::with_capacity((num_samples * 2) as usize);

        for _ in 0..num_samples {
            let sample_left = self.random_gaussian() * amplitude;
            let sample_right = self.random_gaussian() * amplitude;

            let value_left = (sample_left * 32767.0).clamp(-32768.0, 32767.0) as i16;
            let value_right = (sample_right * 32767.0).clamp(-32768.0, 32767.0) as i16;

            samples.push(value_left);
            samples.push(value_right);
        }

        samples
    }

    /// Generates a buffer of stereo Gaussian white noise with controlled correlation between channels.
    ///
    /// This method creates a vector of 16-bit integer samples representing
    /// two channels of Gaussian white noise with a specified correlation coefficient.
    /// This is useful for simulating partially correlated noise sources or
    /// testing stereo processing algorithms with different degrees of channel correlation.
    ///
    /// # Arguments
    ///
    /// * `num_samples` - The number of samples to generate per channel
    /// * `amplitude` - The amplitude scaling factor in the range [0.0, 1.0]
    /// * `correlation` - The correlation coefficient between channels in the range [-1.0, 1.0]
    ///
    /// # Returns
    ///
    /// A vector of i16 samples containing interleaved stereo noise samples.
    /// The length of the vector will be 2 * num_samples.
    ///
    /// # Correlation Coefficient
    ///
    /// The correlation coefficient controls the statistical similarity between channels:
    /// - 1.0: Perfectly correlated (identical channels)
    /// - 0.0: Uncorrelated (independent channels)
    /// - -1.0: Perfectly anti-correlated (inverted channels)
    ///
    /// # Mathematical Implementation
    ///
    /// For two uncorrelated random variables X and Y, we create a new variable Z
    /// that has correlation ρ with X using the formula:
    /// ```text
    /// Z = ρX + √(1-ρ²)Y
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
    ///
    /// let mut generator = NoiseGenerator::new(12345);
    ///
    /// // Generate 1 second of stereo noise at 48kHz with 50% amplitude
    /// // and 0.8 correlation between channels
    /// let samples = generator.generate_correlated_stereo(48000, 0.5, 0.8);
    /// assert_eq!(samples.len(), 96000); // 2 channels * 48000 samples
    /// ```
    pub fn generate_correlated_stereo(
        &mut self,
        num_samples: u32,
        amplitude: f32,
        correlation: f32,
    ) -> Vec<i16> {
        let mut samples = Vec::with_capacity((num_samples * 2) as usize);
        let sqrt_one_minus_corr_squared = (1.0 - correlation * correlation).sqrt();

        for _ in 0..num_samples {
            let sample1 = self.random_gaussian() * amplitude;
            let independent_sample = self.random_gaussian();
            let sample2 = (correlation * sample1
                + sqrt_one_minus_corr_squared * independent_sample)
                * amplitude;

            let value1 = (sample1 * 32767.0).clamp(-32768.0, 32767.0) as i16;
            let value2 = (sample2 * 32767.0).clamp(-32768.0, 32767.0) as i16;

            samples.push(value1);
            samples.push(value2);
        }

        samples
    }

    /// Generates a mono (single channel) mock photoacoustic signal.
    ///
    /// This method creates a vector of 16-bit integer samples representing
    /// a synthetic photoacoustic signal consisting of white noise with
    /// periodic pulsed sinusoidal signals overlaid at the specified frequency.
    ///
    /// # Arguments
    ///
    /// * `num_samples` - The number of samples to generate
    /// * `sample_rate` - The sample rate in Hz
    /// * `noise_amplitude` - The amplitude of the background white noise in the range [0.0, 1.0]
    /// * `pulse_frequency` - The frequency of the pulses in Hz
    /// * `pulse_width` - The width of each pulse in seconds
    /// * `min_pulse_amplitude` - The minimum amplitude of pulses in the range [0.0, 1.0]
    /// * `max_pulse_amplitude` - The maximum amplitude of pulses in the range [0.0, 1.0]
    ///
    /// # Returns
    ///
    /// A vector of i16 samples containing the generated mock signal
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
    ///
    /// let mut generator = NoiseGenerator::new(12345);
    ///
    /// // Generate 1 second of mono mock photoacoustic signal at 48kHz
    /// // with 30% noise amplitude, 2kHz pulse frequency, 40ms pulse width,
    /// // and pulse amplitude between 80% and 100%
    /// let samples = generator.generate_mock_photoacoustic_mono(
    ///     48000,     // num_samples (1 second at 48kHz)
    ///     48000,     // sample_rate
    ///     0.3,       // noise_amplitude
    ///     2000.0,    // pulse_frequency
    ///     0.04,      // pulse_width (40ms)
    ///     0.8,       // min_pulse_amplitude
    ///     1.0        // max_pulse_amplitude
    /// );
    /// ```
    pub fn generate_mock_photoacoustic_mono(
        &mut self,
        num_samples: u32,
        sample_rate: u32,
        noise_amplitude: f32,
        pulse_frequency: f32,
        pulse_width: f32,
        min_pulse_amplitude: f32,
        max_pulse_amplitude: f32,
    ) -> Vec<i16> {
        // Generate the white noise background
        let mut result = self.generate_mono(num_samples, noise_amplitude);

        // Calculate number of samples in one pulse cycle
        let samples_per_cycle = sample_rate as f32 / pulse_frequency;

        // Calculate number of samples in the pulse width
        let samples_per_pulse = (pulse_width * sample_rate as f32) as u32;

        // Amplitude range for random pulse amplitude
        let pulse_amplitude_range = max_pulse_amplitude - min_pulse_amplitude;

        let mut cycle_position: u32 = 0;
        let mut current_pulse_amplitude = 0.0f32;

        // Iterate through all samples
        for i in 0..num_samples as usize {
            // Start of a new cycle
            if cycle_position == 0 {
                // Generate a random pulse amplitude for this cycle
                current_pulse_amplitude =
                    min_pulse_amplitude + pulse_amplitude_range * self.random_float().abs();
            }

            // Check if we're within a pulse
            if cycle_position < samples_per_pulse {
                // Generate sine wave pulse
                let phase = 2.0 * std::f32::consts::PI * pulse_frequency * (i as f32)
                    / (sample_rate as f32);
                let pulse = phase.sin() * current_pulse_amplitude;

                // Add pulse to noise
                let sample_value = result[i] as f32 / 32767.0;
                let combined = sample_value + pulse;

                // Clamp and convert back to i16
                result[i] = (combined * 32767.0).clamp(-32768.0, 32767.0) as i16;
            }

            // Update cycle position
            cycle_position = (cycle_position + 1) % samples_per_cycle as u32;
        }

        result
    }

    /// Generates a stereo (two channel) mock photoacoustic signal with independent channels.
    ///
    /// This method creates a vector of 16-bit integer samples representing
    /// a stereo synthetic photoacoustic signal with independent noise and
    /// pulse signals in each channel.
    ///
    /// # Arguments
    ///
    /// * `num_samples` - The number of samples to generate per channel
    /// * `sample_rate` - The sample rate in Hz
    /// * `noise_amplitude` - The amplitude of the background white noise in the range [0.0, 1.0]
    /// * `pulse_frequency` - The frequency of the pulses in Hz
    /// * `pulse_width` - The width of each pulse in seconds
    /// * `min_pulse_amplitude` - The minimum amplitude of pulses in the range [0.0, 1.0]
    /// * `max_pulse_amplitude` - The maximum amplitude of pulses in the range [0.0, 1.0]
    ///
    /// # Returns
    ///
    /// A vector of i16 samples containing interleaved stereo mock signal samples.
    /// The length of the vector will be 2 * num_samples.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
    ///
    /// let mut generator = NoiseGenerator::new(12345);
    ///
    /// // Generate 1 second of stereo mock photoacoustic signal at 48kHz
    /// let samples = generator.generate_mock_photoacoustic_stereo(
    ///     48000,     // num_samples (1 second at 48kHz)
    ///     48000,     // sample_rate
    ///     0.3,       // noise_amplitude
    ///     2000.0,    // pulse_frequency
    ///     0.04,      // pulse_width (40ms)
    ///     0.8,       // min_pulse_amplitude
    ///     1.0        // max_pulse_amplitude
    /// );
    /// ```
    pub fn generate_mock_photoacoustic_stereo(
        &mut self,
        num_samples: u32,
        sample_rate: u32,
        noise_amplitude: f32,
        pulse_frequency: f32,
        pulse_width: f32,
        min_pulse_amplitude: f32,
        max_pulse_amplitude: f32,
    ) -> Vec<i16> {
        // Generate the white noise background (stereo)
        let mut result = self.generate_stereo(num_samples, noise_amplitude);

        // Calculate number of samples in one pulse cycle
        let samples_per_cycle = sample_rate as f32 / pulse_frequency;

        // Calculate number of samples in the pulse width
        let samples_per_pulse = (pulse_width * sample_rate as f32) as u32;

        // Amplitude range for random pulse amplitude
        let pulse_amplitude_range = max_pulse_amplitude - min_pulse_amplitude;

        let mut cycle_position: u32 = 0;
        let mut left_pulse_amplitude = 0.0f32;
        let mut right_pulse_amplitude = 0.0f32;

        // Iterate through all samples (interleaved L/R)
        for i in 0..num_samples as usize {
            // Start of a new cycle
            if cycle_position == 0 {
                // Generate random pulse amplitudes for left and right channels
                left_pulse_amplitude =
                    min_pulse_amplitude + pulse_amplitude_range * self.random_float().abs();
                right_pulse_amplitude =
                    min_pulse_amplitude + pulse_amplitude_range * self.random_float().abs();
            }

            // Check if we're within a pulse
            if cycle_position < samples_per_pulse {
                // Generate sine wave pulse
                let phase = 2.0 * std::f32::consts::PI * pulse_frequency * (i as f32)
                    / (sample_rate as f32);
                let pulse_shape = phase.sin();

                // Add pulse to both left and right channels
                // Left channel (even indices)
                let left_index = i * 2;
                let left_value = result[left_index] as f32 / 32767.0;
                let combined_left = left_value + pulse_shape * left_pulse_amplitude;
                result[left_index] = (combined_left * 32767.0).clamp(-32768.0, 32767.0) as i16;

                // Right channel (odd indices)
                let right_index = left_index + 1;
                let right_value = result[right_index] as f32 / 32767.0;
                let combined_right = right_value + pulse_shape * right_pulse_amplitude;
                result[right_index] = (combined_right * 32767.0).clamp(-32768.0, 32767.0) as i16;
            }

            // Update cycle position - once per stereo pair
            if i % 2 == 1 {
                cycle_position = (cycle_position + 1) % samples_per_cycle as u32;
            }
        }

        result
    }

    /// Generates a stereo mock photoacoustic signal with controlled correlation between channels.
    ///
    /// This method creates a vector of 16-bit integer samples representing a stereo
    /// synthetic photoacoustic signal with correlated noise and pulse signals between channels.
    ///
    /// # Arguments
    ///
    /// * `num_samples` - The number of samples to generate per channel
    /// * `sample_rate` - The sample rate in Hz
    /// * `noise_amplitude` - The amplitude of the background white noise in the range [0.0, 1.0]
    /// * `pulse_frequency` - The frequency of the pulses in Hz
    /// * `pulse_width` - The width of each pulse in seconds
    /// * `min_pulse_amplitude` - The minimum amplitude of pulses in the range [0.0, 1.0]
    /// * `max_pulse_amplitude` - The maximum amplitude of pulses in the range [0.0, 1.0]
    /// * `correlation` - The correlation coefficient between channels in the range [-1.0, 1.0]
    ///
    /// # Returns
    ///
    /// A vector of i16 samples containing interleaved stereo mock signal samples with the specified correlation.
    /// The length of the vector will be 2 * num_samples.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
    ///
    /// let mut generator = NoiseGenerator::new(12345);
    ///
    /// // Generate 1 second of correlated stereo mock photoacoustic signal at 48kHz with correlation 0.7
    /// let samples = generator.generate_mock_photoacoustic_correlated(
    ///     48000,     // num_samples (1 second at 48kHz)
    ///     48000,     // sample_rate
    ///     0.3,       // noise_amplitude
    ///     2000.0,    // pulse_frequency
    ///     0.04,      // pulse_width (40ms)
    ///     0.8,       // min_pulse_amplitude
    ///     1.0,       // max_pulse_amplitude
    ///     0.7        // correlation
    /// );
    /// ```
    pub fn generate_mock_photoacoustic_correlated(
        &mut self,
        num_samples: u32,
        sample_rate: u32,
        noise_amplitude: f32,
        pulse_frequency: f32,
        pulse_width: f32,
        min_pulse_amplitude: f32,
        max_pulse_amplitude: f32,
        correlation: f32,
    ) -> Vec<i16> {
        // Generate the correlated white noise background
        let mut result = self.generate_correlated_stereo(num_samples, noise_amplitude, correlation);

        // Calculate number of samples in one pulse cycle
        let samples_per_cycle = sample_rate as f32 / pulse_frequency;

        // Calculate number of samples in the pulse width
        let samples_per_pulse = (pulse_width * sample_rate as f32) as u32;

        // Amplitude range for random pulse amplitude
        let pulse_amplitude_range = max_pulse_amplitude - min_pulse_amplitude;

        // Square root term used in correlation calculation
        let sqrt_one_minus_corr_squared = (1.0 - correlation * correlation).sqrt();

        let mut cycle_position: u32 = 0;
        let mut base_pulse_amplitude = 0.0f32;

        // Iterate through all samples (by sample pairs)
        for i in 0..num_samples as usize {
            // Start of a new cycle
            if cycle_position == 0 {
                // Generate a base random pulse amplitude
                base_pulse_amplitude =
                    min_pulse_amplitude + pulse_amplitude_range * self.random_float().abs();
            }

            // Check if we're within a pulse
            if cycle_position < samples_per_pulse {
                // Generate sine wave pulse with correlation
                let phase = 2.0 * std::f32::consts::PI * pulse_frequency * (i as f32)
                    / (sample_rate as f32);
                let pulse_shape = phase.sin();

                // Generate correlated pulse amplitudes
                let left_pulse_amplitude = base_pulse_amplitude;

                // Generate an independent component for right channel
                let independent_component = pulse_amplitude_range * self.random_float().abs();

                // Apply correlation to right channel amplitude
                let right_pulse_amplitude = correlation * base_pulse_amplitude
                    + sqrt_one_minus_corr_squared * independent_component;

                // Clamp to valid range
                let right_pulse_amplitude =
                    right_pulse_amplitude.clamp(min_pulse_amplitude, max_pulse_amplitude);

                // Add pulse to both left and right channels
                // Left channel (even indices)
                let left_index = i * 2;
                let left_value = result[left_index] as f32 / 32767.0;
                let combined_left = left_value + pulse_shape * left_pulse_amplitude;
                result[left_index] = (combined_left * 32767.0).clamp(-32768.0, 32767.0) as i16;

                // Right channel (odd indices)
                let right_index = left_index + 1;
                let right_value = result[right_index] as f32 / 32767.0;
                let combined_right = right_value + pulse_shape * right_pulse_amplitude;
                result[right_index] = (combined_right * 32767.0).clamp(-32768.0, 32767.0) as i16;
            }

            // Update cycle position - once per stereo pair
            if i % 2 == 1 {
                cycle_position = (cycle_position + 1) % samples_per_cycle as u32;
            }
        }

        result
    }

    /// Generates a stereo mock photoacoustic signal simulating a Helmholtz resonance cell system.
    ///
    /// This method creates a more realistic simulation of a photoacoustic analyzer using
    /// a Helmholtz resonance cell. The system features two microphones positioned in
    /// approximate phase opposition, with additional realistic characteristics:
    ///
    /// - Gas flow background noise with 1/f characteristics
    /// - Laser modulation at resonance frequency (~2kHz)
    /// - Phase opposition between channels (with temperature/gas-dependent variations)
    /// - Environmental perturbations and system noise
    /// - Frequency response characteristics of the resonance cell
    /// - Molecular concentration variations (random walk simulation)
    ///
    /// # Arguments
    ///
    /// * `num_samples` - The number of samples to generate per channel
    /// * `sample_rate` - The sample rate in Hz
    /// * `background_noise_amplitude` - Base amplitude of gas flow noise [0.0, 1.0]
    /// * `resonance_frequency` - Cell resonance frequency in Hz (typically ~2000Hz)
    /// * `laser_modulation_depth` - Depth of laser modulation [0.0, 1.0]
    /// * `signal_amplitude` - Amplitude of the photoacoustic signal [0.0, 1.0]
    /// * `phase_opposition_degrees` - Phase difference between mics in degrees (180° = perfect opposition)
    /// * `temperature_drift_factor` - Factor affecting phase and frequency stability [0.0, 0.1]
    /// * `gas_flow_noise_factor` - Factor controlling 1/f gas flow noise characteristics [0.0, 1.0]
    /// * `snr_factor` - Signal-to-noise ratio factor for the output signal in dB
    ///
    /// # Returns
    ///
    /// A vector of i16 samples containing interleaved stereo samples simulating the
    /// Helmholtz cell system. The length will be 2 * num_samples.
    ///
    /// # Physical System Simulation
    ///
    /// The function simulates the following physical phenomena:
    /// - **Helmholtz Resonance**: Enhanced signal at the resonance frequency
    /// - **Gas Flow Noise**: 1/f noise characteristics from gas circulation
    /// - **Microphone Phase Opposition**: Constructive interference in the differential signal
    /// - **Temperature Effects**: Slight variations in resonance frequency and phase
    /// - **Environmental Perturbations**: External acoustic interference
    /// - **Laser Modulation**: Periodic modulation creating the photoacoustic effect
    /// - **Molecular Concentration**: Random walk variations simulating changing analyte concentration (10%-200% of nominal)
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::utility::noise_generator::NoiseGenerator;
    ///
    /// let mut generator = NoiseGenerator::new(12345);
    ///
    /// // Generate 1 second of realistic Helmholtz cell photoacoustic signal
    /// let samples = generator.generate_mock2_photoacoustic_stereo(
    ///     48000,  // num_samples (1 second at 48kHz)
    ///     48000,  // sample_rate
    ///     0.15,   // background_noise_amplitude (15% background)
    ///     2000.0, // resonance_frequency (2kHz typical)
    ///     0.8,    // laser_modulation_depth (80% modulation)
    ///     0.6,    // signal_amplitude (60% signal level)
    ///     175.0,  // phase_opposition_degrees (5° off perfect opposition)
    ///     0.02,   // temperature_drift_factor (2% variation)
    ///     0.7     // gas_flow_noise_factor (70% 1/f characteristic)
    ///     20,     // snr_factor (20dB SNR)
    /// );
    /// assert_eq!(samples.len(), 96000); // 2 channels * 48000 samples
    /// ```
    pub fn generate_mock2_photoacoustic_stereo(
        &mut self,
        num_samples: u32,
        sample_rate: u32,
        background_noise_amplitude: f32,
        resonance_frequency: f32,
        laser_modulation_depth: f32,
        signal_amplitude: f32,
        phase_opposition_degrees: f32,
        temperature_drift_factor: f32,
        gas_flow_noise_factor: f32,
        snr_factor: f32,
    ) -> Vec<i16> {
        let mut result = Vec::with_capacity((num_samples * 2) as usize);

        // Physical constants and system parameters
        let dt = 1.0 / sample_rate as f32;
        let pi = std::f32::consts::PI;
        let phase_opposition_rad = phase_opposition_degrees * pi / 180.0;

        // Initialize state variables for realistic simulation
        let mut concentration_level = 1.0f32; // Relative concentration (100% nominal)
        let mut temperature_phase_drift = 0.0f32;
        let mut frequency_drift = 0.0f32;

        // Parameters for random walk concentration simulation (90% to 110% of nominal)
        let concentration_walk_rate = 0.00005; // Slower concentration changes
        let min_concentration = 0.9;
        let max_concentration = 1.1;

        // Frequency drift limits to prevent excessive wandering
        let max_frequency_drift = resonance_frequency * 0.05; // ±5% maximum drift

        // 1/f noise state variables for gas flow simulation
        let mut pink_noise_state = [0.0f32; 6];

        // Helmholtz resonance characteristics
        let q_factor = 50.0; // Quality factor of the resonance

        // Calculate target SNR from snr_factor (assuming it's in dB)
        let target_snr_linear = 10.0f32.powf(snr_factor / 10.0);

        for i in 0..num_samples {
            let t = i as f32 * dt;

            // === 1. MOLECULAR CONCENTRATION VARIATION (Random Walk) ===
            // Simulate changing analyte concentration over time
            let concentration_change = (self.random_gaussian() * concentration_walk_rate).tanh();
            concentration_level += concentration_change;
            concentration_level = concentration_level.clamp(min_concentration, max_concentration);

            // === 2. TEMPERATURE EFFECTS ===
            // Temperature affects both phase relationships and resonance frequency
            let temp_variation = self.random_gaussian() * temperature_drift_factor;
            temperature_phase_drift += temp_variation * 0.001; // Much slower phase drift

            // Apply frequency drift with mean reversion to prevent excessive wandering
            let drift_change = temp_variation * 0.1; // Smaller frequency changes
            frequency_drift += drift_change;
            // Mean reversion: gradually pull frequency_drift back to zero
            frequency_drift *= 0.9999; // Slow decay back to center frequency
                                       // Hard limit to prevent excessive drift
            frequency_drift = frequency_drift.clamp(-max_frequency_drift, max_frequency_drift);

            // Current effective resonance frequency with bounded drift
            let current_resonance_freq = resonance_frequency + frequency_drift;

            // === 3. GAS FLOW NOISE (1/f characteristics) ===
            // Generate pink noise for realistic gas flow turbulence
            let white_input = self.random_gaussian() * gas_flow_noise_factor;

            // Pink noise filter implementation (approximates 1/f spectrum)
            pink_noise_state[0] = 0.99886 * pink_noise_state[0] + white_input * 0.0555179;
            pink_noise_state[1] = 0.99332 * pink_noise_state[1] + white_input * 0.0750759;
            pink_noise_state[2] = 0.96900 * pink_noise_state[2] + white_input * 0.1538520;
            pink_noise_state[3] = 0.86650 * pink_noise_state[3] + white_input * 0.3104856;
            pink_noise_state[4] = 0.55000 * pink_noise_state[4] + white_input * 0.5329522;
            pink_noise_state[5] = -0.7616 * pink_noise_state[5] + white_input * 0.0168700;

            let gas_flow_state = pink_noise_state.iter().sum::<f32>() + white_input * 0.5362;
            let gas_flow_noise = gas_flow_state * background_noise_amplitude;

            // === 4. LASER MODULATION ===
            // Generate the photoacoustic signal from laser modulation
            let modulation_phase = 2.0 * pi * current_resonance_freq * t;
            let laser_signal = (modulation_phase.sin() * laser_modulation_depth).sin();

            // === 5. HELMHOLTZ RESONANCE ENHANCEMENT ===
            // Apply resonance characteristics - enhanced response at resonance frequency
            let resonance_response = {
                // Simple second-order resonance filter response
                let freq_deviation = (current_resonance_freq - resonance_frequency).abs();
                let normalized_deviation = freq_deviation / (resonance_frequency / q_factor);
                let resonance_gain = 1.0 / (1.0 + normalized_deviation.powi(2)).sqrt();
                laser_signal * resonance_gain
            };

            // === 6. PHOTOACOUSTIC SIGNAL ===
            // Combine concentration-dependent signal with resonance
            let photoacoustic_signal = resonance_response * concentration_level * signal_amplitude;

            // === 7. ENVIRONMENTAL PERTURBATIONS ===
            // Add low-frequency external acoustic interference
            let environmental_noise = {
                let low_freq_noise = (2.0 * pi * 50.0 * t).sin() * 0.1 * self.random_gaussian();
                let mid_freq_noise = (2.0 * pi * 150.0 * t).sin() * 0.05 * self.random_gaussian();
                (low_freq_noise + mid_freq_noise) * background_noise_amplitude
            };

            // === 8. BACKGROUND WHITE NOISE ===
            let white_noise = self.random_gaussian() * background_noise_amplitude * 0.3;

            // === 9. COMBINE ALL NOISE SOURCES ===
            let total_background = gas_flow_noise + environmental_noise + white_noise;

            // === 10. MICROPHONE SIGNALS WITH PHASE OPPOSITION ===
            // Calculate actual phase opposition including temperature drift
            let actual_phase_opposition = phase_opposition_rad + temperature_phase_drift;

            // Microphone 1 (reference)
            let mic1_signal = photoacoustic_signal + total_background;

            // Microphone 2 (phase-shifted, simulating opposite position in cell)
            // Apply phase opposition correctly - signal is inverted, background is correlated
            let mic2_signal =
                -photoacoustic_signal * actual_phase_opposition.cos() + total_background * 0.95;

            // === 11. APPLY SNR CONTROL ===
            // Calculate the actual differential signal (what we want to control)
            let differential_signal = mic1_signal - mic2_signal;
            let signal_component = 2.0 * photoacoustic_signal; // Expected differential signal
            let noise_component = total_background * 0.05; // Remaining noise after differential

            // Scale the entire signal to achieve target SNR
            let current_signal_power = signal_component.abs();
            let current_noise_power = noise_component.abs().max(1e-10); // Avoid division by zero
            let desired_noise_amplitude = current_signal_power / target_snr_linear;
            let noise_scale = if current_noise_power > 0.0 {
                desired_noise_amplitude / current_noise_power
            } else {
                1.0
            };

            let final_mic1 = photoacoustic_signal + total_background * noise_scale;
            let final_mic2 = -photoacoustic_signal * actual_phase_opposition.cos()
                + total_background * noise_scale * 0.95;

            // === 12. CONVERT TO 16-BIT INTEGER SAMPLES ===
            // Apply soft clipping to prevent harsh distortion
            let mic1_clipped = (final_mic1.tanh() * 32767.0) as i16;
            let mic2_clipped = (final_mic2.tanh() * 32767.0) as i16;

            // === 13. INTERLEAVE STEREO SAMPLES ===
            result.push(mic1_clipped); // Left channel
            result.push(mic2_clipped); // Right channel
        }

        result
    }
}
