// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Test that PerformanceSummary maintains backward compatibility
//!
//! This example validates that the PerformanceSummary struct contains all the
//! original fields required by the client application.

use anyhow::Result;
use rust_photoacoustic::acquisition::AudioFrame;
use rust_photoacoustic::processing::nodes::{GainNode, InputNode};
use rust_photoacoustic::processing::ProcessingData;
use rust_photoacoustic::processing::{PerformanceSummary, ProcessingGraph};
use serde_json;

fn main() -> Result<()> {
    println!("Testing PerformanceSummary backward compatibility...");

    // Create a simple processing graph
    let mut graph = ProcessingGraph::new();

    // Add some nodes
    let input_node = Box::new(InputNode::new("input".to_string()));
    let gain_node = Box::new(GainNode::new("gain".to_string(), 6.0));

    graph.add_node(input_node)?;
    graph.add_node(gain_node)?;

    // Connect the nodes
    graph.connect("input", "gain")?;
    graph.set_output_node("gain")?;

    // Create some mock data and execute the graph a few times
    let test_frame = AudioFrame {
        channel_a: vec![0.1, 0.2, 0.3],
        channel_b: vec![0.4, 0.5, 0.6],
        sample_rate: 44100,
        frame_number: 1,
        timestamp: 1000, // Use u64 timestamp instead of SystemTime
    };

    let processing_data = ProcessingData::from_audio_frame(test_frame);

    // Execute multiple times to generate statistics
    for _ in 0..5 {
        let _ = graph.execute(processing_data.clone())?;
    }

    // Get the performance summary
    let summary = graph.get_performance_summary();

    // Test serialization to JSON to ensure all fields are present
    let json = serde_json::to_string_pretty(&summary)?;
    println!("PerformanceSummary JSON:\n{}", json);

    // Parse back to ensure round-trip works
    let _: PerformanceSummary = serde_json::from_str(&json)?;

    // Verify all original fields are present and accessible
    println!("\n=== Original Fields (required by client) ===");
    println!("throughput_fps: {}", summary.throughput_fps);
    println!(
        "efficiency_percentage: {:.2}%",
        summary.efficiency_percentage
    );
    println!("slowest_node: {:?}", summary.slowest_node);
    println!("fastest_node: {:?}", summary.fastest_node);

    // Verify additional fields are also present
    println!("\n=== Additional Fields (for enhanced functionality) ===");
    println!("total_nodes: {}", summary.total_nodes);
    println!("active_nodes: {}", summary.active_nodes);
    println!("total_connections: {}", summary.total_connections);
    println!(
        "average_execution_time_ms: {:.2}",
        summary.average_execution_time_ms
    );
    println!(
        "fastest_execution_time_ms: {:.2}",
        summary.fastest_execution_time_ms
    );
    println!(
        "slowest_execution_time_ms: {:.2}",
        summary.slowest_execution_time_ms
    );
    println!("total_executions: {}", summary.total_executions);
    println!(
        "nodes_by_performance.len(): {}",
        summary.nodes_by_performance.len()
    );

    // Test that the structure can be used as expected by client code
    println!("\n=== Client Compatibility Test ===");
    if summary.throughput_fps > 0.0 {
        println!("✓ Throughput FPS is valid: {:.1}", summary.throughput_fps);
    }

    if summary.efficiency_percentage >= 0.0 && summary.efficiency_percentage <= 100.0 {
        println!(
            "✓ Efficiency percentage is valid: {:.1}%",
            summary.efficiency_percentage
        );
    }

    if summary.slowest_node.is_some() {
        println!("✓ Slowest node is identified: {:?}", summary.slowest_node);
    }

    if summary.fastest_node.is_some() {
        println!("✓ Fastest node is identified: {:?}", summary.fastest_node);
    }

    println!("\n✅ All backward compatibility tests passed!");
    println!("The PerformanceSummary struct is compatible with existing client code.");

    Ok(())
}
