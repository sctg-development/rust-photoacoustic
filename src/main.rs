// Main entry point for the photoacoustic water vapor analyzer
mod acquisition;
mod preprocessing;
mod spectral;
mod visualization;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Water vapor analyzer using photoacoustic spectroscopy
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
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
    window_size: usize,
    
    /// Number of spectra to average
    #[arg(long, default_value_t = 10)]
    averages: usize,

    /// Start web server for visualization
    #[arg(long)]
    web: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    
    println!("Water Vapor Analyzer");
    println!("--------------------");
    
    // Determine input source (device or file)
    let source = if let Some(device) = &args.input_device {
        println!("Using audio device: {}", device);
        acquisition::get_audio_source_from_device(device)?
    } else if let Some(file_path) = &args.input_file {
        println!("Using audio file: {}", file_path.display());
        acquisition::get_audio_source_from_file(file_path)?
    } else {
        println!("No input source specified. Using default device.");
        acquisition::get_default_audio_source()?
    };
    
    // Set up processing pipeline
    let filter = preprocessing::create_bandpass_filter(args.frequency, args.bandwidth);
    let analyzer = spectral::create_spectral_analyzer(args.window_size, args.averages);
    
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
        println!("- Water vapor concentration: {:.2} ppm", result.concentration);
    }
    
    // Start web server if requested
    if args.web {
        println!("Starting web visualization server...");
        visualization::start_server(result)?;
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
    mut source: Box<dyn acquisition::AudioSource>,
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
