// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Processing Consumer
//!
//! This module provides the main processing consumer that reads from the SharedAudioStream
//! and processes frames through the configurable processing graph.

use crate::acquisition::{AudioStreamConsumer, SharedAudioStream};
use crate::processing::result::{FrameInfo, ProcessingMetadata};
use crate::processing::{PhotoacousticAnalysis, ProcessingData, ProcessingGraph, ProcessingResult};
use crate::visualization::shared_state::SharedVisualizationState;
use anyhow::Result;
use log::{debug, error, info, warn};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, RwLock};

/// Processing consumer that applies a processing graph to audio frames
pub struct ProcessingConsumer {
    /// Audio stream to consume from
    audio_stream: Arc<SharedAudioStream>,
    /// Processing graph
    processing_graph: Arc<RwLock<ProcessingGraph>>,
    /// Audio stream consumer
    consumer: Option<AudioStreamConsumer>,
    /// Control flag for the consumer
    running: Arc<AtomicBool>,
    /// Counter of processed frames
    frames_processed: Arc<AtomicU64>,
    /// Counter of failed processing attempts
    processing_failures: Arc<AtomicU64>,
    /// Unique consumer identifier
    consumer_id: String,
    /// Output channel for processing results
    result_sender: Option<broadcast::Sender<ProcessingResult>>,
    /// Processing statistics
    stats: Arc<RwLock<ProcessingStats>>,
    /// Shared visualization state for API access
    visualization_state: Option<Arc<SharedVisualizationState>>,
}

/// Processing statistics
#[derive(Debug, Clone, Default)]
pub struct ProcessingStats {
    pub total_frames_processed: u64,
    pub processing_failures: u64,
    pub average_processing_time_us: f64,
    pub min_processing_time_us: u64,
    pub max_processing_time_us: u64,
    pub detections_count: u64,
    pub last_processing_time_us: u64,
    pub fps: f64,
    pub last_update: u64,
}

impl ProcessingConsumer {
    /// Create a new processing consumer
    pub fn new(audio_stream: Arc<SharedAudioStream>, processing_graph: ProcessingGraph) -> Self {
        let consumer_id = format!(
            "processing_consumer_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );

        Self {
            audio_stream,
            processing_graph: Arc::new(RwLock::new(processing_graph)),
            consumer: None,
            running: Arc::new(AtomicBool::new(false)),
            frames_processed: Arc::new(AtomicU64::new(0)),
            processing_failures: Arc::new(AtomicU64::new(0)),
            consumer_id,
            result_sender: None,
            stats: Arc::new(RwLock::new(ProcessingStats::default())),
            visualization_state: None,
        }
    }

    /// Create a new processing consumer with shared visualization state
    pub fn new_with_visualization_state(
        audio_stream: Arc<SharedAudioStream>,
        processing_graph: ProcessingGraph,
        visualization_state: Arc<SharedVisualizationState>,
    ) -> Self {
        let consumer_id = format!(
            "processing_consumer_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );

        Self {
            audio_stream,
            processing_graph: Arc::new(RwLock::new(processing_graph)),
            consumer: None,
            running: Arc::new(AtomicBool::new(false)),
            frames_processed: Arc::new(AtomicU64::new(0)),
            processing_failures: Arc::new(AtomicU64::new(0)),
            consumer_id,
            result_sender: None,
            stats: Arc::new(RwLock::new(ProcessingStats::default())),
            visualization_state: Some(visualization_state),
        }
    }

    /// Create a new processing consumer with result broadcasting
    pub fn new_with_broadcast(
        audio_stream: Arc<SharedAudioStream>,
        processing_graph: ProcessingGraph,
        result_buffer_size: usize,
    ) -> (Self, broadcast::Receiver<ProcessingResult>) {
        let (sender, receiver) = broadcast::channel(result_buffer_size);
        let mut consumer = Self::new(audio_stream, processing_graph);
        consumer.result_sender = Some(sender);
        (consumer, receiver)
    }

    /// Create a new processing consumer with result broadcasting and visualization state
    pub fn new_with_broadcast_and_visualization(
        audio_stream: Arc<SharedAudioStream>,
        processing_graph: ProcessingGraph,
        result_buffer_size: usize,
        visualization_state: Arc<SharedVisualizationState>,
    ) -> (Self, broadcast::Receiver<ProcessingResult>) {
        let (sender, receiver) = broadcast::channel(result_buffer_size);
        let mut consumer =
            Self::new_with_visualization_state(audio_stream, processing_graph, visualization_state);
        consumer.result_sender = Some(sender);
        (consumer, receiver)
    }

    /// Start the processing consumer
    pub async fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            warn!(
                "ProcessingConsumer '{}' is already running",
                self.consumer_id
            );
            return Ok(());
        }

        info!("Starting ProcessingConsumer '{}'", self.consumer_id);
        self.running.store(true, Ordering::Relaxed);

        // Validate the processing graph
        {
            let graph = self.processing_graph.read().await;
            graph.validate()?;
        }

        // Update shared visualization state with the processing graph if available
        if let Some(ref visualization_state) = self.visualization_state {
            let serializable_graph = {
                let graph = self.processing_graph.read().await;
                // Create a serializable representation of the graph
                graph.to_serializable()
            };

            visualization_state
                .update_processing_graph(serializable_graph)
                .await;
        }

        // Create the audio stream consumer
        self.consumer = Some(AudioStreamConsumer::new(&self.audio_stream));

        info!(
            "ProcessingConsumer '{}' started successfully",
            self.consumer_id
        );

        // Start the main processing loop
        self.processing_loop().await
    }

    /// Stop the processing consumer
    pub async fn stop(&self) {
        info!("Stopping ProcessingConsumer '{}'", self.consumer_id);
        self.running.store(false, Ordering::Relaxed);

        // Clear visualization state when stopping
        if let Some(ref visualization_state) = self.visualization_state {
            visualization_state.clear_all_processing_data().await;
        }
    }

    /// Check if the consumer is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get the number of processed frames
    pub fn frames_processed(&self) -> u64 {
        self.frames_processed.load(Ordering::Relaxed)
    }

    /// Get the number of processing failures
    pub fn processing_failures(&self) -> u64 {
        self.processing_failures.load(Ordering::Relaxed)
    }

    /// Get processing statistics
    pub async fn get_stats(&self) -> ProcessingStats {
        self.stats.read().await.clone()
    }

    /// Update the processing graph at runtime
    pub async fn update_graph(&self, new_graph: ProcessingGraph) -> Result<()> {
        info!(
            "Updating processing graph for consumer '{}'",
            self.consumer_id
        );

        // Validate the new graph
        new_graph.validate()?;

        // Update the graph
        {
            let mut graph = self.processing_graph.write().await;
            *graph = new_graph;
        }

        info!("Processing graph updated successfully");
        Ok(())
    }

    /// Get a subscriber to processing results
    pub fn subscribe_to_results(&self) -> Option<broadcast::Receiver<ProcessingResult>> {
        self.result_sender.as_ref().map(|sender| sender.subscribe())
    }

    /// Main processing loop
    async fn processing_loop(&mut self) -> Result<()> {
        debug!(
            "ProcessingConsumer '{}': Starting main processing loop",
            self.consumer_id
        );

        while self.running.load(Ordering::Relaxed) {
            // Get the next frame from the audio stream
            if let Some(ref mut consumer) = self.consumer {
                match consumer.next_frame().await {
                    Some(frame) => {
                        let start_time = Instant::now();

                        // Process the frame
                        match self.process_frame(frame).await {
                            Ok(Some(result)) => {
                                // Broadcast result if configured
                                if let Some(ref sender) = self.result_sender {
                                    if let Err(e) = sender.send(result.clone()) {
                                        debug!("No active result subscribers: {}", e);
                                    }
                                }

                                // Update success statistics
                                let processing_time = start_time.elapsed().as_micros() as u64;
                                self.update_stats(processing_time, &result).await;

                                // Log processing time each 100 frames
                                if self.frames_processed.load(Ordering::Relaxed) % 100 == 0 {
                                    let stats = self.get_stats().await;
                                    debug!(
                                        "ProcessingConsumer '{}': Processed {} frames, last processing time: {}μs, FPS: {:.2}",
                                        self.consumer_id,
                                        stats.total_frames_processed,
                                        stats.last_processing_time_us,
                                        stats.fps
                                    );
                                }
                            }
                            Ok(None) => {
                                // No result produced (e.g., graph produced no outputs)
                                debug!(
                                    "ProcessingConsumer '{}': No result produced from processing",
                                    self.consumer_id
                                );
                            }
                            Err(e) => {
                                // Processing failed
                                error!(
                                    "ProcessingConsumer '{}': Processing failed: {}",
                                    self.consumer_id, e
                                );
                                self.processing_failures.fetch_add(1, Ordering::Relaxed);
                            }
                        }

                        self.frames_processed.fetch_add(1, Ordering::Relaxed);
                    }
                    None => {
                        // No frame available, stream might be closed
                        debug!(
                            "ProcessingConsumer '{}': No frame available",
                            self.consumer_id
                        );

                        // Add a small delay to prevent busy waiting
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    }
                }
            } else {
                error!(
                    "ProcessingConsumer '{}': No audio consumer available",
                    self.consumer_id
                );
                break;
            }
        }

        info!(
            "ProcessingConsumer '{}': Processing loop stopped",
            self.consumer_id
        );
        Ok(())
    }

    /// Process a single audio frame through the processing graph
    async fn process_frame(
        &self,
        frame: crate::acquisition::AudioFrame,
    ) -> Result<Option<ProcessingResult>> {
        let start_time = Instant::now();

        // Create frame info for the result
        let frame_info = FrameInfo {
            frame_number: frame.frame_number,
            timestamp: frame.timestamp,
            sample_rate: frame.sample_rate,
            channel_a_samples: frame.channel_a.len(),
            channel_b_samples: frame.channel_b.len(),
        };

        // Convert audio frame to processing data
        let input_data = ProcessingData::AudioFrame(frame);

        // Execute the processing graph
        let processing_results = {
            let mut graph = self.processing_graph.write().await;
            graph.execute(input_data)?
        };

        let total_processing_time = start_time.elapsed().as_micros() as u64;

        // If we got results, create a ProcessingResult
        if let Some(final_data) = processing_results.first() {
            match final_data {
                ProcessingData::PhotoacousticResult { signal, metadata } => {
                    // We already have a photoacoustic result
                    let analysis =
                        PhotoacousticAnalysis::from_signal(signal.clone(), frame_info.sample_rate);

                    // Convert nodes::ProcessingMetadata to result::ProcessingMetadata
                    let result_metadata = ProcessingMetadata {
                        processing_chain: metadata
                            .processing_steps
                            .iter()
                            .map(|step| crate::processing::result::ProcessingStep {
                                node_id: "unknown".to_string(),
                                node_type: step.clone(),
                                processing_time_us: 0,
                                input_type: "unknown".to_string(),
                                output_type: "unknown".to_string(),
                            })
                            .collect(),
                        total_processing_time_us: total_processing_time,
                        graph_config_id: "default".to_string(), // TODO: Generate graph ID
                    };

                    let result = ProcessingResult::new(
                        format!(
                            "result_{}",
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos()
                        ),
                        frame_info,
                        analysis,
                        result_metadata,
                    );

                    Ok(Some(result))
                }
                ProcessingData::SingleChannel {
                    samples,
                    sample_rate,
                    ..
                } => {
                    // Convert single channel result to photoacoustic analysis
                    let analysis =
                        PhotoacousticAnalysis::from_signal(samples.clone(), *sample_rate);

                    let metadata = ProcessingMetadata {
                        processing_chain: Vec::new(), // TODO: Track processing steps
                        total_processing_time_us: total_processing_time,
                        graph_config_id: "default".to_string(), // TODO: Generate graph ID
                    };

                    let result = ProcessingResult::new(
                        format!(
                            "result_{}",
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos()
                        ),
                        frame_info,
                        analysis,
                        metadata,
                    );

                    Ok(Some(result))
                }
                _ => {
                    // Other data types - convert to basic result
                    warn!("ProcessingConsumer: Unexpected final data type, creating basic result");

                    let analysis =
                        PhotoacousticAnalysis::from_signal(Vec::new(), frame_info.sample_rate);
                    let metadata = ProcessingMetadata {
                        processing_chain: Vec::new(),
                        total_processing_time_us: total_processing_time,
                        graph_config_id: "default".to_string(),
                    };

                    let result = ProcessingResult::new(
                        format!(
                            "result_{}",
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos()
                        ),
                        frame_info,
                        analysis,
                        metadata,
                    );

                    Ok(Some(result))
                }
            }
        } else {
            // No results from processing graph
            Ok(None)
        }
    }

    /// Update processing statistics
    /// This method is called after processing each frame
    /// to keep track of performance metrics and update the shared visualization state.
    ///
    /// ### Notes
    /// The serializable graph is updated periodically (every 100 frames)
    /// to avoid performance overhead while still providing up-to-date statistics
    /// for the /api/graph endpoint.
    /// 100 frames of 8192 bytes at 48kHz is about 17 seconds
    async fn update_stats(&self, processing_time_us: u64, result: &ProcessingResult) {
        let mut stats = self.stats.write().await;

        stats.total_frames_processed += 1;
        stats.last_processing_time_us = processing_time_us;

        // Update timing statistics
        if stats.total_frames_processed == 1 {
            stats.min_processing_time_us = processing_time_us;
            stats.max_processing_time_us = processing_time_us;
            stats.average_processing_time_us = processing_time_us as f64;
        } else {
            stats.min_processing_time_us = stats.min_processing_time_us.min(processing_time_us);
            stats.max_processing_time_us = stats.max_processing_time_us.max(processing_time_us);

            // Update running average
            let alpha = 0.1; // Smoothing factor
            stats.average_processing_time_us = alpha * processing_time_us as f64
                + (1.0 - alpha) * stats.average_processing_time_us;
        }

        // Count detections
        if result.is_detection() {
            stats.detections_count += 1;
        }

        // Update FPS calculation
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if now - stats.last_update >= 1000 {
            stats.fps =
                stats.total_frames_processed as f64 / ((now - stats.last_update) as f64 / 1000.0);
            stats.last_update = now;
        }

        // Update shared visualization state if available
        if let Some(ref visualization_state) = self.visualization_state {
            // Get the processing graph statistics
            let graph_stats = {
                let graph = self.processing_graph.read().await;
                graph.get_statistics().clone()
            };

            // Update the shared state with current graph statistics
            visualization_state
                .update_processing_statistics(graph_stats)
                .await;

            // Update the serializable graph periodically to reflect latest statistics
            // This ensures the /api/graph endpoint shows current performance data
            // but avoids updating it on every frame for performance reasons
            // 100 frames of 8192 bytes at 48kHz is about 17 seconds
            if stats.total_frames_processed % 100 == 1 {
                let serializable_graph = {
                    let graph = self.processing_graph.read().await;
                    graph.to_serializable()
                };

                visualization_state
                    .update_processing_graph(serializable_graph)
                    .await;
            }
        }
    }

    /// Stop the processing consumer (synchronous version for Drop)
    pub fn stop_sync(&self) {
        info!("Stopping ProcessingConsumer '{}'", self.consumer_id);
        self.running.store(false, Ordering::Relaxed);

        // Note: Cannot clear visualization state in sync context
        // The caller should use stop() method instead for proper cleanup
    }

    /// Update configuration for a specific node in the processing graph
    ///
    /// This method provides a way to update the configuration of a specific node
    /// in the processing graph during runtime. It supports hot-reload for compatible
    /// parameters without requiring a full restart of the processing consumer.
    ///
    /// ### Arguments
    ///
    /// * `node_id` - The ID of the node to update
    /// * `parameters` - New configuration parameters as JSON value
    ///
    /// ### Returns
    ///
    /// * `Ok(true)` - Configuration updated successfully (hot-reload)
    /// * `Ok(false)` - Configuration requires processing graph reconstruction
    /// * `Err(anyhow::Error)` - Node not found or configuration update failed
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::ProcessingConsumer;
    /// use serde_json::json;
    ///
    /// # async fn example(mut consumer: ProcessingConsumer) -> anyhow::Result<()> {
    /// // Update gain parameter for a gain node
    /// let result = consumer.update_node_config("gain_node", &json!({"gain_db": 6.0})).await;
    /// if result? {
    ///     println!("Gain updated with hot-reload");
    /// } else {
    ///     println!("Node requires reconstruction");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_node_config(
        &self,
        node_id: &str,
        parameters: &serde_json::Value,
    ) -> Result<bool> {
        debug!(
            "ProcessingConsumer '{}': Updating configuration for node '{}'",
            self.consumer_id, node_id
        );

        let mut graph = self.processing_graph.write().await;
        let result = graph.update_node_config(node_id, parameters)?;

        if result {
            info!(
                "ProcessingConsumer '{}': Node '{}' configuration updated successfully",
                self.consumer_id, node_id
            );
        } else {
            warn!("ProcessingConsumer '{}': Node '{}' requires reconstruction for configuration change", 
                  self.consumer_id, node_id);
        }

        Ok(result)
    }

    /// Update configuration for multiple nodes in the processing graph
    ///
    /// This method allows batch updates of multiple nodes, which is more efficient
    /// than updating them individually and provides atomicity.
    ///
    /// ### Arguments
    ///
    /// * `node_configs` - Map of node ID to new configuration parameters
    ///
    /// ### Returns
    ///
    /// A HashMap where:
    /// * key = node_id
    /// * value = Result<bool> indicating success and whether hot-reload was possible
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::ProcessingConsumer;
    /// use serde_json::json;
    /// use std::collections::HashMap;
    ///
    /// # async fn example(consumer: ProcessingConsumer) -> anyhow::Result<()> {
    /// let mut updates = HashMap::new();
    /// updates.insert("gain1".to_string(), json!({"gain_db": 6.0}));
    /// updates.insert("gain2".to_string(), json!({"gain_db": -3.0}));
    ///
    /// let results = consumer.update_multiple_node_configs(&updates).await;
    /// for (node_id, result) in results {
    ///     match result {
    ///         Ok(true) => println!("Node {} updated with hot-reload", node_id),
    ///         Ok(false) => println!("Node {} requires reconstruction", node_id),
    ///         Err(e) => println!("Node {} update failed: {}", node_id, e),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_multiple_node_configs(
        &self,
        node_configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, Result<bool>> {
        debug!(
            "ProcessingConsumer '{}': Updating configuration for {} nodes",
            self.consumer_id,
            node_configs.len()
        );

        let mut graph = self.processing_graph.write().await;
        graph.update_multiple_node_configs(node_configs)
    }

    /// Get a reference to the processing graph for read-only operations
    ///
    /// This method provides read-only access to the processing graph, which is
    /// useful for inspecting the current graph structure, getting statistics,
    /// or creating serializable representations.
    ///
    /// ### Returns
    ///
    /// A RwLock guard providing read access to the processing graph
    pub async fn get_processing_graph(&self) -> tokio::sync::RwLockReadGuard<'_, ProcessingGraph> {
        self.processing_graph.read().await
    }
}

impl Drop for ProcessingConsumer {
    fn drop(&mut self) {
        if self.running.load(Ordering::Relaxed) {
            self.stop_sync();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::AudioFrame;
    use crate::processing::{
        nodes::{ChannelSelectorNode, ChannelTarget, InputNode},
        ProcessingGraph,
    };

    #[tokio::test]
    async fn test_processing_consumer_creation() {
        let stream = Arc::new(SharedAudioStream::new(10));
        let graph = ProcessingGraph::new();

        let consumer = ProcessingConsumer::new(stream, graph);
        assert!(!consumer.is_running());
        assert_eq!(consumer.frames_processed(), 0);
    }

    #[tokio::test]
    async fn test_processing_consumer_with_simple_graph() {
        let stream = Arc::new(SharedAudioStream::new(10));
        let mut graph = ProcessingGraph::new();

        // Create a simple graph: input -> channel selector
        let input_node = Box::new(InputNode::new("input".to_string()));
        let selector_node = Box::new(ChannelSelectorNode::new(
            "selector".to_string(),
            ChannelTarget::ChannelA,
        ));

        graph.add_node(input_node).unwrap();
        graph.add_node(selector_node).unwrap();
        graph.connect("input", "selector").unwrap();
        graph.set_output_node("selector").unwrap();

        let consumer = ProcessingConsumer::new(stream.clone(), graph);

        // Test processing a frame
        let frame = AudioFrame::new(vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6], 48000, 1);

        let result = consumer.process_frame(frame).await;
        assert!(result.is_ok());
    }
}
