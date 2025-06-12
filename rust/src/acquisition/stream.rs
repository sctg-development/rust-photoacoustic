// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Audio streaming module
//!
//! This module provides a shared data structure for streaming audio frames
//! between the acquisition daemon and web clients in real-time.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, RwLock};

/// Represents a frame of audio data with metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioFrame {
    /// Channel A audio data
    pub channel_a: Vec<f32>,
    /// Channel B audio data
    pub channel_b: Vec<f32>,
    /// Sample rate of the audio data
    pub sample_rate: u32,
    /// Timestamp when the frame was captured
    pub timestamp: u64,
    /// Sequential frame number
    pub frame_number: u64,
}

impl AudioFrame {
    /// Create a new audio frame
    pub fn new(
        channel_a: Vec<f32>,
        channel_b: Vec<f32>,
        sample_rate: u32,
        frame_number: u64,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            channel_a,
            channel_b,
            sample_rate,
            timestamp,
            frame_number,
        }
    }

    /// Get the duration of this frame in milliseconds
    pub fn duration_ms(&self) -> f64 {
        (self.channel_a.len() as f64 / self.sample_rate as f64) * 1000.0
    }

    /// Check if frame has valid data
    pub fn is_valid(&self) -> bool {
        !self.channel_a.is_empty()
            && !self.channel_b.is_empty()
            && self.channel_a.len() == self.channel_b.len()
    }
}

/// Shared audio stream for broadcasting frames to multiple consumers
#[derive(Clone, Debug)]
pub struct SharedAudioStream {
    /// Broadcast sender for real-time streaming
    sender: broadcast::Sender<AudioFrame>,
    /// Latest frame for new subscribers
    latest_frame: Arc<RwLock<Option<AudioFrame>>>,
    /// Stream statistics
    stats: Arc<RwLock<StreamStats>>,
}

/// Statistics about the audio stream
#[derive(Debug, Clone, Serialize, Deserialize, rocket_okapi::JsonSchema)]
pub struct StreamStats {
    /// Total number of frames processed
    pub total_frames: u64,
    /// Total number of dropped frames
    pub dropped_frames: u64,
    /// Number of active subscribers
    pub active_subscribers: usize,
    /// Average frames per second
    pub fps: f64,
    /// Last update timestamp
    pub last_update: u64,
    /// Frames processed since last FPS calculation
    pub frames_since_last_update: u64,
    /// Sample rate of the audio stream in Hz
    pub sample_rate: u32,
}

impl Default for StreamStats {
    fn default() -> Self {
        Self {
            total_frames: 0,
            dropped_frames: 0,
            active_subscribers: 0,
            fps: 0.0,
            last_update: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            frames_since_last_update: 0,
            sample_rate: 0,
        }
    }
}

impl SharedAudioStream {
    /// Create a new shared audio stream
    ///
    /// ### Parameters
    /// * `buffer_size` - Size of the broadcast channel buffer
    pub fn new(buffer_size: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer_size);

        Self {
            sender,
            latest_frame: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(StreamStats::default())),
        }
    }

    /// Get a receiver for subscribing to the stream
    pub fn subscribe(&self) -> broadcast::Receiver<AudioFrame> {
        self.sender.subscribe()
    }

    /// Publish a new audio frame to all subscribers
    pub async fn publish(&self, frame: AudioFrame) -> Result<()> {
        // Update latest frame
        {
            let mut latest = self.latest_frame.write().await;
            *latest = Some(frame.clone());
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_frames += 1;
            stats.frames_since_last_update += 1;
            stats.active_subscribers = self.sender.receiver_count();

            stats.sample_rate = frame.sample_rate;

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            // Calculate FPS every second
            if now - stats.last_update >= 1000 {
                let time_diff = (now - stats.last_update) as f64 / 1000.0;
                stats.fps = stats.frames_since_last_update as f64 / time_diff;
                stats.last_update = now;
                stats.frames_since_last_update = 0;
            }
        }

        // Broadcast to subscribers
        match self.sender.send(frame) {
            Ok(_) => Ok(()),
            Err(broadcast::error::SendError(_)) => {
                // No active receivers, but this is not an error
                Ok(())
            }
        }
    }

    /// Get the latest frame (for new subscribers)
    pub async fn get_latest_frame(&self) -> Option<AudioFrame> {
        self.latest_frame.read().await.clone()
    }

    /// Get current stream statistics
    pub async fn get_stats(&self) -> StreamStats {
        self.stats.read().await.clone()
    }

    /// Get the number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

/// Consumer interface for reading from the shared stream
pub struct AudioStreamConsumer {
    receiver: broadcast::Receiver<AudioFrame>,
    stream: SharedAudioStream,
}

impl AudioStreamConsumer {
    /// Create a new consumer from a shared stream
    pub fn new(stream: &SharedAudioStream) -> Self {
        let receiver = stream.subscribe();

        Self {
            receiver,
            stream: stream.clone(),
        }
    }

    /// Get the next frame from the stream
    /// Returns None if the stream is closed or on timeout
    pub async fn next_frame(&mut self) -> Option<AudioFrame> {
        match self.receiver.recv().await {
            Ok(frame) => Some(frame),
            Err(broadcast::error::RecvError::Closed) => None,
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                log::warn!(
                    "Audio stream consumer lagged behind, skipped {} frames",
                    skipped
                );
                // Try to get the next frame
                match self.receiver.recv().await {
                    Ok(frame) => Some(frame),
                    Err(_) => None,
                }
            }
        }
    }

    /// Get the latest available frame without waiting
    pub async fn get_latest_frame(&self) -> Option<AudioFrame> {
        self.stream.get_latest_frame().await
    }

    /// Get current stream statistics
    pub async fn get_stats(&self) -> StreamStats {
        self.stream.get_stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_shared_audio_stream() {
        let stream = SharedAudioStream::new(10);
        let mut consumer = AudioStreamConsumer::new(&stream);

        // Create a test frame
        let frame = AudioFrame::new(vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6], 48000, 1);

        // Publish frame
        stream.publish(frame.clone()).await.unwrap();

        // Consumer should receive the frame
        let received = consumer.next_frame().await.unwrap();
        assert_eq!(received.frame_number, 1);
        assert_eq!(received.channel_a, vec![0.1, 0.2, 0.3]);
    }

    #[tokio::test]
    async fn test_multiple_consumers() {
        let stream = SharedAudioStream::new(10);
        let mut consumer1 = AudioStreamConsumer::new(&stream);
        let mut consumer2 = AudioStreamConsumer::new(&stream);

        let frame = AudioFrame::new(vec![1.0, 2.0], vec![3.0, 4.0], 48000, 42);

        stream.publish(frame.clone()).await.unwrap();

        // Both consumers should receive the same frame
        let frame1 = consumer1.next_frame().await.unwrap();
        let frame2 = consumer2.next_frame().await.unwrap();

        assert_eq!(frame1.frame_number, 42);
        assert_eq!(frame2.frame_number, 42);
    }
}
