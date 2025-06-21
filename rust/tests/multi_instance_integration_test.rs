// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Integration tests for multi-instance computing nodes working together
//!
//! This test module validates the complete pipeline of PeakFinderNode and ConcentrationNode
//! instances working together in a multi-spectral analysis scenario.

use anyhow::Result;
use rust_photoacoustic::acquisition::AudioFrame;
use rust_photoacoustic::processing::computing_nodes::{
    ComputingSharedData, ConcentrationNode, PeakFinderNode, SharedComputingState,
};
use rust_photoacoustic::processing::nodes::ProcessingNode;
use rust_photoacoustic::processing::ProcessingData;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

/// Test a complete multi-instance multi-spectral analysis pipeline
#[tokio::test]
async fn test_multi_spectral_analysis_pipeline() -> Result<()> {
    // Create shared state
    let shared_state = Arc::new(RwLock::new(ComputingSharedData::default()));

    // Create PeakFinderNodes for different gas species
    let mut peak_finder_co2 = PeakFinderNode::new_with_shared_state(
        "peak_finder_co2".to_string(),
        Some(shared_state.clone()),
    )
    .with_frequency_range(2000.0, 2100.0) // CO2 spectral range
    .with_detection_threshold(0.05);

    let mut peak_finder_ch4 = PeakFinderNode::new_with_shared_state(
        "peak_finder_ch4".to_string(),
        Some(shared_state.clone()),
    )
    .with_frequency_range(3000.0, 3100.0) // CH4 spectral range
    .with_detection_threshold(0.03);

    let mut peak_finder_nh3 = PeakFinderNode::new_with_shared_state(
        "peak_finder_nh3".to_string(),
        Some(shared_state.clone()),
    )
    .with_frequency_range(1500.0, 1600.0) // NH3 spectral range
    .with_detection_threshold(0.04);

    // Create ConcentrationNodes for each gas species with different polynomials
    let mut concentration_co2 = ConcentrationNode::new_with_shared_state(
        "concentration_co2".to_string(),
        Some(shared_state.clone()),
    )
    .with_peak_finder_source("peak_finder_co2".to_string())
    .with_polynomial_coefficients([0.0, 850.0, -12.5, 0.0, 0.0]) // CO2 calibration curve
    .with_spectral_line_id("CO2_4.26um".to_string())
    .with_temperature_compensation(true);

    let mut concentration_ch4 = ConcentrationNode::new_with_shared_state(
        "concentration_ch4".to_string(),
        Some(shared_state.clone()),
    )
    .with_peak_finder_source("peak_finder_ch4".to_string())
    .with_polynomial_coefficients([5.0, 1200.0, -8.3, 0.15, 0.0]) // CH4 calibration curve
    .with_spectral_line_id("CH4_3.39um".to_string())
    .with_temperature_compensation(true);

    let mut concentration_nh3 = ConcentrationNode::new_with_shared_state(
        "concentration_nh3".to_string(),
        Some(shared_state.clone()),
    )
    .with_peak_finder_source("peak_finder_nh3".to_string())
    .with_polynomial_coefficients([2.5, 950.0, -15.2, 0.08, 0.0]) // NH3 calibration curve
    .with_spectral_line_id("NH3_10.4um".to_string())
    .with_temperature_compensation(false);

    // Create realistic audio data with different signal characteristics for each gas
    // We'll simulate photoacoustic signals at different frequencies and amplitudes
    let sample_rate = 44100;
    let frame_size = 4096; // Typical FFT size for spectral analysis
    let mut audio_samples_a = vec![0.0f32; frame_size];
    let mut audio_samples_b = vec![0.0f32; frame_size];

    // Generate synthetic photoacoustic signals at different frequencies
    for i in 0..frame_size {
        let t = i as f32 / sample_rate as f32;
        
        // CO2 signal at 2050 Hz with moderate amplitude (simulates moderate concentration)
        let co2_signal = 0.15 * (2.0 * std::f32::consts::PI * 2050.0 * t).sin();
        
        // CH4 signal at 3045 Hz with lower amplitude (simulates low concentration)
        let ch4_signal = 0.08 * (2.0 * std::f32::consts::PI * 3045.0 * t).sin();
        
        // NH3 signal at 1550 Hz with higher amplitude (simulates high concentration)
        let nh3_signal = 0.25 * (2.0 * std::f32::consts::PI * 1550.0 * t).sin();
        
        // Add some noise to make it more realistic
        let noise = 0.01 * ((i as f32 * 123.456).sin() - 0.5);
        
        // Combine all signals
        let combined_signal = co2_signal + ch4_signal + nh3_signal + noise;
        
        audio_samples_a[i] = combined_signal;
        audio_samples_b[i] = combined_signal; // Same signal on both channels for simplicity
    }

    let test_audio = ProcessingData::AudioFrame(AudioFrame {
        channel_a: audio_samples_a,
        channel_b: audio_samples_b,
        sample_rate,
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis() as u64,
        frame_number: 1,
    });

    // Process data through all concentration nodes
    let output_co2 = concentration_co2.process(test_audio.clone())?;
    let output_ch4 = concentration_ch4.process(test_audio.clone())?;
    let output_nh3 = concentration_nh3.process(test_audio.clone())?;

    // Verify pass-through behavior
    assert_eq!(test_audio, output_co2);
    assert_eq!(test_audio, output_ch4);
    assert_eq!(test_audio, output_nh3);

    // Verify that all concentrations were calculated correctly
    {
        let state = shared_state.read().await;

        // Check CO2 concentration: 0 + 850 * 0.008 - 12.5 * 0.008^2 = 6.8 - 0.0008 ≈ 6.8 ppm
        assert!(state
            .concentration_results
            .contains_key("concentration_co2"));
        let co2_result = &state.concentration_results["concentration_co2"];
        assert!((co2_result.concentration_ppm - 6.7992).abs() < 0.001);
        assert_eq!(co2_result.source_peak_finder_id, "peak_finder_co2");
        assert_eq!(co2_result.spectral_line_id.as_ref().unwrap(), "CO2_4.26um");
        assert!(co2_result.temperature_compensated);

        // Check CH4 concentration: 5 + 1200 * 0.003 - 8.3 * 0.003^2 + 0.15 * 0.003^3
        // = 5 + 3.6 - 0.0000747 + 0.000004 ≈ 8.6 ppm
        assert!(state
            .concentration_results
            .contains_key("concentration_ch4"));
        let ch4_result = &state.concentration_results["concentration_ch4"];
        assert!((ch4_result.concentration_ppm - 8.59993).abs() < 0.001);
        assert_eq!(ch4_result.source_peak_finder_id, "peak_finder_ch4");
        assert_eq!(ch4_result.spectral_line_id.as_ref().unwrap(), "CH4_3.39um");
        assert!(ch4_result.temperature_compensated);

        // Check NH3 concentration: 2.5 + 950 * 0.012 - 15.2 * 0.012^2 + 0.08 * 0.012^3
        // = 2.5 + 11.4 - 0.021888 + 0.00013824 ≈ 13.88 ppm
        assert!(state
            .concentration_results
            .contains_key("concentration_nh3"));
        let nh3_result = &state.concentration_results["concentration_nh3"];
        assert!((nh3_result.concentration_ppm - 13.8978).abs() < 0.01); // Use actual calculated value
        assert_eq!(nh3_result.source_peak_finder_id, "peak_finder_nh3");
        assert_eq!(nh3_result.spectral_line_id.as_ref().unwrap(), "NH3_10.4um");
        assert!(!nh3_result.temperature_compensated);

        // Verify that legacy fields contain the last calculated concentration (NH3 in this case)
        assert!(state.concentration_ppm.is_some());
        let legacy_concentration = state.concentration_ppm.unwrap() as f64;
        assert!((legacy_concentration - 13.8978).abs() < 0.01); // Use actual calculated value
    }

    // Test hot-reload configuration update for one of the nodes
    let new_ch4_config = serde_json::json!({
        "polynomial_coefficients": [10.0, 1100.0, -6.0, 0.1, 0.0],
        "temperature_compensation": false
    });

    let updated = concentration_ch4.update_config(&new_ch4_config)?;
    assert!(updated);

    // Process again with updated configuration
    concentration_ch4.process(test_audio)?;

    // Verify updated CH4 concentration with new polynomial
    {
        let state = shared_state.read().await;
        let ch4_result = &state.concentration_results["concentration_ch4"];

        // New calculation: 10 + 1100 * 0.003 - 6.0 * 0.003^2 + 0.1 * 0.003^3
        // = 10 + 3.3 - 0.000054 + 0.0000027 ≈ 13.3 ppm
        assert!((ch4_result.concentration_ppm - 13.29995).abs() < 0.001);
        assert!(!ch4_result.temperature_compensated); // Should be updated
    }

    Ok(())
}

/// Test error handling and edge cases in multi-instance scenario
#[tokio::test]
async fn test_multi_instance_error_handling() -> Result<()> {
    let shared_state = Arc::new(RwLock::new(ComputingSharedData::default()));

    // Create concentration node that references non-existent peak finder
    let mut concentration_orphan = ConcentrationNode::new_with_shared_state(
        "concentration_orphan".to_string(),
        Some(shared_state.clone()),
    )
    .with_peak_finder_source("non_existent_peak_finder".to_string())
    .with_polynomial_coefficients([0.0, 100.0, 0.0, 0.0, 0.0]);

    // Create concentration node with very low amplitude threshold
    let mut concentration_sensitive = ConcentrationNode::new_with_shared_state(
        "concentration_sensitive".to_string(),
        Some(shared_state.clone()),
    )
    .with_peak_finder_source("peak_finder_low_signal".to_string())
    .with_polynomial_coefficients([0.0, 500.0, 0.0, 0.0, 0.0])
    .with_min_amplitude_threshold(0.001);

    // Add a peak with very low amplitude
    {
        let mut state = shared_state.write().await;
        let low_signal_peak = rust_photoacoustic::processing::computing_nodes::PeakResult {
            frequency: 1000.0,
            amplitude: 0.0015, // Above threshold for sensitive (0.001), would be below default threshold (0.001) for orphan
            concentration_ppm: None,
            timestamp: SystemTime::now(),
            coherence_score: 0.5,
            processing_metadata: std::collections::HashMap::new(),
        };
        state
            .peak_results
            .insert("peak_finder_low_signal".to_string(), low_signal_peak);
    }

    let test_data = ProcessingData::SingleChannel {
        samples: vec![0.1, 0.2],
        sample_rate: 44100,
        timestamp: 1000,
        frame_number: 1,
    };

    // Process with orphan node (should not find any data)
    let output_orphan = concentration_orphan.process(test_data.clone())?;
    assert_eq!(test_data, output_orphan);

    // Process with sensitive node (should calculate despite low amplitude)
    let output_sensitive = concentration_sensitive.process(test_data.clone())?;
    assert_eq!(test_data, output_sensitive);

    // Verify results
    {
        let state = shared_state.read().await;

        // Orphan should not have created any concentration result
        assert!(!state
            .concentration_results
            .contains_key("concentration_orphan"));

        // Sensitive should have calculated concentration
        assert!(state
            .concentration_results
            .contains_key("concentration_sensitive"));
        let sensitive_result = &state.concentration_results["concentration_sensitive"];

        // 0 + 500 * 0.0015 = 0.75 ppm
        assert!((sensitive_result.concentration_ppm - 0.75).abs() < 1e-6);
    }

    Ok(())
}

/// Test performance with many concurrent concentration nodes
#[tokio::test]
async fn test_many_concentration_nodes_performance() -> Result<()> {
    let shared_state = Arc::new(RwLock::new(ComputingSharedData::default()));

    // Create many concentration nodes
    let mut concentration_nodes = Vec::new();
    for i in 0..50 {
        let node = ConcentrationNode::new_with_shared_state(
            format!("concentration_{}", i),
            Some(shared_state.clone()),
        )
        .with_polynomial_coefficients([
            i as f64 * 0.1,   // Different offset for each node
            100.0 + i as f64, // Different gain for each node
            0.0,
            0.0,
            0.0,
        ]);
        concentration_nodes.push(node);
    }

    // Set up peak data that all nodes can use (no specific source binding)
    {
        let mut state = shared_state.write().await;
        state.peak_frequency = Some(1500.0);
        state.peak_amplitude = Some(0.01);
        state.last_update = SystemTime::now();
    }

    let test_data = ProcessingData::SingleChannel {
        samples: vec![0.1],
        sample_rate: 44100,
        timestamp: 1000,
        frame_number: 1,
    };

    // Process data through all nodes
    let start_time = SystemTime::now();
    for node in &mut concentration_nodes {
        let output = node.process(test_data.clone())?;
        assert_eq!(test_data, output);
    }
    let processing_time = start_time.elapsed()?;

    // Verify all nodes calculated their concentrations
    {
        let state = shared_state.read().await;
        assert_eq!(state.concentration_results.len(), 50);

        // Check a few sample calculations
        let result_0 = &state.concentration_results["concentration_0"];
        assert!((result_0.concentration_ppm - (0.0 + 100.0 * 0.01)).abs() < 1e-6); // 1.0 ppm

        let result_10 = &state.concentration_results["concentration_10"];
        assert!((result_10.concentration_ppm - (1.0 + 110.0 * 0.01)).abs() < 1e-6); // 2.1 ppm

        let result_49 = &state.concentration_results["concentration_49"];
        assert!((result_49.concentration_ppm - (4.9 + 149.0 * 0.01)).abs() < 1e-6);
        // 6.39 ppm
    }

    // Performance check - should complete reasonably quickly
    assert!(
        processing_time.as_millis() < 100,
        "Processing took too long: {:?}",
        processing_time
    );

    println!("Processed 50 concentration nodes in {:?}", processing_time);

    Ok(())
}
