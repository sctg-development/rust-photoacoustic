// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Audio acquisition module
//!
//! This module handles the acquisition of audio data from files.

use super::AudioSource;
use anyhow::{anyhow, Result};
use hound::{WavReader, WavSpec};
use log::debug;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Audio source that reads from a WAV file using hound
pub struct FileSource {
    reader: WavReader<BufReader<File>>,
    spec: WavSpec,
    frame_size: usize,
    samples_read: usize,
}

impl FileSource {
    /// Create a new FileSource for the given WAV file
    pub fn new<P: AsRef<Path>>(file_path: P) -> Result<Self> {
        let file = File::open(&file_path)?;
        let buf_reader = BufReader::new(file);
        let reader = WavReader::new(buf_reader)?;
        let spec = reader.spec();

        // Validate that the file is stereo
        if spec.channels != 2 {
            return Err(anyhow!(
                "WAV file must be stereo (2 channels), got {} channels",
                spec.channels
            ));
        }

        // Use a reasonable frame size (e.g., ~20ms of audio at 48kHz = ~1000 samples)
        let frame_size = (spec.sample_rate as f64 * 0.02) as usize; // 20ms

        println!("Opened WAV file: {}", file_path.as_ref().display());
        println!("  Sample rate: {} Hz", spec.sample_rate);
        println!("  Channels: {}", spec.channels);
        println!("  Bits per sample: {}", spec.bits_per_sample);
        println!("  Sample format: {:?}", spec.sample_format);
        println!("  Frame size: {} samples per channel", frame_size);

        Ok(Self {
            reader,
            spec,
            frame_size,
            samples_read: 0,
        })
    }
}

impl AudioSource for FileSource {
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)> {
        let mut channel_a = Vec::with_capacity(self.frame_size);
        let mut channel_b = Vec::with_capacity(self.frame_size);

        // Read frame_size samples for each channel (interleaved stereo)
        match self.spec.sample_format {
            hound::SampleFormat::Int => {
                // Read as i16 and convert to f32
                let samples: Result<Vec<i16>, _> = self
                    .reader
                    .samples::<i16>()
                    .take(self.frame_size * 2) // frame_size samples per channel * 2 channels
                    .collect();

                match samples {
                    Ok(sample_vec) => {
                        if sample_vec.is_empty() {
                            println!(
                                "Reached end of WAV file after reading {} total samples",
                                self.samples_read
                            );
                            return Ok((Vec::new(), Vec::new()));
                        }

                        // Convert interleaved stereo to separate channels
                        for chunk in sample_vec.chunks_exact(2) {
                            let left = chunk[0] as f32 / i16::MAX as f32;
                            let right = chunk[1] as f32 / i16::MAX as f32;
                            channel_a.push(left);
                            channel_b.push(right);
                        }

                        self.samples_read += sample_vec.len();
                    }
                    Err(e) => {
                        println!("Error reading samples: {:?}", e);
                        return Ok((Vec::new(), Vec::new()));
                    }
                }
            }
            hound::SampleFormat::Float => {
                // Read as f32
                let samples: Result<Vec<f32>, _> = self
                    .reader
                    .samples::<f32>()
                    .take(self.frame_size * 2) // frame_size samples per channel * 2 channels
                    .collect();

                match samples {
                    Ok(sample_vec) => {
                        if sample_vec.is_empty() {
                            println!(
                                "Reached end of WAV file after reading {} total samples",
                                self.samples_read
                            );
                            return Ok((Vec::new(), Vec::new()));
                        }

                        // Convert interleaved stereo to separate channels
                        for chunk in sample_vec.chunks_exact(2) {
                            channel_a.push(chunk[0]);
                            channel_b.push(chunk[1]);
                        }

                        self.samples_read += sample_vec.len();
                    }
                    Err(e) => {
                        println!("Error reading samples: {:?}", e);
                        return Ok((Vec::new(), Vec::new()));
                    }
                }
            }
        };

        // If we couldn't read any samples, we've reached the end
        if channel_a.is_empty() {
            println!(
                "Reached end of WAV file after reading {} total samples",
                self.samples_read
            );
            return Ok((Vec::new(), Vec::new()));
        }

        // show debug information each 30s only
        if self.samples_read % (self.spec.sample_rate as usize * 30) == 0 {
            debug!(
                "Read {} samples from WAV file (total samples read: {})",
                channel_a.len(),
                self.samples_read
            );
        }

        Ok((channel_a, channel_b))
    }

    fn sample_rate(&self) -> u32 {
        self.spec.sample_rate
    }
}
