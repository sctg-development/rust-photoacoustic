// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Differential Signal Processing
//!
//! This module provides functionality for calculating differential signals between two audio channels
//! or sample streams. Differential processing is commonly used in photoacoustic analysis to:
//!
//! - Extract relevant photoacoustic responses from background noise
//! - Isolate signals from stereo recordings where one channel contains signal+noise and the other contains reference noise
//! - Compare before/after recordings to isolate effects of a stimulus
//! - Remove common-mode interference from sensor data
//!
//! ## Features
//!
//! - Sample-wise subtraction with overflow protection for integer samples
//! - Trait-based interface for implementing different differential calculation strategies
//! - Error handling for mismatched channel lengths
//!
//! ## Examples
//!
//! Basic usage with i16 (integer) samples:
//!
//! ```
//! use rust_photoacoustic::preprocessing::differential::calculate_differential;
//!
//! let channel_a = vec![100, 200, 300];
//! let channel_b = vec![50, 75, 125];
//! let diff = calculate_differential(&channel_a, &channel_b);
//! assert_eq!(diff, vec![50, 125, 175]);
//! ```
//!
//! Using the trait-based interface with f32 (floating point) samples:
//!
//! ```
//! use rust_photoacoustic::preprocessing::differential::{DifferentialCalculator, SimpleDifferential};
//! use anyhow::Result;
//!
//! fn process_channels() -> Result<()> {
//!     let channel_a = vec![1.0, 2.0, 3.0];
//!     let channel_b = vec![0.5, 0.7, 0.9];
//!     
//!     let calculator = SimpleDifferential::new();
//!     let diff = calculator.calculate(&channel_a, &channel_b)?;
//!     
//!     assert_eq!(diff, vec![0.5, 1.3, 2.1]);
//!     Ok(())
//! }
//! ```

use anyhow::Result;

/// Calculate the differential signal between two i16 sample vectors
///
/// This function calculates signal_a - signal_b for each sample pair with overflow protection.
/// It uses saturation arithmetic to handle cases where the subtraction would result in
/// values outside the i16 range.
///
/// Used primarily by the differential binary utility for WAV file processing.
///
/// # Arguments
///
/// * `signal_a` - First signal (minuend)
/// * `signal_b` - Second signal (subtrahend, to be subtracted from signal_a)
///
/// # Returns
///
/// A new vector containing the sample-wise difference. If the input vectors have different
/// lengths, the result will have length equal to the shorter of the two vectors.
///
/// # Overflow Behavior
///
/// When subtraction would cause an integer overflow/underflow:
/// - If signal_a\[i\] < signal_b\[i\] and the result would be below i16::MIN, the result is clamped to i16::MIN
/// - If signal_a\[i\] > signal_b\[i\] and the result would be above i16::MAX, the result is clamped to i16::MAX
///
/// # Examples
///
/// ```
/// use rust_photoacoustic::preprocessing::differential::calculate_differential;
///
/// let left = vec![100, 200, 30000];
/// let right = vec![50, 100, 10000];
/// let diff = calculate_differential(&left, &right);
///
/// assert_eq!(diff, vec![50, 100, 20000]);
/// ```
pub fn calculate_differential(signal_a: &[i16], signal_b: &[i16]) -> Vec<i16> {
    let mut result = Vec::with_capacity(signal_a.len());

    let length = std::cmp::min(signal_a.len(), signal_b.len());
    for i in 0..length {
        // Calculate difference with saturation to prevent overflow
        let diff = match signal_a[i].checked_sub(signal_b[i]) {
            Some(val) => val,
            None => {
                // Handle underflow with saturation
                if signal_a[i] < signal_b[i] {
                    i16::MIN
                } else {
                    i16::MAX
                }
            }
        };
        result.push(diff);
    }

    result
}

/// Trait for implementing differential signal calculation strategies
///
/// This trait defines the interface for differential calculators that work with
/// floating-point samples. Implementing this trait allows for different differential
/// calculation algorithms to be used interchangeably.
///
/// All implementations must handle the case where input channels have different lengths
/// and provide appropriate error reporting.
///
/// # Type Parameters
///
/// The trait is automatically implemented for types that are `Send` and `Sync`,
/// making implementations safe to use across thread boundaries.
pub trait DifferentialCalculator: Send + Sync {
    /// Calculate the differential signal A-B
    ///
    /// Computes the sample-wise difference between two channels of floating-point audio samples.
    ///
    /// # Arguments
    ///
    /// * `channel_a` - First channel (minuend)
    /// * `channel_b` - Second channel (subtrahend, to be subtracted from channel_a)
    ///
    /// # Returns
    ///
    /// * `Result<Vec<f32>>` - A vector containing the sample-wise difference if successful
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The input channels have different lengths
    /// - Any other implementation-specific error occurs during calculation
    fn calculate(&self, channel_a: &[f32], channel_b: &[f32]) -> Result<Vec<f32>>;
}

/// A simple differential calculator that subtracts channel B from channel A
///
/// This is a basic implementation of the `DifferentialCalculator` trait that performs
/// straightforward sample-wise subtraction of floating-point audio samples.
///
/// # Features
///
/// - Validates that input channels have the same length
/// - Performs element-wise subtraction (A-B)
/// - Returns error for mismatched channel lengths
///
/// # Examples
///
/// ```
/// use rust_photoacoustic::preprocessing::differential::{DifferentialCalculator, SimpleDifferential};
/// use anyhow::Result;
///
/// fn process() -> Result<()> {
///     let channel_a = vec![1.0, 2.0, 3.0];
///     let channel_b = vec![0.5, 1.0, 1.5];
///
///     let calculator = SimpleDifferential::new();
///     let result = calculator.calculate(&channel_a, &channel_b)?;
///
///     assert_eq!(result, vec![0.5, 1.0, 1.5]);
///     Ok(())
/// }
/// ```
pub struct SimpleDifferential {
    // No state needed for this simple implementation
}

impl Default for SimpleDifferential {
    /// Creates a new `SimpleDifferential` instance with default configuration.
    ///
    /// This is equivalent to calling `SimpleDifferential::new()`.
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleDifferential {
    /// Create a new simple differential calculator
    ///
    /// Instantiates a new `SimpleDifferential` that can be used to calculate
    /// the difference between two audio channels.
    ///
    /// # Returns
    ///
    /// A new `SimpleDifferential` instance
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::differential::SimpleDifferential;
    ///
    /// let calculator = SimpleDifferential::new();
    /// ```
    pub fn new() -> Self {
        Self {}
    }
}

impl DifferentialCalculator for SimpleDifferential {
    /// Calculate the differential signal between two floating-point sample vectors
    ///
    /// This implementation:
    /// 1. Validates that both channels have equal length
    /// 2. Computes the sample-wise difference (A-B)
    ///
    /// # Arguments
    ///
    /// * `channel_a` - First channel (minuend)
    /// * `channel_b` - Second channel (subtrahend, to be subtracted from channel_a)
    ///
    /// # Returns
    ///
    /// * `Result<Vec<f32>>` - Vector containing sample-wise differences if successful
    ///
    /// # Errors
    ///
    /// Returns an error if the input channels have different lengths
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::preprocessing::differential::{DifferentialCalculator, SimpleDifferential};
    ///
    /// let a = vec![1.0, 2.0, 3.0];
    /// let b = vec![0.5, 1.0, 1.5];
    /// let calculator = SimpleDifferential::new();
    /// let diff = calculator.calculate(&a, &b).unwrap();
    /// assert_eq!(diff, vec![0.5, 1.0, 1.5]);
    /// ```
    fn calculate(&self, channel_a: &[f32], channel_b: &[f32]) -> Result<Vec<f32>> {
        if channel_a.len() != channel_b.len() {
            return Err(anyhow::anyhow!(
                "Channel lengths don't match: A={}, B={}",
                channel_a.len(),
                channel_b.len()
            ));
        }

        let mut result = Vec::with_capacity(channel_a.len());

        for (&a, &b) in channel_a.iter().zip(channel_b.iter()) {
            result.push(a - b);
        }

        Ok(result)
    }
}
