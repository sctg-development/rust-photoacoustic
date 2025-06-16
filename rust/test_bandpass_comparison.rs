//! Test to compare the corrected IIR bandpass filter implementation
//!
//! This test demonstrates the improvements made to the BandpassFilter IIR implementation

#[cfg(test)]
mod tests {
    use rust_photoacoustic::preprocessing::filters::{BandpassFilter, Filter};
    use std::f32::consts::PI;

    #[test]
    fn test_corrected_iir_bandpass_implementation() {
        println!("Testing corrected IIR BandpassFilter implementation");

        // Create a test signal with multiple frequency components
        let sample_rate = 48000.0;
        let duration = 0.1; // 100ms
        let num_samples = (duration * sample_rate) as usize;

        let mut test_signal = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate;

            // Mix of frequencies:
            // 500Hz (should be attenuated)
            // 1000Hz (should pass through - center frequency)
            // 2000Hz (should be attenuated)
            // 5000Hz (should be heavily attenuated)
            let sample = 0.5 * (2.0 * PI * 500.0 * t).sin() +   // Low frequency
                1.0 * (2.0 * PI * 1000.0 * t).sin() +  // Center frequency (target)
                0.5 * (2.0 * PI * 2000.0 * t).sin() +  // High frequency
                0.2 * (2.0 * PI * 5000.0 * t).sin(); // Very high frequency

            test_signal.push(sample);
        }

        // Test different filter orders
        let orders = vec![2, 4, 6];

        for order in orders {
            println!(
                "\nTesting {}th-order bandpass filter (center: 1000Hz, bandwidth: 400Hz)",
                order
            );

            let filter = BandpassFilter::new(1000.0, 400.0)
                .with_sample_rate(48000)
                .with_order(order);

            let filtered = filter.apply(&test_signal);

            // Verify that we have proper filtering
            assert_eq!(filtered.len(), test_signal.len());

            // Calculate RMS for different frequency bands to verify filtering
            let original_rms = calculate_rms(&test_signal);
            let filtered_rms = calculate_rms(&filtered);

            println!("  Original signal RMS: {:.6}", original_rms);
            println!("  Filtered signal RMS: {:.6}", filtered_rms);

            // Test frequency response at specific frequencies
            test_frequency_response(&filter, 500.0, sample_rate as u32);
            test_frequency_response(&filter, 1000.0, sample_rate as u32);
            test_frequency_response(&filter, 2000.0, sample_rate as u32);
            test_frequency_response(&filter, 5000.0, sample_rate as u32);

            // The filtered signal should have content (not all zeros)
            assert!(filtered_rms > 0.01, "Filter output should not be zero");

            // Verify no NaN or infinite values
            assert!(
                filtered.iter().all(|&x| x.is_finite()),
                "All output values should be finite"
            );
        }

        println!("\nTesting with configuration updates (hot-reload capability):");

        let filter = BandpassFilter::new(1000.0, 200.0);

        // Test initial configuration
        let result1 = filter.apply(&test_signal);
        let rms1 = calculate_rms(&result1);

        // Update center frequency to 2000Hz
        let update_result = filter.update_config(&serde_json::json!({
            "center_freq": 2000.0,
            "bandwidth": 300.0
        }));

        if update_result.is_ok() && update_result.unwrap() {
            println!("Successfully updated filter configuration");

            let result2 = filter.apply(&test_signal);
            let rms2 = calculate_rms(&result2);

            println!("RMS before config change: {:.6}", rms1);
            println!("RMS after config change:  {:.6}", rms2);

            // Both results should be valid
            assert!(rms1 > 0.0);
            assert!(rms2 > 0.0);
        }

        println!("\n✅ IIR BandpassFilter implementation test completed successfully!");
    }

    fn calculate_rms(signal: &[f32]) -> f32 {
        let sum_squares: f32 = signal.iter().map(|&x| x * x).sum();
        (sum_squares / signal.len() as f32).sqrt()
    }

    fn test_frequency_response(filter: &BandpassFilter, freq: f32, sample_rate: u32) {
        let duration = 0.05; // 50ms
        let num_samples = ((duration * sample_rate as f32) as usize).max(1024);

        // Generate pure sine wave at test frequency
        let mut test_tone = Vec::with_capacity(num_samples);
        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            test_tone.push((2.0 * PI * freq * t).sin());
        }

        let input_rms = calculate_rms(&test_tone);
        let filtered_tone = filter.apply(&test_tone);
        let output_rms = calculate_rms(&filtered_tone);

        let gain_db = if input_rms > 0.0 {
            20.0 * (output_rms / input_rms).log10()
        } else {
            -100.0
        };

        println!("  {}Hz: {:.2} dB gain", freq, gain_db);
    }
}
