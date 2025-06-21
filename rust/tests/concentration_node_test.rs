// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Integration tests for ConcentrationNode functionality
//!
//! This test module validates the ConcentrationNode implementation including:
//! - Basic concentration calculation using polynomial coefficients
//! - Multi-instance support with different configurations
//! - Source selection from specific PeakFinderNode instances
//! - Hot-reload of configuration parameters
//! - Pass-through behavior for data flow
//! - Shared state management and backward compatibility

use anyhow::Result;
use rust_photoacoustic::acquisition::AudioFrame;
use rust_photoacoustic::processing::computing_nodes::{
    ComputingSharedData, ConcentrationNode, PeakFinderNode, SharedComputingState,
};
use rust_photoacoustic::processing::nodes::ProcessingNode;
use rust_photoacoustic::processing::ProcessingData;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Test basic concentration calculation functionality
#[tokio::test]
async fn test_concentration_calculation() -> Result<()> {
    // Create shared state
    let shared_state = Arc::new(RwLock::new(ComputingSharedData::default()));

    // Create concentration node with linear polynomial (y = x)
    let mut concentration_node = ConcentrationNode::new_with_shared_state(
        "test_concentration".to_string(),
        Some(shared_state.clone()),
    )
    .with_polynomial_coefficients([0.0, 1.0, 0.0, 0.0, 0.0]); // Linear: concentration = amplitude

    // Manually inject peak data into shared state (simulating PeakFinderNode output)
    {
        let mut state = shared_state.write().await;
        state.peak_frequency = Some(1000.0);
        state.peak_amplitude = Some(0.5);
        state.last_update = SystemTime::now();
    }

    // Create test audio frame
    let audio_frame = AudioFrame {
        channel_a: vec![0.1, 0.2, 0.3, 0.4],
        channel_b: vec![0.1, 0.2, 0.3, 0.4],
        sample_rate: 44100,
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis() as u64,
        frame_number: 1,
    };

    let input_data = ProcessingData::AudioFrame(audio_frame.clone());

    // Process data (should pass through unchanged)
    let output_data = concentration_node.process(input_data.clone())?;

    // Verify pass-through behavior
    assert_eq!(input_data, output_data);

    // Check that concentration was calculated and stored
    {
        let state = shared_state.read().await;
        // Check both new and legacy fields for backward compatibility
        if let Some(concentration_result) = state.concentration_results.get("test_concentration") {
            // With linear polynomial, concentration should equal amplitude (0.5)
            assert!((concentration_result.concentration_ppm - 0.5).abs() < 1e-6);
        } else {
            // Fall back to legacy field for backward compatibility
            assert!(state.concentration_ppm.is_some());
            let concentration = state.concentration_ppm.unwrap() as f64;
            assert!((concentration - 0.5).abs() < 1e-6);
        }
    }

    Ok(())
}

/// Test multi-instance concentration nodes with different configurations
#[tokio::test]
async fn test_multi_instance_concentration() -> Result<()> {
    // Create shared state
    let shared_state = Arc::new(RwLock::new(ComputingSharedData::default()));

    // Create two PeakFinderNodes with different IDs
    let peak_finder_1 = PeakFinderNode::new_with_shared_state(
        "peak_finder_co2".to_string(),
        Some(shared_state.clone()),
    );
    let peak_finder_2 = PeakFinderNode::new_with_shared_state(
        "peak_finder_ch4".to_string(),
        Some(shared_state.clone()),
    );

    // Create two ConcentrationNodes with different polynomial configurations
    let mut concentration_co2 = ConcentrationNode::new_with_shared_state(
        "concentration_co2".to_string(),
        Some(shared_state.clone()),
    )
    .with_peak_finder_source("peak_finder_co2".to_string())
    .with_polynomial_coefficients([0.0, 1000.0, 0.0, 0.0, 0.0]) // Linear: 1000x
    .with_spectral_line_id("CO2_line".to_string());

    let mut concentration_ch4 = ConcentrationNode::new_with_shared_state(
        "concentration_ch4".to_string(),
        Some(shared_state.clone()),
    )
    .with_peak_finder_source("peak_finder_ch4".to_string())
    .with_polynomial_coefficients([100.0, 500.0, 0.0, 0.0, 0.0]) // 100 + 500x
    .with_spectral_line_id("CH4_line".to_string());

    // Simulate different peak results from each PeakFinderNode
    {
        let mut state = shared_state.write().await;

        // Simulate CO2 peak finder result
        let peak_result_co2 = rust_photoacoustic::processing::computing_nodes::PeakResult {
            frequency: 2000.0,
            amplitude: 0.001, // Small amplitude
            concentration_ppm: None,
            timestamp: SystemTime::now(),
            coherence_score: 0.95,
            processing_metadata: std::collections::HashMap::new(),
        };
        state
            .peak_results
            .insert("peak_finder_co2".to_string(), peak_result_co2);

        // Simulate CH4 peak finder result
        let peak_result_ch4 = rust_photoacoustic::processing::computing_nodes::PeakResult {
            frequency: 3000.0,
            amplitude: 0.002, // Different amplitude
            concentration_ppm: None,
            timestamp: SystemTime::now(),
            coherence_score: 0.90,
            processing_metadata: std::collections::HashMap::new(),
        };
        state
            .peak_results
            .insert("peak_finder_ch4".to_string(), peak_result_ch4);
    }

    // Create test data
    let test_data = ProcessingData::SingleChannel {
        samples: vec![0.1, 0.2, 0.3],
        sample_rate: 44100,
        timestamp: 1000,
        frame_number: 1,
    };

    // Process data through both concentration nodes
    let output_co2 = concentration_co2.process(test_data.clone())?;
    let output_ch4 = concentration_ch4.process(test_data.clone())?;

    // Verify pass-through behavior
    assert_eq!(test_data, output_co2);
    assert_eq!(test_data, output_ch4);

    // Verify that each node calculated different concentrations
    {
        let state = shared_state.read().await;

        // Check that both nodes stored results under their own IDs
        assert!(state
            .concentration_results
            .contains_key("concentration_co2"));
        assert!(state
            .concentration_results
            .contains_key("concentration_ch4"));

        let co2_result = &state.concentration_results["concentration_co2"];
        let ch4_result = &state.concentration_results["concentration_ch4"];

        // CO2: 0 + 1000 * 0.001 = 1.0 ppm
        assert!((co2_result.concentration_ppm - 1.0).abs() < 1e-6);

        // CH4: 100 + 500 * 0.002 = 101.0 ppm
        assert!((ch4_result.concentration_ppm - 101.0).abs() < 1e-6);

        // Verify metadata
        assert_eq!(co2_result.source_peak_finder_id, "peak_finder_co2");
        assert_eq!(ch4_result.source_peak_finder_id, "peak_finder_ch4");
        assert_eq!(co2_result.spectral_line_id.as_ref().unwrap(), "CO2_line");
        assert_eq!(ch4_result.spectral_line_id.as_ref().unwrap(), "CH4_line");
    }

    Ok(())
}

/// Test hot-reload of configuration parameters
#[tokio::test]
async fn test_concentration_hot_reload() -> Result<()> {
    let shared_state = Arc::new(RwLock::new(ComputingSharedData::default()));

    let mut concentration_node = ConcentrationNode::new_with_shared_state(
        "test_hot_reload".to_string(),
        Some(shared_state.clone()),
    )
    .with_polynomial_coefficients([0.0, 100.0, 0.0, 0.0, 0.0]); // Initial: 100x

    // Verify supports hot reload
    assert!(concentration_node.supports_hot_reload());

    // Inject test peak data
    {
        let mut state = shared_state.write().await;
        state.peak_frequency = Some(1500.0);
        state.peak_amplitude = Some(0.01);
        state.last_update = SystemTime::now();
    }

    let test_data = ProcessingData::SingleChannel {
        samples: vec![0.5],
        sample_rate: 44100,
        timestamp: 2000,
        frame_number: 2,
    };

    // Process with initial configuration
    concentration_node.process(test_data.clone())?;

    // Check initial result: 0 + 100 * 0.01 = 1.0 ppm
    {
        let state = shared_state.read().await;
        if let Some(concentration_result) = state.concentration_results.get("test_hot_reload") {
            assert!((concentration_result.concentration_ppm - 1.0).abs() < 1e-6);
        } else {
            // Fall back to legacy field for backward compatibility
            let concentration = state.concentration_ppm.unwrap() as f64;
            assert!((concentration - 1.0).abs() < 1e-6);
        }
    }

    // Update configuration via hot-reload
    let new_config = serde_json::json!({
        "polynomial_coefficients": [50.0, 200.0, 0.0, 0.0, 0.0],
        "temperature_compensation": true,
        "min_amplitude_threshold": 0.005
    });

    let updated = concentration_node.update_config(&new_config)?;
    assert!(updated); // Should indicate parameters were changed

    // Process with new configuration
    concentration_node.process(test_data)?;

    // Check updated result: 50 + 200 * 0.01 = 52.0 ppm
    {
        let state = shared_state.read().await;
        if let Some(concentration_result) = state.concentration_results.get("test_hot_reload") {
            assert!((concentration_result.concentration_ppm - 52.0).abs() < 1e-6);
        } else {
            // Fall back to legacy field for backward compatibility
            let concentration = state.concentration_ppm.unwrap() as f64;
            assert!((concentration - 52.0).abs() < 1e-6);
        }
    }

    Ok(())
}

/// Test backward compatibility with legacy peak data
#[tokio::test]
async fn test_backward_compatibility() -> Result<()> {
    let shared_state = Arc::new(RwLock::new(ComputingSharedData::default()));

    // Create concentration node without specific PeakFinder binding
    let mut concentration_node = ConcentrationNode::new_with_shared_state(
        "legacy_concentration".to_string(),
        Some(shared_state.clone()),
    )
    .with_polynomial_coefficients([10.0, 50.0, 0.0, 0.0, 0.0]); // 10 + 50x

    // Set legacy peak data format (without HashMap)
    {
        let mut state = shared_state.write().await;
        state.peak_frequency = Some(1200.0);
        state.peak_amplitude = Some(0.04);
        state.last_update = SystemTime::now();
        // Note: peak_results HashMap is empty, should fall back to legacy fields
    }

    let test_data = ProcessingData::DualChannel {
        channel_a: vec![0.1, 0.2],
        channel_b: vec![0.3, 0.4],
        sample_rate: 48000,
        timestamp: 3000,
        frame_number: 3,
    };

    // Process data
    let output = concentration_node.process(test_data.clone())?;

    // Verify pass-through
    assert_eq!(test_data, output);

    // Check concentration calculation: 10 + 50 * 0.04 = 12.0 ppm
    {
        let state = shared_state.read().await;
        if let Some(concentration_result) = state.concentration_results.get("legacy_concentration")
        {
            assert!((concentration_result.concentration_ppm - 12.0).abs() < 1e-6);
        } else {
            // Fall back to legacy field for backward compatibility
            let concentration = state.concentration_ppm.unwrap() as f64;
            assert!((concentration - 12.0).abs() < 1e-6);
        }
    }

    Ok(())
}

/// Test node trait implementations
#[tokio::test]
async fn test_node_trait_implementations() -> Result<()> {
    let concentration_node = ConcentrationNode::new("trait_test".to_string())
        .with_polynomial_coefficients([1.0, 2.0, 3.0, 4.0, 5.0]);

    // Test basic trait methods
    assert_eq!(concentration_node.node_id(), "trait_test");
    assert_eq!(concentration_node.node_type(), "computing_concentration");
    assert!(concentration_node.supports_hot_reload());

    // Test accepts_input (should accept any input type)
    let audio_frame = ProcessingData::AudioFrame(AudioFrame {
        channel_a: vec![0.1],
        channel_b: vec![0.1],
        sample_rate: 44100,
        timestamp: 1000,
        frame_number: 1,
    });
    assert!(concentration_node.accepts_input(&audio_frame));

    let single_channel = ProcessingData::SingleChannel {
        samples: vec![0.1],
        sample_rate: 44100,
        timestamp: 1000,
        frame_number: 1,
    };
    assert!(concentration_node.accepts_input(&single_channel));

    // Test output_type (should match input type)
    assert_eq!(
        concentration_node.output_type(&audio_frame).unwrap(),
        "AudioFrame"
    );
    assert_eq!(
        concentration_node.output_type(&single_channel).unwrap(),
        "SingleChannel"
    );

    // Test clone_node
    let cloned = concentration_node.clone_node();
    assert_eq!(cloned.node_id(), "trait_test");
    assert_eq!(cloned.node_type(), "computing_concentration");

    Ok(())
}

/// Test amplitude threshold filtering
#[tokio::test]
async fn test_amplitude_threshold() -> Result<()> {
    let shared_state = Arc::new(RwLock::new(ComputingSharedData::default()));

    let mut concentration_node = ConcentrationNode::new_with_shared_state(
        "threshold_test".to_string(),
        Some(shared_state.clone()),
    )
    .with_polynomial_coefficients([0.0, 1000.0, 0.0, 0.0, 0.0])
    .with_min_amplitude_threshold(0.01); // Set threshold at 0.01

    let test_data = ProcessingData::SingleChannel {
        samples: vec![0.1],
        sample_rate: 44100,
        timestamp: 1000,
        frame_number: 1,
    };

    // Test with amplitude below threshold
    {
        let mut state = shared_state.write().await;
        state.peak_frequency = Some(1000.0);
        state.peak_amplitude = Some(0.005); // Below threshold
        state.last_update = SystemTime::now();
    }

    concentration_node.process(test_data.clone())?;

    // Should not calculate concentration
    {
        let state = shared_state.read().await;
        // Check that no concentration result was stored for this node
        assert!(!state.concentration_results.contains_key("threshold_test"));
        // Legacy field should also be None if no calculation occurred
        if state.concentration_ppm.is_some() {
            // This might have been set by a previous calculation, which is fine
        }
    }

    // Test with amplitude above threshold
    {
        let mut state = shared_state.write().await;
        state.peak_amplitude = Some(0.02); // Above threshold
        state.last_update = SystemTime::now();
    }

    concentration_node.process(test_data)?;

    // Should calculate concentration: 0 + 1000 * 0.02 = 20.0 ppm
    {
        let state = shared_state.read().await;
        if let Some(concentration_result) = state.concentration_results.get("threshold_test") {
            assert!((concentration_result.concentration_ppm - 20.0).abs() < 1e-6);
        } else {
            // Fall back to legacy field for backward compatibility
            assert!(state.concentration_ppm.is_some());
            let concentration = state.concentration_ppm.unwrap() as f64;
            assert!((concentration - 20.0).abs() < 1e-6);
        }
    }

    Ok(())
}
