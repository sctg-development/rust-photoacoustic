// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Noise Generator
//!
//! A command-line utility for generating white noise audio files for testing and calibration.
//! This tool creates WAV files containing Gaussian white noise with configurable parameters.
//!
//! ## Features
//!
//! * Generates mono or stereo white noise signals
//! * Supports different sample rates (44.1kHz, 48kHz, 192kHz)
//! * Configurable amplitude and duration
//! * Option for correlated noise between stereo channels
//!
//! ## Usage
//!
//! Generate a basic 5-second stereo white noise file:
//! ```shell
//! noise_generator --output noise.wav
//! ```
//!
//! Generate a 10-second mono noise file with 75% amplitude:
//! ```shell
//! noise_generator --output calibration.wav --duration 10 --channels 1 --amplitude 0.75
//! ```
//!
//! Generate correlated stereo noise with correlation coefficient of 0.8:
//! ```shell
//! noise_generator --output correlated.wav --correlated --correlation 0.8
//! ```
//!
//! ## Applications in Photoacoustic Analysis
//!
//! White noise signals are useful in photoacoustic applications for:
//! 
//! * System calibration and response testing
//! * SNR evaluation and performance benchmarking
//! * Measuring frequency response of photoacoustic cells
//! * Testing filter implementations
//! * Simulating background noise for robustness testing

use clap::Parser;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::path::PathBuf;

// Import the NoiseGenerator from our library
use rust_photoacoustic::utility::noise_generator::NoiseGenerator;

/// Command line arguments for the noise generator utility.
///
/// This structure defines all parameters that can be configured to control
/// the generation of white noise audio files. It uses clap's derive feature
/// for convenient command-line parsing.
#[derive(Debug, Parser)]
#[command(author, version, about = "Generates white noise WAV files for audio testing and calibration", long_about = None)]
struct Args {
    /// Output file path (.wav)
    ///
    /// The path where the generated WAV file will be saved. The file extension
    /// should be .wav as the output is in WAV PCM format.
    #[arg(short, long, value_name = "FILE")]
    output: PathBuf,

    /// Duration in seconds
    ///
    /// The length of the generated white noise audio in seconds.
    /// Longer durations create larger files but may be necessary for certain tests.
    #[arg(short, long, default_value_t = 5.0)]
    duration: f32,

    /// Sample rate (44100, 48000, or 192000)
    ///
    /// The number of samples per second in the generated audio.
    /// Higher sample rates provide better frequency resolution but create larger files.
    /// Common values are:
    /// - 44100 Hz (CD quality)
    /// - 48000 Hz (standard for digital audio)
    /// - 192000 Hz (high-resolution audio)
    #[arg(short, long, default_value_t = 48000)]
    sample_rate: u32,

    /// Amplitude of the noise (0.0 to 1.0)
    ///
    /// Controls the volume of the generated white noise.
    /// A value of 1.0 represents the maximum possible amplitude without clipping.
    /// Lower values create quieter noise.
    #[arg(short, long, default_value_t = 0.5)]
    amplitude: f32,

    /// Number of channels (1 for mono, 2 for stereo)
    ///
    /// Determines whether to generate mono (1) or stereo (2) audio.
    /// Stereo is useful for testing channel separation or correlation effects.
    #[arg(short, long, default_value_t = 2)]
    channels: u16,

    /// Set to true to use correlations between channels (default is independent)
    ///
    /// When enabled for stereo output, the noise in the left and right channels
    /// will be correlated according to the specified correlation coefficient.
    /// By default, channels contain independent noise.
    #[arg(long, default_value_t = false)]
    correlated: bool,

    /// Correlation coefficient between channels (-1.0 to 1.0)
    ///
    /// Controls the degree of correlation between stereo channels when correlated mode is enabled:
    /// - 1.0: Perfectly correlated (identical channels)
    /// - 0.0: Uncorrelated (independent channels)
    /// - -1.0: Perfectly anti-correlated (inverted channels)
    /// This parameter only has an effect when --correlated is set and channels = 2.
    #[arg(short = 'r', long, default_value_t = 0.0)]
    correlation: f32,
}

/// Main entry point for the noise generator utility.
///
/// Parses command-line arguments, validates parameters, generates the
/// requested type of white noise, and saves it to a WAV file.
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - Success or an error with description
///
/// # Errors
///
/// Will return an error if:
/// - Invalid parameters are provided (checked before generation)
/// - The output file cannot be created or written to
/// - The WAV file processing encounters an issue
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
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

    // Create a noise generator with a seed based on system time
    // This ensures different noise patterns on each run
    let mut generator = NoiseGenerator::new_from_system_time();

    // Calculate number of samples based on duration and sample rate
    let num_samples = (args.duration * args.sample_rate as f32) as u32;

    // Generate samples based on the requested configuration
    let samples = if args.channels == 1 {
        // Mono white noise - single channel of random samples
        generator.generate_mono(num_samples, args.amplitude)
    } else if args.correlated && args.correlation != 0.0 {
        // Correlated stereo white noise - two channels with specified correlation
        println!(
            "Generating correlated channels with correlation coefficient: {}",
            args.correlation
        );
        generator.generate_correlated_stereo(num_samples, args.amplitude, args.correlation)
    } else {
        // Independent stereo white noise - two channels of independent random samples
        generator.generate_stereo(num_samples, args.amplitude)
    };

    // Write all samples to the WAV file
    // For stereo files, samples are interleaved (L,R,L,R,...)
    for &sample in &samples {
        writer.write_sample(sample)?;
    }

    // Finalize the WAV file to ensure all data is written properly
    writer.finalize()?;
    println!(
        "White noise successfully generated and saved to: {}",
        args.output.display()
    );

    Ok(())
}
