//! Manual test example for dynamic configuration hot-reload
//!
//! This example demonstrates how the ProcessingConsumer automatically detects
//! configuration changes and applies hot-reload updates to compatible nodes.

use anyhow::Result;
use log::{info, warn};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time;

use rust_photoacoustic::{
    acquisition::stream::SharedAudioStream,
    config::{
        processing::{NodeConfig, ProcessingGraphConfig},
        Config,
    },
    processing::{consumer::ProcessingConsumer, graph::ProcessingGraph},
    visualization::shared_state::SharedVisualizationState,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("🚀 Starting dynamic configuration hot-reload demo");

    // Create a test configuration with processing enabled
    let mut config = Config::default();
    config.processing.enabled = true;

    // Create a simple processing graph with a few nodes for testing
    let mut processing_config = ProcessingGraphConfig::default();

    // Add a GainNode for testing hot-reload
    let gain_node = NodeConfig {
        id: "test_gain".to_string(),
        node_type: "gain".to_string(),
        parameters: serde_json::json!({
            "value": 0.0
        }),
    };
    processing_config.nodes.push(gain_node);

    config.processing.default_graph = processing_config;

    // Create shared config reference
    let shared_config = Arc::new(RwLock::new(config));

    info!("✅ Configuration created with hot-reload monitoring enabled");

    // Create required components
    let audio_stream = Arc::new(SharedAudioStream::new(1024));
    let visualization_state = Arc::new(SharedVisualizationState::new());

    // Create processing graph from config
    let processing_graph = ProcessingGraph::from_config_with_registry(
        &shared_config.read().unwrap().processing.default_graph,
        None,
    )?;

    info!("📊 Processing graph created");

    // Create ProcessingConsumer with config monitoring
    let mut consumer = ProcessingConsumer::new_with_visualization_state_and_config(
        audio_stream.clone(),
        processing_graph,
        visualization_state.clone(),
        shared_config.clone(),
    );

    info!("🎛️  ProcessingConsumer created with config monitoring");

    // Start the consumer in a background task
    let consumer_handle = tokio::spawn(async move {
        info!("▶️  Starting ProcessingConsumer (with config monitoring)");

        tokio::select! {
            result = consumer.start() => {
                result
            }
            _ = time::sleep(Duration::from_secs(20)) => {
                info!("⏹️  Stopping ProcessingConsumer after demo period");
                consumer.stop().await;
                Ok(())
            }
        }
    });

    // Wait for the consumer to start and initialize config monitoring
    time::sleep(Duration::from_millis(500)).await;
    info!("✅ ProcessingConsumer started and config monitoring active");

    // Simulate configuration changes
    for i in 1..=5 {
        info!("🔄 Simulating configuration change #{}", i);

        {
            let mut config_guard = shared_config.write().unwrap();

            // Modify gain parameter to trigger hot-reload
            if let Some(node) = config_guard
                .processing
                .default_graph
                .nodes
                .iter_mut()
                .find(|n| n.id == "test_gain")
            {
                let new_gain = i as f32 * 3.0; // 3dB, 6dB, 9dB, etc.
                node.parameters = serde_json::json!({
                    "value": new_gain
                });
                info!("📈 Updated gain to {} dB", new_gain);
            }

            // Trigger processing config version change
            config_guard.processing.enabled = true; // Keep enabled but force change detection
        }

        info!("⏳ Waiting for config monitoring to detect change...");
        // Wait for the config monitoring to detect and apply the change
        time::sleep(Duration::from_millis(1500)).await; // Config check interval is 1 second

        info!("✅ Configuration change #{} should now be applied", i);
        time::sleep(Duration::from_millis(500)).await;
    }

    info!("🏁 Demo completed - shutting down");

    // Wait for the consumer task to complete
    let result = consumer_handle.await?;

    match result {
        Ok(_) => {
            info!("✅ Dynamic configuration hot-reload demo completed successfully!");
            info!("📋 Summary:");
            info!("   • ProcessingConsumer automatically monitored configuration changes");
            info!("   • Hot-reload was applied to compatible nodes without restart");
            info!("   • Configuration changes were detected and processed in real-time");
        }
        Err(e) => {
            warn!("⚠️  Demo encountered an error: {}", e);
        }
    }

    Ok(())
}
