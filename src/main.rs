// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

// Main entry point for the photoacoustic water vapor analyzer
mod acquisition;
mod config;
mod daemon;
mod modbus;
mod preprocessing;
mod spectral;
mod utility;
mod visualization;

use anyhow::Result;
use clap::Parser;
use config::Config;
use log::info;

use std::path::PathBuf;
use tokio::signal;

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
    #[arg(long)]
    frequency: Option<f32>,

    /// Filter bandwidth in Hz
    #[arg(long)]
    bandwidth: Option<f32>,

    /// Output file for results (JSON)
    #[arg(long)]
    output: Option<PathBuf>,

    /// Window size for FFT analysis
    #[arg(long)]
    window_size: Option<u16>,

    /// Number of spectra to average
    #[arg(long)]
    averages: Option<u16>,

    /// Start in server mode
    #[arg(long, default_value_t = true)]
    server: bool,

    /// Web server port (default: 8080) only used if --web is set
    #[arg(short = 'p')]
    web_port: Option<u16>,

    /// Web server address (default: localhost) only used if --web is set
    #[arg(short)]
    web_address: Option<String>,

    /// HMAC secret for JWT signing
    #[arg(long)]
    hmac_secret: Option<String>,

    /// Path to configuration file (YAML format)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Output the configuration schema as JSON and exit
    #[arg(long)]
    show_config_schema: bool,

    /// Modbus enabled
    #[arg(long)]
    modbus_enabled: Option<bool>,

    /// Modbus server address
    #[arg(long)]
    modbus_address: Option<String>,

    /// Modbus server port
    #[arg(long)]
    modbus_port: Option<u16>,
}

#[rocket::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    // Check if --show-config-schema flag is set
    if args.show_config_schema {
        return config::output_config_schema();
    }

    // Load configuration
    let config_path = args
        .config
        .clone()
        .unwrap_or_else(|| PathBuf::from("config.yaml"));
    let mut config = Config::from_file(&config_path)?;

    // Apply command line overrides
    config.apply_args(
        args.web_port,
        args.web_address.clone(),
        args.hmac_secret.clone(),
        args.server,
        args.input_device.clone(),
        args.input_file.clone(),
        args.frequency,
        args.bandwidth,
        args.window_size,
        args.averages,
        args.modbus_enabled,
        args.modbus_address.clone(),
        args.modbus_port,
    );

    // Configure Rocket
    if args.server {
        info!("Starting in daemon mode");
        let mut daemon = daemon::launch_daemon::Daemon::new();

        // Launch all configured tasks
        daemon.launch(&config).await?;

        // Wait for termination signal
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("Received shutdown signal, terminating daemon");
                daemon.shutdown();
                daemon.join().await?;
            }
            Err(err) => {
                eprintln!("Error waiting for shutdown signal: {}", err);
            }
        }

        return Ok(());
    } else {
        println!("Web server disabled");
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
