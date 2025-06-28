// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

// Note: Individual node configs don't exist as separate structs
// Nodes are created with direct parameters
use rust_photoacoustic::preprocessing::filter::BandpassFilter;
use rust_photoacoustic::processing::graph::{ProcessingGraph, SerializableNode};
use rust_photoacoustic::processing::nodes::filter::ChannelTarget;
use rust_photoacoustic::processing::nodes::streaming_registry::StreamingNodeRegistry;
use rust_photoacoustic::processing::nodes::{
    ChannelMixerNode, FilterNode, GainNode, InputNode, MixStrategy, RecordNode, StreamingNode,
};
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_serializable_node_backward_compatibility() {
    // Create a simple processing graph
    let mut graph = ProcessingGraph::new();

    // Add an input node
    let input_node = InputNode::new("input".to_string());
    graph.add_node(Box::new(input_node)).unwrap();

    // Add a record node
    let record_node = RecordNode::new(
        "recorder".to_string(),
        std::path::PathBuf::from("./test.wav"),
        100,        // max_size_kb
        false,      // auto_delete
        Some(1000), // total_limit
    );
    graph.add_node(Box::new(record_node)).unwrap();

    // Add a filter node
    let bandpass_filter = BandpassFilter::new(1000.0, 200.0);
    let filter_node = FilterNode::new(
        "filter".to_string(),
        Box::new(bandpass_filter),
        ChannelTarget::Both,
    );
    graph.add_node(Box::new(filter_node)).unwrap();

    // Add a channel mixer node
    let mixer_node = ChannelMixerNode::new("mixer".to_string(), MixStrategy::Add);
    graph.add_node(Box::new(mixer_node)).unwrap();

    // Add a gain node (use f32 for gain_db)
    let gain_node = GainNode::new("gain".to_string(), 10.0);
    graph.add_node(Box::new(gain_node)).unwrap();

    // Add a streaming node
    let registry = Arc::new(StreamingNodeRegistry::new());
    let streaming_node = StreamingNode::new_with_string_id(
        "streaming",
        "streaming_output",
        registry.as_ref().clone(),
    );
    graph.add_node(Box::new(streaming_node)).unwrap();

    // Connect nodes
    graph.connect("input", "recorder").unwrap();
    graph.connect("recorder", "mixer").unwrap();
    graph.connect("mixer", "filter").unwrap();
    graph.connect("filter", "gain").unwrap();
    graph.connect("gain", "streaming").unwrap();

    // Serialize the graph
    let serializable_graph = graph.to_serializable();

    // Check that all nodes have the required fields for backward compatibility
    for node in &serializable_graph.nodes {
        // Required fields that must be present
        assert!(!node.id.is_empty(), "Node ID should not be empty");
        assert!(!node.node_type.is_empty(), "Node type should not be empty");
        assert!(
            !node.accepts_input_types.is_empty(),
            "Accepts input types should not be empty"
        );
        assert!(
            !node.output_type.is_empty(),
            "Output type should not be empty"
        );

        // Parameters should be present (may be empty object for some nodes)
        // All nodes should have some form of parameters object

        println!("Node: {}", node.id);
        println!("  Type: {}", node.node_type);
        println!("  Accepts: {:?}", node.accepts_input_types);
        println!("  Output: {}", node.output_type);
        println!("  Parameters: {:?}", node.parameters);
        if let Some(stats) = &node.statistics {
            println!("  Has statistics: true");
        }
        println!();
    }

    // Test JSON serialization
    let json = serde_json::to_string_pretty(&serializable_graph).unwrap();
    println!("Serialized JSON:");
    println!("{}", json);

    // Test that we can deserialize back
    let _deserialized: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Verify specific fields are present in JSON
    let json_value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let nodes = json_value["nodes"].as_array().unwrap();

    for node in nodes {
        assert!(node.get("id").is_some(), "JSON node should have 'id' field");
        assert!(
            node.get("node_type").is_some(),
            "JSON node should have 'node_type' field"
        );
        assert!(
            node.get("accepts_input_types").is_some(),
            "JSON node should have 'accepts_input_types' field"
        );
        assert!(
            node.get("output_type").is_some(),
            "JSON node should have 'output_type' field"
        );
        assert!(
            node.get("parameters").is_some(),
            "JSON node should have 'parameters' field"
        );
    }

    println!("✅ All backward compatibility checks passed!");
}
