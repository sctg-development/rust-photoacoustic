// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Audio acquisition module
//!
//! This module handles the acquisition of audio data from files.

use crate::acquisition::{AudioFrame, RealTimeAudioSource, SharedAudioStream};

use super::AudioSource;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hound::{WavReader, WavSpec};
use log::{debug, error, info};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

/// Audio source that reads from a WAV file using hound
pub struct FileSource {
    reader: WavReader<BufReader<File>>,
    spec: WavSpec,
    frame_size: usize,
    samples_read: usize,
    // Timing control for real-time simulation
    last_frame_time: Option<Instant>,
    frame_duration: Duration,
    real_time_mode: bool,
    // Real-time streaming support
    streaming: Arc<AtomicBool>,
    stream_handle: Option<tokio::task::JoinHandle<()>>,
    input_file: String,
}

#[async_trait]
impl RealTimeAudioSource for FileSource {
    async fn start_streaming(&mut self, stream: Arc<SharedAudioStream>) -> Result<()> {
        if self.streaming.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.streaming.store(true, Ordering::Relaxed);

        let frame_size = self.frame_size;
        let frame_duration = self.frame_duration;
        let streaming = self.streaming.clone();
        let input_file = self.input_file.clone();

        let handle = tokio::spawn(async move {
            // Reopen the file in the async context
            let file = match File::open(&input_file) {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to reopen WAV file: {}", e);
                    return;
                }
            };

            let buf_reader = BufReader::new(file);
            let mut reader = match WavReader::new(buf_reader) {
                Ok(r) => r,
                Err(e) => {
                    error!("Failed to create WAV reader: {}", e);
                    return;
                }
            };

            let spec = reader.spec();
            let mut frame_number = 0u64;
            let mut last_frame_time = Instant::now();

            while streaming.load(Ordering::Relaxed) {
                // Real-time timing simulation
                let now = Instant::now();
                let elapsed = now.duration_since(last_frame_time);
                if elapsed < frame_duration {
                    let sleep_duration = frame_duration - elapsed;
                    tokio::time::sleep(sleep_duration).await;
                }
                last_frame_time = Instant::now();

                // Read frame from file
                let (channel_a, channel_b) =
                    match Self::read_frame_from_reader(&mut reader, &spec, frame_size) {
                        Ok((a, b)) if !a.is_empty() => (a, b),
                        Ok(_) => {
                            info!("Reached end of WAV file, stopping stream");
                            break;
                        }
                        Err(e) => {
                            error!("Error reading WAV frame: {}", e);
                            break;
                        }
                    };

                frame_number += 1;
                let audio_frame =
                    AudioFrame::new(channel_a, channel_b, spec.sample_rate, frame_number);

                if let Err(e) = stream.publish(audio_frame).await {
                    error!("Failed to publish file frame: {}", e);
                    break;
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
        self.spec.sample_rate
    }
}

impl FileSource {
    /// Create a new FileSource for the given WAV file
    pub fn new(config: crate::config::PhotoacousticConfig) -> Result<Self> {
        // Validate that the input file is provided
        if config.input_file.is_none() {
            return Err(anyhow!("Input file is not set in configuration"));
        }
        let input_file = config.input_file.as_ref().unwrap().clone();
        let file_path = Path::new(&input_file);
        if !file_path.exists() {
            return Err(anyhow!("WAV file does not exist: {}", file_path.display()));
        }
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

        // Use frame_size from configuration instead of calculating
        let frame_size = config.frame_size as usize;

        // Calculate frame duration for real-time simulation
        let frame_duration = Duration::from_secs_f64(frame_size as f64 / spec.sample_rate as f64);

        info!("Opened WAV file: {}", file_path.display());
        info!("  Sample rate: {} Hz", spec.sample_rate);
        info!("  Channels: {}", spec.channels);
        info!("  Bits per sample: {}", spec.bits_per_sample);
        info!("  Sample format: {:?}", spec.sample_format);
        info!("  Frame size: {} samples per channel", frame_size);
        info!(
            "  Frame duration: {:.1}ms",
            frame_duration.as_secs_f64() * 1000.0
        );
        info!("  Expected FPS: {:.1}", 1.0 / frame_duration.as_secs_f64());

        Ok(Self {
            reader,
            spec,
            frame_size,
            samples_read: 0,
            last_frame_time: None,
            frame_duration,
            real_time_mode: true,
            streaming: Arc::new(AtomicBool::new(false)),
            stream_handle: None,
            input_file,
        })
    }

    /// Enable or disable real-time simulation
    pub fn set_real_time_mode(&mut self, enabled: bool) {
        self.real_time_mode = enabled;
        if !enabled {
            self.last_frame_time = None;
        }
    }

    // Helper method to read frame from reader (moved from read_frame)
    fn read_frame_from_reader(
        reader: &mut WavReader<BufReader<File>>,
        spec: &WavSpec,
        frame_size: usize,
    ) -> Result<(Vec<f32>, Vec<f32>)> {
        let mut channel_a = Vec::with_capacity(frame_size);
        let mut channel_b = Vec::with_capacity(frame_size);

        match spec.sample_format {
            hound::SampleFormat::Int => {
                let samples: Result<Vec<i16>, _> =
                    reader.samples::<i16>().take(frame_size * 2).collect();

                match samples {
                    Ok(sample_vec) => {
                        if sample_vec.is_empty() {
                            return Ok((Vec::new(), Vec::new()));
                        }

                        for chunk in sample_vec.chunks_exact(2) {
                            let left = chunk[0] as f32 / i16::MAX as f32;
                            let right = chunk[1] as f32 / i16::MAX as f32;
                            channel_a.push(left);
                            channel_b.push(right);
                        }
                    }
                    Err(_) => return Ok((Vec::new(), Vec::new())),
                }
            }
            hound::SampleFormat::Float => {
                let samples: Result<Vec<f32>, _> =
                    reader.samples::<f32>().take(frame_size * 2).collect();

                match samples {
                    Ok(sample_vec) => {
                        if sample_vec.is_empty() {
                            return Ok((Vec::new(), Vec::new()));
                        }

                        for chunk in sample_vec.chunks_exact(2) {
                            channel_a.push(chunk[0]);
                            channel_b.push(chunk[1]);
                        }
                    }
                    Err(_) => return Ok((Vec::new(), Vec::new())),
                }
            }
        }

        Ok((channel_a, channel_b))
    }
}

impl AudioSource for FileSource {
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)> {
        // Real-time timing simulation
        if self.real_time_mode {
            let now = Instant::now();

            if let Some(last_time) = self.last_frame_time {
                let elapsed = now.duration_since(last_time);
                if elapsed < self.frame_duration {
                    let sleep_duration = self.frame_duration - elapsed;
                    // debug!(
                    //     "File timing: sleeping for {:.1}ms to maintain real-time playback",
                    //     sleep_duration.as_secs_f64() * 1000.0
                    // );
                    std::thread::sleep(sleep_duration);
                }
            }

            self.last_frame_time = Some(Instant::now());
        }

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
                "Read {} samples from WAV file (total samples read: {}, real-time mode: {})",
                channel_a.len(),
                self.samples_read,
                self.real_time_mode
            );
        }

        Ok((channel_a, channel_b))
    }

    fn sample_rate(&self) -> u32 {
        self.spec.sample_rate
    }
}
