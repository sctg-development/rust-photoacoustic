// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Processing data types and node interfaces
//!
//! This module defines the core data structures and traits for the audio processing graph.
//! It provides the fundamental building blocks for constructing audio processing pipelines
//! in the photoacoustic system.
//!
//! # Overview
//!
//! The module contains:
//! - [`ProcessingData`] - The data types that flow through the processing graph
//! - [`ProcessingNode`] - The trait that all processing nodes must implement
//! - Concrete node implementations for various audio processing operations
//!
//! # Architecture
//!
//! Processing nodes are connected in a directed graph where:
//! - Each node implements the [`ProcessingNode`] trait
//! - Data flows as [`ProcessingData`] variants between nodes
//! - Nodes can transform data from one type to another (e.g., dual-channel to single-channel)
//!
//! # Examples
//!
//! Basic node usage:
//!
//! ```no_run
//! use rust_photoacoustic::processing::{
//!     InputNode, ProcessingNode, ProcessingData
//! };
//! use rust_photoacoustic::acquisition::AudioFrame;
//!
//! // Create an input node
//! let mut input_node = InputNode::new("input".to_string());
//!
//! // Create sample audio frame
//! let frame = AudioFrame {
//!     channel_a: vec![0.1, 0.2, 0.3],
//!     channel_b: vec![0.4, 0.5, 0.6],
//!     sample_rate: 44100,
//!     timestamp: 1000,
//!     frame_number: 1,
//! };
//!
//! // Process the frame
//! let result = input_node.process(ProcessingData::AudioFrame(frame));
//! assert!(result.is_ok());
//! ```

use crate::acquisition::AudioFrame;
use crate::preprocessing::{DifferentialCalculator, Filter};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Unique identifier for processing nodes
///
/// Each node in the processing graph must have a unique ID that can be used
/// for graph construction, debugging, and node management.
///
/// # Examples
///
/// ```no_run
/// use rust_photoacoustic::processing::NodeId;
///
/// let node_id: NodeId = "audio_input".to_string();
/// let filter_id: NodeId = "lowpass_filter".to_string();
/// ```
pub type NodeId = String;

/// Data flowing through the processing graph
///
/// This enum represents all possible data types that can flow between processing nodes.
/// Each variant carries the necessary information for audio processing operations.
///
/// # Variants
///
/// - [`AudioFrame`](ProcessingData::AudioFrame) - Raw dual-channel audio from acquisition
/// - [`SingleChannel`](ProcessingData::SingleChannel) - Single channel processed audio
/// - [`DualChannel`](ProcessingData::DualChannel) - Dual channel processed audio  
/// - [`PhotoacousticResult`](ProcessingData::PhotoacousticResult) - Final processed result
///
/// # Examples
///
/// Creating different data types:
///
/// ```no_run
/// use rust_photoacoustic::processing::{ProcessingData};
/// use rust_photoacoustic::processing::nodes::ProcessingMetadata;
/// use rust_photoacoustic::acquisition::AudioFrame;
///
/// // Single channel data
/// let single_channel = ProcessingData::SingleChannel {
///     samples: vec![0.1, 0.2, 0.3, 0.4],
///     sample_rate: 44100,
///     timestamp: 1000,
///     frame_number: 1,
/// };
///
/// // Dual channel data
/// let dual_channel = ProcessingData::DualChannel {
///     channel_a: vec![0.1, 0.2, 0.3],
///     channel_b: vec![0.4, 0.5, 0.6],
///     sample_rate: 44100,
///     timestamp: 1000,
///     frame_number: 1,
/// };
///
/// // Photoacoustic result
/// let result = ProcessingData::PhotoacousticResult {
///     signal: vec![0.1, 0.05, 0.2],
///     metadata: ProcessingMetadata {
///         original_frame_number: 1,
///         original_timestamp: 1000,
///         sample_rate: 44100,
///         processing_steps: vec!["filter".to_string(), "differential".to_string()],
///         processing_latency_us: 1500,
///     },
/// };
/// ```
#[derive(Debug, Clone)]
pub enum ProcessingData {
    /// Raw audio frame from acquisition
    AudioFrame(AudioFrame),
    /// Single channel audio data
    SingleChannel {
        samples: Vec<f32>,
        sample_rate: u32,
        timestamp: u64,
        frame_number: u64,
    },
    /// Dual channel audio data
    DualChannel {
        channel_a: Vec<f32>,
        channel_b: Vec<f32>,
        sample_rate: u32,
        timestamp: u64,
        frame_number: u64,
    },
    /// Processed photoacoustic result
    PhotoacousticResult {
        signal: Vec<f32>,
        metadata: ProcessingMetadata,
    },
}

/// Metadata about the processing chain
///
/// Contains information about how the data was processed, including timing,
/// processing steps, and original source information.
///
/// # Fields
///
/// - `original_frame_number` - Frame number from the original audio acquisition
/// - `original_timestamp` - Timestamp from the original audio acquisition  
/// - `sample_rate` - Sample rate of the processed audio
/// - `processing_steps` - List of processing operations applied
/// - `processing_latency_us` - Total processing time in microseconds
///
/// # Examples
///
/// ```no_run
/// use rust_photoacoustic::processing::nodes::ProcessingMetadata;
///
/// let metadata = ProcessingMetadata {
///     original_frame_number: 42,
///     original_timestamp: 1234567890,
///     sample_rate: 44100,
///     processing_steps: vec![
///         "input".to_string(),
///         "highpass_filter".to_string(),
///         "differential".to_string(),
///         "photoacoustic_analysis".to_string(),
///     ],
///     processing_latency_us: 2500,
/// };
///
/// println!("Processing took {} steps", metadata.processing_steps.len());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingMetadata {
    pub original_frame_number: u64,
    pub original_timestamp: u64,
    pub sample_rate: u32,
    pub processing_steps: Vec<String>,
    pub processing_latency_us: u64,
}

impl ProcessingData {
    /// Get the sample rate from the data
    ///
    /// Returns the sample rate if the data type contains this information.
    /// PhotoacousticResult doesn't contain sample rate information directly.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::ProcessingData;
    ///
    /// let data = ProcessingData::SingleChannel {
    ///     samples: vec![0.1, 0.2, 0.3],
    ///     sample_rate: 44100,
    ///     timestamp: 1000,
    ///     frame_number: 1,
    /// };
    ///
    /// assert_eq!(data.sample_rate(), Some(44100));
    /// ```
    pub fn sample_rate(&self) -> Option<u32> {
        match self {
            ProcessingData::AudioFrame(frame) => Some(frame.sample_rate),
            ProcessingData::SingleChannel { sample_rate, .. } => Some(*sample_rate),
            ProcessingData::DualChannel { sample_rate, .. } => Some(*sample_rate),
            ProcessingData::PhotoacousticResult { .. } => None,
        }
    }

    /// Get the timestamp from the data
    ///
    /// Returns the timestamp if the data type contains this information.
    /// PhotoacousticResult doesn't contain timestamp information directly.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::ProcessingData;
    ///
    /// let data = ProcessingData::DualChannel {
    ///     channel_a: vec![0.1, 0.2],
    ///     channel_b: vec![0.3, 0.4],
    ///     sample_rate: 44100,
    ///     timestamp: 1234567890,
    ///     frame_number: 1,
    /// };
    ///
    /// assert_eq!(data.timestamp(), Some(1234567890));
    /// ```
    pub fn timestamp(&self) -> Option<u64> {
        match self {
            ProcessingData::AudioFrame(frame) => Some(frame.timestamp),
            ProcessingData::SingleChannel { timestamp, .. } => Some(*timestamp),
            ProcessingData::DualChannel { timestamp, .. } => Some(*timestamp),
            ProcessingData::PhotoacousticResult { .. } => None,
        }
    }

    /// Get the frame number from the data
    ///
    /// Returns the frame number if the data type contains this information.
    /// PhotoacousticResult doesn't contain frame number information directly.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::ProcessingData;
    ///
    /// let data = ProcessingData::SingleChannel {
    ///     samples: vec![0.1, 0.2, 0.3],
    ///     sample_rate: 44100,
    ///     timestamp: 1000,
    ///     frame_number: 42,
    /// };
    ///
    /// assert_eq!(data.frame_number(), Some(42));
    /// ```
    pub fn frame_number(&self) -> Option<u64> {
        match self {
            ProcessingData::AudioFrame(frame) => Some(frame.frame_number),
            ProcessingData::SingleChannel { frame_number, .. } => Some(*frame_number),
            ProcessingData::DualChannel { frame_number, .. } => Some(*frame_number),
            ProcessingData::PhotoacousticResult { .. } => None,
        }
    }

    /// Convert AudioFrame to DualChannel format
    ///
    /// This is a convenience method for converting raw audio frames from
    /// the acquisition system into the processing graph's dual-channel format.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::ProcessingData;
    /// use rust_photoacoustic::acquisition::AudioFrame;
    ///
    /// let frame = AudioFrame {
    ///     channel_a: vec![0.1, 0.2, 0.3],
    ///     channel_b: vec![0.4, 0.5, 0.6],
    ///     sample_rate: 44100,
    ///     timestamp: 1000,
    ///     frame_number: 1,
    /// };
    ///
    /// let dual_channel = ProcessingData::from_audio_frame(frame);
    /// match dual_channel {
    ///     ProcessingData::DualChannel { channel_a, channel_b, .. } => {
    ///         assert_eq!(channel_a.len(), 3);
    ///         assert_eq!(channel_b.len(), 3);
    ///     }
    ///     _ => panic!("Expected DualChannel data"),
    /// }
    /// ```
    pub fn from_audio_frame(frame: AudioFrame) -> Self {
        ProcessingData::DualChannel {
            channel_a: frame.channel_a,
            channel_b: frame.channel_b,
            sample_rate: frame.sample_rate,
            timestamp: frame.timestamp,
            frame_number: frame.frame_number,
        }
    }
}

/// Trait for processing nodes in the audio graph
///
/// This trait defines the interface that all processing nodes must implement.
/// Nodes are the fundamental building blocks of the audio processing pipeline,
/// each performing a specific operation on audio data.
///
/// # Thread Safety
///
/// All processing nodes must be `Send + Sync` to allow for multi-threaded
/// processing graphs and parallel execution.
///
/// # Examples
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
    /// # Arguments
    ///
    /// * `input` - The input data to process
    ///
    /// # Returns
    ///
    /// * `Ok(ProcessingData)` - Successfully processed output data
    /// * `Err(anyhow::Error)` - Processing error with details
    ///
    /// # Examples
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
    /// # Arguments
    ///
    /// * `input` - The input data to check
    ///
    /// # Examples
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
    /// # Arguments
    ///
    /// * `input` - The input data to analyze
    ///
    /// # Returns
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
    /// # Returns
    ///
    /// A boxed clone of this node with the same configuration
    fn clone_node(&self) -> Box<dyn ProcessingNode>;
}

/// Input node that accepts audio frames from the stream
///
/// The input node is typically the first node in a processing graph.
/// It converts raw [`AudioFrame`] data from the acquisition system into
/// the graph's [`ProcessingData::DualChannel`] format.
///
/// # Behavior
///
/// - Accepts any input data type (acts as a passthrough for non-AudioFrame data)
/// - Converts [`AudioFrame`] to [`ProcessingData::DualChannel`]
/// - Preserves all timing and metadata information
///
/// # Examples
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
/// # Ok::<(), anyhow::Error>(())
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
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    ///
    /// # Examples
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
}

/// Filter node that applies a digital filter to audio channels
///
/// The filter node applies digital signal processing filters to either individual
/// channels or both channels of audio data. It supports both single and dual-channel
/// input data.
///
/// # Supported Operations
///
/// - Apply filter to Channel A only
/// - Apply filter to Channel B only  
/// - Apply filter to both channels
/// - Process single-channel data
///
/// # Examples
///
/// Using with a bandpass filter:
///
/// ```no_run
/// use rust_photoacoustic::processing::{FilterNode, ChannelTarget, ProcessingNode, ProcessingData};
/// use rust_photoacoustic::preprocessing::BandpassFilter;
///
/// // Create a bandpass filter for both channels
/// let filter = Box::new(BandpassFilter::new(1000.0, 100.0)); // 1kHz center, 100Hz bandwidth
/// let mut filter_node = FilterNode::new(
///     "bandpass".to_string(),
///     filter,
///     ChannelTarget::Both
/// );
///
/// let input = ProcessingData::DualChannel {
///     channel_a: vec![0.1, 0.5, 0.3, 0.8],
///     channel_b: vec![0.2, 0.4, 0.6, 0.9],
///     sample_rate: 44100,
///     timestamp: 1000,
///     frame_number: 1,
/// };
///
/// let result = filter_node.process(input)?;
/// match result {
///     ProcessingData::DualChannel { channel_a, channel_b, .. } => {
///         // Both channels have been filtered
///         assert_eq!(channel_a.len(), 4);
///         assert_eq!(channel_b.len(), 4);
///     }
///     _ => panic!("Expected DualChannel output"),
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// Channel-specific filtering:
///
/// ```no_run
/// use rust_photoacoustic::processing::{FilterNode, ChannelTarget, ProcessingNode};
/// use rust_photoacoustic::preprocessing::BandpassFilter;
///
/// // Create a bandpass filter for channel A only
/// let filter = Box::new(BandpassFilter::new(2000.0, 200.0)); // 2kHz center, 200Hz bandwidth
/// let filter_node = FilterNode::new(
///     "bandpass_a".to_string(),
///     filter,
///     ChannelTarget::ChannelA
/// );
///
/// assert_eq!(filter_node.node_type(), "filter");
/// ```
pub struct FilterNode {
    id: String,
    filter: Box<dyn Filter>,
    target_channel: ChannelTarget,
}

/// Channel targeting options for filter and other dual-channel operations
///
/// Specifies which channel(s) should be affected by processing operations
/// that can target individual channels.
///
/// # Variants
///
/// - [`ChannelA`](ChannelTarget::ChannelA) - Target only the first audio channel
/// - [`ChannelB`](ChannelTarget::ChannelB) - Target only the second audio channel
/// - [`Both`](ChannelTarget::Both) - Target both audio channels
///
/// # Examples
///
/// ```no_run
/// use rust_photoacoustic::processing::ChannelTarget;
///
/// // Select different channels
/// let target_a = ChannelTarget::ChannelA;
/// let target_b = ChannelTarget::ChannelB;
/// let target_both = ChannelTarget::Both;
///
/// // Use in match expressions
/// match target_a {
///     ChannelTarget::ChannelA => println!("Processing channel A"),
///     ChannelTarget::ChannelB => println!("Processing channel B"),
///     ChannelTarget::Both => println!("Processing both channels"),
/// }
/// ```
#[derive(Debug, Clone)]
pub enum ChannelTarget {
    ChannelA,
    ChannelB,
    Both,
}

impl FilterNode {
    /// Create a new filter node
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `filter` - The digital filter to apply (must implement [`Filter`] trait)
    /// * `target_channel` - Which channel(s) to apply the filter to
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::{FilterNode, ChannelTarget};
    /// use rust_photoacoustic::preprocessing::BandpassFilter;
    ///
    /// let filter = Box::new(BandpassFilter::new(1000.0, 100.0)); // 1kHz center, 100Hz bandwidth
    /// let filter_node = FilterNode::new(
    ///     "bandpass_filter".to_string(),
    ///     filter,
    ///     ChannelTarget::Both
    /// );
    /// ```
    pub fn new(id: String, filter: Box<dyn Filter>, target_channel: ChannelTarget) -> Self {
        Self {
            id,
            filter,
            target_channel,
        }
    }
}

impl ProcessingNode for FilterNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        match input {
            ProcessingData::DualChannel {
                mut channel_a,
                mut channel_b,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                match self.target_channel {
                    ChannelTarget::ChannelA => {
                        channel_a = self.filter.apply(&channel_a);
                    }
                    ChannelTarget::ChannelB => {
                        channel_b = self.filter.apply(&channel_b);
                    }
                    ChannelTarget::Both => {
                        channel_a = self.filter.apply(&channel_a);
                        channel_b = self.filter.apply(&channel_b);
                    }
                }

                Ok(ProcessingData::DualChannel {
                    channel_a,
                    channel_b,
                    sample_rate,
                    timestamp,
                    frame_number,
                })
            }
            ProcessingData::SingleChannel {
                samples,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                let filtered_samples = self.filter.apply(&samples);
                Ok(ProcessingData::SingleChannel {
                    samples: filtered_samples,
                    sample_rate,
                    timestamp,
                    frame_number,
                })
            }
            _ => anyhow::bail!("FilterNode can only process DualChannel or SingleChannel data"),
        }
    }

    fn node_id(&self) -> &str {
        &self.id
    }

    fn node_type(&self) -> &str {
        "filter"
    }

    fn accepts_input(&self, input: &ProcessingData) -> bool {
        matches!(
            input,
            ProcessingData::DualChannel { .. } | ProcessingData::SingleChannel { .. }
        )
    }

    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        match input {
            ProcessingData::DualChannel { .. } => Some("DualChannel".to_string()),
            ProcessingData::SingleChannel { .. } => Some("SingleChannel".to_string()),
            _ => None,
        }
    }

    fn reset(&mut self) {
        // Filters might have internal state, but our current implementation is stateless
    }

    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        // TODO: Implement proper cloning when filter cloning is supported
        panic!("FilterNode cloning not yet implemented")
    }
}

/// Differential node that calculates the difference between two channels
///
/// The differential node performs differential signal analysis by calculating
/// the difference between two audio channels. This is a common operation in
/// photoacoustic signal processing to enhance signal-to-noise ratio and
/// reject common-mode interference.
///
/// # Input/Output
///
/// - **Input**: [`ProcessingData::DualChannel`] with two audio channels
/// - **Output**: [`ProcessingData::SingleChannel`] with the differential signal
///
/// # Signal Processing
///
/// The node uses a [`DifferentialCalculator`] implementation to compute the
/// difference signal, which may include:
/// - Simple subtraction (A - B)
/// - Weighted differential
/// - Phase-corrected differential
/// - Adaptive differential algorithms
///
/// # Examples
///
/// Basic differential calculation:
///
/// ```no_run
/// use rust_photoacoustic::processing::{DifferentialNode, ProcessingNode, ProcessingData};
/// use rust_photoacoustic::preprocessing::differential::SimpleDifferential;
///
/// let calculator = Box::new(SimpleDifferential::new());
/// let mut diff_node = DifferentialNode::new("differential".to_string(), calculator);
///
/// let input = ProcessingData::DualChannel {
///     channel_a: vec![0.5, 0.3, 0.8, 0.2],
///     channel_b: vec![0.1, 0.2, 0.3, 0.1],
///     sample_rate: 44100,
///     timestamp: 1000,
///     frame_number: 1,
/// };
///
/// let result = diff_node.process(input)?;
/// match result {
///     ProcessingData::SingleChannel { samples, .. } => {
///         // Differential signal: [0.4, 0.1, 0.5, 0.1]
///         assert_eq!(samples.len(), 4);
///         assert!((samples[0] - 0.4).abs() < 0.001);
///     }
///     _ => panic!("Expected SingleChannel output"),
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// In a processing chain:
///
/// ```no_run
/// use rust_photoacoustic::processing::{DifferentialNode, ProcessingNode};
/// use rust_photoacoustic::preprocessing::differential::SimpleDifferential;
///
/// // Create differential node with simple algorithm
/// let calculator = Box::new(SimpleDifferential::new());
/// let diff_node = DifferentialNode::new("simple_diff".to_string(), calculator);
///
/// assert_eq!(diff_node.node_type(), "differential");
/// assert_eq!(diff_node.node_id(), "simple_diff");
/// ```
pub struct DifferentialNode {
    id: String,
    calculator: Box<dyn DifferentialCalculator>,
}

impl DifferentialNode {
    /// Create a new differential node
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `calculator` - The differential calculator implementation to use
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::{DifferentialNode, ProcessingNode};
    /// use rust_photoacoustic::preprocessing::differential::SimpleDifferential;
    ///
    /// let calculator = Box::new(SimpleDifferential::new());
    /// let node = DifferentialNode::new("diff".to_string(), calculator);
    /// assert_eq!(node.node_id(), "diff");
    /// ```
    pub fn new(id: String, calculator: Box<dyn DifferentialCalculator>) -> Self {
        Self { id, calculator }
    }
}

impl ProcessingNode for DifferentialNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        match input {
            ProcessingData::DualChannel {
                channel_a,
                channel_b,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                let differential_signal = self.calculator.calculate(&channel_a, &channel_b)?;

                Ok(ProcessingData::SingleChannel {
                    samples: differential_signal,
                    sample_rate,
                    timestamp,
                    frame_number,
                })
            }
            _ => anyhow::bail!("DifferentialNode requires DualChannel input data"),
        }
    }

    fn node_id(&self) -> &str {
        &self.id
    }

    fn node_type(&self) -> &str {
        "differential"
    }

    fn accepts_input(&self, input: &ProcessingData) -> bool {
        matches!(input, ProcessingData::DualChannel { .. })
    }

    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        match input {
            ProcessingData::DualChannel { .. } => Some("SingleChannel".to_string()),
            _ => None,
        }
    }

    fn reset(&mut self) {
        // No state to reset for differential calculation
    }

    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        // TODO: Implement proper cloning when calculator cloning is supported
        panic!("DifferentialNode cloning not yet implemented")
    }
}

/// Channel selector node that extracts a specific channel from dual-channel data
///
/// The channel selector node extracts one channel from dual-channel audio data,
/// converting it to single-channel format. This is useful when you only need
/// to process one channel of a stereo signal or when splitting channels for
/// parallel processing paths.
///
/// # Input/Output
///
/// - **Input**: [`ProcessingData::DualChannel`] with two audio channels
/// - **Output**: [`ProcessingData::SingleChannel`] with the selected channel
///
/// # Channel Selection
///
/// The node can select:
/// - Channel A (left channel)
/// - Channel B (right channel)
/// - Note: [`ChannelTarget::Both`] is not valid for this node as it produces single-channel output
///
/// # Examples
///
/// Selecting channel A:
///
/// ```no_run
/// use rust_photoacoustic::processing::{ChannelSelectorNode, ChannelTarget, ProcessingNode, ProcessingData};
///
/// let mut selector = ChannelSelectorNode::new("select_a".to_string(), ChannelTarget::ChannelA);
///
/// let input = ProcessingData::DualChannel {
///     channel_a: vec![0.1, 0.2, 0.3],
///     channel_b: vec![0.4, 0.5, 0.6],
///     sample_rate: 44100,
///     timestamp: 1000,
///     frame_number: 1,
/// };
///
/// let result = selector.process(input)?;
/// match result {
///     ProcessingData::SingleChannel { samples, .. } => {
///         // Should contain channel A data: [0.1, 0.2, 0.3]
///         assert_eq!(samples, vec![0.1, 0.2, 0.3]);
///     }
///     _ => panic!("Expected SingleChannel output"),
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// Selecting channel B:
///
/// ```no_run
/// use rust_photoacoustic::processing::{ChannelSelectorNode, ChannelTarget, ProcessingNode};
///
/// let selector = ChannelSelectorNode::new("select_b".to_string(), ChannelTarget::ChannelB);
/// assert_eq!(selector.node_type(), "channel_selector");
/// ```
///
/// In parallel processing:
///
/// ```no_run
/// use rust_photoacoustic::processing::{ChannelSelectorNode, ChannelTarget};
///
/// // Create selectors for parallel processing of each channel
/// let selector_a = ChannelSelectorNode::new("path_a".to_string(), ChannelTarget::ChannelA);
/// let selector_b = ChannelSelectorNode::new("path_b".to_string(), ChannelTarget::ChannelB);
///
/// // Each can process the same dual-channel input independently
/// ```
pub struct ChannelSelectorNode {
    id: String,
    target_channel: ChannelTarget,
}

impl ChannelSelectorNode {
    /// Create a new channel selector node
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `target_channel` - Which channel to select (ChannelA or ChannelB only)
    ///
    /// # Panics
    ///
    /// This constructor does not validate the target_channel, but the process method
    /// will return an error if [`ChannelTarget::Both`] is used.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::{ChannelSelectorNode, ChannelTarget};
    ///
    /// let selector_a = ChannelSelectorNode::new("sel_a".to_string(), ChannelTarget::ChannelA);
    /// let selector_b = ChannelSelectorNode::new("sel_b".to_string(), ChannelTarget::ChannelB);
    /// ```
    pub fn new(id: String, target_channel: ChannelTarget) -> Self {
        Self { id, target_channel }
    }
}

impl ProcessingNode for ChannelSelectorNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        match input {
            ProcessingData::DualChannel {
                channel_a,
                channel_b,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                let samples = match self.target_channel {
                    ChannelTarget::ChannelA => channel_a,
                    ChannelTarget::ChannelB => channel_b,
                    ChannelTarget::Both => {
                        anyhow::bail!("ChannelSelectorNode cannot select 'Both' channels for SingleChannel output")
                    }
                };

                Ok(ProcessingData::SingleChannel {
                    samples,
                    sample_rate,
                    timestamp,
                    frame_number,
                })
            }
            _ => anyhow::bail!("ChannelSelectorNode requires DualChannel input data"),
        }
    }

    fn node_id(&self) -> &str {
        &self.id
    }

    fn node_type(&self) -> &str {
        "channel_selector"
    }

    fn accepts_input(&self, input: &ProcessingData) -> bool {
        matches!(input, ProcessingData::DualChannel { .. })
    }

    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        match input {
            ProcessingData::DualChannel { .. } => Some("SingleChannel".to_string()),
            _ => None,
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }

    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        Box::new(ChannelSelectorNode::new(
            self.id.clone(),
            self.target_channel.clone(),
        ))
    }
}

/// Channel mixer node that combines two channels using various strategies
///
/// The channel mixer node combines dual-channel audio data into single-channel data
/// using different mixing strategies. This is useful for creating mono signals from
/// stereo sources or implementing custom channel combination algorithms.
///
/// # Input/Output
///
/// - **Input**: [`ProcessingData::DualChannel`] with two audio channels
/// - **Output**: [`ProcessingData::SingleChannel`] with the mixed signal
///
/// # Mixing Strategies
///
/// The node supports several mixing strategies via [`MixStrategy`]:
/// - **Add**: Simple addition (A + B)
/// - **Subtract**: Subtraction (A - B)
/// - **Average**: Mean of both channels ((A + B) / 2)
/// - **Weighted**: Custom weighted combination (A × weight_a + B × weight_b)
///
/// # Examples
///
/// Simple addition mixing:
///
/// ```no_run
/// use rust_photoacoustic::processing::{ChannelMixerNode, MixStrategy, ProcessingNode, ProcessingData};
///
/// let mut mixer = ChannelMixerNode::new("add_mixer".to_string(), MixStrategy::Add);
///
/// let input = ProcessingData::DualChannel {
///     channel_a: vec![0.3, 0.4],
///     channel_b: vec![0.1, 0.2],
///     sample_rate: 44100,
///     timestamp: 1000,
///     frame_number: 1,
/// };
///
/// let result = mixer.process(input)?;
/// match result {
///     ProcessingData::SingleChannel { samples, .. } => {
///         // Addition result: [0.4, 0.6]
///         assert_eq!(samples, vec![0.4, 0.6]);
///     }
///     _ => panic!("Expected SingleChannel output"),
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// Weighted mixing:
///
/// ```no_run
/// use rust_photoacoustic::processing::{ChannelMixerNode, MixStrategy, ProcessingNode};
///
/// // Mix with 70% channel A, 30% channel B
/// let weighted_strategy = MixStrategy::Weighted { a_weight: 0.7, b_weight: 0.3 };
/// let mixer = ChannelMixerNode::new("weighted_mixer".to_string(), weighted_strategy);
/// assert_eq!(mixer.node_type(), "channel_mixer");
/// ```
///
/// Differential mixing (subtraction):
///
/// ```no_run
/// use rust_photoacoustic::processing::{ChannelMixerNode, MixStrategy};
///
/// // Create differential signal (A - B)
/// let diff_mixer = ChannelMixerNode::new("diff_mixer".to_string(), MixStrategy::Subtract);
/// ```
pub struct ChannelMixerNode {
    id: String,
    mix_strategy: MixStrategy,
}

/// Mixing strategies for combining two audio channels
///
/// Defines different mathematical operations for combining two audio channels
/// into a single channel output.
///
/// # Variants
///
/// - [`Add`](MixStrategy::Add) - Simple addition: `output[i] = a[i] + b[i]`
/// - [`Subtract`](MixStrategy::Subtract) - Subtraction: `output[i] = a[i] - b[i]`
/// - [`Average`](MixStrategy::Average) - Average: `output[i] = (a[i] + b[i]) / 2`
/// - [`Weighted`](MixStrategy::Weighted) - Weighted sum: `output[i] = a[i] * weight_a + b[i] * weight_b`
///
/// # Examples
///
/// Creating different mixing strategies:
///
/// ```no_run
/// use rust_photoacoustic::processing::MixStrategy;
///
/// // Simple strategies
/// let add_strategy = MixStrategy::Add;
/// let subtract_strategy = MixStrategy::Subtract;
/// let average_strategy = MixStrategy::Average;
///
/// // Weighted mixing (75% A, 25% B)
/// let weighted_strategy = MixStrategy::Weighted { a_weight: 0.75, b_weight: 0.25 };
///
/// // Inverting B channel before mixing
/// let inverted_strategy = MixStrategy::Weighted { a_weight: 1.0, b_weight: -1.0 };
/// ```
///
/// Using in calculations:
///
/// ```no_run
/// use rust_photoacoustic::processing::MixStrategy;
///
/// let strategy = MixStrategy::Average;
/// let sample_a = 0.8;
/// let sample_b = 0.4;
///
/// let result = match strategy {
///     MixStrategy::Add => sample_a + sample_b,
///     MixStrategy::Subtract => sample_a - sample_b,
///     MixStrategy::Average => (sample_a + sample_b) / 2.0,
///     MixStrategy::Weighted { a_weight, b_weight } => sample_a * a_weight + sample_b * b_weight,
/// };
/// ```
#[derive(Debug, Clone)]
pub enum MixStrategy {
    Add,                                       // A + B
    Subtract,                                  // A - B
    Average,                                   // (A + B) / 2
    Weighted { a_weight: f32, b_weight: f32 }, // A * a_weight + B * b_weight
}

impl ChannelMixerNode {
    /// Create a new channel mixer node
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `mix_strategy` - The mixing strategy to use for combining channels
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::{ChannelMixerNode, MixStrategy};
    ///
    /// // Simple average mixer
    /// let avg_mixer = ChannelMixerNode::new("average".to_string(), MixStrategy::Average);
    ///
    /// // Custom weighted mixer
    /// let weighted = MixStrategy::Weighted { a_weight: 0.8, b_weight: 0.2 };
    /// let custom_mixer = ChannelMixerNode::new("custom".to_string(), weighted);
    /// ```
    pub fn new(id: String, mix_strategy: MixStrategy) -> Self {
        Self { id, mix_strategy }
    }
}

impl ProcessingNode for ChannelMixerNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        match input {
            ProcessingData::DualChannel {
                channel_a,
                channel_b,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                if channel_a.len() != channel_b.len() {
                    anyhow::bail!("Channel lengths must match for mixing");
                }

                let mixed_samples: Vec<f32> = channel_a
                    .iter()
                    .zip(channel_b.iter())
                    .map(|(a, b)| match self.mix_strategy {
                        MixStrategy::Add => a + b,
                        MixStrategy::Subtract => a - b,
                        MixStrategy::Average => (a + b) / 2.0,
                        MixStrategy::Weighted { a_weight, b_weight } => a * a_weight + b * b_weight,
                    })
                    .collect();

                Ok(ProcessingData::SingleChannel {
                    samples: mixed_samples,
                    sample_rate,
                    timestamp,
                    frame_number,
                })
            }
            _ => anyhow::bail!("ChannelMixerNode requires DualChannel input data"),
        }
    }

    fn node_id(&self) -> &str {
        &self.id
    }

    fn node_type(&self) -> &str {
        "channel_mixer"
    }

    fn accepts_input(&self, input: &ProcessingData) -> bool {
        matches!(input, ProcessingData::DualChannel { .. })
    }

    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        match input {
            ProcessingData::DualChannel { .. } => Some("SingleChannel".to_string()),
            _ => None,
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }

    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        Box::new(ChannelMixerNode::new(
            self.id.clone(),
            self.mix_strategy.clone(),
        ))
    }
}

/// Photoacoustic output node that converts processed signal to final photoacoustic result
///
/// The photoacoustic output node is typically the final node in a processing chain.
/// It performs photoacoustic-specific signal analysis and converts processed audio
/// signals into [`ProcessingData::PhotoacousticResult`] format with metadata.
///
/// # Input/Output
///
/// - **Input**: [`ProcessingData::SingleChannel`] with processed audio signal
/// - **Output**: [`ProcessingData::PhotoacousticResult`] with analysis results and metadata
///
/// # Signal Analysis
///
/// The node performs several analysis operations:
/// - Signal amplitude analysis (peak and RMS)
/// - Detection threshold comparison
/// - Basic signal characterization
/// - Processing metadata generation
///
/// # Configuration
///
/// The node can be configured with:
/// - Detection threshold for signal presence
/// - Analysis window size for signal processing
/// - Custom analysis parameters
///
/// # Examples
///
/// Basic photoacoustic output:
///
/// ```no_run
/// use rust_photoacoustic::processing::{PhotoacousticOutputNode, ProcessingNode, ProcessingData};
///
/// let mut output_node = PhotoacousticOutputNode::new("pa_output".to_string());
///
/// let input = ProcessingData::SingleChannel {
///     samples: vec![0.05, 0.1, 0.15, 0.02],
///     sample_rate: 44100,
///     timestamp: 1000,
///     frame_number: 5,
/// };
///
/// let result = output_node.process(input)?;
/// match result {
///     ProcessingData::PhotoacousticResult { signal, metadata } => {
///         assert_eq!(signal.len(), 4);
///         assert_eq!(metadata.original_frame_number, 5);
///         assert_eq!(metadata.original_timestamp, 1000);
///         assert!(metadata.processing_steps.contains(&"photoacoustic_analysis".to_string()));
///     }
///     _ => panic!("Expected PhotoacousticResult output"),
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// Configured output node:
///
/// ```no_run
/// use rust_photoacoustic::processing::{PhotoacousticOutputNode, ProcessingNode};
///
/// let output_node = PhotoacousticOutputNode::new("configured_output".to_string())
///     .with_detection_threshold(0.05)  // 5% threshold
///     .with_analysis_window_size(2048); // 2048 sample window
///
/// assert_eq!(output_node.node_type(), "photoacoustic_output");
/// ```
///
/// Processing chain integration:
///
/// ```no_run
/// use rust_photoacoustic::processing::PhotoacousticOutputNode;
///
/// // Create output node as final stage
/// let output_node = PhotoacousticOutputNode::new("final_output".to_string())
///     .with_detection_threshold(0.01)   // 1% detection threshold
///     .with_analysis_window_size(1024); // 1024 sample analysis window
///
/// // This would typically be connected after filtering and differential processing
/// ```
pub struct PhotoacousticOutputNode {
    id: String,
    /// Minimum signal threshold for detection
    detection_threshold: f32,
    /// Signal analysis window size (samples)
    analysis_window_size: usize,
}

impl PhotoacousticOutputNode {
    /// Create a new photoacoustic output node with default settings
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    ///
    /// # Default Settings
    ///
    /// - Detection threshold: 0.01 (1%)
    /// - Analysis window size: 1024 samples
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::{PhotoacousticOutputNode, ProcessingNode};
    ///
    /// let output_node = PhotoacousticOutputNode::new("output".to_string());
    /// assert_eq!(output_node.node_id(), "output");
    /// ```
    pub fn new(id: String) -> Self {
        Self {
            id,
            detection_threshold: 0.01,  // Default threshold
            analysis_window_size: 1024, // Default window size
        }
    }

    /// Set the detection threshold for signal presence
    ///
    /// The detection threshold is used to determine whether a significant
    /// photoacoustic signal is present in the processed audio data.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Signal amplitude threshold (0.0 to 1.0)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::PhotoacousticOutputNode;
    ///
    /// let node = PhotoacousticOutputNode::new("output".to_string())
    ///     .with_detection_threshold(0.05); // 5% threshold
    /// ```
    pub fn with_detection_threshold(mut self, threshold: f32) -> Self {
        self.detection_threshold = threshold;
        self
    }

    /// Set the analysis window size for signal processing
    ///
    /// The analysis window size determines how many samples are used
    /// for signal analysis operations.
    ///
    /// # Arguments
    ///
    /// * `window_size` - Number of samples in the analysis window
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::PhotoacousticOutputNode;
    ///
    /// let node = PhotoacousticOutputNode::new("output".to_string())
    ///     .with_analysis_window_size(2048); // 2048 sample window
    /// ```
    pub fn with_analysis_window_size(mut self, window_size: usize) -> Self {
        self.analysis_window_size = window_size;
        self
    }

    /// Perform basic photoacoustic analysis on the signal
    fn analyze_signal(&self, signal: &[f32], sample_rate: u32) -> ProcessingMetadata {
        let mut processing_steps = Vec::new();
        processing_steps.push("photoacoustic_analysis".to_string());

        // Calculate basic signal statistics
        let max_amplitude = signal.iter().fold(0.0f32, |acc, &x| acc.max(x.abs()));
        let rms = (signal.iter().map(|&x| x * x).sum::<f32>() / signal.len() as f32).sqrt();

        // Simple detection logic
        let is_detection = max_amplitude > self.detection_threshold;

        if is_detection {
            processing_steps.push("detection_confirmed".to_string());
        }

        ProcessingMetadata {
            original_frame_number: 0, // Will be set by caller
            original_timestamp: 0,    // Will be set by caller
            sample_rate,
            processing_steps,
            processing_latency_us: 0, // Will be calculated by caller
        }
    }
}

impl ProcessingNode for PhotoacousticOutputNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        match input {
            ProcessingData::SingleChannel {
                samples,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                // Perform photoacoustic analysis
                let mut metadata = self.analyze_signal(&samples, sample_rate);
                metadata.original_frame_number = frame_number;
                metadata.original_timestamp = timestamp;

                Ok(ProcessingData::PhotoacousticResult {
                    signal: samples,
                    metadata,
                })
            }
            ProcessingData::PhotoacousticResult { .. } => {
                // Already a photoacoustic result, pass through
                Ok(input)
            }
            _ => anyhow::bail!("PhotoacousticOutputNode requires SingleChannel input data"),
        }
    }

    fn node_id(&self) -> &str {
        &self.id
    }

    fn node_type(&self) -> &str {
        "photoacoustic_output"
    }

    fn accepts_input(&self, input: &ProcessingData) -> bool {
        matches!(
            input,
            ProcessingData::SingleChannel { .. } | ProcessingData::PhotoacousticResult { .. }
        )
    }

    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        match input {
            ProcessingData::SingleChannel { .. } => Some("PhotoacousticResult".to_string()),
            ProcessingData::PhotoacousticResult { .. } => Some("PhotoacousticResult".to_string()),
            _ => None,
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }

    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        Box::new(
            PhotoacousticOutputNode::new(self.id.clone())
                .with_detection_threshold(self.detection_threshold)
                .with_analysis_window_size(self.analysis_window_size),
        )
    }
}
