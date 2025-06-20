// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Example demonstrating system statistics collection and integration
//! with the processing pipeline monitoring.

use anyhow::Result;
use log::info;
use rust_photoacoustic::utility::system_stats::{SystemMonitor, SystemStats};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("=== System Statistics Collection Example ===\n");

    // 1. Simple one-time statistics collection
    println!("1. Current system statistics:");
    let current_stats = SystemStats::current()?;
    println!("{}\n", current_stats);
    println!(
        "Formatted for logging: {}\n",
        current_stats.format_for_logging()
    );

    // 2. Check system health thresholds
    println!("2. System health checks:");
    if current_stats.is_cpu_high(80.0) {
        println!(
            "⚠️  High CPU usage detected: {:.1}%",
            current_stats.cpu_usage_percent
        );
    } else {
        println!(
            "✅ CPU usage normal: {:.1}%",
            current_stats.cpu_usage_percent
        );
    }

    if current_stats.is_memory_high(80.0) {
        println!(
            "⚠️  High memory usage detected: {:.1}%",
            current_stats.memory_usage_percent()
        );
    } else {
        println!(
            "✅ Memory usage normal: {:.1}%",
            current_stats.memory_usage_percent()
        );
    }

    println!(
        "Threads: {} (on {} CPU cores)",
        current_stats.thread_count, current_stats.total_cpu_cores
    );

    // 3. Periodic monitoring example
    println!("\n3. Starting periodic monitoring for 10 seconds...");

    let mut monitor = SystemMonitor::new(Duration::from_secs(2))?;

    // Create a monitoring task that runs for a limited time
    let monitoring_task = tokio::spawn(async move {
        let mut samples = 0;
        let max_samples = 5; // 10 seconds with 2-second intervals

        let result = monitor
            .start_monitoring(move |stats| {
                samples += 1;
                info!("System Stats [{}]: {}", samples, stats.format_for_logging());

                // Example alerts
                if stats.is_cpu_high(50.0) {
                    println!("🔥 Alert: High CPU usage: {:.1}%", stats.cpu_usage_percent);
                }

                if samples >= max_samples {
                    return; // Exit monitoring
                }
            })
            .await;

        result
    });

    // Wait for monitoring to complete or timeout
    tokio::select! {
        result = monitoring_task => {
            match result {
                Ok(Ok(())) => println!("✅ Monitoring completed successfully"),
                Ok(Err(e)) => println!("❌ Monitoring error: {}", e),
                Err(e) => println!("❌ Task error: {}", e),
            }
        }
        _ = tokio::time::sleep(Duration::from_secs(12)) => {
            println!("⏰ Monitoring timeout reached");
        }
    }

    // 4. Integration example with processing statistics
    println!("\n4. Integration with processing pipeline (simulated):");

    simulate_processing_with_system_monitoring().await?;

    println!("\n✅ Example completed successfully!");
    Ok(())
}

/// Simulate how system statistics would integrate with processing pipeline monitoring
async fn simulate_processing_with_system_monitoring() -> Result<()> {
    use rust_photoacoustic::acquisition::AudioFrame;
    use rust_photoacoustic::processing::nodes::{GainNode, InputNode, ProcessingData};
    use rust_photoacoustic::processing::ProcessingGraph;

    // Create a simple processing graph
    let mut graph = ProcessingGraph::new();

    let input_node = Box::new(InputNode::new("input".to_string()));
    let gain_node = Box::new(GainNode::new("gain".to_string(), 6.0));

    graph.add_node(input_node)?;
    graph.add_node(gain_node)?;
    graph.connect("input", "gain")?;
    graph.set_output_node("gain")?;

    // Create system stats collector
    let mut system_collector =
        rust_photoacoustic::utility::system_stats::SystemStatsCollector::new()?;

    println!("Simulating processing pipeline with system monitoring...");

    // Simulate processing multiple frames while monitoring system stats
    for i in 1..=5 {
        // Collect system stats before processing
        let stats_before = system_collector.collect_stats()?;

        // Simulate processing
        let test_frame = AudioFrame {
            channel_a: vec![0.1 * i as f32; 1024],
            channel_b: vec![0.2 * i as f32; 1024],
            sample_rate: 44100,
            timestamp: i * 1000,
            frame_number: i,
        };

        let processing_data = ProcessingData::from_audio_frame(test_frame);
        let _results = graph.execute(processing_data)?;

        // Collect system stats after processing
        let stats_after = system_collector.collect_stats()?;

        // Show combined statistics
        let processing_stats = graph.get_performance_summary();

        println!(
            "Frame {}: Processing efficiency: {:.1}%, System: {}",
            i,
            processing_stats.efficiency_percentage,
            stats_after.format_for_logging()
        );

        // Simulate some processing delay
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Final report
    let final_processing_stats = graph.get_performance_summary();
    let final_system_stats = system_collector.collect_stats()?;

    println!("\n📊 Final Combined Report:");
    println!("Processing Pipeline:");
    println!(
        "  - Total executions: {}",
        final_processing_stats.total_executions
    );
    println!(
        "  - Average execution time: {:.2}ms",
        final_processing_stats.average_execution_time_ms
    );
    println!(
        "  - Efficiency: {:.1}%",
        final_processing_stats.efficiency_percentage
    );

    println!("System Resources:");
    println!(
        "  - CPU usage: {:.1}%",
        final_system_stats.cpu_usage_percent
    );
    println!(
        "  - Memory usage: {} MB ({:.1}%)",
        final_system_stats.memory_usage_mb,
        final_system_stats.memory_usage_percent()
    );
    println!("  - Active threads: {}", final_system_stats.thread_count);

    Ok(())
}
