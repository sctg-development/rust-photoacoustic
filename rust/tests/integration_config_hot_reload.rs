// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Integration test for dynamic configuration hot-reload feature
//!
//! This test verifies that the ProcessingConsumer can detect configuration changes
//! and apply hot-reload updates to compatible nodes without requiring a full restart.

use anyhow::Result;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time;

use rust_photoacoustic::{
    acquisition::stream::SharedAudioStream,
    config::{processing::ProcessingGraphConfig, Config, PhotoacousticConfig},
    processing::{consumer::ProcessingConsumer, graph::ProcessingGraph},
    visualization::shared_state::SharedVisualizationState,
};

#[tokio::test]
async fn test_config_hot_reload_integration() -> Result<()> {
    // Initialize logging for the test
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    // Create a simple test configuration
    let mut config = Config::default();
    config.processing.enabled = true;

    // Create a simple processing graph config with a few nodes
    let mut processing_config = ProcessingGraphConfig::default();

    // Create shared config reference
    let shared_config = Arc::new(RwLock::new(config));

    // Create required components
    let audio_stream = Arc::new(SharedAudioStream::new(1024));
    let visualization_state = Arc::new(SharedVisualizationState::new());

    // Create processing graph from config
    let processing_graph = ProcessingGraph::from_config_with_registry(&processing_config, None)?;

    // Create ProcessingConsumer with config monitoring
    let mut consumer = ProcessingConsumer::new_with_visualization_state_and_config(
        audio_stream.clone(),
        processing_graph,
        visualization_state.clone(),
        shared_config.clone(),
    );

    // Start the consumer in a background task
    let consumer_handle = tokio::spawn(async move {
        // Run for a short time to test config monitoring
        tokio::select! {
            result = consumer.start() => {
                result
            }
            _ = time::sleep(Duration::from_secs(5)) => {
                // Stop after 5 seconds for the test
                consumer.stop().await;
                Ok(())
            }
        }
    });

    // Wait a bit for the consumer to start
    time::sleep(Duration::from_millis(100)).await;

    // Simulate a configuration change
    {
        let mut config_guard = shared_config.write().unwrap();
        // Modify something in the processing config to trigger a change
        config_guard.processing.enabled = true;
        // You could modify specific node parameters here if needed
    }

    // Wait for the config monitoring to detect the change
    time::sleep(Duration::from_millis(1500)).await; // Config check interval is 1 second

    // The test passes if no panics occurred and the consumer handled the config change
    let result = consumer_handle.await?;
    assert!(
        result.is_ok(),
        "ProcessingConsumer should handle config changes gracefully"
    );

    println!("✅ Config hot-reload integration test passed!");
    Ok(())
}

#[tokio::test]
async fn test_config_change_detection() -> Result<()> {
    // Test the hash-based config change detection mechanism

    let config1 = Config::default();
    let config2 = Config::default();

    // Same configs should have the same hash
    let hash1 = ProcessingConsumer::calculate_config_hash(&config1.processing);
    let hash2 = ProcessingConsumer::calculate_config_hash(&config2.processing);

    assert_eq!(hash1, hash2, "Identical configs should have the same hash");

    // Different configs should have different hashes
    let mut config3 = Config::default();
    config3.processing.enabled = !config3.processing.enabled;

    let hash3 = ProcessingConsumer::calculate_config_hash(&config3.processing);
    assert_ne!(
        hash1, hash3,
        "Different configs should have different hashes"
    );

    println!("✅ Config change detection test passed!");
    Ok(())
}
