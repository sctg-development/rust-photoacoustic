// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use anyhow::Result;
use clap::Parser;
use rocket::config;
use rust_photoacoustic::{acquisition, config::PhotoacousticConfig, preprocessing, spectral};

use std::path::PathBuf;

/// Water vapor analyzer using photoacoustic spectroscopy
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Audio input device
    #[arg(long)]
    input_device: Option<String>,

    /// Input audio file (WAV format)
    #[arg(long)]
    input_file: Option<PathBuf>,

    /// Excitation frequency in Hz
    #[arg(long, default_value_t = 2000.0)]
    frequency: f32,

    /// Filter bandwidth in Hz
    #[arg(long, default_value_t = 100.0)]
    bandwidth: f32,

    /// Output file for results (JSON)
    #[arg(long)]
    output: Option<PathBuf>,

    /// Window size for FFT analysis
    #[arg(long, default_value_t = 4096)]
    frame_size: usize,

    /// Number of spectra to average
    #[arg(long, default_value_t = 10)]
    averages: usize,
}

#[rocket::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    println!("Flexible Gas Analyzer");
    println!("--------------------");

    let input_file: Option<String> = args
        .input_file
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    let config = PhotoacousticConfig {
        input_device: args.input_device.clone(),
        input_file: args
            .input_file
            .clone()
            .map(|p| p.to_string_lossy().to_string()),
        frequency: args.frequency,
        sample_rate: 48000, // Default sample rate
        bandwidth: args.bandwidth,
        frame_size: args.frame_size as u16,
        averages: args.averages as u16,
        precision: 16,              // Default precision,
        simulated_source: None,     // No simulated source in standalone mode
        record_consumer: false,     // No record consumer in standalone mode
        record_file: String::new(), // No record file in standalone mode
    };
    // Determine input source (device or file)
    let source = if let Some(device) = &args.input_device {
        println!("Using audio device: {}", device);
        acquisition::get_audio_source_from_device(config)?
    } else if let Some(file_path) = &args.input_file {
        println!("Using audio file: {}", file_path.display());
        acquisition::get_audio_source_from_file(config)?
    } else {
        println!("No input source specified. Using default device.");
        acquisition::get_default_audio_source(config)?
    };

    // Set up processing pipeline
    let filter = preprocessing::create_bandpass_filter(args.frequency, args.bandwidth);
    let analyzer = spectral::create_spectral_analyzer(args.frame_size, args.averages);

    // Process audio data
    println!("Processing audio data...");
    let result = process_audio(source, filter, analyzer)?;

    // Output results
    if let Some(output_path) = args.output {
        println!("Saving results to: {}", output_path.display());
        std::fs::write(output_path, serde_json::to_string_pretty(&result)?)?;
    } else {
        println!("Results:");
        println!("- Frequency: {} Hz", args.frequency);
        println!("- Amplitude: {:.6}", result.amplitude);
        println!(
            "- Water vapor concentration: {:.2} ppm",
            result.concentration
        );
    }

    Ok(())
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct AnalysisResult {
    frequency: f32,
    amplitude: f32,
    concentration: f32,
    timestamp: chrono::DateTime<chrono::Utc>,
}

/// Process audio data through the signal processing pipeline
fn process_audio(
    source: Box<dyn acquisition::AudioSource>,
    filter: Box<dyn preprocessing::Filter>,
    analyzer: Box<dyn spectral::SpectralAnalyzer>,
) -> Result<AnalysisResult> {
    // Simulate processing
    // In a real implementation, this would read data from source, apply filter, and perform spectral analysis
    let frequency = 2000.0;
    let amplitude = 0.05;
    let concentration = amplitude * 1000.0; // Simulated conversion factor

    Ok(AnalysisResult {
        frequency,
        amplitude,
        concentration,
        timestamp: chrono::Utc::now(),
    })
}
