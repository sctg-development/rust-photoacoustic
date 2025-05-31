// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Test example for chunk-level timing analysis
//!
//! This example tests how audio chunks arrive from the microphone
//! at a lower level to understand the streaming behavior better.

use anyhow::Result;
use rust_photoacoustic::{
    acquisition::{AudioSource, MicrophoneSource},
    config::PhotoacousticConfig,
};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("Testing Chunk-Level Timing");
    println!("==========================");

    // Test with smaller window size to see chunks more clearly
    test_microphone_chunk_timing().await?;

    Ok(())
}

async fn test_microphone_chunk_timing() -> Result<()> {
    println!("Testing Microphone Chunk-Level Timing");
    println!("======================================");
    
    let mut config = PhotoacousticConfig::default();
    config.window_size = 2048; // Smaller window for faster response
    config.input_device = Some("first".to_string());
    
    let mut mic_source = MicrophoneSource::new(config)?;
    
    let sample_rate = mic_source.sample_rate();
    let expected_duration_ms = (2048.0 / sample_rate as f64 * 1000.0) as u64;
    
    println!("Sample rate: {} Hz", sample_rate);
    println!("Window size: 2048 samples");
    println!("Expected frame duration: {} ms", expected_duration_ms);
    println!("Target chunk size: approximately 20ms = {} samples", (sample_rate as f32 * 0.02) as usize);
    
    let mut frame_times = Vec::new();
    let start_time = Instant::now();
    
    // Read 20 frames with detailed timing
    for i in 0..20 {
        let frame_start = Instant::now();
        
        match mic_source.read_frame() {
            Ok((channel_a, channel_b)) => {
                let frame_duration = frame_start.elapsed();
                frame_times.push(frame_duration);
                
                println!("Frame {}: {} samples/channel, took {:.1}ms", 
                    i + 1, channel_a.len(), frame_duration.as_micros() as f64 / 1000.0);
                
                // Check for signal presence
                let rms_a = (channel_a.iter().map(|x| x * x).sum::<f32>() / channel_a.len() as f32).sqrt();
                let rms_b = (channel_b.iter().map(|x| x * x).sum::<f32>() / channel_b.len() as f32).sqrt();
                
                if rms_a > 0.001 || rms_b > 0.001 {
                    println!("  Signal: A={:.6}, B={:.6}", rms_a, rms_b);
                } else {
                    println!("  Silence");
                }
            }
            Err(e) => {
                println!("Frame {}: Failed - {}", i + 1, e);
                break;
            }
        }
        
        // Short delay to avoid overwhelming the output
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    
    // Analyze timing patterns
    if !frame_times.is_empty() {
        let total_duration = start_time.elapsed();
        let avg_frame_time = frame_times.iter().sum::<Duration>() / frame_times.len() as u32;
        let min_frame_time = frame_times.iter().min().unwrap();
        let max_frame_time = frame_times.iter().max().unwrap();
        
        println!("\nDetailed Timing Analysis:");
        println!("  Total test duration: {:.1}ms", total_duration.as_micros() as f64 / 1000.0);
        println!("  Frames processed: {}", frame_times.len());
        println!("  Average frame time: {:.1}ms", avg_frame_time.as_micros() as f64 / 1000.0);
        println!("  Min frame time: {:.1}ms", min_frame_time.as_micros() as f64 / 1000.0);
        println!("  Max frame time: {:.1}ms", max_frame_time.as_micros() as f64 / 1000.0);
        
        // Calculate standard deviation
        let avg_micros = avg_frame_time.as_micros() as f64;
        let variance: f64 = frame_times.iter()
            .map(|t| {
                let diff = t.as_micros() as f64 - avg_micros;
                diff * diff
            })
            .sum::<f64>() / frame_times.len() as f64;
        let std_dev = variance.sqrt() / 1000.0; // Convert to ms
        
        println!("  Standard deviation: {:.1}ms", std_dev);
        
        // Show timing distribution
        println!("\nTiming Distribution:");
        for (i, time) in frame_times.iter().enumerate() {
            println!("  Frame {}: {:.1}ms", i + 1, time.as_micros() as f64 / 1000.0);
        }
        
        // Analysis
        if std_dev < 10.0 {
            println!("\n✓ Low timing variance - good streaming consistency");
        } else if std_dev < 30.0 {
            println!("\n⚠ Moderate timing variance - some inconsistency");
        } else {
            println!("\n❌ High timing variance - poor streaming consistency");
        }
        
        // Check expected vs actual
        let expected_ms = expected_duration_ms as f64;
        let actual_ms = avg_frame_time.as_micros() as f64 / 1000.0;
        let ratio = actual_ms / expected_ms;
        
        println!("  Expected frame duration: {:.1}ms", expected_ms);
        println!("  Actual average duration: {:.1}ms", actual_ms);
        println!("  Ratio (actual/expected): {:.2}", ratio);
        
        if ratio < 1.5 {
            println!("  ✓ Frame times close to expected duration");
        } else {
            println!("  ⚠ Frame times significantly longer than expected");
        }
    }
    
    println!("\n✓ Chunk timing test completed");
    Ok(())
}
