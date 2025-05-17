// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use super::filters::{BandpassFilter, Filter};
use anyhow::Result;
use hound;
use std::fs;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to read a WAV file and return the samples as a Vec<f32>
    fn read_wav_file(path: &str) -> (Vec<f32>, u32) {
        let mut reader = hound::WavReader::open(path).expect("Failed to open WAV file");
        let spec = reader.spec();
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .map(|s| s.expect("Failed to read WAV sample") as f32 / 32768.0)
            .collect();

        (samples, spec.sample_rate)
    }

    // Helper function to save samples as a WAV file
    fn save_wav_file(samples: &[f32], sample_rate: u32, path: &str) -> Result<(), String> {
        // Create the directory if it doesn't exist
        if let Some(parent) = std::path::Path::new(path).parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;

        for &sample in samples {
            // Convert to i16 and clamp values to prevent overflow
            let amplitude = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            writer.write_sample(amplitude).map_err(|e| e.to_string())?;
        }

        writer.finalize().map_err(|e| e.to_string())?;
        println!("Saved WAV file to: {}", path);

        Ok(())
    }

    #[test]
    fn test_bandpass_filter_with_wav_file() {
        // Load the test WAV file
        let wav_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("16_48k_PerfectTest.wav");

        let (samples, sample_rate) = read_wav_file(wav_path.to_str().unwrap());

        // Create a bandpass filter with 2kHz center frequency and 100Hz bandwidth
        let filter = BandpassFilter::new(2000.0, 100.0).with_sample_rate(sample_rate);

        // Apply the filter to the samples
        let filtered_samples = filter.apply(&samples);

        // Ensure the filtered signal has the same length as the original
        assert_eq!(filtered_samples.len(), samples.len());

        // Verify that the filter attenuates frequencies outside the passband
        // This requires spectral analysis, but we can at least check that the signal
        // energy is preserved within reasonable bounds
        let original_energy: f32 = samples.iter().map(|&x| x * x).sum();
        let filtered_energy: f32 = filtered_samples.iter().map(|&x| x * x).sum();

        // The filtered energy should be less than the original energy
        // since we're removing frequencies outside the passband
        assert!(filtered_energy < original_energy);

        // The ratio should be within a reasonable range - this value may need adjustment
        // based on the actual content of the test file
        let energy_ratio = filtered_energy / original_energy;
        println!("Energy ratio (filtered/original): {}", energy_ratio);

        // This threshold is just an example and should be adjusted based on the
        // expected frequency content of the test file
        assert!(energy_ratio > 0.001, "Filter may be too aggressive");
        assert!(energy_ratio < 0.9, "Filter may not be effective");

        // Save the original and filtered signals to the out directory
        let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("out")
            .join("filters");

        // Save the original and filtered signals
        save_wav_file(
            &samples,
            sample_rate,
            out_dir.join("original_signal.wav").to_str().unwrap(),
        )
        .expect("Failed to save original signal");

        save_wav_file(
            &filtered_samples,
            sample_rate,
            out_dir
                .join("filtered_signal_2kHz_100Hz.wav")
                .to_str()
                .unwrap(),
        )
        .expect("Failed to save filtered signal");
    }

    #[test]
    fn test_bandpass_filter_frequency_response() {
        // Create test signals at different frequencies
        let sample_rate = 48000;
        let duration_seconds = 0.1;
        let num_samples = (sample_rate as f32 * duration_seconds) as usize;

        // Create a bandpass filter centered at 2kHz with 100Hz bandwidth
        let filter = BandpassFilter::new(2000.0, 100.0).with_sample_rate(sample_rate);

        // Test frequencies: one at the center, one at each edge of the passband,
        // and two outside the passband
        let test_frequencies = [
            1900.0, // Just below the passband
            1950.0, // Lower edge of the passband
            2000.0, // Center of the passband
            2050.0, // Upper edge of the passband
            2100.0, // Just above the passband
        ];

        let mut amplitudes = Vec::new();

        for &freq in &test_frequencies {
            // Generate a sine wave at the test frequency
            let test_signal: Vec<f32> = (0..num_samples)
                .map(|i| {
                    let t = i as f32 / sample_rate as f32;
                    (2.0 * std::f32::consts::PI * freq * t).sin()
                })
                .collect();

            // Apply the filter
            let filtered_signal = filter.apply(&test_signal);

            // Calculate RMS amplitude of the filtered signal
            let squared_sum: f32 = filtered_signal.iter().map(|&x| x * x).sum();
            let rms = (squared_sum / num_samples as f32).sqrt();

            amplitudes.push(rms);
        }

        // Print the frequency response for debugging
        println!("Frequency response:");
        for (i, &freq) in test_frequencies.iter().enumerate() {
            println!("{} Hz: {}", freq, amplitudes[i]);
        }

        // Verify that the center frequency has the highest amplitude
        assert!(
            amplitudes[2] > amplitudes[0],
            "Center frequency should be higher than below passband"
        );
        assert!(
            amplitudes[2] > amplitudes[4],
            "Center frequency should be higher than above passband"
        );

        // Check that the passband edges have at least 50% the amplitude of the center
        // (This is approximately a -6dB cutoff, which is sometimes used for bandwidth)
        assert!(
            amplitudes[1] >= amplitudes[2] * 0.5,
            "Lower edge should have at least 50% amplitude of center"
        );
        assert!(
            amplitudes[3] >= amplitudes[2] * 0.5,
            "Upper edge should have at least 50% amplitude of center"
        );

        // Check that frequencies outside the passband are significantly attenuated
        assert!(
            amplitudes[0] < amplitudes[2] * 0.5,
            "Below passband should be attenuated"
        );
        assert!(
            amplitudes[4] < amplitudes[2] * 0.5,
            "Above passband should be attenuated"
        );

        // Save test signals to the out directory for visual inspection
        let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("out")
            .join("filters")
            .join("frequency_response");

        for (i, &freq) in test_frequencies.iter().enumerate() {
            let test_signal: Vec<f32> = (0..num_samples)
                .map(|i| {
                    let t = i as f32 / sample_rate as f32;
                    (2.0 * std::f32::consts::PI * freq * t).sin()
                })
                .collect();

            // Apply the filter
            let filtered_signal = filter.apply(&test_signal);

            // Save both the original and filtered signals
            save_wav_file(
                &test_signal,
                sample_rate,
                out_dir
                    .join(format!("original_{}_Hz.wav", freq))
                    .to_str()
                    .unwrap(),
            )
            .expect(&format!("Failed to save original signal at {} Hz", freq));

            save_wav_file(
                &filtered_signal,
                sample_rate,
                out_dir
                    .join(format!("filtered_{}_Hz.wav", freq))
                    .to_str()
                    .unwrap(),
            )
            .expect(&format!("Failed to save filtered signal at {} Hz", freq));
        }
    }
}
