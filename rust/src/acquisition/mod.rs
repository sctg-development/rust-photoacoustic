// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Audio acquisition module
//!
//! This module handles the acquisition of audio data from microphones
//! or from WAV files, with support for real-time streaming.

use anyhow::Result;
use async_trait::async_trait;
use log::info;
use std::sync::Arc;

pub mod daemon;
mod file;
mod microphone;
mod mock;
pub mod realtime_daemon;
pub mod stream;

pub use daemon::AcquisitionDaemon;
use file::FileSource;
pub use microphone::MicrophoneSource;
pub use mock::MockSource;
pub use realtime_daemon::RealTimeAcquisitionDaemon;
pub use stream::{AudioFrame, AudioStreamConsumer, SharedAudioStream, StreamStats};

use crate::config::PhotoacousticConfig;

/// Represents an audio source (either live or from file)
pub trait AudioSource: Send {
    /// Read the next frame of audio data from both channels
    /// Returns a tuple containing (channel_A, channel_B) data as `Vec<f32>`
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)>;

    /// Get the sample rate of this audio source
    fn sample_rate(&self) -> u32;
}

/// Trait for real-time audio sources that can stream directly to SharedAudioStream
#[async_trait]
pub trait RealTimeAudioSource: Send + Sync {
    /// Start streaming audio frames to the shared stream
    async fn start_streaming(&mut self, stream: Arc<SharedAudioStream>) -> Result<()>;

    /// Stop the streaming
    async fn stop_streaming(&mut self) -> Result<()>;

    /// Check if currently streaming
    fn is_streaming(&self) -> bool;

    /// Get the sample rate of this audio source
    fn sample_rate(&self) -> u32;
}

/// Get an audio source from the specified device
pub fn get_audio_source_from_device(config: PhotoacousticConfig) -> Result<Box<dyn AudioSource>> {
    Ok(Box::new(MicrophoneSource::new(config)?))
}

/// Get an audio source from the specified WAV file
pub fn get_audio_source_from_file(config: PhotoacousticConfig) -> Result<Box<dyn AudioSource>> {
    Ok(Box::new(FileSource::new(config)?))
}

/// Get a mock audio source that generates synthetic photoacoustic signals
///
/// # Arguments
///
/// * `config` - PhotoacousticConfig containing frequency, sample_rate, and precision settings
/// * `frame_size` - Number of samples per frame per channel
/// * `correlation` - Correlation coefficient between channels [-1.0, 1.0]
///
/// # Returns
///
/// A boxed MockSource that implements AudioSource trait
///
/// # Examples
///
/// ```
/// use rust_photoacoustic::acquisition::get_mock_audio_source;
/// use rust_photoacoustic::config::PhotoacousticConfig;
///
/// let config = PhotoacousticConfig::default();
/// let mock_source = get_mock_audio_source(config);
/// ```
pub fn get_mock_audio_source(
    config: crate::config::PhotoacousticConfig,
) -> Result<Box<dyn AudioSource>> {
    Ok(Box::new(MockSource::new(config)?))
}

/// Get the default audio source (first available device)
pub fn get_default_audio_source(
    config: crate::config::PhotoacousticConfig,
) -> Result<Box<dyn AudioSource>> {
    // In a real implementation, this would enumerate devices and pick the first one
    info!("Using first audio device");
    let mut config: PhotoacousticConfig = config.clone();
    config.input_device = Some("first".to_string()); // Set default device
    Ok(Box::new(MicrophoneSource::new(config)?))
}

/// Get a real-time audio source from the specified device
pub fn get_realtime_audio_source_from_device(
    config: PhotoacousticConfig,
) -> Result<Box<dyn RealTimeAudioSource>> {
    Ok(Box::new(MicrophoneSource::new(config)?))
}

/// Get a real-time audio source from the specified WAV file
pub fn get_realtime_audio_source_from_file(
    config: PhotoacousticConfig,
) -> Result<Box<dyn RealTimeAudioSource>> {
    Ok(Box::new(FileSource::new(config)?))
}

/// Get a real-time mock audio source that generates synthetic photoacoustic signals
pub fn get_realtime_mock_audio_source(
    config: PhotoacousticConfig,
) -> Result<Box<dyn RealTimeAudioSource>> {
    Ok(Box::new(MockSource::new(config)?))
}

/// Get the default real-time audio source (first available device)
pub fn get_default_realtime_audio_source(
    config: PhotoacousticConfig,
) -> Result<Box<dyn RealTimeAudioSource>> {
    let mut config: PhotoacousticConfig = config.clone();
    config.input_device = Some("first".to_string());
    Ok(Box::new(MicrophoneSource::new(config)?))
}

pub mod record_consumer;
