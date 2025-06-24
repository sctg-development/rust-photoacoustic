// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Core trait definitions for processing nodes
//!
//! This module defines the fundamental traits that all processing nodes must implement
//! to participate in the audio processing graph.

use super::data::ProcessingData;
use crate::processing::computing_nodes::SharedComputingState;
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

    /// Check if this node supports hot-reload configuration updates
    ///
    /// Returns true if this node can dynamically update its configuration
    /// without requiring reconstruction, false if configuration changes
    /// require node reconstruction.
    ///
    /// This method helps the configuration system determine whether to
    /// attempt hot-reload or schedule node reconstruction.
    ///
    /// ### Returns
    ///
    /// * `true` - Node supports hot-reload for some or all parameters
    /// * `false` - Node requires reconstruction for configuration changes
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::{GainNode, ProcessingNode};
    ///
    /// let gain_node = GainNode::new("amp".to_string(), 0.0);
    /// assert!(gain_node.supports_hot_reload()); // GainNode supports hot-reload
    /// ```
    fn supports_hot_reload(&self) -> bool {
        // Default implementation: no hot-reload support
        false
    }

    /// Update the node's configuration dynamically
    ///
    /// Attempts to update the node's parameters with new configuration values.
    /// This method supports hot-reload of compatible parameters without requiring
    /// node reconstruction. Returns true if the update was successful, false if
    /// the change requires node reconstruction.
    ///
    /// ### Arguments
    ///
    /// * `parameters` - New configuration parameters as JSON value
    ///
    /// ### Returns
    ///
    /// * `Ok(true)` - Configuration updated successfully (hot-reload)
    /// * `Ok(false)` - Configuration requires node reconstruction
    /// * `Err(anyhow::Error)` - Configuration update failed
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::{GainNode, ProcessingNode};
    /// use serde_json::json;
    ///
    /// let mut gain_node = GainNode::new("amp".to_string(), 0.0);
    ///
    /// // This should succeed with hot-reload
    /// let result = gain_node.update_config(&json!({"gain_db": 6.0}));
    /// assert!(result.unwrap()); // true = hot-reload successful
    ///
    /// // Check that the gain was updated
    /// assert_eq!(gain_node.get_gain_db(), 6.0);
    /// ```
    fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
        // Default implementation: no dynamic configuration support
        let _ = parameters;
        Ok(false)
    }

    /// Set the shared computing state for this node
    ///
    /// This method allows the processing graph to provide access to shared computing data
    /// that can be read and written by computing nodes (like PeakFinderNode).
    /// Regular processing nodes can ignore this or use it for read-only access.
    ///
    /// ### Arguments
    ///
    /// * `shared_state` - Optional shared computing state to attach to this node
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::computing_nodes::{ComputingSharedData, SharedComputingState};
    /// use rust_photoacoustic::processing::nodes::{GainNode, ProcessingNode};
    /// use std::sync::Arc;
    /// use tokio::sync::RwLock;
    ///
    /// let mut gain_node = GainNode::new("amp".to_string(), 0.0);
    /// let shared_state = Arc::new(RwLock::new(ComputingSharedData::default()));
    ///
    /// gain_node.set_shared_computing_state(Some(shared_state));
    /// ```
    fn set_shared_computing_state(&mut self, _shared_state: Option<SharedComputingState>) {
        // Default implementation: no-op for nodes that don't need shared computing state
    }

    /// Get the shared computing state for this node
    ///
    /// Returns the shared computing state if available, allowing nodes to read
    /// analytical results computed by other nodes in the graph.
    ///
    /// ### Returns
    ///
    /// * `Some(SharedComputingState)` - The shared computing state if available
    /// * `None` - No shared computing state is available
    ///
    /// ### Examples
    ///
    /// ```ignore
    /// use rust_photoacoustic::processing::computing_nodes::PeakFinderNode;
    /// use rust_photoacoustic::processing::nodes::ProcessingNode;
    ///
    /// let peak_finder = PeakFinderNode::new("peak".to_string());
    /// if let Some(shared_state) = peak_finder.get_shared_computing_state() {
    ///     // In an async context:
    ///     // let state = shared_state.read().await;
    ///     // if let Some(freq) = state.peak_frequency {
    ///     //     println!("Peak frequency: {} Hz", freq);
    ///     // }
    /// }
    /// ```
    fn get_shared_computing_state(&self) -> Option<SharedComputingState> {
        // Default implementation: no shared computing state available
        None
    }

    /// Get a reference to this node as Any for downcasting
    ///
    /// This method allows safe downcasting of ProcessingNode trait objects
    /// to concrete types when needed for specialized access.
    ///
    /// ### Returns
    ///
    /// * `&dyn std::any::Any` - Reference that can be downcast to concrete type
    ///
    /// ### Examples
    ///
    /// ```ignore
    /// use rust_photoacoustic::processing::computing_nodes::UniversalActionNode;
    /// use rust_photoacoustic::processing::nodes::ProcessingNode;
    ///
    /// let node: Box<dyn ProcessingNode> = Box::new(
    ///     UniversalActionNode::new("action".to_string())
    ///         .with_history_buffer_capacity(100)
    /// );
    ///
    /// if let Some(action_node) = node.as_any().downcast_ref::<UniversalActionNode>() {
    ///     let history = action_node.get_measurement_history(None);
    ///     println!("History contains {} entries", history.len());
    /// }
    /// ```
    fn as_any(&self) -> &dyn std::any::Any;
}
