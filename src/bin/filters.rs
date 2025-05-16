// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
//! Audio filter utility
//! 
//! This binary tool applies various filters to WAV files.
//! It can:
//! 1. Apply a bandpass filter with configurable center frequency and bandwidth
//! 2. Apply a lowpass filter with configurable cutoff frequency

use clap::{Parser, ValueEnum};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::path::PathBuf;
use rust_photoacoustic::preprocessing::filters::{Filter, BandpassFilter, LowpassFilter};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum FilterType {
    /// Bandpass filter
    Bandpass,
    /// Lowpass filter
    Lowpass,
}

#[derive(Parser, Debug)]
#[command(name = "filters")]
#[command(author = "Ronan Le Meillat")]
#[command(version = "1.0")]
#[command(about = "Apply audio filters to WAV files", long_about = None)]
struct Args {
    /// Input WAV file
    #[arg(short = 'i', long)]
    input: PathBuf,

    /// Output WAV file
    #[arg(short = 'o', long)]
    output: PathBuf,

    /// Filter type
    #[arg(short = 't', long, value_enum, default_value_t = FilterType::Bandpass)]
    filter_type: FilterType,

    /// Center frequency in Hz (for bandpass filter)
    #[arg(short = 'f', long, default_value_t = 2000.0)]
    center_freq: f32,

    /// Bandwidth in Hz (for bandpass filter)
    #[arg(short = 'b', long, default_value_t = 100.0)]
    bandwidth: f32,

    /// Cutoff frequency in Hz (for lowpass filter)
    #[arg(short = 'c', long, default_value_t = 5000.0)]
    cutoff_freq: f32,

    /// Filter order for bandpass filter (must be even)
    #[arg(short = 'n', long, default_value_t = 4)]
    order: usize,

    /// Apply filter to specific channel (none = all channels)
    #[arg(short = 'l', long)]
    channel: Option<usize>,

    /// Gain to apply to the output (multiplier)
    #[arg(short = 'g', long, default_value_t = 1.0)]
    gain: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("Reading WAV file: {:?}", args.input);
    let mut reader = WavReader::open(&args.input)?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    
    println!("Input WAV specifications:");
    println!("- Sample rate: {} Hz", spec.sample_rate);
    println!("- Bits per sample: {}", spec.bits_per_sample);
    println!("- Channels: {}", spec.channels);

    // Read all samples
    let samples: Vec<i16> = reader.samples::<i16>().collect::<Result<Vec<i16>, _>>()?;
    
    // Process based on filter type
    println!("Processing with filter: {:?}", args.filter_type);
    
    // Convert samples to f32 for filtering
    let channels = spec.channels as usize;
    let mut channel_samples = vec![Vec::new(); channels];
    
    // Split samples into channels
    for (i, &sample) in samples.iter().enumerate() {
        channel_samples[i % channels].push(sample as f32 / 32768.0);
    }
    
    // Create the appropriate filter
    let filter: Box<dyn Filter> = match args.filter_type {
        FilterType::Bandpass => {
            println!("Bandpass filter parameters:");
            println!("- Center frequency: {:.1} Hz", args.center_freq);
            println!("- Bandwidth: {:.1} Hz", args.bandwidth);
            println!("- Order: {}", args.order);
            
            Box::new(
                BandpassFilter::new(args.center_freq, args.bandwidth)
                    .with_sample_rate(sample_rate)
                    .with_order(args.order)
            )
        },
        FilterType::Lowpass => {
            println!("Lowpass filter parameters:");
            println!("- Cutoff frequency: {:.1} Hz", args.cutoff_freq);
            
            Box::new(
                LowpassFilter::new(args.cutoff_freq)
                    .with_sample_rate(sample_rate)
            )
        }
    };
    
    // Apply filter to channels
    let filtered_channels = match args.channel {
        Some(ch) if ch < channels => {
            println!("Filtering only channel {}", ch);
            let mut result = channel_samples.clone();
            result[ch] = filter.apply(&channel_samples[ch]);
            result
        },
        _ => {
            println!("Filtering all channels");
            channel_samples
                .iter()
                .map(|samples| filter.apply(samples))
                .collect()
        }
    };
    
    // Interleave and convert back to i16
    let mut output_samples = Vec::with_capacity(samples.len());
    
    for i in 0..filtered_channels[0].len() {
        for ch in 0..channels {
            let sample = filtered_channels[ch][i] * args.gain;
            let value = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            output_samples.push(value);
        }
    }
    
    // Write output WAV file
    println!("Writing output to {:?}", args.output);
    let mut writer = WavWriter::create(&args.output, spec)?;
    
    for &sample in &output_samples {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    
    println!("Filtering complete!");
    Ok(())
}
