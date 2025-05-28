// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Audio acquisition module
//!
//! This module handles the acquisition of audio data from microphones
//! or from WAV files.

use anyhow::Result;
use std::path::Path;

mod file;
mod mock_microphone;

use file::FileSource;
use mock_microphone::MicrophoneSource;

/// Represents an audio source (either live or from file)
pub trait AudioSource: Send {
    /// Read the next frame of audio data from both channels
    /// Returns a tuple containing (channel_A, channel_B) data as `Vec<f32>`
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)>;

    /// Get the sample rate of this audio source
    fn sample_rate(&self) -> u32;
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
