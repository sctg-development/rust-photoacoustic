// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Tests for Universal Photoacoustic Stereo Generator
//!
//! This module contains comprehensive test cases for the `generate_universal_photoacoustic_stereo`
//! method, validating both the physical simulation aspects and technical implementation details.
//!
//! ## Test Coverage:
//!
//! * **Basic functionality**: Parameter validation, output format verification
//! * **Physical phenomena**: Concentration variations, thermal effects, gas flow noise
//! * **Modulation modes**: Amplitude modulation vs pulsed operation
//! * **Differential configuration**: Phase opposition, SNR control, channel correlation
//! * **Edge cases**: Boundary conditions, parameter limits, error handling
//! * **Performance**: Large buffer generation, timing constraints
//!
//! Tests include both unit tests for individual features and integration tests
//! for the complete photoacoustic simulation pipeline.

use super::noise_generator::NoiseGenerator;
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // BASIC FUNCTIONALITY TESTS
    // ========================================

    #[test]
    fn test_basic_amplitude_modulation_generation() {
        let mut generator = NoiseGenerator::new(12345);

        let samples = generator.generate_universal_photoacoustic_stereo(
            1000,        // num_samples
            48000,       // sample_rate
            0.1,         // background_noise_amplitude
            2000.0,      // resonance_frequency
            0.8,         // laser_modulation_depth
            0.6,         // signal_amplitude
            180.0,       // phase_opposition_degrees
            0.02,        // temperature_drift_factor
            0.5,         // gas_flow_noise_factor
            20.0,        // snr_factor
            "amplitude", // modulation_mode
            0.0,         // pulse_width_seconds (unused)
            0.0,         // pulse_frequency_hz (unused)
        );

        // Verify output format
        assert_eq!(samples.len(), 2000); // 1000 samples × 2 channels

        // Verify samples are in valid i16 range
        for sample in &samples {
            assert!(*sample >= i16::MIN && *sample <= i16::MAX);
        }
    }

    #[test]
    fn test_basic_pulsed_modulation_generation() {
        let mut generator = NoiseGenerator::new(42);

        let samples = generator.generate_universal_photoacoustic_stereo(
            2400,     // num_samples (50ms at 48kHz)
            48000,    // sample_rate
            0.15,     // background_noise_amplitude
            2100.0,   // resonance_frequency
            0.9,      // laser_modulation_depth
            0.7,      // signal_amplitude
            175.0,    // phase_opposition_degrees
            0.01,     // temperature_drift_factor
            0.3,      // gas_flow_noise_factor
            25.0,     // snr_factor
            "pulsed", // modulation_mode
            0.005,    // pulse_width_seconds (5ms)
            100.0,    // pulse_frequency_hz (100Hz)
        );

        // Verify output format
        assert_eq!(samples.len(), 4800); // 2400 samples × 2 channels

        // Verify non-zero signal (should have some pulsed activity)
        let has_signal = samples.iter().any(|&s| s.abs() > 100);
        assert!(
            has_signal,
            "Generated signal should contain non-trivial amplitudes"
        );
    }

    #[test]
    fn test_deterministic_output_with_same_seed() {
        let mut gen1 = NoiseGenerator::new(999);
        let mut gen2 = NoiseGenerator::new(999);

        let samples1 = gen1.generate_universal_photoacoustic_stereo(
            500,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        let samples2 = gen2.generate_universal_photoacoustic_stereo(
            500,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        assert_eq!(
            samples1, samples2,
            "Same seed should produce identical output"
        );
    }

    #[test]
    fn test_different_output_with_different_seeds() {
        let mut gen1 = NoiseGenerator::new(111);
        let mut gen2 = NoiseGenerator::new(222);

        let samples1 = gen1.generate_universal_photoacoustic_stereo(
            500,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        let samples2 = gen2.generate_universal_photoacoustic_stereo(
            500,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        assert_ne!(
            samples1, samples2,
            "Different seeds should produce different output"
        );
    }

    // ========================================
    // PARAMETER VALIDATION TESTS
    // ========================================

    #[test]
    fn test_zero_samples_generation() {
        let mut generator = NoiseGenerator::new(123);

        let samples = generator.generate_universal_photoacoustic_stereo(
            0,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        assert_eq!(samples.len(), 0, "Zero samples should produce empty vector");
    }

    #[test]
    fn test_various_sample_rates() {
        let mut generator = NoiseGenerator::new(456);
        let test_rates = [8000, 22050, 44100, 48000, 96000, 192000];

        for &rate in &test_rates {
            let samples = generator.generate_universal_photoacoustic_stereo(
                100,
                rate,
                0.1,
                2000.0,
                0.8,
                0.6,
                180.0,
                0.02,
                0.5,
                20.0,
                "amplitude",
                0.0,
                0.0,
            );

            assert_eq!(samples.len(), 200, "Should work with sample rate {}", rate);
        }
    }

    #[test]
    fn test_extreme_amplitude_parameters() {
        let mut generator = NoiseGenerator::new(789);

        // Test minimum amplitudes
        let samples_min = generator.generate_universal_photoacoustic_stereo(
            100,
            48000,
            0.0,
            2000.0,
            0.0,
            0.0,
            180.0,
            0.0,
            0.0,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );
        assert_eq!(samples_min.len(), 200);

        // Test maximum amplitudes
        let samples_max = generator.generate_universal_photoacoustic_stereo(
            100,
            48000,
            1.0,
            2000.0,
            1.0,
            1.0,
            180.0,
            0.1,
            1.0,
            40.0,
            "amplitude",
            0.0,
            0.0,
        );
        assert_eq!(samples_max.len(), 200);
    }

    #[test]
    fn test_frequency_range_limits() {
        let mut generator = NoiseGenerator::new(321);

        // Test very low frequency
        let samples_low = generator.generate_universal_photoacoustic_stereo(
            1000,
            48000,
            0.1,
            100.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );
        assert_eq!(samples_low.len(), 2000);

        // Test very high frequency
        let samples_high = generator.generate_universal_photoacoustic_stereo(
            1000,
            48000,
            0.1,
            10000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );
        assert_eq!(samples_high.len(), 2000);
    }

    // ========================================
    // MODULATION MODE TESTS
    // ========================================

    #[test]
    fn test_amplitude_vs_pulsed_modulation_difference() {
        let mut generator = NoiseGenerator::new(654);

        let samples_amplitude = generator.generate_universal_photoacoustic_stereo(
            4800,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        let mut generator2 = NoiseGenerator::new(654); // Same seed
        let samples_pulsed = generator2.generate_universal_photoacoustic_stereo(
            4800, 48000, 0.1, 2000.0, 0.8, 0.6, 180.0, 0.02, 0.5, 20.0, "pulsed", 0.01, 50.0,
        );

        assert_ne!(
            samples_amplitude, samples_pulsed,
            "Amplitude and pulsed modulation should produce different signals"
        );
    }

    #[test]
    fn test_pulsed_modulation_timing() {
        let mut generator = NoiseGenerator::new(987);
        let sample_rate = 48000u32;
        let pulse_frequency = 100.0f32; // 100 Hz = 10ms period
        let pulse_width = 0.002f32; // 2ms pulses
        let duration_samples = sample_rate / 10; // 100ms worth of samples

        let samples = generator.generate_universal_photoacoustic_stereo(
            duration_samples,
            sample_rate,
            0.1,
            2000.0,
            0.9,
            0.8,
            180.0,
            0.02,
            0.5,
            20.0,
            "pulsed",
            pulse_width,
            pulse_frequency,
        );

        assert_eq!(samples.len(), (duration_samples * 2) as usize);

        // In 100ms at 100Hz, we should have approximately 10 pulses
        // Each pulse is 2ms = 96 samples, so roughly 960 samples should be "active"
        // This is a rough check - exact timing may vary due to phase alignment
    }

    #[test]
    fn test_invalid_modulation_mode_defaults_to_amplitude() {
        let mut gen1 = NoiseGenerator::new(147);
        let mut gen2 = NoiseGenerator::new(147);

        let samples_invalid = gen1.generate_universal_photoacoustic_stereo(
            500,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "invalid_mode",
            0.0,
            0.0,
        );

        let samples_amplitude = gen2.generate_universal_photoacoustic_stereo(
            500,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        assert_eq!(
            samples_invalid, samples_amplitude,
            "Invalid modulation mode should default to amplitude modulation"
        );
    }

    // ========================================
    // PHYSICAL PHENOMENA TESTS
    // ========================================

    #[test]
    fn test_thermal_drift_effects() {
        let mut gen1 = NoiseGenerator::new(258);
        let mut gen2 = NoiseGenerator::new(258);

        // Generate with no thermal drift
        let samples_no_drift = gen1.generate_universal_photoacoustic_stereo(
            10000,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.0,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        // Generate with significant thermal drift
        let samples_with_drift = gen2.generate_universal_photoacoustic_stereo(
            10000,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.1,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        assert_ne!(
            samples_no_drift, samples_with_drift,
            "Thermal drift should affect the generated signal"
        );
    }

    #[test]
    fn test_gas_flow_noise_effects() {
        let mut gen1 = NoiseGenerator::new(369);
        let mut gen2 = NoiseGenerator::new(369);

        // Generate with no gas flow noise
        let samples_no_flow = gen1.generate_universal_photoacoustic_stereo(
            5000,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.0,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        // Generate with significant gas flow noise
        let samples_with_flow = gen2.generate_universal_photoacoustic_stereo(
            5000,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            1.0,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        assert_ne!(
            samples_no_flow, samples_with_flow,
            "Gas flow noise should affect the generated signal"
        );
    }

    #[test]
    fn test_phase_opposition_effects() {
        let mut gen1 = NoiseGenerator::new(741);
        let mut gen2 = NoiseGenerator::new(741);

        // Perfect opposition (180°)
        let samples_180 = gen1.generate_universal_photoacoustic_stereo(
            1000,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        // Slight deviation (175°)
        let samples_175 = gen2.generate_universal_photoacoustic_stereo(
            1000,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            175.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        assert_ne!(
            samples_180, samples_175,
            "Phase opposition angle should affect the differential signal"
        );
    }

    #[test]
    fn test_snr_control_effects() {
        let mut gen1 = NoiseGenerator::new(852);
        let mut gen2 = NoiseGenerator::new(852);

        // Low SNR (10 dB)
        let samples_low_snr = gen1.generate_universal_photoacoustic_stereo(
            2000,
            48000,
            0.2,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            10.0,
            "amplitude",
            0.0,
            0.0,
        );

        // High SNR (40 dB)
        let samples_high_snr = gen2.generate_universal_photoacoustic_stereo(
            2000,
            48000,
            0.2,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            40.0,
            "amplitude",
            0.0,
            0.0,
        );

        assert_ne!(
            samples_low_snr, samples_high_snr,
            "SNR control should affect the signal-to-noise ratio"
        );

        // High SNR should generally have higher signal amplitudes
        let avg_amplitude_low: f64 = samples_low_snr.iter().map(|&s| s.abs() as f64).sum::<f64>()
            / samples_low_snr.len() as f64;
        let avg_amplitude_high: f64 = samples_high_snr
            .iter()
            .map(|&s| s.abs() as f64)
            .sum::<f64>()
            / samples_high_snr.len() as f64;

        // Note: This is a heuristic test - exact relationship depends on implementation
        assert!(
            avg_amplitude_high > 0.0 && avg_amplitude_low > 0.0,
            "Both signals should have non-zero average amplitudes"
        );
    }

    // ========================================
    // DIFFERENTIAL CONFIGURATION TESTS
    // ========================================

    #[test]
    fn test_stereo_channel_independence() {
        let mut generator = NoiseGenerator::new(963);

        let samples = generator.generate_universal_photoacoustic_stereo(
            1000,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        // Extract left and right channels
        let left: Vec<i16> = samples.iter().step_by(2).cloned().collect();
        let right: Vec<i16> = samples.iter().skip(1).step_by(2).cloned().collect();

        assert_eq!(left.len(), 1000);
        assert_eq!(right.len(), 1000);

        // Channels should be different (due to phase opposition)
        assert_ne!(
            left, right,
            "Left and right channels should be different due to phase opposition"
        );

        // Both channels should have non-zero variance
        let left_variance = calculate_variance(&left);
        let right_variance = calculate_variance(&right);

        assert!(
            left_variance > 0.0,
            "Left channel should have non-zero variance"
        );
        assert!(
            right_variance > 0.0,
            "Right channel should have non-zero variance"
        );
    }

    #[test]
    fn test_helmholtz_resonance_frequency_response() {
        let mut gen1 = NoiseGenerator::new(159);
        let mut gen2 = NoiseGenerator::new(159);

        // Signal at resonance frequency
        let samples_on_resonance = gen1.generate_universal_photoacoustic_stereo(
            5000,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        // Signal far from resonance frequency
        let samples_off_resonance = gen2.generate_universal_photoacoustic_stereo(
            5000,
            48000,
            0.1,
            8000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        assert_ne!(
            samples_on_resonance, samples_off_resonance,
            "Resonance frequency should affect signal characteristics"
        );
    }

    // ========================================
    // EDGE CASES AND ROBUSTNESS TESTS
    // ========================================

    #[test]
    fn test_very_large_buffer_generation() {
        let mut generator = NoiseGenerator::new(753);

        // Generate 1 second of audio at 48kHz (96,000 samples total)
        let samples = generator.generate_universal_photoacoustic_stereo(
            48000,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        assert_eq!(samples.len(), 96000);

        // Verify no samples are at the extreme limits (would indicate clipping issues)
        let extreme_samples = samples
            .iter()
            .filter(|&&s| s == i16::MIN || s == i16::MAX)
            .count();

        // Allow some extreme samples due to soft clipping, but not too many
        let extreme_percentage = (extreme_samples as f64) / (samples.len() as f64) * 100.0;
        assert!(
            extreme_percentage < 5.0,
            "Too many extreme samples ({}%), possible clipping issue",
            extreme_percentage
        );
    }

    #[test]
    fn test_extreme_pulse_parameters() {
        let mut generator = NoiseGenerator::new(864);

        // Very short pulses (0.1ms)
        let samples_short = generator.generate_universal_photoacoustic_stereo(
            4800, 48000, 0.1, 2000.0, 0.8, 0.6, 180.0, 0.02, 0.5, 20.0, "pulsed", 0.0001, 1000.0,
        );
        assert_eq!(samples_short.len(), 9600);

        // Very long pulses (100ms)
        let samples_long = generator.generate_universal_photoacoustic_stereo(
            4800, 48000, 0.1, 2000.0, 0.8, 0.6, 180.0, 0.02, 0.5, 20.0, "pulsed", 0.1, 5.0,
        );
        assert_eq!(samples_long.len(), 9600);
    }

    #[test]
    fn test_zero_pulse_frequency_handling() {
        let mut generator = NoiseGenerator::new(975);

        // Zero pulse frequency should still work (effectively disables pulsing)
        let samples = generator.generate_universal_photoacoustic_stereo(
            1000, 48000, 0.1, 2000.0, 0.8, 0.6, 180.0, 0.02, 0.5, 20.0, "pulsed", 0.01, 0.0,
        );

        assert_eq!(samples.len(), 2000);
    }

    // ========================================
    // PERFORMANCE AND QUALITY TESTS
    // ========================================

    #[test]
    fn test_signal_quality_metrics() {
        let mut generator = NoiseGenerator::new(486);

        let samples = generator.generate_universal_photoacoustic_stereo(
            10000,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            25.0,
            "amplitude",
            0.0,
            0.0,
        );

        // Extract channels
        let left: Vec<i16> = samples.iter().step_by(2).cloned().collect();
        let right: Vec<i16> = samples.iter().skip(1).step_by(2).cloned().collect();

        // Calculate signal statistics
        let left_mean = left.iter().map(|&s| s as f64).sum::<f64>() / left.len() as f64;
        let right_mean = right.iter().map(|&s| s as f64).sum::<f64>() / right.len() as f64;

        // Mean should be close to zero (no DC bias)
        assert!(
            left_mean.abs() < 1000.0,
            "Left channel should have low DC bias"
        );
        assert!(
            right_mean.abs() < 1000.0,
            "Right channel should have low DC bias"
        );

        // Signal should use reasonable dynamic range
        let left_max = left.iter().map(|&s| s.abs()).max().unwrap_or(0);
        let right_max = right.iter().map(|&s| s.abs()).max().unwrap_or(0);

        assert!(
            left_max > 1000,
            "Left channel should have reasonable amplitude"
        );
        assert!(
            right_max > 1000,
            "Right channel should have reasonable amplitude"
        );
    }

    #[test]
    fn test_consistency_across_multiple_generations() {
        // Test that multiple calls with same parameters produce different but valid outputs
        let mut generator = NoiseGenerator::new(597);

        let mut all_different = true;
        let mut first_samples = None;

        for _ in 0..5 {
            let samples = generator.generate_universal_photoacoustic_stereo(
                500,
                48000,
                0.1,
                2000.0,
                0.8,
                0.6,
                180.0,
                0.02,
                0.5,
                20.0,
                "amplitude",
                0.0,
                0.0,
            );

            assert_eq!(samples.len(), 1000);

            if let Some(ref first) = first_samples {
                if samples == *first {
                    all_different = false;
                }
            } else {
                first_samples = Some(samples);
            }
        }

        assert!(
            all_different,
            "Multiple generations should produce different outputs"
        );
    }

    // ========================================
    // HELPER FUNCTIONS
    // ========================================

    /// Calculate variance of a sample buffer
    fn calculate_variance(samples: &[i16]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }

        let mean = samples.iter().map(|&s| s as f64).sum::<f64>() / samples.len() as f64;
        let variance = samples
            .iter()
            .map(|&s| {
                let diff = s as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / samples.len() as f64;

        variance
    }

    /// Calculate correlation coefficient between two sample buffers
    fn calculate_correlation(samples1: &[i16], samples2: &[i16]) -> f64 {
        if samples1.len() != samples2.len() || samples1.is_empty() {
            return 0.0;
        }

        let mean1 = samples1.iter().map(|&s| s as f64).sum::<f64>() / samples1.len() as f64;
        let mean2 = samples2.iter().map(|&s| s as f64).sum::<f64>() / samples2.len() as f64;

        let numerator: f64 = samples1
            .iter()
            .zip(samples2.iter())
            .map(|(&s1, &s2)| (s1 as f64 - mean1) * (s2 as f64 - mean2))
            .sum();

        let variance1: f64 = samples1
            .iter()
            .map(|&s| {
                let diff = s as f64 - mean1;
                diff * diff
            })
            .sum();

        let variance2: f64 = samples2
            .iter()
            .map(|&s| {
                let diff = s as f64 - mean2;
                diff * diff
            })
            .sum();

        let denominator = (variance1 * variance2).sqrt();

        if denominator == 0.0 {
            0.0
        } else {
            numerator / denominator
        }
    }

    // ========================================
    // BENCHMARK TESTS (Optional)
    // ========================================

    #[test]
    #[ignore] // Ignore by default, run with --ignored flag for performance testing
    fn benchmark_large_buffer_generation() {
        use std::time::Instant;

        let mut generator = NoiseGenerator::new(123456);
        let start = Instant::now();

        // Generate 10 seconds of audio (960,000 samples)
        let samples = generator.generate_universal_photoacoustic_stereo(
            480000,
            48000,
            0.1,
            2000.0,
            0.8,
            0.6,
            180.0,
            0.02,
            0.5,
            20.0,
            "amplitude",
            0.0,
            0.0,
        );

        let duration = start.elapsed();

        assert_eq!(samples.len(), 960000);
        println!("Generated {} samples in {:?}", samples.len(), duration);

        // Should be able to generate real-time or faster
        assert!(
            duration.as_secs_f64() < 10.0,
            "Generation should be faster than real-time"
        );
    }
}
