// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Spectrum Analyzer
//!
//! A simple command-line tool to analyze the frequency spectrum of WAV files.
//! This tool is specifically designed to verify the output of the noise generator
//! and check for the presence of specific frequency components.

use clap::Parser;
use hound::{WavReader, WavSpec};
use rustfft::{num_complex::Complex, FftPlanner};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[derive(Parser)]
#[command(name = "analyze_spectrum")]
#[command(about = "Analyze the frequency spectrum of a WAV file")]
struct Args {
    /// Input WAV file to analyze
    #[arg(value_name = "INPUT_FILE")]
    input: String,

    /// Target frequency to check (in Hz)
    #[arg(short, long, default_value_t = 2000.0)]
    target_frequency: f32,

    /// Frequency range around target to analyze (±Hz)
    #[arg(short, long, default_value_t = 100.0)]
    range: f32,
    /// FFT size (power of 2)
    #[arg(long, default_value_t = 8192)]
    fft_size: usize,
    /// Show full spectrum in analyzed range
    #[arg(long, short = 'f')]
    full_spectrum: bool,

    /// Position in seconds from start of file to analyze
    #[arg(short, long)]
    position: Option<f32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Check if input file exists
    if !Path::new(&args.input).exists() {
        eprintln!("Error: Input file '{}' does not exist", args.input);
        std::process::exit(1);
    }
    println!("Analyzing spectrum of: {}", args.input);
    println!("Target frequency: {} Hz", args.target_frequency);
    println!("Analysis range: ±{} Hz", args.range);
    println!("FFT size: {}", args.fft_size);
    if let Some(pos) = args.position {
        println!("Analysis position: {:.3}s", pos);
    }
    println!();

    // Read WAV file
    let file = File::open(&args.input)?;
    let reader = BufReader::new(file);
    let mut wav_reader = WavReader::new(reader)?;

    let spec = wav_reader.spec();
    println!("WAV file information:");
    println!("  Sample rate: {} Hz", spec.sample_rate);
    println!("  Channels: {}", spec.channels);
    println!("  Bits per sample: {}", spec.bits_per_sample);
    println!("  Sample format: {:?}", spec.sample_format);

    // Read samples (use only left channel if stereo)
    let samples: Vec<f32> = if spec.channels == 1 {
        wav_reader
            .samples::<i16>()
            .map(|s| s.unwrap() as f32 / 32768.0)
            .collect()
    } else {
        wav_reader
            .samples::<i16>()
            .enumerate()
            .filter(|(i, _)| i % spec.channels as usize == 0) // Take only left channel
            .map(|(_, s)| s.unwrap() as f32 / 32768.0)
            .collect()
    };

    let duration = samples.len() as f32 / spec.sample_rate as f32;
    println!("  Duration: {:.2} seconds", duration);
    println!("  Total samples: {}", samples.len());
    println!(); // Perform FFT analysis
    let fft_size = args.fft_size.min(samples.len());
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size); // Calculate start sample based on position or use middle of signal
    let start_sample = if let Some(position_sec) = args.position {
        let position_sample = (position_sec * spec.sample_rate as f32) as usize;

        // Validate that the analysis window fits within the file
        if position_sample + fft_size > samples.len() {
            let max_position_sec =
                (samples.len().saturating_sub(fft_size)) as f32 / spec.sample_rate as f32;
            eprintln!(
                "Error: Position {:.3}s would place analysis window beyond file end",
                position_sec
            );
            eprintln!(
                "       File length: {:.3}s",
                samples.len() as f32 / spec.sample_rate as f32
            );
            eprintln!(
                "       Analysis window: {:.3}s",
                fft_size as f32 / spec.sample_rate as f32
            );
            eprintln!("       Maximum allowed position: {:.3}s", max_position_sec);
            std::process::exit(1);
        }

        position_sample
    } else {
        // Use middle portion of the signal for analysis
        if samples.len() > fft_size {
            (samples.len() - fft_size) / 2
        } else {
            0
        }
    };

    // Display analysis window information
    let analysis_start_time = start_sample as f32 / spec.sample_rate as f32;
    let analysis_end_time = (start_sample + fft_size) as f32 / spec.sample_rate as f32;
    println!(
        "Analysis window: {:.3}s - {:.3}s ({} samples)",
        analysis_start_time, analysis_end_time, fft_size
    );

    // Apply Hann window and prepare complex input
    let mut buffer: Vec<Complex<f32>> = (0..fft_size)
        .map(|i| {
            let sample = if start_sample + i < samples.len() {
                samples[start_sample + i]
            } else {
                0.0
            };
            // Apply Hann window
            let window_val =
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos());
            Complex::new(sample * window_val, 0.0)
        })
        .collect();

    // Perform FFT
    fft.process(&mut buffer);

    // Calculate power spectrum
    let power_spectrum: Vec<f32> = buffer
        .iter()
        .take(fft_size / 2) // Only positive frequencies
        .map(|c| c.norm_sqr())
        .collect();

    // Calculate frequency bins
    let freq_resolution = spec.sample_rate as f32 / fft_size as f32;
    println!("Frequency resolution: {:.2} Hz", freq_resolution);

    // Find peak in target frequency range
    let target_bin = (args.target_frequency / freq_resolution) as usize;
    let range_bins = (args.range / freq_resolution) as usize;

    let start_bin = target_bin.saturating_sub(range_bins);
    let end_bin = (target_bin + range_bins).min(power_spectrum.len() - 1);

    println!(
        "Analyzing frequency range: {:.1} - {:.1} Hz",
        start_bin as f32 * freq_resolution,
        end_bin as f32 * freq_resolution
    );

    // Find the peak in the target range
    let mut max_power = 0.0f32;
    let mut max_bin = start_bin;
    let mut noise_floor = 0.0f32;

    // Calculate noise floor (average power excluding the peak region)
    let mut noise_samples = 0;
    for i in 0..power_spectrum.len() {
        if i < start_bin || i > end_bin {
            noise_floor += power_spectrum[i];
            noise_samples += 1;
        }
    }
    noise_floor /= noise_samples as f32;

    // Find peak in target range
    for i in start_bin..=end_bin {
        if power_spectrum[i] > max_power {
            max_power = power_spectrum[i];
            max_bin = i;
        }
    }

    let peak_frequency = max_bin as f32 * freq_resolution;
    let snr_linear = max_power / noise_floor;
    let snr_db = 10.0 * snr_linear.log10();

    println!();
    println!("Analysis Results:");
    println!("  Peak frequency: {:.1} Hz", peak_frequency);
    println!("  Peak power: {:.2e}", max_power);
    println!("  Noise floor: {:.2e}", noise_floor);
    println!("  SNR: {:.1} dB", snr_db);

    let freq_error = (peak_frequency - args.target_frequency).abs();
    println!("  Frequency error: {:.1} Hz", freq_error); // Check if signal is detectable
    if snr_db > 6.0 {
        println!("  Status: ✓ Signal clearly detectable!");
    } else if snr_db > 3.0 {
        println!("  Status: ⚠ Signal weakly detectable");
    } else {
        println!("  Status: ✗ Signal not detectable above noise floor");
    } // Show spectrum around target frequency
    println!();
    println!("Spectrum around target frequency:");
    println!("Frequency (Hz) | Power | Power (dB)");
    println!("{}", "-".repeat(40));

    // Show spectrum around the target frequency, with a reasonable range
    let display_range = 10; // Show ±10 bins around the target
    let target_display_start = target_bin.saturating_sub(display_range).max(start_bin);
    let target_display_end = (target_bin + display_range)
        .min(end_bin)
        .min(power_spectrum.len() - 1);

    for i in target_display_start..=target_display_end {
        let freq = i as f32 * freq_resolution;
        let power = power_spectrum[i];
        let power_db = 10.0 * (power / noise_floor).log10();
        let mut marker = "";
        if i == max_bin {
            marker = " ←PEAK";
        } else if i == target_bin {
            marker = " ←TARGET";
        }
        println!(
            "{:10.1} | {:8.2e} | {:7.1} dB{}",
            freq, power, power_db, marker
        );
    } // If peak is outside the displayed range, show spectrum around peak too
    if max_bin < target_display_start || max_bin > target_display_end {
        println!();
        println!("Spectrum around detected peak:");
        println!("Frequency (Hz) | Power | Power (dB)");
        println!("{}", "-".repeat(40));

        let peak_display_start = max_bin.saturating_sub(display_range).max(start_bin);
        let peak_display_end = (max_bin + display_range)
            .min(end_bin)
            .min(power_spectrum.len() - 1);

        for i in peak_display_start..=peak_display_end {
            let freq = i as f32 * freq_resolution;
            let power = power_spectrum[i];
            let power_db = 10.0 * (power / noise_floor).log10();
            let marker = if i == max_bin { " ←PEAK" } else { "" };
            println!(
                "{:10.1} | {:8.2e} | {:7.1} dB{}",
                freq, power, power_db, marker
            );
        }
    }

    // Show full spectrum if requested
    if args.full_spectrum {
        println!();
        println!("Full spectrum in analyzed range:");
        println!("Frequency (Hz) | Power | Power (dB)");
        println!("{}", "-".repeat(40));

        for i in start_bin..=end_bin {
            let freq = i as f32 * freq_resolution;
            let power = power_spectrum[i];
            let power_db = 10.0 * (power / noise_floor).log10();
            let mut marker = "";
            if i == max_bin {
                marker = " ←PEAK";
            } else if i == target_bin {
                marker = " ←TARGET";
            }
            println!(
                "{:10.1} | {:8.2e} | {:7.1} dB{}",
                freq, power, power_db, marker
            );
        }
    }

    Ok(())
}
