// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Real-time Acquisition Daemon
//!
//! This module provides a daemon that manages real-time audio acquisition
//! using the RealTimeAudioSource trait for direct streaming to SharedAudioStream.

use super::{RealTimeAudioSource, SharedAudioStream, StreamStats};
use anyhow::Result;
use log::{debug, error, info, warn};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::time::{interval, Duration};

/// Real-time acquisition daemon that manages audio streaming
pub struct RealTimeAcquisitionDaemon {
    /// Real-time audio source
    source: Box<dyn RealTimeAudioSource>,
    /// Shared audio stream for broadcasting
    stream: Arc<SharedAudioStream>,
    /// Control flag for the daemon
    running: Arc<AtomicBool>,
    /// Statistics tracking
    stats_handle: Option<tokio::task::JoinHandle<()>>,
}

impl RealTimeAcquisitionDaemon {
    /// Create a new real-time acquisition daemon
    pub fn new(source: Box<dyn RealTimeAudioSource>, buffer_size: usize) -> Self {
        let stream = Arc::new(SharedAudioStream::new(buffer_size));

        Self {
            source,
            stream,
            running: Arc::new(AtomicBool::new(false)),
            stats_handle: None,
        }
    }

    /// Get a reference to the shared audio stream
    pub fn get_shared_stream(&self) -> Arc<SharedAudioStream> {
        self.stream.clone()
    }

    /// Start the daemon
    pub async fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            warn!("RealTimeAcquisitionDaemon is already running");
            return Ok(());
        }

        info!("Starting RealTimeAcquisitionDaemon");
        self.running.store(true, Ordering::Relaxed);

        // Start the real-time audio source streaming
        info!("Starting real-time audio source streaming");
        self.source.start_streaming(self.stream.clone()).await?;

        // Start statistics monitoring task
        let stats_stream = self.stream.clone();
        let stats_running = self.running.clone();

        self.stats_handle = Some(tokio::spawn(async move {
            Self::statistics_task(stats_stream, stats_running).await;
        }));

        info!("RealTimeAcquisitionDaemon started successfully");
        Ok(())
    }

    /// Stop the daemon
    pub async fn stop(&mut self) -> Result<()> {
        if !self.running.load(Ordering::Relaxed) {
            warn!("RealTimeAcquisitionDaemon is not running");
            return Ok(());
        }

        info!("Stopping RealTimeAcquisitionDaemon");
        self.running.store(false, Ordering::Relaxed);

        // Stop the audio source streaming
        if let Err(e) = self.source.stop_streaming().await {
            error!("Error stopping audio source: {}", e);
        }

        // Stop statistics task
        if let Some(handle) = self.stats_handle.take() {
            handle.abort();
        }

        info!("RealTimeAcquisitionDaemon stopped");
        Ok(())
    }

    /// Check if the daemon is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Check if the audio source is streaming
    pub fn is_streaming(&self) -> bool {
        self.source.is_streaming()
    }

    /// Get current stream statistics
    pub async fn get_stats(&self) -> StreamStats {
        self.stream.get_stats().await
    }

    /// Statistics monitoring task
    async fn statistics_task(stream: Arc<SharedAudioStream>, running: Arc<AtomicBool>) {
        let mut interval = interval(Duration::from_secs(5));
        let mut last_frame_count = 0u64;

        while running.load(Ordering::Relaxed) {
            interval.tick().await;

            let stats = stream.get_stats().await;
            let frames_processed = stats.total_frames - last_frame_count;
            last_frame_count = stats.total_frames;

            debug!(
                "RealTimeAcquisitionDaemon Stats - Processed {} frames, {} subscribers, {:.1} FPS",
                stats.total_frames, stats.active_subscribers, stats.fps
            );

            if frames_processed == 0 && running.load(Ordering::Relaxed) {
                warn!("No frames processed in the last 5 seconds - audio source may have stopped");
            }
        }
    }
}

impl Drop for RealTimeAcquisitionDaemon {
    fn drop(&mut self) {
        if self.running.load(Ordering::Relaxed) {
            warn!(
                "RealTimeAcquisitionDaemon dropped while running - this may cause resource leaks"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::{get_realtime_mock_audio_source, AudioStreamConsumer};
    use crate::config::PhotoacousticConfig;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_realtime_daemon_creation() {
        let config = PhotoacousticConfig::default();
        let source = get_realtime_mock_audio_source(config).unwrap();
        let daemon = RealTimeAcquisitionDaemon::new(source, 100);

        assert!(!daemon.is_running());
        assert!(!daemon.is_streaming());
    }

    #[tokio::test]
    async fn test_realtime_daemon_start_stop() {
        let config = PhotoacousticConfig::default();
        let source = get_realtime_mock_audio_source(config).unwrap();
        let mut daemon = RealTimeAcquisitionDaemon::new(source, 100);

        // Start daemon
        daemon.start().await.unwrap();
        assert!(daemon.is_running());
        assert!(daemon.is_streaming());

        // Wait a bit to let it process some frames
        sleep(Duration::from_millis(200)).await;

        // Check that frames are being processed
        let stats = daemon.get_stats().await;
        assert!(stats.total_frames > 0);

        // Stop daemon
        daemon.stop().await.unwrap();
        assert!(!daemon.is_running());
        assert!(!daemon.is_streaming());
    }

    #[tokio::test]
    async fn test_realtime_daemon_consumer() {
        let config = PhotoacousticConfig::default();
        let source = get_realtime_mock_audio_source(config).unwrap();
        let mut daemon = RealTimeAcquisitionDaemon::new(source, 100);

        // Create a consumer
        let stream = daemon.get_shared_stream();
        let mut consumer = AudioStreamConsumer::new(&stream);

        // Start daemon
        daemon.start().await.unwrap();

        // Consume some frames
        let mut frame_count = 0;
        for _ in 0..5 {
            if let Some(frame) = consumer.next_frame().await {
                frame_count += 1;
                assert!(frame.is_valid());
                assert_eq!(frame.channel_a.len(), frame.channel_b.len());
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert!(frame_count > 0, "Should have received at least some frames");

        daemon.stop().await.unwrap();
    }
}
