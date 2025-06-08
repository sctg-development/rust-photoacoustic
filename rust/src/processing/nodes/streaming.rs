// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Streaming processing node implementation.
//!
//! This module provides the `StreamingNode` which implements the `ProcessingNode` trait
//! to create real-time audio streams that can be consumed via HTTP endpoints. Unlike
//! the `RecordNode` which saves data to files, the `StreamingNode` produces a
//! `SharedAudioStream` that can be accessed through dynamic API endpoints.

use super::data::ProcessingData;
use super::streaming_registry::StreamingNodeRegistry;
use super::traits::ProcessingNode;
use crate::acquisition::stream::{AudioFrame, SharedAudioStream};
use anyhow::Result;
use uuid::Uuid;

/// A processing node that creates real-time audio streams for HTTP consumption.
///
/// The `StreamingNode` acts as a pass-through node in the processing graph while
/// simultaneously providing a `SharedAudioStream` that can be consumed via HTTP
/// endpoints. It registers itself with a `StreamingNodeRegistry` to enable
/// dynamic routing based on node IDs.
///
/// # Examples
///
/// ```no_run
/// use rust_photoacoustic::processing::nodes::{StreamingNode, StreamingNodeRegistry};
/// use uuid::Uuid;
///
/// // Create a registry and a streaming node
/// let registry = StreamingNodeRegistry::new();
/// let node_id = Uuid::new_v4();
/// let mut streaming_node = StreamingNode::new(
///     node_id,
///     "Live Audio Stream",
///     registry.clone()
/// );
///
/// // The node automatically registers its stream with the registry
/// assert!(registry.get_stream(&node_id).is_some());
/// ```
#[derive(Debug)]
pub struct StreamingNode {
    /// Unique identifier for this node (as string for trait compatibility)
    id_str: String,
    /// UUID identifier for registry operations
    id_uuid: Uuid,
    /// Human-readable name for the node
    name: String,
    /// Shared audio stream for real-time consumption
    stream: SharedAudioStream,
    /// Registry for managing the stream lifecycle
    registry: StreamingNodeRegistry,
}

impl StreamingNode {
    /// Creates a new streaming node with the specified ID and name.
    ///
    /// The node automatically registers its audio stream with the provided registry,
    /// making it available for consumption via HTTP endpoints.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `name` - Human-readable name for the node
    /// * `registry` - Registry to manage the stream lifecycle
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::{StreamingNode, StreamingNodeRegistry};
    /// use uuid::Uuid;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// let node_id = Uuid::new_v4();
    /// let node = StreamingNode::new(
    ///     node_id,
    ///     "Real-time Audio Stream",
    ///     registry
    /// );
    /// ```
    pub fn new(id: Uuid, name: &str, registry: StreamingNodeRegistry) -> Self {
        let stream = SharedAudioStream::new(1024); // Default buffer size

        // Register the stream with the registry using the name
        registry.register_stream_with_name(id, name, stream.clone());

        Self {
            id_str: id.to_string(),
            id_uuid: id,
            name: name.to_string(),
            stream,
            registry,
        }
    }

    /// Creates a new streaming node with the specified string ID and name.
    ///
    /// This variant is designed for use with processing graphs where node IDs
    /// are defined as strings in configuration files. It generates a UUID
    /// internally for registry operations while using the string ID for
    /// graph connectivity.
    ///
    /// # Arguments
    ///
    /// * `id_str` - String identifier for this node (used in graph connections)
    /// * `name` - Human-readable name for the node
    /// * `registry` - Registry to manage the stream lifecycle
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::{StreamingNode, StreamingNodeRegistry};
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// let node = StreamingNode::new_with_string_id(
    ///     "streaming_input",
    ///     "Real-time Audio Stream",
    ///     registry
    /// );
    /// ```
    pub fn new_with_string_id(id_str: &str, name: &str, registry: StreamingNodeRegistry) -> Self {
        let stream = SharedAudioStream::new(1024); // Default buffer size
        let id_uuid = Uuid::new_v4(); // Generate UUID for registry operations

        // Register the stream with the registry using the UUID and name
        registry.register_stream_with_name(id_uuid, name, stream.clone());

        Self {
            id_str: id_str.to_string(),
            id_uuid,
            name: name.to_string(),
            stream,
            registry,
        }
    }

    /// Returns a reference to the shared audio stream.
    ///
    /// This can be used to access the stream directly without going through
    /// the registry, useful for testing or advanced use cases.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::{StreamingNode, StreamingNodeRegistry};
    /// use uuid::Uuid;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// let node = StreamingNode::new(
    ///     Uuid::new_v4(),
    ///     "Test Stream",
    ///     registry
    /// );
    ///
    /// let stream = node.get_stream();
    /// // Use stream for direct access...
    /// ```
    pub fn get_stream(&self) -> &SharedAudioStream {
        &self.stream
    }

    /// Returns the node ID.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::{StreamingNode, StreamingNodeRegistry};
    /// use uuid::Uuid;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// let node_id = Uuid::new_v4();
    /// let node = StreamingNode::new(node_id, "Test", registry);
    ///
    /// assert_eq!(node.get_id(), node_id);
    /// ```
    pub fn get_id(&self) -> Uuid {
        self.id_uuid
    }

    /// Converts ProcessingData to AudioFrame for streaming.
    ///
    /// This helper method converts different ProcessingData variants into
    /// AudioFrame format that can be published to the stream.
    fn convert_to_audio_frame(&self, input: &ProcessingData) -> Option<AudioFrame> {
        match input {
            ProcessingData::AudioFrame(frame) => Some(frame.clone()),
            ProcessingData::DualChannel {
                channel_a,
                channel_b,
                sample_rate,
                timestamp,
                frame_number,
            } => Some(AudioFrame {
                channel_a: channel_a.clone(),
                channel_b: channel_b.clone(),
                sample_rate: *sample_rate,
                timestamp: *timestamp,
                frame_number: *frame_number,
            }),
            ProcessingData::SingleChannel {
                samples,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                // For single channel, duplicate to both channels
                Some(AudioFrame {
                    channel_a: samples.clone(),
                    channel_b: samples.clone(),
                    sample_rate: *sample_rate,
                    timestamp: *timestamp,
                    frame_number: *frame_number,
                })
            }
            ProcessingData::PhotoacousticResult { .. } => {
                // Cannot convert photoacoustic result to audio frame
                None
            }
        }
    }
}

impl ProcessingNode for StreamingNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Convert input to AudioFrame and publish to stream if possible
        if let Some(audio_frame) = self.convert_to_audio_frame(&input) {
            // Publish to stream in a non-blocking way
            tokio::spawn({
                let stream = self.stream.clone();
                async move {
                    if let Err(e) = stream.publish(audio_frame).await {
                        log::warn!("Failed to publish to audio stream: {}", e);
                    }
                }
            });
        }

        // Pass through the input data unchanged to the next node in the graph
        Ok(input)
    }

    fn node_id(&self) -> &str {
        &self.id_str
    }

    fn node_type(&self) -> &str {
        "streaming"
    }

    fn accepts_input(&self, input: &ProcessingData) -> bool {
        // Accept all input types except PhotoacousticResult
        !matches!(input, ProcessingData::PhotoacousticResult { .. })
    }

    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        // Pass-through node, output type matches input type
        match input {
            ProcessingData::AudioFrame(_) => Some("AudioFrame".to_string()),
            ProcessingData::SingleChannel { .. } => Some("SingleChannel".to_string()),
            ProcessingData::DualChannel { .. } => Some("DualChannel".to_string()),
            ProcessingData::PhotoacousticResult { .. } => None,
        }
    }

    fn reset(&mut self) {
        // No internal state to reset
    }

    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        // Create a new StreamingNode with the same configuration
        Box::new(StreamingNode::new(
            self.id_uuid,
            &self.name,
            self.registry.clone(),
        ))
    }
}

impl Drop for StreamingNode {
    /// Ensures proper cleanup when the node is dropped.
    ///
    /// This removes the stream from the registry to prevent memory leaks
    /// and stale references.
    fn drop(&mut self) {
        if !self.registry.unregister_stream(&self.id_uuid) {
            log::warn!(
                "Stream for node {} was not registered in the registry",
                self.id_uuid
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_node_creation() {
        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();
        let node = StreamingNode::new(node_id, "Test Stream", registry.clone());

        assert_eq!(node.get_id(), node_id);
        assert_eq!(node.node_type(), "streaming");

        // Verify stream is registered
        assert!(registry.get_stream(&node_id).is_some());
    }

    #[tokio::test]
    async fn test_process_pass_through() {
        let registry = StreamingNodeRegistry::new();
        let mut node = StreamingNode::new(Uuid::new_v4(), "Test Stream", registry);

        let test_data = ProcessingData::SingleChannel {
            samples: vec![1.0, 2.0, 3.0, 4.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 0,
        };

        let result = node.process(test_data.clone()).unwrap();

        // Verify pass-through behavior
        match (&result, &test_data) {
            (
                ProcessingData::SingleChannel {
                    samples: result_samples,
                    ..
                },
                ProcessingData::SingleChannel {
                    samples: test_samples,
                    ..
                },
            ) => {
                assert_eq!(result_samples, test_samples);
            }
            _ => panic!("Expected matching ProcessingData::SingleChannel"),
        }
    }

    #[test]
    fn test_accepts_input() {
        let registry = StreamingNodeRegistry::new();
        let node = StreamingNode::new(Uuid::new_v4(), "Test Stream", registry);

        // Test accepting various input types
        let audio_frame = ProcessingData::AudioFrame(AudioFrame {
            channel_a: vec![1.0, 2.0],
            channel_b: vec![3.0, 4.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 0,
        });
        assert!(node.accepts_input(&audio_frame));

        let single_channel = ProcessingData::SingleChannel {
            samples: vec![1.0, 2.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 0,
        };
        assert!(node.accepts_input(&single_channel));

        let dual_channel = ProcessingData::DualChannel {
            channel_a: vec![1.0, 2.0],
            channel_b: vec![3.0, 4.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 0,
        };
        assert!(node.accepts_input(&dual_channel));

        let photoacoustic = ProcessingData::PhotoacousticResult {
            signal: vec![1.0, 2.0, 3.0, 4.0],
            metadata: super::super::data::ProcessingMetadata {
                original_frame_number: 0,
                original_timestamp: 1000,
                sample_rate: 44100,
                processing_steps: vec!["test".to_string()],
                processing_latency_us: 100,
            },
        };
        assert!(!node.accepts_input(&photoacoustic));
    }

    #[test]
    fn test_output_type() {
        let registry = StreamingNodeRegistry::new();
        let node = StreamingNode::new(Uuid::new_v4(), "Test Stream", registry);

        let audio_frame = ProcessingData::AudioFrame(AudioFrame {
            channel_a: vec![1.0, 2.0],
            channel_b: vec![3.0, 4.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 0,
        });
        assert_eq!(
            node.output_type(&audio_frame),
            Some("AudioFrame".to_string())
        );

        let single_channel = ProcessingData::SingleChannel {
            samples: vec![1.0, 2.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 0,
        };
        assert_eq!(
            node.output_type(&single_channel),
            Some("SingleChannel".to_string())
        );
    }

    #[test]
    fn test_reset() {
        let registry = StreamingNodeRegistry::new();
        let mut node = StreamingNode::new(Uuid::new_v4(), "Test Stream", registry);

        // Reset should not fail
        node.reset();
    }

    #[test]
    fn test_drop_cleanup() {
        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();

        {
            let _node = StreamingNode::new(node_id, "Test Stream", registry.clone());
            // Verify stream is registered
            assert!(registry.get_stream(&node_id).is_some());
        } // Node is dropped here

        // Verify stream is automatically unregistered
        assert!(registry.get_stream(&node_id).is_none());
    }

    #[test]
    fn test_convert_to_audio_frame() {
        let registry = StreamingNodeRegistry::new();
        let node = StreamingNode::new(Uuid::new_v4(), "Test Stream", registry);

        // Test AudioFrame conversion (should clone)
        let audio_frame = ProcessingData::AudioFrame(AudioFrame {
            channel_a: vec![1.0, 2.0],
            channel_b: vec![3.0, 4.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 0,
        });
        let converted = node.convert_to_audio_frame(&audio_frame);
        assert!(converted.is_some());

        // Test SingleChannel conversion (should duplicate to both channels)
        let single_channel = ProcessingData::SingleChannel {
            samples: vec![1.0, 2.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 0,
        };
        let converted = node.convert_to_audio_frame(&single_channel);
        assert!(converted.is_some());
        if let Some(frame) = converted {
            assert_eq!(frame.channel_a, frame.channel_b);
        }

        // Test PhotoacousticResult conversion (should return None)
        let photoacoustic = ProcessingData::PhotoacousticResult {
            signal: vec![1.0, 2.0],
            metadata: super::super::data::ProcessingMetadata {
                original_frame_number: 0,
                original_timestamp: 1000,
                sample_rate: 44100,
                processing_steps: vec!["test".to_string()],
                processing_latency_us: 100,
            },
        };
        let converted = node.convert_to_audio_frame(&photoacoustic);
        assert!(converted.is_none());
    }
}
