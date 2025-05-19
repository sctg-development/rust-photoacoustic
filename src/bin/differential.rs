// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Differential Processor Utility
//!
//! This binary tool processes WAV audio files to create differential signals for photoacoustic analysis.
//! It supports several processing modes to help extract the relevant signal components from raw recordings.
//!
//! ## Features
//!
//! * Process stereo files and output the difference between channels (L-R or R-L)
//! * Process two mono files and output their difference (file1-file2)
//! * Apply gain adjustment to the resulting differential signal
//!
//! ## Usage
//!
//! ```
//! differential --input input.wav --output output.wav --mode LeftMinusRight --gain 1.0
//! ```
//!
//! For File1MinusFile2 mode:
//!
//! ```
//! differential --input file1.wav --input2 file2.wav --output result.wav --mode File1MinusFile2
//! ```
//!
//! ## Applications in Photoacoustic Analysis
//!
//! In photoacoustic spectroscopy, differential signals help isolate the actual acoustic response
//! by removing common mode noise. This is particularly useful when:
//!
//! 1. Using stereo recordings where one channel contains the signal+noise and the other contains just noise
//! 2. Comparing before/after recordings to isolate the effect of a stimulus

use clap::{Parser, ValueEnum};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use rust_photoacoustic::preprocessing::differential;
use std::path::PathBuf;

/// Defines the different modes of differential signal processing.
///
/// The mode determines which channels or files are subtracted from each other
/// to create the output signal.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum DifferentialMode {
    /// Left channel minus Right channel (for stereo files).
    ///
    /// This mode is useful when the left channel contains signal+noise and
    /// the right channel contains a reference noise recording.
    LeftMinusRight,

    /// Right channel minus Left channel (for stereo files).
    ///
    /// This mode is useful when the right channel contains signal+noise and
    /// the left channel contains a reference noise recording.
    RightMinusLeft,

    /// First file minus Second file (for two mono files).
    ///
    /// This mode allows processing two separate recordings, such as with/without
    /// stimulus or before/after a treatment.
    File1MinusFile2,
}

/// Command line arguments for the differential signal processor.
///
/// This structure defines all the parameters that can be provided via command line
/// to control the differential processing of WAV files.
#[derive(Parser, Debug)]
#[command(name = "differential")]
#[command(author = "Ronan LE MEILLAT")]
#[command(version = "1.0")]
#[command(about = "Create differential signals from WAV files", long_about = None)]
struct Args {
    /// Input WAV file (stereo or mono).
    ///
    /// This is the primary input file. For LeftMinusRight and RightMinusLeft modes,
    /// this must be a stereo file. For File1MinusFile2 mode, this should be a mono file.
    #[arg(short = 'i', long)]
    input: PathBuf,

    /// Second input WAV file (only used in File1MinusFile2 mode).
    ///
    /// This file is subtracted from the primary input file. It must be mono
    /// and have the same sample rate and bit depth as the primary input.
    #[arg(short = '2', long)]
    input2: Option<PathBuf>,

    /// Output WAV file path where the differential signal will be saved.
    ///
    /// The output will always be a mono WAV file with the same sample rate
    /// and bit depth as the input.
    #[arg(short, long)]
    output: PathBuf,

    /// Differential mode determining how the signals are combined.
    ///
    /// Specifies which processing method to use: LeftMinusRight (default),
    /// RightMinusLeft, or File1MinusFile2.
    #[arg(short, long, value_enum, default_value_t = DifferentialMode::LeftMinusRight)]
    mode: DifferentialMode,

    /// Gain to apply to the output (multiplier).
    ///
    /// This value multiplies each sample of the differential signal.
    /// Values greater than 1.0 amplify the signal, values less than 1.0 attenuate it.
    /// The result is clamped to prevent digital clipping.
    #[arg(short, long, default_value_t = 1.0)]
    gain: f32,
}

/// Main entry point for the differential signal processing utility.
///
/// Parses command line arguments and routes processing to the appropriate function
/// based on the selected differential mode.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.mode {
        DifferentialMode::LeftMinusRight | DifferentialMode::RightMinusLeft => {
            process_stereo_file(&args)?;
        }
        DifferentialMode::File1MinusFile2 => {
            if args.input2.is_none() {
                return Err(
                    "Second input file (--input2) is required for File1MinusFile2 mode".into(),
                );
            }
            process_two_mono_files(&args)?;
        }
    }

    println!(
        "Differential signal successfully written to {:?}",
        args.output
    );
    Ok(())
}

/// Processes a stereo WAV file to create a differential signal.
///
/// This function handles the LeftMinusRight and RightMinusLeft modes.
/// It reads a stereo file, separates the channels, and creates a mono
/// output file with the difference between the channels.
///
/// # Arguments
///
/// * `args` - Command line arguments containing input/output paths and processing parameters
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - Success or an error with description
///
/// # Errors
///
/// Will return an error if:
/// - The input file cannot be read
/// - The input file is not stereo
/// - There is an issue writing the output file
fn process_stereo_file(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    println!("Reading stereo file {:?}", args.input);

    // Open input WAV file
    let mut reader = WavReader::open(&args.input)?;
    let spec = reader.spec();

    // Check that input is stereo
    if spec.channels != 2 {
        return Err(format!("Input file must be stereo (has {} channels)", spec.channels).into());
    }

    // Read all samples
    let samples: Vec<i16> = reader.samples::<i16>().collect::<Result<Vec<i16>, _>>()?;

    // Calculate output based on mode
    println!("Processing in mode: {:?}", args.mode);

    // Create left and right channel vectors
    let mut left_channel: Vec<i16> = Vec::with_capacity(samples.len() / 2);
    let mut right_channel: Vec<i16> = Vec::with_capacity(samples.len() / 2);

    for i in (0..samples.len()).step_by(2) {
        left_channel.push(samples[i]);
        right_channel.push(samples[i + 1]);
    }

    // Calculate differential signal
    let diff_signal = match args.mode {
        DifferentialMode::LeftMinusRight => {
            differential::calculate_differential(&left_channel, &right_channel)
        }
        DifferentialMode::RightMinusLeft => {
            differential::calculate_differential(&right_channel, &left_channel)
        }
        _ => unreachable!(),
    };

    // Apply gain
    let diff_signal_with_gain: Vec<i16> = diff_signal
        .iter()
        .map(|&sample| {
            let value = sample as f32 * args.gain;
            value.clamp(-32768.0, 32767.0) as i16
        })
        .collect();

    // Create mono output spec
    let out_spec = WavSpec {
        channels: 1,
        sample_rate: spec.sample_rate,
        bits_per_sample: spec.bits_per_sample,
        sample_format: SampleFormat::Int,
    };

    // Write output file
    let mut writer = WavWriter::create(&args.output, out_spec)?;
    for &sample in &diff_signal_with_gain {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;

    Ok(())
}

/// Processes two mono WAV files to create a differential signal.
///
/// This function handles the File1MinusFile2 mode. It reads two mono files,
/// verifies their compatibility, and creates an output file with their difference.
///
/// # Arguments
///
/// * `args` - Command line arguments containing input/output paths and processing parameters
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - Success or an error with description
///
/// # Errors
///
/// Will return an error if:
/// - Either input file cannot be read
/// - Either input file is not mono
/// - The input files have incompatible sample rates or bit depths
/// - There is an issue writing the output file
fn process_two_mono_files(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let input2 = args.input2.as_ref().unwrap(); // Safe because we checked earlier

    println!("Reading first file {:?}", args.input);
    let mut reader1 = WavReader::open(&args.input)?;
    let spec1 = reader1.spec();

    println!("Reading second file {:?}", input2);
    let mut reader2 = WavReader::open(input2)?;
    let spec2 = reader2.spec();

    // Check compatibility
    if spec1.channels != 1 || spec2.channels != 1 {
        return Err("Both input files must be mono for File1MinusFile2 mode".into());
    }

    if spec1.sample_rate != spec2.sample_rate {
        return Err(format!(
            "Sample rates must match (file1: {}, file2: {})",
            spec1.sample_rate, spec2.sample_rate
        )
        .into());
    }

    if spec1.bits_per_sample != spec2.bits_per_sample {
        return Err(format!(
            "Bit depths must match (file1: {}, file2: {})",
            spec1.bits_per_sample, spec2.bits_per_sample
        )
        .into());
    }

    // Read samples
    let samples1: Vec<i16> = reader1.samples::<i16>().collect::<Result<Vec<i16>, _>>()?;
    let samples2: Vec<i16> = reader2.samples::<i16>().collect::<Result<Vec<i16>, _>>()?;

    // Check lengths match
    if samples1.len() != samples2.len() {
        println!("Warning: Files have different lengths. Using the shorter file's length.");
    }

    // Calculate differential signal
    let length = std::cmp::min(samples1.len(), samples2.len());
    let file1 = &samples1[0..length];
    let file2 = &samples2[0..length];

    println!("Calculating differential signal (file1 - file2)");
    let diff_signal = differential::calculate_differential(file1, file2);

    // Apply gain
    let diff_signal_with_gain: Vec<i16> = diff_signal
        .iter()
        .map(|&sample| {
            let value = sample as f32 * args.gain;
            value.clamp(-32768.0, 32767.0) as i16
        })
        .collect();

    // Create output spec
    let out_spec = WavSpec {
        channels: 1,
        sample_rate: spec1.sample_rate,
        bits_per_sample: spec1.bits_per_sample,
        sample_format: SampleFormat::Int,
    };

    // Write output file
    println!("Writing output to {:?}", args.output);
    let mut writer = WavWriter::create(&args.output, out_spec)?;
    for &sample in &diff_signal_with_gain {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;

    Ok(())
}
