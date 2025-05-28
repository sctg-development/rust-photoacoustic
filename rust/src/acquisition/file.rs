// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Audio acquisition module
//!
//! This module handles the acquisition of audio data from files.

use super::AudioSource;
use anyhow::Result;
use std::path::Path;

/// Audio source that reads from a WAV file using hound
pub struct FileSource {
    // In a real implementation, this would hold the hound WavReader
    sample_rate: u32,
    frame_size: usize,
    position: usize,
    total_frames: usize,
}

impl FileSource {
    /// Create a new FileSource for the given WAV file
    pub fn new<P: AsRef<Path>>(file_path: P) -> Result<Self> {
        // For mock implementation, we'll just return a dummy source
        println!("Reading WAV file: {}", file_path.as_ref().display());

        Ok(Self {
            sample_rate: 48000,
            frame_size: 1024,
            position: 0,
            total_frames: 48000 * 10, // 10 seconds of audio
        })
    }
}

impl AudioSource for FileSource {
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)> {
        if self.position >= self.total_frames {
            // End of file, return empty buffers
            return Ok((Vec::new(), Vec::new()));
        }

        // Mock implementation generates sine waves with noise
        let mut channel_a = Vec::with_capacity(self.frame_size);
        let mut channel_b = Vec::with_capacity(self.frame_size);

        for i in 0..self.frame_size {
            if self.position + i >= self.total_frames {
                break;
            }

            let t = (self.position + i) as f32 / self.sample_rate as f32;
            let freq = 2000.0; // 2 kHz

            // Add some noise to make it more realistic
            let noise_a = rand::random::<f32>() * 0.1;
            let noise_b = rand::random::<f32>() * 0.1;

            let sample_a = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5 + noise_a;
            let sample_b = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.3 + noise_b;

            channel_a.push(sample_a);
            channel_b.push(sample_b);
        }

        self.position += self.frame_size;

        Ok((channel_a, channel_b))
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}
