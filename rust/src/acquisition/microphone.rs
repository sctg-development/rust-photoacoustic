// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Audio acquisition module
//!
//! This module handles the acquisition of audio data from microphones using CPAL

use crate::acquisition::{AudioFrame, RealTimeAudioSource, SharedAudioStream};
use crate::config::PhotoacousticConfig;

use super::AudioSource;
use anyhow::{Context, Result};
use async_trait::async_trait;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Host, SampleFormat, Stream, StreamConfig,
};
use log::{debug, error, info, warn};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};
use std::time::Duration;

/// Error types for microphone source
#[derive(thiserror::Error, Debug)]
pub enum MicrophoneError {
    #[error("No audio devices found")]
    NoDevicesFound,
    #[error("Device '{0}' not found")]
    DeviceNotFound(String),
    #[error("Failed to get device configuration: {0}")]
    ConfigurationError(String),
    #[error("Unsupported sample format: {0:?}")]
    UnsupportedFormat(SampleFormat),
    #[error("Audio stream error: {0}")]
    StreamError(String),
}

/// Audio source that reads from a microphone device using CPAL
pub struct MicrophoneSource {
    device: Device,
    config: StreamConfig,
    sample_rate: u32,
    frame_size: usize,
    receiver: Arc<Mutex<Receiver<(Vec<f32>, Vec<f32>)>>>,
    // Internal buffer for smoother streaming
    internal_buffer_a: Vec<f32>,
    internal_buffer_b: Vec<f32>,
    target_chunk_size: usize,
    // Real-time streaming support
    streaming: Arc<AtomicBool>,
    stream_handle: Option<tokio::task::JoinHandle<()>>,
}

#[async_trait]
impl RealTimeAudioSource for MicrophoneSource {
    async fn start_streaming(&mut self, stream: Arc<SharedAudioStream>) -> Result<()> {
        if self.streaming.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.streaming.store(true, Ordering::Relaxed);
        let receiver = self.receiver.clone();
        let frame_size = self.frame_size;
        let sample_rate = self.sample_rate;
        let streaming = self.streaming.clone();

        let handle = tokio::spawn(async move {
            let mut frame_number = 0u64;
            let mut internal_buffer_a = Vec::new();
            let mut internal_buffer_b = Vec::new();

            while streaming.load(Ordering::Relaxed) {
                // Wait for audio chunks from the CPAL stream
                let chunk_result = {
                    let receiver = receiver.lock().unwrap();
                    receiver.recv_timeout(Duration::from_millis(100))
                };

                match chunk_result {
                    Ok((chunk_a, chunk_b)) => {
                        internal_buffer_a.extend_from_slice(&chunk_a);
                        internal_buffer_b.extend_from_slice(&chunk_b);

                        // When we have enough data for a complete frame, publish it
                        while internal_buffer_a.len() >= frame_size {
                            let frame_a: Vec<f32> = internal_buffer_a.drain(..frame_size).collect();
                            let frame_b: Vec<f32> = internal_buffer_b.drain(..frame_size).collect();

                            frame_number += 1;
                            let audio_frame =
                                AudioFrame::new(frame_a, frame_b, sample_rate, frame_number);

                            if let Err(e) = stream.publish(audio_frame).await {
                                error!("Failed to publish microphone frame: {}", e);
                                break;
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // No data available, continue waiting
                        continue;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        warn!("Microphone audio stream disconnected");
                        break;
                    }
                }
            }
        });

        self.stream_handle = Some(handle);
        Ok(())
    }

    async fn stop_streaming(&mut self) -> Result<()> {
        self.streaming.store(false, Ordering::Relaxed);

        if let Some(handle) = self.stream_handle.take() {
            handle.abort();
        }

        Ok(())
    }

    fn is_streaming(&self) -> bool {
        self.streaming.load(Ordering::Relaxed)
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

impl MicrophoneSource {
    /// Create a new MicrophoneSource for the device specified in the configuration
    pub fn new(config: PhotoacousticConfig) -> Result<Self> {
        let mut config = config.clone();
        let host = cpal::default_host();

        // If the input_device is first use the first device found
        if config.input_device.is_some() && config.input_device.as_deref() == Some("first") {
            // Use the first available input device
            let devices: Vec<Device> = host
                .input_devices()
                .context("Failed to get input devices")?
                .collect();
            if devices.is_empty() {
                Self::list_available_devices(&host);
                return Err(MicrophoneError::NoDevicesFound.into());
            }
            info!("Using first available input device: {}", devices[0].name()?);
            config.input_device = Some(devices[0].name()?);
        }

        let device = Self::find_device(&host, config.input_device.as_deref())?;

        info!(
            "Selected audio device: {}",
            device.name().unwrap_or_else(|_| "Unknown".to_string())
        );
        // Get the device's default input configuration
        let supported_config = device
            .default_input_config()
            .context("Failed to get default input configuration")?;

        // Use the device's native configuration
        let stream_config: StreamConfig = supported_config.clone().into();
        let sample_rate = stream_config.sample_rate.0;
        let frame_size = config.frame_size as usize;

        info!(
            "Audio configuration: {} Hz, {} channels, format: {:?}",
            sample_rate,
            stream_config.channels,
            supported_config.sample_format()
        );
        info!(
            "Frame configuration: {} samples per channel, {:.1}ms duration, expected {:.1} FPS",
            frame_size,
            (frame_size as f64 / sample_rate as f64) * 1000.0,
            sample_rate as f64 / frame_size as f64
        ); // Create channel for passing audio data
        let (sender, receiver) = mpsc::channel();

        // Calculate optimal chunk size for smoother streaming
        // Use smaller chunks (about 20-50ms) instead of the full frame
        let target_chunk_size = (sample_rate as f32 * 0.02) as usize; // 20ms chunks
        let target_chunk_size = target_chunk_size.max(512).min(frame_size / 4); // Clamp between 512 and 1/4 frame

        // Clone necessary data for the stream thread
        let device_clone = device.clone();
        let stream_config_clone = stream_config.clone();
        let sample_format = supported_config.sample_format(); // Spawn a detached thread to manage the stream
                                                              // This keeps the stream alive without requiring Send trait
        std::thread::spawn(move || {
            // Create and start the stream in this thread
            match Self::create_stream(
                &device_clone,
                &stream_config_clone,
                sample_format,
                sender,
                target_chunk_size, // Use smaller chunks for the stream
            ) {
                Ok(stream) => {
                    if let Err(e) = stream.play() {
                        error!("Failed to start audio stream: {}", e);
                        return;
                    }

                    info!("Audio stream started successfully");

                    // Keep the stream alive by holding it in this thread
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        // The stream will be automatically dropped when this thread exits
                        // or when the main thread terminates
                    }
                }
                Err(e) => {
                    error!("Failed to create audio stream: {}", e);
                }
            }
        });
        Ok(Self {
            device,
            config: stream_config,
            sample_rate,
            frame_size,
            receiver: Arc::new(Mutex::new(receiver)),
            internal_buffer_a: Vec::new(),
            internal_buffer_b: Vec::new(),
            target_chunk_size,
            streaming: Arc::new(AtomicBool::new(false)),
            stream_handle: None,
        })
    }

    /// Find the audio device to use
    fn find_device(host: &Host, device_name: Option<&str>) -> Result<Device> {
        let devices: Vec<Device> = host
            .input_devices()
            .context("Failed to get input devices")?
            .collect();

        if devices.is_empty() {
            Self::list_available_devices(host);
            return Err(MicrophoneError::NoDevicesFound.into());
        }

        let device = if let Some(name) = device_name {
            // Find device by name
            devices
                .into_iter()
                .find(|d| d.name().map(|n| n.contains(name)).unwrap_or(false))
                .ok_or_else(|| {
                    Self::list_available_devices(host);
                    MicrophoneError::DeviceNotFound(name.to_string())
                })?
        } else {
            // Use default device
            host.default_input_device().ok_or_else(|| {
                Self::list_available_devices(host);
                MicrophoneError::NoDevicesFound
            })?
        };

        Ok(device)
    }

    /// List all available audio input devices
    fn list_available_devices(host: &Host) {
        error!("Available audio input devices:");
        if let Ok(devices) = host.input_devices() {
            for (i, device) in devices.enumerate() {
                let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                error!("  {}: {}", i, name);

                // Show device capabilities
                if let Ok(config) = device.default_input_config() {
                    error!("    - Sample rate: {} Hz", config.sample_rate().0);
                    error!("    - Channels: {}", config.channels());
                    error!("    - Format: {:?}", config.sample_format());
                }
            }
        } else {
            error!("  Failed to enumerate devices");
        }
    }
    /// Create the audio input stream
    fn create_stream(
        device: &Device,
        config: &StreamConfig,
        sample_format: SampleFormat,
        sender: Sender<(Vec<f32>, Vec<f32>)>,
        chunk_size: usize, // Now using smaller chunks
    ) -> Result<Stream> {
        let channels = config.channels as usize;
        let sender = Arc::new(Mutex::new(sender));
        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));

        let stream = match sample_format {
            SampleFormat::F32 => {
                let buffer = buffer.clone();
                let sender = sender.clone();
                device.build_input_stream(
                    config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        Self::process_audio_data(data, &buffer, &sender, channels, chunk_size);
                    },
                    |err| error!("Audio stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::I16 => {
                let buffer = buffer.clone();
                let sender = sender.clone();
                device.build_input_stream(
                    config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        // Convert i16 to f32
                        let float_data: Vec<f32> = data
                            .iter()
                            .map(|&sample| sample as f32 / i16::MAX as f32)
                            .collect();
                        Self::process_audio_data(
                            &float_data,
                            &buffer,
                            &sender,
                            channels,
                            chunk_size,
                        );
                    },
                    |err| error!("Audio stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::U16 => {
                let buffer = buffer.clone();
                let sender = sender.clone();
                device.build_input_stream(
                    config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        // Convert u16 to f32
                        let float_data: Vec<f32> = data
                            .iter()
                            .map(|&sample| (sample as f32 - 32768.0) / 32768.0)
                            .collect();
                        Self::process_audio_data(
                            &float_data,
                            &buffer,
                            &sender,
                            channels,
                            chunk_size,
                        );
                    },
                    |err| error!("Audio stream error: {}", err),
                    None,
                )?
            }
            _ => return Err(MicrophoneError::UnsupportedFormat(sample_format).into()),
        };
        Ok(stream)
    }
    /// Process incoming audio data and send chunks when ready
    fn process_audio_data(
        data: &[f32],
        buffer: &Arc<Mutex<Vec<f32>>>,
        sender: &Arc<Mutex<Sender<(Vec<f32>, Vec<f32>)>>>,
        channels: usize,
        chunk_size: usize, // Now using smaller chunks instead of full frames
    ) {
        let mut buffer = buffer.lock().unwrap();
        let input_samples = data.len();
        buffer.extend_from_slice(data);

        // Process complete chunks (smaller than full frames)
        let samples_per_chunk = chunk_size * channels;
        let mut chunks_sent = 0;

        while buffer.len() >= samples_per_chunk {
            let chunk_data: Vec<f32> = buffer.drain(..samples_per_chunk).collect();

            // Separate channels
            let (channel_a, channel_b) = if channels >= 2 {
                // Stereo: separate left and right channels
                let mut ch_a = Vec::with_capacity(chunk_size);
                let mut ch_b = Vec::with_capacity(chunk_size);

                for chunk in chunk_data.chunks_exact(channels) {
                    ch_a.push(chunk[0]);
                    ch_b.push(chunk[1]);
                }
                (ch_a, ch_b)
            } else {
                // Mono: duplicate channel
                let mono_data: Vec<f32> = chunk_data;
                (mono_data.clone(), mono_data)
            };

            // Send the chunk
            if let Ok(sender) = sender.lock() {
                if let Err(_) = sender.send((channel_a, channel_b)) {
                    // Receiver dropped, stream should stop
                    break;
                }
                chunks_sent += 1;
            }
        }

        // Debug logging every 100 calls to avoid spam
        // static mut CALL_COUNT: u32 = 0;
        // unsafe {
        //     CALL_COUNT += 1;
        //     if CALL_COUNT % 100 == 0 {
        //         info!(
        //             "Audio processing: {} input samples, {} buffered, {} chunks sent (chunk_size={})",
        //             input_samples, buffer.len(), chunks_sent, chunk_size
        //         );
        //     }
        //}

        // Prevent buffer from growing too large (prevent memory issues)
        if buffer.len() > samples_per_chunk * 4 {
            let buffer_len = buffer.len();
            warn!(
                "Audio buffer overflow, dropping {} samples",
                buffer_len - samples_per_chunk
            );
            buffer.drain(..buffer_len - samples_per_chunk);
        }
    }

    /// Get information about the selected device
    pub fn device_info(&self) -> String {
        format!(
            "Device: {}, Sample Rate: {} Hz, Channels: {}",
            self.device.name().unwrap_or_else(|_| "Unknown".to_string()),
            self.sample_rate,
            self.config.channels
        )
    }
}

impl AudioSource for MicrophoneSource {
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)> {
        // Pre-buffer strategy: collect some initial data to smooth streaming
        let min_buffer_frames = 2; // Keep at least 2 frames worth of data buffered
        let target_buffer_size = self.frame_size * min_buffer_frames;

        // Keep collecting chunks until we have enough for smooth streaming
        while self.internal_buffer_a.len() < target_buffer_size {
            // Wait for a chunk from the audio thread
            let (chunk_a, chunk_b) = {
                let receiver = self.receiver.lock().unwrap();
                receiver.recv().context("Audio stream has stopped")?
            };

            // Add to internal buffers
            self.internal_buffer_a.extend_from_slice(&chunk_a);
            self.internal_buffer_b.extend_from_slice(&chunk_b);
        }

        // Extract a full frame from the internal buffers
        let frame_a: Vec<f32> = self.internal_buffer_a.drain(..self.frame_size).collect();
        let frame_b: Vec<f32> = self.internal_buffer_b.drain(..self.frame_size).collect();

        Ok((frame_a, frame_b))
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}
