// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Core trait definitions for processing nodes
//!
//! This module defines the fundamental traits that all processing nodes must implement
//! to participate in the audio processing graph.

use super::data::ProcessingData;
use anyhow::Result;

/// Trait for processing nodes in the audio graph
///
/// This trait defines the interface that all processing nodes must implement.
/// Nodes are the fundamental building blocks of the audio processing pipeline,
/// each performing a specific operation on audio data.
///
/// ### Thread Safety
///
/// All processing nodes must be `Send + Sync` to allow for multi-threaded
/// processing graphs and parallel execution.
///
/// ### Examples
///
/// Implementing a custom processing node:
///
/// ```no_run
/// use rust_photoacoustic::processing::{ProcessingNode, ProcessingData};
/// use anyhow::Result;
///
/// struct GainNode {
///     id: String,
///     gain: f32,
/// }
///
/// impl GainNode {
///     fn new(id: String, gain: f32) -> Self {
///         Self { id, gain }
///     }
/// }
///
/// impl ProcessingNode for GainNode {
///     fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
///         match input {
///             ProcessingData::SingleChannel { mut samples, sample_rate, timestamp, frame_number } => {
///                 // Apply gain to all samples
///                 for sample in &mut samples {
///                     *sample *= self.gain;
///                 }
///                 Ok(ProcessingData::SingleChannel { samples, sample_rate, timestamp, frame_number })
///             }
///             _ => anyhow::bail!("GainNode only supports SingleChannel data"),
///         }
///     }
///
///     fn node_id(&self) -> &str { &self.id }
///     fn node_type(&self) -> &str { "gain" }
///     fn accepts_input(&self, input: &ProcessingData) -> bool {
///         matches!(input, ProcessingData::SingleChannel { .. })
///     }
///     fn output_type(&self, input: &ProcessingData) -> Option<String> {
///         match input {
///             ProcessingData::SingleChannel { .. } => Some("SingleChannel".to_string()),
///             _ => None,
///         }
///     }
///     fn reset(&mut self) { /* No state to reset */ }
///     fn clone_node(&self) -> Box<dyn ProcessingNode> {
///         Box::new(GainNode::new(self.id.clone(), self.gain))
///     }
/// }
/// ```
pub trait ProcessingNode: Send + Sync {
    /// Process input data and return output data
    ///
    /// This is the main processing method that transforms input data into output data.
    /// Each node implementation defines how it processes the specific data types it supports.
    ///
    /// ### Arguments
    ///
    /// * `input` - The input data to process
    ///
    /// ### Returns
    ///
    /// * `Ok(ProcessingData)` - Successfully processed output data
    /// * `Err(anyhow::Error)` - Processing error with details
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::{InputNode, ProcessingNode, ProcessingData};
    /// use rust_photoacoustic::acquisition::AudioFrame;
    ///
    /// let mut node = InputNode::new("input".to_string());
    /// let frame = AudioFrame {
    ///     channel_a: vec![0.1, 0.2],
    ///     channel_b: vec![0.3, 0.4],
    ///     sample_rate: 44100,
    ///     timestamp: 1000,
    ///     frame_number: 1,
    /// };
    ///
    /// let result = node.process(ProcessingData::AudioFrame(frame));
    /// assert!(result.is_ok());
    /// ```
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData>;

    /// Get the node's unique identifier
    ///
    /// Returns the unique ID assigned to this node instance.
    /// This ID is used for graph construction and debugging.
    fn node_id(&self) -> &str;

    /// Get the node's type description
    ///
    /// Returns a string describing the type of processing this node performs.
    /// This is used for configuration, debugging, and graph visualization.
    fn node_type(&self) -> &str;

    /// Check if this node can accept the given input type
    ///
    /// Returns true if this node can process the given input data type.
    /// This is used for graph validation and type checking.
    ///
    /// ### Arguments
    ///
    /// * `input` - The input data to check
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::{InputNode, ProcessingNode, ProcessingData};
    ///
    /// let node = InputNode::new("input".to_string());
    /// let data = ProcessingData::SingleChannel {
    ///     samples: vec![0.1, 0.2],
    ///     sample_rate: 44100,
    ///     timestamp: 1000,
    ///     frame_number: 1,
    /// };
    ///
    /// assert!(node.accepts_input(&data));
    /// ```
    fn accepts_input(&self, input: &ProcessingData) -> bool;

    /// Get the expected output type for the given input
    ///
    /// Returns the expected output data type name for the given input,
    /// or None if the input is not supported.
    ///
    /// ### Arguments
    ///
    /// * `input` - The input data to analyze
    ///
    /// ### Returns
    ///
    /// * `Some(String)` - Name of the expected output type
    /// * `None` - Input type not supported
    fn output_type(&self, input: &ProcessingData) -> Option<String>;

    /// Reset internal state (if any)
    ///
    /// Resets any internal state that the node might maintain.
    /// This is useful for restarting processing or clearing buffers.
    fn reset(&mut self);

    /// Clone the node (for graph reconfiguration)
    ///
    /// Creates a deep copy of the node for use in graph reconfiguration
    /// or parallel processing scenarios.
    ///
    /// ### Returns
    ///
    /// A boxed clone of this node with the same configuration
    fn clone_node(&self) -> Box<dyn ProcessingNode>;
}
