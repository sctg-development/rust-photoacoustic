// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Spectrum Analyzer
//!
//! A simple command-line tool to analyze the frequency spectrum of WAV files.
//! This tool is specifically designed to verify the output of the noise generator
//! and check for the presence of specific frequency components.

use clap::Parser;
use hound::WavReader;
use rustfft::{num_complex::Complex, FftPlanner};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Unicode block characters for vertical bar charts (U+2580-U+258F)
const CHART_CHARS: [char; 9] = [
    ' ', // U+0020 - Empty
    '▁', // U+2581 - Lower one eighth block
    '▂', // U+2582 - Lower one quarter block
    '▃', // U+2583 - Lower three eighths block
    '▄', // U+2584 - Lower half block
    '▅', // U+2585 - Lower five eighths block
    '▆', // U+2586 - Lower three quarters block
    '▇', // U+2587 - Lower seven eighths block
    '█', // U+2588 - Full block
];

/// Convert a normalized value (0.0-1.0) to a vertical bar character
fn value_to_chart_char(value: f32) -> char {
    let index = (value * (CHART_CHARS.len() - 1) as f32).round() as usize;
    CHART_CHARS[index.min(CHART_CHARS.len() - 1)]
}

/// Display spectrum as a vertical bar chart with 10 lines height
fn display_spectrum_chart(
    power_spectrum: &[f32],
    freq_resolution: f32,
    start_bin: usize,
    end_bin: usize,
    noise_floor: f32,
    max_bin: usize,
    target_bin: usize,
    target_frequency: f32,
) {
    println!();
    println!(
        "Spectrum Chart (frequency range: {:.1} - {:.1} Hz):",
        start_bin as f32 * freq_resolution,
        end_bin as f32 * freq_resolution
    );

    // Calculate the range for normalization (in dB relative to noise floor)
    let mut max_db = -60.0f32;
    let mut min_db = 60.0f32;

    // Find actual range in the analysis window
    for i in start_bin..=end_bin {
        let power_db = 10.0 * (power_spectrum[i] / noise_floor).log10();
        max_db = max_db.max(power_db);
        min_db = min_db.min(power_db);
    }

    // Extend range slightly for better visualization
    let range_db = max_db - min_db;
    println!(
        "Power range: {:.1} to {:.1} dB (relative to noise floor)",
        min_db, max_db
    );
    println!();

    // Calculate how many bins to display (max 80 characters wide)
    let total_bins = end_bin - start_bin + 1;
    let max_display_width = 80;
    let bin_step = if total_bins <= max_display_width {
        1
    } else {
        (total_bins + max_display_width - 1) / max_display_width
    };

    // Calculate bar heights (0-10 scale)
    let chart_height = 10;
    let mut bar_heights = Vec::new();
    let mut freq_labels = Vec::new();
    let mut markers = Vec::new();

    for display_pos in 0..((total_bins + bin_step - 1) / bin_step) {
        let bin_index = start_bin + display_pos * bin_step;
        if bin_index > end_bin {
            break;
        }

        // Average power over the bin_step range for smoother display
        let mut avg_power = 0.0;
        let mut count = 0;
        for i in 0..bin_step {
            let actual_bin = bin_index + i;
            if actual_bin <= end_bin {
                avg_power += power_spectrum[actual_bin];
                count += 1;
            }
        }
        avg_power /= count as f32;

        let power_db = 10.0 * (avg_power / noise_floor).log10();
        let normalized_value = if range_db > 0.0 {
            ((power_db - min_db) / range_db).clamp(0.0, 1.0)
        } else {
            0.5 // Default middle value if no range
        };

        // Convert to bar height (0-80 scale for sub-character precision)
        let bar_height_eighths = (normalized_value * chart_height as f32 * 8.0).round() as usize;
        bar_heights.push(bar_height_eighths);

        // Store frequency for labels (every 10 characters or at important points)
        let freq = bin_index as f32 * freq_resolution;
        if display_pos % 10 == 0 || bin_index == max_bin || bin_index == target_bin {
            freq_labels.push((display_pos, freq));
        } // Mark important points
        if bin_index <= max_bin && max_bin < bin_index + bin_step {
            markers.push((display_pos, "P")); // Peak
        } else if bin_index <= target_bin && target_bin < bin_index + bin_step {
            markers.push((display_pos, "T")); // Target
        }
    }

    // Display the chart from top to bottom (line by line) with scale on the left
    for line in (0..chart_height).rev() {
        // Calculate the dB value for this line
        let db_value = min_db + (line as f32 + 0.5) * range_db / chart_height as f32;

        // Build the chart line
        let mut chart_line = String::new();
        for &bar_height_eighths in &bar_heights {
            let full_blocks = bar_height_eighths / 8;
            let partial_eighths = bar_height_eighths % 8;

            if full_blocks > line {
                // Full block
                chart_line.push('█');
            } else if full_blocks == line && partial_eighths > 0 {
                // Partial block at the top of this line
                chart_line.push(CHART_CHARS[partial_eighths]);
            } else {
                // Empty space
                chart_line.push(' ');
            }
        }

        // Print scale + chart line
        println!("{:6.1} dB ┤{}", db_value, chart_line);
    }

    // Print bottom scale line
    let bottom_line = format!(
        "{:6.1} dB └{}",
        min_db,
        "─".repeat(bar_heights.len().min(80))
    );
    println!("{}", bottom_line);

    // Display markers below the chart
    if !markers.is_empty() {
        let mut marker_line = vec![' '; bar_heights.len()];
        for (pos, marker) in markers {
            if pos < marker_line.len() {
                marker_line[pos] = marker.chars().next().unwrap();
            }
        }
        println!("         {}", marker_line.iter().collect::<String>());
    }

    // Display frequency labels
    println!();
    let mut label_positions = String::new();
    for (i, (pos, freq)) in freq_labels.iter().enumerate() {
        // Calculate spacing from previous label
        let prev_end = if i == 0 {
            0
        } else {
            let (prev_pos, prev_freq) = freq_labels[i - 1];
            prev_pos + format!("{:.0}Hz", prev_freq).len()
        };

        let current_start = *pos;
        let spacing_needed = current_start.saturating_sub(prev_end);

        if i > 0 && spacing_needed > 0 {
            label_positions.push_str(&" ".repeat(spacing_needed));
        }

        label_positions.push_str(&format!("{:.0}Hz", freq));
    }
    // Add spacing to align with the chart (account for the scale labels)
    println!("         {}", label_positions);
    println!();
    println!(
        "Legend: P=Peak, T=Target frequency ({:.0}Hz)",
        target_frequency
    );
}

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

    /// Display spectrum as vertical bar chart
    #[arg(long, short = 'c')]
    chart: bool,

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
    } // Show full spectrum if requested
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

    // Show chart if requested
    if args.chart {
        display_spectrum_chart(
            &power_spectrum,
            freq_resolution,
            start_bin,
            end_bin,
            noise_floor,
            max_bin,
            target_bin,
            args.target_frequency,
        );
    }

    Ok(())
}
