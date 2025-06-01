//! Record Consumer Daemon Module
//!
//! This module provides a mock consumer daemon to validate the real-time audio
//! producer/consumer system. It consumes audio frames from the SharedAudioStream
//! and saves them to a WAV file with the same precision and sample rate as the producer.
//!
//! The record consumer also produces detailed log messages to analyze the behavior
//! of the audio consumption system.

use crate::acquisition::{AudioFrame, AudioStreamConsumer, SharedAudioStream};
use anyhow::{anyhow, Result};
use hound::{SampleFormat, WavSpec, WavWriter};
use log::{debug, error, info, warn};
use std::fs::File;
use std::io::BufWriter;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::timeout;

/// Record Consumer Daemon for producer/consumer system validation
///
/// This daemon consumes audio frames from the SharedAudioStream and writes them
/// to a WAV file for validation. It produces detailed logs to analyze
/// consumer behavior.
pub struct RecordConsumer {
    /// Shared audio stream to consume
    audio_stream: Arc<SharedAudioStream>,
    /// Execution control flag
    running: Arc<AtomicBool>,
    /// Counter of consumed frames
    frames_consumed: Arc<AtomicU64>,
    /// Counter of dropped frames (lag)
    frames_dropped: Arc<AtomicU64>,
    /// Output WAV file path
    output_path: String,
    /// WAV writer for saving audio
    wav_writer: Option<WavWriter<BufWriter<File>>>,
    /// Audio stream consumer
    consumer: Option<AudioStreamConsumer>,
    /// Timestamp of last received frame for delay measurement
    last_frame_time: Option<Instant>,
    /// Throughput statistics
    throughput_stats: ThroughputStats,
}

/// Throughput statistics for the record consumer
#[derive(Debug, Clone)]
struct ThroughputStats {
    /// Number of frames in current window
    frames_in_window: u64,
    /// Timestamp of current window start
    window_start: Instant,
    /// Measurement window duration (in seconds)
    window_duration: Duration,
    /// Average FPS of current window
    current_fps: f64,
    /// Average delay between frames (in ms)
    avg_frame_delay: f64,
    /// Observed min/max delays
    min_frame_delay: f64,
    max_frame_delay: f64,
}

impl ThroughputStats {
    fn new(window_duration_secs: u64) -> Self {
        Self {
            frames_in_window: 0,
            window_start: Instant::now(),
            window_duration: Duration::from_secs(window_duration_secs),
            current_fps: 0.0,
            avg_frame_delay: 0.0,
            min_frame_delay: f64::MAX,
            max_frame_delay: 0.0,
        }
    }

    #[allow(dead_code)]
    fn stop(&self) -> u64 {
        self.frames_in_window
    }

    #[allow(dead_code)]
    fn frames_dropped(&self) -> f64 {
        self.max_frame_delay
    }
    #[allow(dead_code)]
    fn get_throughput_stats(&self) -> (f64, f64, f64, f64) {
        (
            self.current_fps,
            self.avg_frame_delay,
            self.min_frame_delay,
            self.max_frame_delay,
        )
    }

    fn update(&mut self, frame_delay_ms: f64) {
        self.frames_in_window += 1;

        // Update min/max delays
        self.min_frame_delay = self.min_frame_delay.min(frame_delay_ms);
        self.max_frame_delay = self.max_frame_delay.max(frame_delay_ms);

        let now = Instant::now();
        let elapsed = now.duration_since(self.window_start);

        if elapsed >= self.window_duration {
            // Calculate FPS for this window
            self.current_fps = self.frames_in_window as f64 / elapsed.as_secs_f64();

            // Calculate average delay (approximate)
            self.avg_frame_delay = (self.min_frame_delay + self.max_frame_delay) / 2.0;

            // Log statistics
            debug!(
                "RecordConsumer Stats - FPS: {:.2}, Avg Delay: {:.2}ms, Min: {:.2}ms, Max: {:.2}ms, Frames: {}",
                self.current_fps,
                self.avg_frame_delay,
                self.min_frame_delay,
                self.max_frame_delay,
                self.frames_in_window
            );

            // Reset for next window
            self.frames_in_window = 0;
            self.window_start = now;
            self.min_frame_delay = f64::MAX;
            self.max_frame_delay = 0.0;
        }
    }
}

impl RecordConsumer {
    /// Create a new RecordConsumerDaemon
    ///
    /// # Arguments
    ///
    /// * `audio_stream` - Shared audio stream to consume
    /// * `output_path` - Output WAV file path
    ///
    /// # Returns
    ///
    /// A new RecordConsumerDaemon instance
    pub fn new(audio_stream: Arc<SharedAudioStream>, output_path: String) -> Self {
        info!("Creating RecordConsumerDaemon with output: {}", output_path);

        Self {
            audio_stream,
            running: Arc::new(AtomicBool::new(false)),
            frames_consumed: Arc::new(AtomicU64::new(0)),
            frames_dropped: Arc::new(AtomicU64::new(0)),
            output_path,
            wav_writer: None,
            consumer: None,
            last_frame_time: None,
            throughput_stats: ThroughputStats::new(5), // 5-second window
        }
    }

    /// Start the record consumer daemon
    pub async fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            warn!("RecordConsumerDaemon is already running");
            return Ok(());
        }

        info!("Starting RecordConsumerDaemon");
        self.running.store(true, Ordering::Relaxed);

        // Create the consumer
        self.consumer = Some(AudioStreamConsumer::new(&self.audio_stream));

        debug!("RecordConsumerDaemon: Consumer created, waiting for first frame");

        // Wait for first frame to determine WAV specifications
        if let Some(first_frame) = self.wait_for_first_frame().await? {
            debug!(
                "RecordConsumerDaemon: First frame received - Sample Rate: {}Hz, Channels: A={}, B={}",
                first_frame.sample_rate,
                first_frame.channel_a.len(),
                first_frame.channel_b.len()
            );

            // Initialize WAV writer with first frame specifications
            self.initialize_wav_writer(&first_frame)?;

            // Process the first frame
            self.process_frame(&first_frame)?;

            // Main consumption loop
            while self.running.load(Ordering::Relaxed) {
                match self.consume_next_frame().await {
                    Ok(true) => {
                        // Frame processed successfully
                        let count = self.frames_consumed.fetch_add(1, Ordering::Relaxed);

                        if count % 100 == 0 {
                            debug!("RecordConsumerDaemon: {} frames consumed", count);
                        }
                    }
                    Ok(false) => {
                        // Timeout - no new frame
                        debug!("RecordConsumerDaemon: Timeout waiting for frame");
                    }
                    Err(e) => {
                        error!("RecordConsumerDaemon: Error consuming frame: {}", e);
                        break;
                    }
                }
            }
        } else {
            warn!("RecordConsumerDaemon: No frames received, stopping");
        }

        // Cleanup
        self.cleanup();
        info!(
            "RecordConsumerDaemon stopped - {} frames consumed, {} frames dropped",
            self.frames_consumed.load(Ordering::Relaxed),
            self.frames_dropped.load(Ordering::Relaxed)
        );

        Ok(())
    }
    /// Stop the daemon
    #[allow(dead_code)]
    pub fn stop(&self) {
        info!("Stopping RecordConsumerDaemon");
        self.running.store(false, Ordering::Relaxed);
    }

    /// Check if the daemon is running
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get the number of consumed frames
    #[allow(dead_code)]
    pub fn frames_consumed(&self) -> u64 {
        self.frames_consumed.load(Ordering::Relaxed)
    }

    /// Get the number of dropped frames
    #[allow(dead_code)]
    pub fn frames_dropped(&self) -> u64 {
        self.frames_dropped.load(Ordering::Relaxed)
    }

    /// Wait for the first frame to determine specifications
    async fn wait_for_first_frame(&mut self) -> Result<Option<AudioFrame>> {
        debug!("RecordConsumerDaemon: Waiting for first frame");

        let timeout_duration = Duration::from_secs(10);
        let consumer = self
            .consumer
            .as_mut()
            .ok_or_else(|| anyhow!("Consumer not initialized"))?;

        match timeout(timeout_duration, consumer.next_frame()).await {
            Ok(Some(frame)) => {
                info!("RecordConsumerDaemon: First frame received successfully");
                Ok(Some(frame))
            }
            Ok(None) => {
                warn!("RecordConsumerDaemon: Stream closed before receiving first frame");
                Ok(None)
            }
            Err(_) => {
                error!("RecordConsumerDaemon: Timeout waiting for first frame");
                Err(anyhow!("Timeout waiting for first frame"))
            }
        }
    }

    /// Initialize WAV writer with frame specifications
    fn initialize_wav_writer(&mut self, frame: &AudioFrame) -> Result<()> {
        let spec = WavSpec {
            channels: 2, // Stereo (channel_a and channel_b)
            sample_rate: frame.sample_rate,
            bits_per_sample: 32, // Use 32 bits for f32 data
            sample_format: SampleFormat::Float,
        };

        debug!(
            "RecordConsumerDaemon: Initializing WAV writer - {}Hz, {} channels, {} bits",
            spec.sample_rate, spec.channels, spec.bits_per_sample
        );

        let writer = WavWriter::create(&self.output_path, spec)
            .map_err(|e| anyhow!("Failed to create WAV writer: {}", e))?;

        self.wav_writer = Some(writer);

        info!(
            "RecordConsumerDaemon: WAV file created: {} ({}Hz, {} channels)",
            self.output_path, frame.sample_rate, 2
        );

        Ok(())
    }

    /// Consume the next frame
    async fn consume_next_frame(&mut self) -> Result<bool> {
        let timeout_duration = Duration::from_millis(100);
        let consumer = self
            .consumer
            .as_mut()
            .ok_or_else(|| anyhow!("Consumer not initialized"))?;

        let now = Instant::now();

        match timeout(timeout_duration, consumer.next_frame()).await {
            Ok(Some(frame)) => {
                // Calculate delay since last frame
                if let Some(last_time) = self.last_frame_time {
                    let delay_ms = now.duration_since(last_time).as_millis() as f64;
                    self.throughput_stats.update(delay_ms);
                }
                self.last_frame_time = Some(now);

                // Process the frame
                self.process_frame(&frame)?;
                Ok(true)
            }
            Ok(None) => {
                debug!("RecordConsumerDaemon: Stream closed");
                Ok(false)
            }
            Err(_) => {
                // Timeout - no new frame available
                Ok(false)
            }
        }
    }

    /// Process an audio frame
    fn process_frame(&mut self, frame: &AudioFrame) -> Result<()> {
        let writer = self
            .wav_writer
            .as_mut()
            .ok_or_else(|| anyhow!("WAV writer not initialized"))?;

        // Check that both channels have the same size
        if frame.channel_a.len() != frame.channel_b.len() {
            return Err(anyhow!(
                "Channel size mismatch: A={}, B={}",
                frame.channel_a.len(),
                frame.channel_b.len()
            ));
        }

        // Interleave samples from both channels (LRLRLR...)
        for (sample_a, sample_b) in frame.channel_a.iter().zip(frame.channel_b.iter()) {
            writer
                .write_sample(*sample_a)
                .map_err(|e| anyhow!("Failed to write channel A sample: {}", e))?;
            writer
                .write_sample(*sample_b)
                .map_err(|e| anyhow!("Failed to write channel B sample: {}", e))?;
        }

        // Detailed logging to analyze behavior
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        debug!(
            "RecordConsumerDaemon: Frame processed - #{}, {} samples/channel, timestamp: {}ms",
            frame.frame_number,
            frame.channel_a.len(),
            timestamp
        );

        Ok(())
    }

    /// Clean up resources
    fn cleanup(&mut self) {
        debug!("RecordConsumerDaemon: Cleaning up resources");

        if let Some(writer) = self.wav_writer.take() {
            if let Err(e) = writer.finalize() {
                error!("RecordConsumerDaemon: Failed to finalize WAV file: {}", e);
            } else {
                info!("RecordConsumerDaemon: WAV file finalized successfully");
            }
        }

        self.consumer = None;
        self.last_frame_time = None;
    }
    /// Get current throughput statistics
    #[allow(dead_code)]
    pub fn get_throughput_stats(&self) -> (f64, f64, f64, f64) {
        (
            self.throughput_stats.current_fps,
            self.throughput_stats.avg_frame_delay,
            self.throughput_stats.min_frame_delay,
            self.throughput_stats.max_frame_delay,
        )
    }
}

impl Drop for RecordConsumer {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::{AudioFrame, SharedAudioStream};
    use std::time::Duration;
    use tempfile::NamedTempFile;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_record_consumer_creation() {
        let stream = Arc::new(SharedAudioStream::new(10));
        let temp_file = NamedTempFile::new().unwrap();
        let output_path = temp_file.path().to_string_lossy().to_string();

        let consumer = RecordConsumer::new(stream, output_path);
        assert!(!consumer.is_running());
        assert_eq!(consumer.frames_consumed(), 0);
    }

    #[tokio::test]
    async fn test_record_consumer_with_frames() {
        let stream = Arc::new(SharedAudioStream::new(10));
        let temp_file = NamedTempFile::new().unwrap();
        let output_path = temp_file.path().to_string_lossy().to_string();

        let mut consumer = RecordConsumer::new(stream.clone(), output_path);

        // Create test frames
        let frame1 = AudioFrame::new(vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6], 48000, 1);
        let frame2 = AudioFrame::new(vec![0.7, 0.8, 0.9], vec![1.0, 1.1, 1.2], 48000, 2);

        // References to control the consumer
        let running = consumer.running.clone();
        let frames_consumed = consumer.frames_consumed.clone();

        // Start the consumer in a separate task
        let consumer_task = tokio::spawn(async move {
            consumer.start().await.unwrap();
        });

        // Wait a bit for the consumer to be ready to receive
        sleep(Duration::from_millis(50)).await;

        // Now publish frames after the consumer is listening
        stream.publish(frame1).await.unwrap();
        stream.publish(frame2).await.unwrap();

        // Wait for the consumer to process frames
        sleep(Duration::from_millis(200)).await;

        // Stop the consumer
        running.store(false, Ordering::Relaxed);

        // Wait for the task to finish
        let _ = tokio::time::timeout(Duration::from_secs(2), consumer_task).await;

        // Check that frames were consumed
        let consumed = frames_consumed.load(Ordering::Relaxed);
        println!("Frames consumed: {}", consumed);
        assert!(
            consumed > 0,
            "Expected frames to be consumed, but got {}",
            consumed
        );
    }
}
