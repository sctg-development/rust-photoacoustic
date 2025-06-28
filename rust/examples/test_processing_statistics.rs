// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Example demonstrating the ProcessingGraph statistics functionality

use anyhow::Result;
use rust_photoacoustic::preprocessing::filter::BandpassFilter;
use rust_photoacoustic::processing::graph::ProcessingGraph;
use rust_photoacoustic::processing::nodes::{
    ChannelSelectorNode, ChannelTarget, FilterNode, InputNode, PhotoacousticOutputNode,
    ProcessingData,
};
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    println!("=== ProcessingGraph Statistics Test ===\n");

    // Create a processing graph with multiple nodes
    let mut graph = ProcessingGraph::new(); // Add an input node
    let input_node = Box::new(InputNode::new("input".to_string()));
    graph.add_node(input_node)?;

    // Add a channel selector
    let channel_selector = Box::new(ChannelSelectorNode::new(
        "channel_select".to_string(),
        ChannelTarget::ChannelA,
    ));
    graph.add_node(channel_selector)?;

    // Add a bandpass filter (BandpassFilter::new takes 2 parameters: center_freq, bandwidth)
    let filter = Box::new(FilterNode::new(
        "bandpass_filter".to_string(),
        Box::new(BandpassFilter::new(1000.0, 100.0)), // center_freq: 1000.0 Hz, bandwidth: 100.0 Hz
        ChannelTarget::Both,
    ));
    graph.add_node(filter)?; // Add an output node
    let output_node = Box::new(PhotoacousticOutputNode::new("output".to_string()));
    graph.add_node(output_node)?;
    graph.set_output_node("output")?;

    // Connect the nodes
    graph.connect("input", "channel_select")?;
    graph.connect("channel_select", "bandpass_filter")?;
    graph.connect("bandpass_filter", "output")?;

    // Validate the graph
    graph.validate()?;

    println!(
        "Graph created with {} nodes and {} connections\n",
        graph.node_count(),
        graph.connection_count()
    );

    // Display initial statistics
    println!("=== Initial Statistics ===");
    println!("{}\n", graph.get_statistics());

    // Simulate multiple graph executions
    println!("=== Execution of 10 Test Frames ===");

    for i in 1..=10 {
        // Create test data
        let test_data = create_test_data(i, 1024);

        // Execute the graph
        let start_time = std::time::Instant::now();
        let _results = graph.execute(test_data)?;
        let execution_time = start_time.elapsed();

        println!(
            "Frame {}: Executed in {:.2}ms",
            i,
            execution_time.as_secs_f64() * 1000.0
        );

        // Simulate variable processing time
        thread::sleep(Duration::from_millis((i % 5) as u64));
    }

    println!("\n=== Final Statistics ===");
    println!("{}\n", graph.get_statistics());

    // Display performance summary
    println!("=== Performance Summary ===");
    let summary: String = graph.get_performance_summary().to_string();
    println!("{}\n", summary);

    // Display individual node statistics
    println!("=== Statistics by Node ===");
    for node_id in ["input", "channel_select", "bandpass_filter", "output"] {
        if let Some(stats) = graph.get_node_statistics(node_id) {
            println!("{}", stats);
        }
    }

    // Test serialization
    println!("\n=== Serialization Test ===");
    let stats_json = serde_json::to_string_pretty(graph.get_statistics())?;
    println!("Statistics serialized to JSON:\n{}", stats_json);

    // Test deserialization
    let _deserialized_stats: rust_photoacoustic::processing::graph::ProcessingGraphStatistics =
        serde_json::from_str(&stats_json)?;
    println!("✅ Deserialization successful");

    // Test cloning
    println!("\n=== Cloning Test ===");
    let _cloned_stats = graph.get_statistics().clone();
    println!("✅ Cloning successful");

    // Reset statistics
    println!("\n=== Statistics Reset ===");
    graph.reset_statistics();
    println!("Statistics after reset:");
    println!("{}", graph.get_statistics());

    Ok(())
}

/// Create simulated test data
fn create_test_data(frame_num: u32, samples: usize) -> ProcessingData {
    let channel_a: Vec<f32> = (0..samples)
        .map(|i| {
            let t = i as f32 / 44100.0;
            let freq = 1000.0 + (frame_num as f32 * 10.0);
            (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5
        })
        .collect();

    let channel_b: Vec<f32> = (0..samples)
        .map(|i| {
            let t = i as f32 / 44100.0;
            let freq = 1200.0 + (frame_num as f32 * 8.0);
            (2.0 * std::f32::consts::PI * freq * t).cos() * 0.3
        })
        .collect();
    ProcessingData::DualChannel {
        channel_a,
        channel_b,
        sample_rate: 44100,
        timestamp: frame_num as u64 * 1000, // 1ms per frame
        frame_number: frame_num as u64,
    }
}
