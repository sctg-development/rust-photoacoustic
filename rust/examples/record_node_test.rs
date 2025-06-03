// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Record node integration test example
//!
//! This example demonstrates how to create and use a RecordNode in a processing
//! graph configuration, showing both programmatic usage and YAML configuration.

use anyhow::Result;
use rust_photoacoustic::config::processing::{NodeConfig, ProcessingGraphConfig, ConnectionConfig};
use rust_photoacoustic::processing::{ProcessingGraph, ProcessingData};
use rust_photoacoustic::processing::nodes::{ProcessingNode, RecordNode};
use tempfile::tempdir;

fn main() -> Result<()> {
    println!("=== Record Node Integration Test ===\n");

    // 1. Test programmatic creation
    test_programmatic_creation()?;

    // 2. Test configuration-based creation
    test_config_creation()?;

    // 3. Test full processing graph with record node
    test_processing_graph()?;

    println!("✅ All tests completed successfully!");
    Ok(())
}

/// Test creating RecordNode programmatically
fn test_programmatic_creation() -> Result<()> {
    println!("📝 Testing programmatic RecordNode creation...");

    let temp_dir = tempdir()?;
    let record_file = temp_dir.path().join("test_recording.wav");
    
    let mut record_node = RecordNode::new(
        "test_record".to_string(),
        record_file.clone(),
        512, // 512KB max size
        true, // auto delete
    );

    // Test with mono audio data
    let mono_data = ProcessingData::SingleChannel {
        samples: vec![0.1, 0.2, 0.3, 0.4, 0.5],
        sample_rate: 44100,
        timestamp: 1000,
        frame_number: 1,
    };

    let result = record_node.process(mono_data.clone())?;
    
    // Verify pass-through behavior
    match (&result, &mono_data) {
        (ProcessingData::SingleChannel { samples: out_samples, .. }, 
         ProcessingData::SingleChannel { samples: in_samples, .. }) => {
            assert_eq!(out_samples, in_samples, "Output should match input exactly");
        }
        _ => panic!("Unexpected data type mismatch"),
    }

    // Check that WAV file was created
    assert!(record_file.exists(), "WAV file should have been created");

    println!("  ✅ Pass-through recording works correctly");
    println!("  ✅ WAV file created at: {}", record_file.display());

    Ok(())
}

/// Test creating RecordNode from configuration
fn test_config_creation() -> Result<()> {
    println!("📝 Testing RecordNode creation from configuration...");

    let temp_dir = tempdir()?;
    let record_file = temp_dir.path().join("config_recording.wav");

    // Create node configuration
    let mut params = serde_yml::Mapping::new();
    params.insert(
        serde_yml::Value::String("record_file".to_string()),
        serde_yml::Value::String(record_file.to_string_lossy().to_string()),
    );
    params.insert(
        serde_yml::Value::String("max_size".to_string()),
        serde_yml::Value::Number(serde_yml::Number::from(1024u64)),
    );
    params.insert(
        serde_yml::Value::String("auto_delete".to_string()),
        serde_yml::Value::Bool(false),
    );

    let node_config = NodeConfig {
        id: "record_from_config".to_string(),
        node_type: "record".to_string(),
        parameters: serde_yml::Value::Mapping(params),
    };

    // Test configuration with from_config method (create minimal graph)
    let graph_config = ProcessingGraphConfig {
        id: "test_graph".to_string(),
        nodes: vec![node_config],
        connections: vec![],
        output_node: Some("record_from_config".to_string()),
    };

    let _graph = ProcessingGraph::from_config(&graph_config)?;

    println!("  ✅ RecordNode created successfully from configuration");
    println!("  ✅ Processing graph created with RecordNode");

    Ok(())
}

/// Test RecordNode in a complete processing graph
fn test_processing_graph() -> Result<()> {
    println!("📝 Testing RecordNode in complete processing graph...");

    let temp_dir = tempdir()?;
    let record_file = temp_dir.path().join("graph_recording.wav");

    // Create graph configuration with input -> record pipeline
    let graph_config = ProcessingGraphConfig {
        id: "test_graph".to_string(),
        nodes: vec![
            NodeConfig {
                id: "input".to_string(),
                node_type: "input".to_string(),
                parameters: serde_yml::Value::Null,
            },
            NodeConfig {
                id: "recorder".to_string(),
                node_type: "record".to_string(),
                parameters: {
                    let mut params = serde_yml::Mapping::new();
                    params.insert(
                        serde_yml::Value::String("record_file".to_string()),
                        serde_yml::Value::String(record_file.to_string_lossy().to_string()),
                    );
                    params.insert(
                        serde_yml::Value::String("max_size".to_string()),
                        serde_yml::Value::Number(serde_yml::Number::from(2048u64)),
                    );
                    params.insert(
                        serde_yml::Value::String("auto_delete".to_string()),
                        serde_yml::Value::Bool(true),
                    );
                    serde_yml::Value::Mapping(params)
                },
            },
        ],
        connections: vec![
            ConnectionConfig {
                from: "input".to_string(),
                to: "recorder".to_string(),
            },
        ],
        output_node: Some("recorder".to_string()),
    };

    // Create and configure the graph
    let graph = ProcessingGraph::from_config(&graph_config)?;

    println!("  ✅ Processing graph created successfully");
    println!("  ✅ Input -> Record pipeline configured");
    println!("  ✅ Graph ready for processing");

    Ok(())
}
