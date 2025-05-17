// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

// White noise generator for audio testing
// Generates stereo Gaussian white noise at different sample rates

use clap::Parser;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::path::PathBuf;

// Import the NoiseGenerator from our library
use rust_photoacoustic::utility::noise_generator::NoiseGenerator;

/// White noise generator for audio testing
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output file path (.wav)
    #[arg(short, long, value_name = "FILE")]
    output: PathBuf,

    /// Duration in seconds
    #[arg(short, long, default_value_t = 5.0)]
    duration: f32,

    /// Sample rate (44100, 48000, or 192000)
    #[arg(short, long, default_value_t = 48000)]
    sample_rate: u32,

    /// Amplitude of the noise (0.0 to 1.0)
    #[arg(short, long, default_value_t = 0.5)]
    amplitude: f32,

    /// Number of channels (1 for mono, 2 for stereo)
    #[arg(short, long, default_value_t = 2)]
    channels: u16,

    /// Set to true to use correlations between channels (default is independent)
    #[arg(long, default_value_t = false)]
    correlated: bool,

    /// Correlation coefficient between channels (-1.0 to 1.0)
    #[arg(short = 'r', long, default_value_t = 0.0)]
    correlation: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Validate sample rate
    match args.sample_rate {
        44100 | 48000 | 192000 => {}
        _ => {
            eprintln!("Error: Sample rate must be 44100, 48000, or 192000 Hz");
            std::process::exit(1);
        }
    }

    // Validate number of channels
    if args.channels != 1 && args.channels != 2 {
        eprintln!("Error: Number of channels must be 1 (mono) or 2 (stereo)");
        std::process::exit(1);
    }

    // Validate amplitude
    if args.amplitude <= 0.0 || args.amplitude > 1.0 {
        eprintln!("Error: Amplitude must be between 0.0 and 1.0");
        std::process::exit(1);
    }

    // Validate correlation coefficient
    if args.correlation < -1.0 || args.correlation > 1.0 {
        eprintln!("Error: Correlation coefficient must be between -1.0 and 1.0");
        std::process::exit(1);
    }

    // Create WAV file specification
    let spec = WavSpec {
        channels: args.channels,
        sample_rate: args.sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    println!("Generating {} seconds of white noise...", args.duration);
    println!("Sample rate: {} Hz", args.sample_rate);
    println!("Channels: {}", args.channels);
    println!("Amplitude: {}", args.amplitude);

    // Open the output file
    let mut writer = WavWriter::create(&args.output, spec)?;

    // Create a noise generator
    let mut generator = NoiseGenerator::new_from_system_time();

    // Calculate number of samples
    let num_samples = (args.duration * args.sample_rate as f32) as u32;

    // Generate samples based on the requested configuration
    let samples = if args.channels == 1 {
        // Mono white noise
        generator.generate_mono(num_samples, args.amplitude)
    } else if args.correlated && args.correlation != 0.0 {
        // Correlated stereo white noise
        println!(
            "Generating correlated channels with correlation coefficient: {}",
            args.correlation
        );
        generator.generate_correlated_stereo(num_samples, args.amplitude, args.correlation)
    } else {
        // Independent stereo white noise
        generator.generate_stereo(num_samples, args.amplitude)
    };

    // Write all samples to the WAV file
    for &sample in &samples {
        writer.write_sample(sample)?;
    }

    // Finalize the WAV file
    writer.finalize()?;
    println!(
        "White noise successfully generated and saved to: {}",
        args.output.display()
    );

    Ok(())
}
