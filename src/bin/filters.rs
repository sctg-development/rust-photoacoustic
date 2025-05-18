// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Audio Filter Utility
//!
//! This binary tool applies various digital signal processing filters to WAV audio files.
//! It's particularly useful for processing photoacoustic measurements to isolate relevant 
//! frequency components and reduce noise.
//!
//! ## Available Filters
//!
//! * **Bandpass Filter**: Isolates a specific frequency band, defined by center frequency and bandwidth
//! * **Lowpass Filter**: Removes high-frequency components above a specified cutoff frequency
//!
//! ## Usage Examples
//!
//! Apply a bandpass filter centered at 2000 Hz with 100 Hz bandwidth:
//! ```
//! filters --input input.wav --output filtered.wav --filter-type Bandpass --center-freq 2000 --bandwidth 100
//! ```
//!
//! Apply a lowpass filter with 5000 Hz cutoff:
//! ```
//! filters --input input.wav --output filtered.wav --filter-type Lowpass --cutoff-freq 5000
//! ```
//!
//! Filter only the left channel of a stereo file (channel 0):
//! ```
//! filters --input stereo.wav --output filtered.wav --filter-type Bandpass --channel 0
//! ```
//!
//! ## Photoacoustic Applications
//!
//! In photoacoustic spectroscopy, filtering is essential for:
//!
//! 1. Isolating the resonant frequency of photoacoustic cells
//! 2. Removing environmental noise (e.g., mechanical vibrations, electrical interference)
//! 3. Enhancing signal-to-noise ratio for weak photoacoustic signals
//! 4. Pre-processing signals before further analysis or visualization

use clap::{Parser, ValueEnum};
use hound::{WavReader, WavWriter};
use rust_photoacoustic::preprocessing::filters::{BandpassFilter, Filter, LowpassFilter};
use std::path::PathBuf;

/// Types of audio filters available in this utility.
///
/// This enum defines the different filter algorithms that can be applied to the audio data.
/// Each filter type has different parameters and is optimized for specific audio processing tasks.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum FilterType {
    /// Bandpass filter - isolates frequencies within a specific range.
    ///
    /// A bandpass filter allows signals between two specific frequencies to pass through
    /// while attenuating frequencies outside that range. This is useful for isolating
    /// a specific frequency component, such as the resonance frequency in photoacoustic cells.
    /// 
    /// Parameters:
    /// - Center frequency (Hz)
    /// - Bandwidth (Hz)
    /// - Filter order (must be even)
    Bandpass,
    
    /// Lowpass filter - allows low frequencies to pass through while attenuating higher frequencies.
    ///
    /// A lowpass filter attenuates frequencies higher than the cutoff frequency.
    /// It's useful for removing high-frequency noise while preserving the main signal components.
    ///
    /// Parameters:
    /// - Cutoff frequency (Hz)
    Lowpass,
}

/// Command line arguments for the audio filter utility.
///
/// This structure defines the parameters that control how filters are applied
/// to the input audio data. It includes input/output file paths, filter selection,
/// and filter-specific parameters.
#[derive(Parser, Debug)]
#[command(name = "filters")]
#[command(author = "Ronan Le Meillat")]
#[command(version = "1.0")]
#[command(about = "Apply audio filters to WAV files", long_about = None)]
struct Args {
    /// Input WAV file path.
    ///
    /// The path to the WAV file that will be processed. The file must be a valid WAV file
    /// with PCM encoding. Both mono and multi-channel files are supported.
    #[arg(short = 'i', long)]
    input: PathBuf,

    /// Output WAV file path.
    ///
    /// The path where the filtered WAV file will be saved. The output file will have
    /// the same format (sample rate, bit depth, channels) as the input file.
    #[arg(short = 'o', long)]
    output: PathBuf,

    /// Type of filter to apply.
    ///
    /// Selects which filtering algorithm to use. Different filters have different
    /// parameters and are suited for different audio processing tasks.
    #[arg(short = 't', long, value_enum, default_value_t = FilterType::Bandpass)]
    filter_type: FilterType,

    /// Center frequency in Hz for the bandpass filter.
    ///
    /// Specifies the center of the frequency band to isolate with a bandpass filter.
    /// This parameter is only used when filter_type is Bandpass.
    /// For photoacoustic applications, this is typically set to the resonance
    /// frequency of the photoacoustic cell.
    #[arg(short = 'f', long, default_value_t = 2000.0)]
    center_freq: f32,

    /// Bandwidth in Hz for the bandpass filter.
    ///
    /// Specifies the width of the frequency band to isolate. A narrower bandwidth
    /// provides better frequency selectivity but may introduce more ringing artifacts.
    /// This parameter is only used when filter_type is Bandpass.
    #[arg(short = 'b', long, default_value_t = 100.0)]
    bandwidth: f32,

    /// Cutoff frequency in Hz for the lowpass filter.
    ///
    /// Specifies the frequency above which signals will be attenuated.
    /// This parameter is only used when filter_type is Lowpass.
    #[arg(short = 'c', long, default_value_t = 5000.0)]
    cutoff_freq: f32,

    /// Filter order for bandpass filter (must be even).
    ///
    /// Controls the steepness of the filter roll-off and the precision of the filter.
    /// Higher orders provide sharper cutoffs but introduce more delay and potential artifacts.
    /// Must be an even number for the bandpass implementation.
    /// This parameter is primarily used for the bandpass filter.
    #[arg(short = 'n', long, default_value_t = 4)]
    order: usize,

    /// Apply filter to specific channel (none = all channels).
    ///
    /// When specified, the filter will only be applied to the indicated channel
    /// and other channels will remain unmodified. Channels are zero-indexed
    /// (0 = first channel, 1 = second channel, etc.).
    /// When not specified, all channels will be processed.
    #[arg(short = 'l', long)]
    channel: Option<usize>,

    /// Gain to apply to the output (multiplier).
    ///
    /// Multiplies the amplitude of the output signal after filtering.
    /// Values greater than 1.0 increase the volume, values less than 1.0 decrease it.
    /// The result is clamped to prevent digital clipping.
    #[arg(short = 'g', long, default_value_t = 1.0)]
    gain: f32,
}

/// Main entry point for the audio filter utility.
///
/// This function parses the command line arguments, reads the input WAV file,
/// applies the selected filter, and writes the result to the output file.
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - Success or an error with description
///
/// # Errors
///
/// Will return an error if:
/// - The input file cannot be read or is not a valid WAV file
/// - There is an issue writing the output file
/// - The specified channel is out of range
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();

    println!("Reading WAV file: {:?}", args.input);
    let mut reader = WavReader::open(&args.input)?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;

    // Display input file information
    println!("Input WAV specifications:");
    println!("- Sample rate: {} Hz", spec.sample_rate);
    println!("- Bits per sample: {}", spec.bits_per_sample);
    println!("- Channels: {}", spec.channels);

    // Read all samples from input file
    let samples: Vec<i16> = reader.samples::<i16>().collect::<Result<Vec<i16>, _>>()?;

    // Process based on filter type
    println!("Processing with filter: {:?}", args.filter_type);

    // Convert samples to f32 for filtering (normalized to range [-1.0, 1.0])
    let channels = spec.channels as usize;
    let mut channel_samples = vec![Vec::new(); channels];

    // Split interleaved samples into separate channel vectors
    for (i, &sample) in samples.iter().enumerate() {
        channel_samples[i % channels].push(sample as f32 / 32768.0);
    }

    // Create the appropriate filter based on user selection
    let filter: Box<dyn Filter> = match args.filter_type {
        FilterType::Bandpass => {
            // Configure and create bandpass filter
            println!("Bandpass filter parameters:");
            println!("- Center frequency: {:.1} Hz", args.center_freq);
            println!("- Bandwidth: {:.1} Hz", args.bandwidth);
            println!("- Order: {}", args.order);

            Box::new(
                BandpassFilter::new(args.center_freq, args.bandwidth)
                    .with_sample_rate(sample_rate)
                    .with_order(args.order),
            )
        }
        FilterType::Lowpass => {
            // Configure and create lowpass filter
            println!("Lowpass filter parameters:");
            println!("- Cutoff frequency: {:.1} Hz", args.cutoff_freq);

            Box::new(LowpassFilter::new(args.cutoff_freq).with_sample_rate(sample_rate))
        }
    };

    // Apply filter to specified channel(s)
    let filtered_channels = match args.channel {
        // Filter only the specified channel
        Some(ch) if ch < channels => {
            println!("Filtering only channel {}", ch);
            let mut result = channel_samples.clone();
            result[ch] = filter.apply(&channel_samples[ch]);
            result
        }
        // Filter all channels
        _ => {
            println!("Filtering all channels");
            channel_samples
                .iter()
                .map(|samples| filter.apply(samples))
                .collect()
        }
    };

    // Interleave channels and convert back to i16 samples
    let mut output_samples = Vec::with_capacity(samples.len());

    for i in 0..filtered_channels[0].len() {
        for ch in 0..channels {
            // Apply gain and convert to i16, with clipping protection
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
