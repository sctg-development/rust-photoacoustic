// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Acquisition daemon module
//!
//! This module provides a daemon for continuous audio acquisition with real-time streaming
//! capabilities to web clients.

use crate::acquisition::{AudioFrame, AudioSource, SharedAudioStream};
use anyhow::Result;
use log::{debug, error, info, warn};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::time::{interval, sleep};

/// Acquisition daemon that continuously reads from an audio source
/// and streams the data to connected clients
#[deprecated(note = "Use RealTimeAcquisitionDaemon instead for real-time streaming")]
pub struct AcquisitionDaemon {
    /// Audio source (microphone or file)
    audio_source: Box<dyn AudioSource>,
    /// Shared stream for broadcasting frames
    stream: SharedAudioStream,
    /// Flag to control daemon execution
    running: Arc<AtomicBool>,
    /// Frame counter
    frame_counter: Arc<AtomicU64>,
    /// Target frames per second
    target_fps: f64,
}

impl AcquisitionDaemon {
    /// Create a new acquisition daemon
    ///
    /// ### Parameters
    /// * `audio_source` - The audio source to read from
    /// * `target_fps` - Target frames per second for streaming
    /// * `buffer_size` - Size of the broadcast buffer
    pub fn new(audio_source: Box<dyn AudioSource>, target_fps: f64, buffer_size: usize) -> Self {
        Self {
            audio_source,
            stream: SharedAudioStream::new(buffer_size),
            running: Arc::new(AtomicBool::new(false)),
            frame_counter: Arc::new(AtomicU64::new(0)),
            target_fps,
        }
    }

    /// Get a reference to the shared stream for consumers
    pub fn get_stream(&self) -> &SharedAudioStream {
        &self.stream
    }

    /// Start the acquisition daemon
    pub async fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            warn!("Acquisition daemon is already running");
            return Ok(());
        }

        self.running.store(true, Ordering::Relaxed);
        info!(
            "Starting acquisition daemon with target FPS: {}",
            self.target_fps
        );

        let frame_duration = Duration::from_secs_f64(1.0 / self.target_fps);
        let mut interval = interval(frame_duration);
        info!(
            "Acquitision daemon started, frame duration: {:?}ms, interval: {}ms",
            frame_duration.as_millis(),
            interval.period().as_millis()
        );
        while self.running.load(Ordering::Relaxed) {
            interval.tick().await;

            match self.read_and_publish_frame().await {
                Ok(true) => {
                    // Frame successfully published
                    let frame_num = self.frame_counter.fetch_add(1, Ordering::Relaxed);

                    if frame_num % 100 == 0 {
                        let stats = self.stream.get_stats().await;
                        debug!(
                            "Processed {} frames, {} subscribers, {:.1} FPS",
                            stats.total_frames, stats.active_subscribers, stats.fps
                        );
                    }
                }
                Ok(false) => {
                    // End of stream (e.g., end of file)
                    info!("End of audio stream reached");
                    break;
                }
                Err(e) => {
                    error!("Error reading audio frame: {}", e);
                    // Continue running despite errors
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }

        info!("Acquisition daemon stopped");
        Ok(())
    }

    /// Stop the acquisition daemon
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        info!("Stopping acquisition daemon");
    }

    /// Check if the daemon is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get current frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_counter.load(Ordering::Relaxed)
    }

    /// Read a frame from the audio source and publish it to the stream
    async fn read_and_publish_frame(&mut self) -> Result<bool> {
        let (channel_a, channel_b) = self.audio_source.read_frame()?;

        // Check if we've reached end of stream
        if channel_a.is_empty() || channel_b.is_empty() {
            return Ok(false);
        }

        let sample_rate = self.audio_source.sample_rate();
        let frame_number = self.frame_counter.load(Ordering::Relaxed);

        let frame = AudioFrame::new(channel_a, channel_b, sample_rate, frame_number);

        self.stream.publish(frame).await?;

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::{get_default_audio_source, AudioStreamConsumer};
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_acquisition_daemon() {
        // Create a mock audio source
        let config = crate::config::PhotoacousticConfig::default();
        let audio_source = get_default_audio_source(config).unwrap();
        let mut daemon = AcquisitionDaemon::new(audio_source, 10.0, 50);

        // Create a consumer
        let mut consumer = AudioStreamConsumer::new(daemon.get_stream());

        // Start daemon in background
        let daemon_running = daemon.running.clone();
        tokio::spawn(async move {
            daemon.start().await.unwrap();
        });

        // Wait a bit for daemon to start
        sleep(Duration::from_millis(100)).await;

        // Try to receive a frame with timeout
        let result = timeout(Duration::from_secs(2), consumer.next_frame()).await;

        assert!(result.is_ok());
        let frame = result.unwrap();
        assert!(frame.is_some());

        // Stop daemon
        daemon_running.store(false, Ordering::Relaxed);
    }
}
