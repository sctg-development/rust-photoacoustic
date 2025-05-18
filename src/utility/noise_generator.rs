// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Gaussian Noise Generator
//!
//! This module provides a lightweight implementation for generating Gaussian white noise,
//! which is commonly used in photoacoustic signal processing for:
//!
//! - Testing and calibration of signal processing algorithms
//! - Simulating background noise in photoacoustic signals
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
//!
//! ## Examples
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
}
