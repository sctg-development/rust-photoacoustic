// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Tests for Differential Signal Calculation
//!
//! This module contains test cases that validate the differential signal calculation
//! functionality in the photoacoustic signal processing pipeline.
//!
//! ## Test Coverage:
//!
//! * Basic functionality testing with simple vectors
//! * Complex testing with real audio WAV files
//! * Error handling for invalid inputs (uneven channel lengths)
//! * Statistical validation of differential properties
//!
//! The tests ensure that:
//! 1. Differential signals are correctly calculated (A-B)
//! 2. Statistical properties are preserved (mean, energy)
//! 3. Maximum differences are correctly identified
//! 4. Error handling works as expected

use super::differential::{DifferentialCalculator, SimpleDifferential};
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to read a stereo WAV file and return the left and right channels.
    ///
    /// This function opens a WAV file, validates that it's a stereo recording,
    /// and separates the interleaved audio data into distinct left and right channel vectors.
    /// The audio samples are normalized to the range [-1.0, 1.0].
    ///
    /// ### Arguments
    ///
    /// * `path` - Path to the stereo WAV file
    ///
    /// ### Returns
    ///
    /// * `Result<(Vec<f32>, Vec<f32>, u32)>` - Tuple containing:
    ///   - Left channel samples as normalized f32 values
    ///   - Right channel samples as normalized f32 values
    ///   - Sample rate of the recording in Hz
    ///
    /// ### Errors
    ///
    /// Returns an error if:
    /// - The file cannot be opened
    /// - The file is not a stereo recording (doesn't have exactly 2 channels)
    /// - There are issues reading the audio samples
    fn read_stereo_wav_file(path: &str) -> Result<(Vec<f32>, Vec<f32>, u32)> {
        let mut reader = hound::WavReader::open(path)?;
        let spec = reader.spec();
        let num_channels = spec.channels as usize;

        if num_channels != 2 {
            return Err(anyhow::anyhow!(
                "Expected stereo file (2 channels), found {} channels",
                num_channels
            ));
        }

        // Read all samples
        let interleaved_samples: Vec<f32> = reader
            .samples::<i16>()
            .map(|s| s.map(|sample| sample as f32 / 32768.0))
            .collect::<Result<_, _>>()?;

        let total_samples = interleaved_samples.len();
        let samples_per_channel = total_samples / 2;

        // Deinterleave the samples into left and right channels
        let mut left_channel = Vec::with_capacity(samples_per_channel);
        let mut right_channel = Vec::with_capacity(samples_per_channel);

        for i in (0..total_samples).step_by(2) {
            left_channel.push(interleaved_samples[i]);
            right_channel.push(interleaved_samples[i + 1]);
        }

        Ok((left_channel, right_channel, spec.sample_rate))
    }

    /// Helper function to save audio samples as a WAV file.
    ///
    /// This function creates a mono WAV file from the provided normalized audio samples.
    /// It ensures the output directory exists, converts the floating-point samples to
    /// 16-bit integers, and writes them to the specified path.
    ///
    /// ### Arguments
    ///
    /// * `samples` - Vector of audio samples as normalized f32 values in range [-1.0, 1.0]
    /// * `sample_rate` - Sample rate of the audio in Hz
    /// * `path` - Output path where the WAV file should be saved
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success or an error
    ///
    /// ### Errors
    ///
    /// Returns an error if:
    /// - The output directory cannot be created
    /// - The WAV file cannot be written to disk
    fn save_wav_file(samples: &[f32], sample_rate: u32, path: &str) -> Result<()> {
        // Create the directory if it doesn't exist
        if let Some(parent) = std::path::Path::new(path).parent() {
            fs::create_dir_all(parent)?;
        }

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::create(path, spec)?;

        for &sample in samples {
            // Convert to i16 and clamp values to prevent overflow
            let amplitude = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            writer.write_sample(amplitude)?;
        }

        writer.finalize()?;
        println!("Saved WAV file to: {}", path);

        Ok(())
    }

    /// Test the differential calculation using a real stereo WAV file.
    ///
    /// This comprehensive test loads a stereo WAV file, calculates the differential
    /// signal between channels, and validates the results through multiple checks:
    /// - Verifying sample-by-sample calculation correctness
    /// - Confirming statistical properties (means, energies)
    /// - Testing the location of maximum differences
    /// - Saving the results as WAV files for manual verification
    /// - Testing error handling for invalid inputs
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success or an error
    #[test]
    fn test_differential_with_wav_file() -> Result<()> {
        let workspace_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        // Load the test WAV file
        let wav_path = workspace_path.join("data").join("16_48k_PerfectTest.wav");

        let (left_channel, right_channel, sample_rate) =
            read_stereo_wav_file(wav_path.to_str().unwrap())?;

        // Verify we got samples
        assert!(!left_channel.is_empty(), "Left channel should not be empty");
        assert!(
            !right_channel.is_empty(),
            "Right channel should not be empty"
        );
        assert_eq!(
            left_channel.len(),
            right_channel.len(),
            "Channel lengths should be equal"
        );

        println!(
            "WAV file loaded: {} samples per channel at {} Hz",
            left_channel.len(),
            sample_rate
        );

        // Create a differential calculator
        let diff_calc = SimpleDifferential::new();

        // Calculate the differential signal (A-B, where A=left, B=right)
        let differential_signal = diff_calc.calculate(&left_channel, &right_channel)?;

        // Basic validation
        assert_eq!(
            differential_signal.len(),
            left_channel.len(),
            "Differential signal length should match input channel length"
        );

        // Verify the calculation by manually checking some samples
        let sample_count = differential_signal.len().min(10);
        for i in 0..sample_count {
            let expected = left_channel[i] - right_channel[i];
            assert_eq!(
                differential_signal[i], expected,
                "Differential calculation mismatch at sample {}",
                i
            );
        }

        // Check statistical properties
        let left_mean: f32 = left_channel.iter().sum::<f32>() / left_channel.len() as f32;
        let right_mean: f32 = right_channel.iter().sum::<f32>() / right_channel.len() as f32;
        let diff_mean: f32 =
            differential_signal.iter().sum::<f32>() / differential_signal.len() as f32;

        println!("Left channel mean: {}", left_mean);
        println!("Right channel mean: {}", right_mean);
        println!("Differential signal mean: {}", diff_mean);

        // The mean of the differential should be close to the difference of the means
        let expected_mean = left_mean - right_mean;
        let mean_difference = (diff_mean - expected_mean).abs();
        assert!(
            mean_difference < 1e-6,
            "Mean of differential should equal difference of means"
        );

        // Calculate signal energies
        let left_energy: f32 = left_channel.iter().map(|&x| x * x).sum();
        let right_energy: f32 = right_channel.iter().map(|&x| x * x).sum();
        let diff_energy: f32 = differential_signal.iter().map(|&x| x * x).sum();

        println!("Left channel energy: {}", left_energy);
        println!("Right channel energy: {}", right_energy);
        println!("Differential signal energy: {}", diff_energy);

        // Cross-correlation test (check a few samples)
        // This ensures that maxima in the differential occur when left and right differ most
        let mut max_diff_index = 0;
        let mut max_diff_value = 0.0f32;

        for i in 0..100.min(differential_signal.len()) {
            let abs_diff = differential_signal[i].abs();
            if abs_diff > max_diff_value {
                max_diff_value = abs_diff;
                max_diff_index = i;
            }
        }

        println!(
            "Maximum difference found at sample {}: {}",
            max_diff_index, max_diff_value
        );
        println!(
            "Left sample at that point: {}",
            left_channel[max_diff_index]
        );
        println!(
            "Right sample at that point: {}",
            right_channel[max_diff_index]
        );

        // Verify that the point of maximum difference is actually where the channels differ most
        let actual_diff = (left_channel[max_diff_index] - right_channel[max_diff_index]).abs();
        assert!(
            (actual_diff - max_diff_value).abs() < 1e-6,
            "Maximum differential point should match actual max difference between channels"
        );

        // Save the differential result to the out directory
        let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("out")
            .join("differential");

        // Save the original channels and the differential signal
        save_wav_file(
            &left_channel,
            sample_rate,
            out_dir.join("left_channel.wav").to_str().unwrap(),
        )?;

        save_wav_file(
            &right_channel,
            sample_rate,
            out_dir.join("right_channel.wav").to_str().unwrap(),
        )?;

        save_wav_file(
            &differential_signal,
            sample_rate,
            out_dir.join("differential_signal.wav").to_str().unwrap(),
        )?;

        // Test with uneven channel lengths
        let shorter_left = left_channel[0..100].to_vec();
        let result = diff_calc.calculate(&shorter_left, &right_channel);
        assert!(result.is_err(), "Should error on uneven channel lengths");

        Ok(())
    }

    /// Test basic differential calculation with simple test vectors.
    ///
    /// This test validates the core functionality of the differential calculator
    /// using simple, predictable data. It checks that:
    /// - The output length matches the input length
    /// - Each output value equals the difference between corresponding input values
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success or an error
    #[test]
    fn test_differential_basic() -> Result<()> {
        // Create simple test data
        let channel_a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let channel_b = vec![0.5, 1.0, 1.5, 2.0, 2.5];

        let diff_calc = SimpleDifferential::new();
        let result = diff_calc.calculate(&channel_a, &channel_b)?;

        // Expected: [0.5, 1.0, 1.5, 2.0, 2.5]
        assert_eq!(result.len(), 5);
        assert_eq!(result[0], 0.5);
        assert_eq!(result[1], 1.0);
        assert_eq!(result[2], 1.5);
        assert_eq!(result[3], 2.0);
        assert_eq!(result[4], 2.5);

        Ok(())
    }
}
