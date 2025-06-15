// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Input node implementation
//!
//! This module provides the input node which serves as the entry point
//! for audio data from the acquisition system into the processing graph.

use super::data::ProcessingData;
use super::traits::ProcessingNode;
use anyhow::Result;

/// Input node that accepts audio frames from the stream
///
/// The input node is typically the first node in a processing graph.
/// It converts raw [`AudioFrame`] data from the acquisition system into
/// the graph's [`ProcessingData::DualChannel`] format.
///
/// ### Behavior
///
/// - Accepts any input data type (acts as a passthrough for non-AudioFrame data)
/// - Converts [`AudioFrame`] to [`ProcessingData::DualChannel`]
/// - Preserves all timing and metadata information
///
/// ### Examples
///
/// Basic usage:
///
/// ```no_run
/// use rust_photoacoustic::processing::{InputNode, ProcessingNode, ProcessingData};
/// use rust_photoacoustic::acquisition::AudioFrame;
///
/// let mut input_node = InputNode::new("audio_input".to_string());
///
/// let frame = AudioFrame {
///     channel_a: vec![0.1, 0.2, 0.3],
///     channel_b: vec![0.4, 0.5, 0.6],
///     sample_rate: 44100,
///     timestamp: 1000,
///     frame_number: 1,
/// };
///
/// let result = input_node.process(ProcessingData::AudioFrame(frame))?;
/// match result {
///     ProcessingData::DualChannel { channel_a, channel_b, sample_rate, .. } => {
///         assert_eq!(channel_a.len(), 3);
///         assert_eq!(channel_b.len(), 3);
///         assert_eq!(sample_rate, 44100);
///     }
///     _ => panic!("Expected DualChannel output"),
/// }
/// Ok::<(), anyhow::Error>(())
/// ```
///
/// In a processing graph:
///
/// ```no_run
/// use rust_photoacoustic::processing::{InputNode, ProcessingNode};
///
/// // Create input node as first stage of processing
/// let input_node = InputNode::new("input".to_string());
/// assert_eq!(input_node.node_type(), "input");
/// assert_eq!(input_node.node_id(), "input");
/// ```
pub struct InputNode {
    id: String,
}

impl InputNode {
    /// Create a new input node with the given ID
    ///
    /// ### Arguments
    ///
    /// * `id` - Unique identifier for this node
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::{InputNode, ProcessingNode};
    ///
    /// let node = InputNode::new("main_input".to_string());
    /// assert_eq!(node.node_id(), "main_input");
    /// ```
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

impl ProcessingNode for InputNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        match input {
            ProcessingData::AudioFrame(frame) => Ok(ProcessingData::from_audio_frame(frame)),
            other => Ok(other), // Pass through other types
        }
    }

    fn node_id(&self) -> &str {
        &self.id
    }

    fn node_type(&self) -> &str {
        "input"
    }

    fn accepts_input(&self, _input: &ProcessingData) -> bool {
        true // Input node accepts any data
    }

    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        match input {
            ProcessingData::AudioFrame(_) => Some("DualChannel".to_string()),
            _ => Some("PassThrough".to_string()),
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }

    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        Box::new(InputNode::new(self.id.clone()))
    }

    fn supports_hot_reload(&self) -> bool {
        false // InputNode has no configurable parameters
    }
}
