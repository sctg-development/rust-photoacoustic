// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
//! Differential processor utility
//!
//! This binary tool processes WAV files to create differential signals.
//! It can:
//! 1. Process a stereo file and output the difference between channels (L-R or R-L)
//! 2. Process two mono files and output their difference (file1-file2)

use clap::{Parser, ValueEnum};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use rust_photoacoustic::preprocessing::differential;
use std::path::PathBuf;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum DifferentialMode {
    /// Left minus Right (for stereo files)
    LeftMinusRight,
    /// Right minus Left (for stereo files)
    RightMinusLeft,
    /// File1 minus File2 (for two mono files)
    File1MinusFile2,
}

#[derive(Parser, Debug)]
#[command(name = "differential")]
#[command(author = "Romain Lemeill")]
#[command(version = "1.0")]
#[command(about = "Create differential signals from WAV files", long_about = None)]
struct Args {
    /// Input WAV file (stereo or mono)
    #[arg(short = 'i', long)]
    input: PathBuf,

    /// Second input WAV file (only used in File1MinusFile2 mode)
    #[arg(short = '2', long)]
    input2: Option<PathBuf>,

    /// Output WAV file
    #[arg(short, long)]
    output: PathBuf,

    /// Differential mode
    #[arg(short, long, value_enum, default_value_t = DifferentialMode::LeftMinusRight)]
    mode: DifferentialMode,

    /// Gain to apply to the output (multiplier)
    #[arg(short, long, default_value_t = 1.0)]
    gain: f32,
}

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
