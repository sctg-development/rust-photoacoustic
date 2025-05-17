// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Audio acquisition module
//!
//! This module handles the acquisition of audio data from microphones
//! or from WAV files.

use anyhow::{Context, Result};
use rand::Rng;
use std::path::Path;

/// Represents an audio source (either live or from file)
pub trait AudioSource: Send {
    /// Read the next frame of audio data from both channels
    /// Returns a tuple containing (channel_A, channel_B) data as Vec<f32>
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)>;

    /// Get the sample rate of this audio source
    fn sample_rate(&self) -> u32;
}

/// Audio source that reads from a microphone device using CPAL
pub struct MicrophoneSource {
    // In a real implementation, this would hold the CPAL host, device, stream, etc.
    sample_rate: u32,
    frame_size: usize,
}

impl MicrophoneSource {
    /// Create a new MicrophoneSource for the given device
    pub fn new(device_name: &str) -> Result<Self> {
        // For mock implementation, we'll just return a dummy source
        println!("Initializing microphone source for device: {}", device_name);

        Ok(Self {
            sample_rate: 48000,
            frame_size: 1024,
        })
    }
}

impl AudioSource for MicrophoneSource {
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)> {
        // Mock implementation generates sine waves with different phase for the two channels
        let mut channel_a = Vec::with_capacity(self.frame_size);
        let mut channel_b = Vec::with_capacity(self.frame_size);

        for i in 0..self.frame_size {
            // Channel A: 2 kHz sine wave
            let t = i as f32 / self.sample_rate as f32;
            let freq_a = 2000.0; // 2 kHz
            let sample_a = (2.0 * std::f32::consts::PI * freq_a * t).sin() * 0.5;
            channel_a.push(sample_a);

            // Channel B: 2 kHz sine wave with different amplitude and slight phase difference
            let phase_shift = 0.1; // Phase shift for channel B
            let sample_b = (2.0 * std::f32::consts::PI * freq_a * (t + phase_shift)).sin() * 0.3;
            channel_b.push(sample_b);
        }

        Ok((channel_a, channel_b))
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

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

/// Get an audio source from the specified device
pub fn get_audio_source_from_device(device_name: &str) -> Result<Box<dyn AudioSource>> {
    Ok(Box::new(MicrophoneSource::new(device_name)?))
}

/// Get an audio source from the specified WAV file
pub fn get_audio_source_from_file<P: AsRef<Path>>(file_path: P) -> Result<Box<dyn AudioSource>> {
    Ok(Box::new(FileSource::new(file_path)?))
}

/// Get the default audio source (first available device)
pub fn get_default_audio_source() -> Result<Box<dyn AudioSource>> {
    // In a real implementation, this would enumerate devices and pick the first one
    println!("Using default audio device");
    Ok(Box::new(MicrophoneSource::new("default")?))
}
