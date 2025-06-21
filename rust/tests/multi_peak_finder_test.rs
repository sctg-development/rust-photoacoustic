// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Tests for multi-peak-finder functionality
//!
//! This module tests the refactored ComputingSharedData structure that supports
//! multiple PeakFinderNode instances storing their results independently.

use anyhow::Result;
use rust_photoacoustic::processing::computing_nodes::{
    ComputingSharedData, PeakFinderNode, PeakResult, SharedComputingState,
};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_multi_peak_finder_shared_data() -> Result<()> {
    // Create shared computing state
    let shared_state: SharedComputingState = Arc::new(RwLock::new(ComputingSharedData::default()));

    // Create two peak finder nodes sharing the same state
    let mut peak_finder_1 = PeakFinderNode::new_with_shared_state(
        "peak_finder_1".to_string(),
        Some(shared_state.clone()),
    );
    let mut peak_finder_2 = PeakFinderNode::new_with_shared_state(
        "peak_finder_2".to_string(),
        Some(shared_state.clone()),
    );

    // Simulate peak detection updates from both nodes
    {
        let mut state = shared_state.write().await;

        // Update from first peak finder
        let result_1 = PeakResult {
            frequency: 1000.0,
            amplitude: 0.8,
            concentration_ppm: Some(42.5),
            timestamp: SystemTime::now(),
            coherence_score: 0.95,
            processing_metadata: std::collections::HashMap::new(),
        };
        state.update_peak_result("peak_finder_1".to_string(), result_1);

        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Update from second peak finder
        let result_2 = PeakResult {
            frequency: 1200.0,
            amplitude: 0.9,
            concentration_ppm: Some(38.2),
            timestamp: SystemTime::now(),
            coherence_score: 0.92,
            processing_metadata: std::collections::HashMap::new(),
        };
        state.update_peak_result("peak_finder_2".to_string(), result_2);
    }

    // Verify that both results are stored independently
    {
        let state = shared_state.read().await;

        // Check that both nodes have their results
        assert!(state.peak_results.contains_key("peak_finder_1"));
        assert!(state.peak_results.contains_key("peak_finder_2"));

        let result_1 = state.get_peak_result("peak_finder_1").unwrap();
        assert_eq!(result_1.frequency, 1000.0);
        assert_eq!(result_1.amplitude, 0.8);
        assert_eq!(result_1.concentration_ppm, Some(42.5));

        let result_2 = state.get_peak_result("peak_finder_2").unwrap();
        assert_eq!(result_2.frequency, 1200.0);
        assert_eq!(result_2.amplitude, 0.9);
        assert_eq!(result_2.concentration_ppm, Some(38.2));

        // Check legacy fields (should contain the most recent update)
        assert_eq!(state.peak_frequency, Some(1200.0));
        assert_eq!(state.peak_amplitude, Some(0.9));
        assert_eq!(state.concentration_ppm, Some(38.2));

        // Check utility methods
        let node_ids = state.get_peak_finder_node_ids();
        assert!(node_ids.contains(&"peak_finder_1".to_string()));
        assert!(node_ids.contains(&"peak_finder_2".to_string()));

        let latest_result = state.get_latest_peak_result().unwrap();
        assert_eq!(latest_result.frequency, 1200.0); // Should be the most recent

        // Check recent data detection
        assert!(state.has_recent_peak_data("peak_finder_1"));
        assert!(state.has_recent_peak_data("peak_finder_2"));
    }

    println!("✓ Multi-peak-finder functionality test passed");
    Ok(())
}

#[tokio::test]
async fn test_backward_compatibility() -> Result<()> {
    // Create shared computing state the old way
    let shared_state: SharedComputingState = Arc::new(RwLock::new(ComputingSharedData::default()));

    // Manually update legacy fields to simulate old behavior
    {
        let mut state = shared_state.write().await;
        state.peak_frequency = Some(950.0);
        state.peak_amplitude = Some(0.7);
        state.concentration_ppm = Some(35.0);
        state.last_update = SystemTime::now();
    }

    // Verify that legacy field access still works
    {
        let state = shared_state.read().await;
        assert_eq!(state.peak_frequency, Some(950.0));
        assert_eq!(state.peak_amplitude, Some(0.7));
        assert_eq!(state.concentration_ppm, Some(35.0));

        // New HashMap should be empty
        assert!(state.peak_results.is_empty());

        // Utility methods should handle the case gracefully
        assert!(state.get_peak_finder_node_ids().is_empty());
        assert!(state.get_latest_peak_result().is_none());
    }

    println!("✓ Backward compatibility test passed");
    Ok(())
}

#[tokio::test]
async fn test_mixed_mode_operation() -> Result<()> {
    // Test operation where both new HashMap and legacy fields are used
    let shared_state: SharedComputingState = Arc::new(RwLock::new(ComputingSharedData::default()));

    // First, use the new API to add a peak result
    {
        let mut state = shared_state.write().await;
        let result = PeakResult {
            frequency: 1100.0,
            amplitude: 0.85,
            concentration_ppm: Some(40.0),
            timestamp: SystemTime::now(),
            coherence_score: 0.88,
            processing_metadata: std::collections::HashMap::new(),
        };
        state.update_peak_result("new_node".to_string(), result);
    }

    // Verify that both new and legacy fields are populated
    {
        let state = shared_state.read().await;

        // New HashMap should have the data
        assert!(state.peak_results.contains_key("new_node"));
        let result = state.get_peak_result("new_node").unwrap();
        assert_eq!(result.frequency, 1100.0);

        // Legacy fields should also be updated
        assert_eq!(state.peak_frequency, Some(1100.0));
        assert_eq!(state.peak_amplitude, Some(0.85));
        assert_eq!(state.concentration_ppm, Some(40.0));
    }

    println!("✓ Mixed mode operation test passed");
    Ok(())
}
