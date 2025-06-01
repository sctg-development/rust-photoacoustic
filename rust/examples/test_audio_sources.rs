// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Test example for audio sources (Mock, File, and Microphone)
//!
//! This example demonstrates how to use the different audio sources
//! and shows their basic functionality.

use anyhow::Result;
use rust_photoacoustic::{
    acquisition::{get_mock_audio_source, AudioSource, MicrophoneSource},
    config::PhotoacousticConfig,
};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("Testing Audio Sources");
    println!("====================");

    // Test Mock Source
    test_mock_source().await?;

    // Test Microphone Source (may fail if no microphone available)
    test_microphone_source().await;

    Ok(())
}

async fn test_mock_source() -> Result<()> {
    println!("\n1. Testing Mock Audio Source");
    println!("-----------------------------");

    let mut config = PhotoacousticConfig::default();
    config.frame_size = 512;
    config.mock_source = true;
    config.mock_correlation = 0.8;
    config.frequency = 2000.0;

    let mut mock_source = get_mock_audio_source(config)?;

    println!("Mock source sample rate: {} Hz", mock_source.sample_rate());

    // Read a few frames
    for i in 0..3 {
        let (channel_a, channel_b) = mock_source.read_frame()?;
        println!("Frame {}: {} samples per channel", i + 1, channel_a.len());

        // Check signal properties
        let avg_a = channel_a.iter().sum::<f32>() / channel_a.len() as f32;
        let avg_b = channel_b.iter().sum::<f32>() / channel_b.len() as f32;
        let max_a = channel_a.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
        let max_b = channel_b.iter().fold(0.0f32, |a, &b| a.max(b.abs()));

        println!("  Channel A: avg={:.4}, max={:.4}", avg_a, max_a);
        println!("  Channel B: avg={:.4}, max={:.4}", avg_b, max_b);
    }

    println!("✓ Mock source test completed successfully");
    Ok(())
}

async fn test_microphone_source() {
    println!("\n2. Testing Microphone Audio Source");
    println!("-----------------------------------");

    let mut config = PhotoacousticConfig::default();
    config.frame_size = 1024;
    config.input_device = Some("first".to_string()); // Use the first device

    match MicrophoneSource::new(config) {
        Ok(mut mic_source) => {
            println!(
                "Microphone source sample rate: {} Hz",
                mic_source.sample_rate()
            );
            println!("{}", mic_source.device_info());

            // Try to read a few frames
            for i in 0..3 {
                match mic_source.read_frame() {
                    Ok((channel_a, channel_b)) => {
                        println!("Frame {}: {} samples per channel", i + 1, channel_a.len());

                        // Basic signal analysis
                        let rms_a = (channel_a.iter().map(|x| x * x).sum::<f32>()
                            / channel_a.len() as f32)
                            .sqrt();
                        let rms_b = (channel_b.iter().map(|x| x * x).sum::<f32>()
                            / channel_b.len() as f32)
                            .sqrt();

                        println!("  Channel A RMS: {:.6}", rms_a);
                        println!("  Channel B RMS: {:.6}", rms_b);
                    }
                    Err(e) => {
                        println!("Failed to read frame {}: {}", i + 1, e);
                        break;
                    }
                }

                // Small delay between reads
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }

            println!("✓ Microphone source test completed successfully");
        }
        Err(e) => {
            println!("⚠ Microphone source test skipped: {}", e);
            println!("  This is normal if no audio input device is available");
        }
    }
}
