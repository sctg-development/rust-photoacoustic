// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Test example for streaming continuity analysis
//!
//! This example tests the continuity of audio streaming from different sources
//! and measures timing to identify chunky vs smooth delivery.

use anyhow::Result;
use rust_photoacoustic::{
    acquisition::{get_mock_audio_source, AudioSource, MicrophoneSource},
    config::PhotoacousticConfig,
};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("Testing Streaming Continuity");
    println!("============================");

    // Test Mock Source timing
    test_source_timing("Mock Source", create_mock_source()?).await?;

    // Test Microphone Source timing (may fail if no microphone available)
    if let Ok(mic_source) = create_microphone_source() {
        test_source_timing("Microphone Source", mic_source).await?;
    } else {
        println!("⚠ Microphone source not available for timing test");
    }

    Ok(())
}

fn create_mock_source() -> Result<Box<dyn AudioSource>> {
    let mut config = PhotoacousticConfig::default();
    config.frame_size = 8192; // Match the actual config
    let mut simulated_config = rust_photoacoustic::config::SimulatedSourceConfig::default();
    simulated_config.correlation = 0.8;
    config.simulated_source = Some(simulated_config);
    config.frequency = 2000.0;

    Ok(get_mock_audio_source(config)?)
}

fn create_microphone_source() -> Result<Box<dyn AudioSource>> {
    let mut config = PhotoacousticConfig::default();
    config.frame_size = 8192; // Match the actual config
    config.input_device = Some("first".to_string());

    Ok(Box::new(MicrophoneSource::new(config)?))
}

async fn test_source_timing(name: &str, mut source: Box<dyn AudioSource>) -> Result<()> {
    println!("\nTesting {}", name);
    println!("{}", "=".repeat(name.len() + 8));

    let sample_rate = source.sample_rate();
    let expected_duration_ms = (8192.0 / sample_rate as f64 * 1000.0) as u64;

    println!("Sample rate: {} Hz", sample_rate);
    println!("Window size: 8192 samples");
    println!("Expected frame duration: {} ms", expected_duration_ms);

    let mut frame_times = Vec::new();
    let mut successful_reads = 0;
    let start_time = Instant::now();

    // Read 10 frames and measure timing
    for i in 0..10 {
        let frame_start = Instant::now();

        match source.read_frame() {
            Ok((channel_a, channel_b)) => {
                let frame_duration = frame_start.elapsed();
                frame_times.push(frame_duration);
                successful_reads += 1;

                println!(
                    "Frame {}: {} samples/channel, took {:.2}ms",
                    i + 1,
                    channel_a.len(),
                    frame_duration.as_millis()
                );

                // Check for signal presence
                let rms_a =
                    (channel_a.iter().map(|x| x * x).sum::<f32>() / channel_a.len() as f32).sqrt();
                let rms_b =
                    (channel_b.iter().map(|x| x * x).sum::<f32>() / channel_b.len() as f32).sqrt();

                if rms_a > 0.001 || rms_b > 0.001 {
                    println!("  Signal detected - A: {:.6}, B: {:.6}", rms_a, rms_b);
                } else {
                    println!("  Silence detected");
                }
            }
            Err(e) => {
                println!("Frame {}: Failed - {}", i + 1, e);
            }
        }

        // Add a small delay between frames to simulate real usage
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Analyze timing
    if !frame_times.is_empty() {
        let total_duration = start_time.elapsed();
        let avg_frame_time = frame_times.iter().sum::<Duration>() / frame_times.len() as u32;
        let min_frame_time = frame_times.iter().min().unwrap();
        let max_frame_time = frame_times.iter().max().unwrap();

        println!("\nTiming Analysis:");
        println!("  Total test duration: {:.2}ms", total_duration.as_millis());
        println!("  Successful reads: {}/{}", successful_reads, 10);
        println!(
            "  Average frame read time: {:.2}ms",
            avg_frame_time.as_millis()
        );
        println!("  Min frame read time: {:.2}ms", min_frame_time.as_millis());
        println!("  Max frame read time: {:.2}ms", max_frame_time.as_millis());

        // Check for consistency
        let time_variance = max_frame_time.as_millis() as i64 - min_frame_time.as_millis() as i64;
        println!("  Time variance: {}ms", time_variance);

        if time_variance > 50 {
            println!("  ⚠ High time variance detected - possible chunky delivery");
        } else {
            println!("  ✓ Low time variance - smooth delivery");
        }
        // Check if frame read times are much longer than expected frame duration
        if avg_frame_time.as_millis() > (expected_duration_ms * 2) as u128 {
            println!("  ⚠ Frame read times much longer than expected - possible buffering issues");
        } else {
            println!("  ✓ Frame read times within reasonable range");
        }
    }

    println!("✓ {} timing test completed", name);
    Ok(())
}
