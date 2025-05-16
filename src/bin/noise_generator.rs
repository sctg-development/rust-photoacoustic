// White noise generator for audio testing
// Generates stereo Gaussian white noise at different sample rates

use hound::{WavSpec, WavWriter, SampleFormat};
use clap::Parser;
use std::path::PathBuf;
use std::time::SystemTime;

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
        44100 | 48000 | 192000 => {},
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
    
    // Create a simple random number generator
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as u32;
    let mut rng_state = seed;
    
    // Function to generate random f32 between -1 and 1 using xorshift
    let mut random_float = move || {
        // XOR Shift algorithm for pseudo-random numbers
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 17;
        rng_state ^= rng_state << 5;
        
        // Convert to float between -1.0 and 1.0
        let random_u32 = rng_state;
        (random_u32 as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    
    // Create random gaussian distribution using Box-Muller transform
    let mut random_gaussian = move || {
        // Use Box-Muller transform to generate gaussian distributed values
        let u1 = (random_float() + 1.0) / 2.0; // remap to (0,1)
        let u2 = (random_float() + 1.0) / 2.0;
        
        // Avoid ln(0)
        let u1 = if u1 < 0.0001 { 0.0001 } else { u1 };
        
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
        z
    };
    
    // Calculate number of samples
    let num_samples = (args.duration * args.sample_rate as f32) as u32;
    
    // Generate and write samples
    if args.channels == 1 {
        // Mono
        for _ in 0..num_samples {
            // Generate a random Gaussian value using our Box-Muller transform
            let sample = random_gaussian() * args.amplitude;
            
            // Convert to i16 and clamp to prevent overflow
            let value = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            writer.write_sample(value)?;
        }
    } else {
        // Stereo (with optional correlation)
        if args.correlated && args.correlation != 0.0 {
            println!("Generating correlated channels with correlation coefficient: {}", args.correlation);
            
            // For correlated noise, we generate one channel and then create the other
            // using the correlation coefficient
            let sqrt_one_minus_corr_squared = (1.0 - args.correlation * args.correlation).sqrt();
            
            for _ in 0..num_samples {
                // Generate first channel - gaussian distribution
                let sample1 = random_gaussian() * args.amplitude;
                
                // Generate second channel with specified correlation
                let independent_sample = random_gaussian();
                let sample2 = (args.correlation * sample1 + sqrt_one_minus_corr_squared * independent_sample) * args.amplitude;
                
                // Convert to i16 and clamp to prevent overflow
                let value1 = (sample1 * 32767.0).clamp(-32768.0, 32767.0) as i16;
                let value2 = (sample2 * 32767.0).clamp(-32768.0, 32767.0) as i16;
                
                writer.write_sample(value1)?;
                writer.write_sample(value2)?;
            }
        } else {
            // Independent channels
            for _ in 0..num_samples {
                // Generate independent gaussian samples for each channel
                let sample_left = random_gaussian() * args.amplitude;
                let sample_right = random_gaussian() * args.amplitude;
                
                // Convert to i16 and clamp to prevent overflow
                let value_left = (sample_left * 32767.0).clamp(-32768.0, 32767.0) as i16;
                let value_right = (sample_right * 32767.0).clamp(-32768.0, 32767.0) as i16;
                
                writer.write_sample(value_left)?;
                writer.write_sample(value_right)?;
            }
        }
    }
    
    // Finalize the WAV file
    writer.finalize()?;
    println!("White noise successfully generated and saved to: {}", args.output.display());
    
    Ok(())
}
