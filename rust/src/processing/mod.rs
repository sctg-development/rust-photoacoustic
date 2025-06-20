// Copyright (c) 2025 Ronan LE MEILLART and SC
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Processing Module
//!
//! This module provides a modular audio processing pipeline architecture similar to an audio graph.
//! It allows for real-time processing of audio frames with configurable processing nodes that can
//! be rearranged at runtime.
//!
//! ## Architecture Overview
//!
//! The processing system consists of:
//! - **ProcessingConsumer**: Main consumer that receives audio frames from SharedAudioStream
//! - **ProcessingGraph**: Container that manages processing nodes and their connections
//! - **ProcessingNode**: Individual processing units with specific roles:
//!   - `InputNode`: Entry point for audio data from acquisition
//!   - `FilterNode`: Applies filters (bandpass, lowpass, highpass) to audio channels
//!   - `DifferentialNode`: Calculates differential between channels
//!   - `ChannelSelectorNode`: Selects a specific channel (A or B)
//!   - `ChannelMixerNode`: Mixes channels using various strategies
//!   - `PhotoacousticOutputNode`: Final analysis node producing photoacoustic results
//! - **ProcessingResult**: Final photoacoustic analysis result with metadata
//!
//! ## Design Principles
//!
//! - **Modular**: Each processing step is encapsulated in a separate node
//! - **Configurable**: Processing graph can be created from YAML configuration
//! - **Real-time**: Designed for low-latency streaming processing
//! - **Type-safe**: Uses Rust's type system to ensure correct data flow
//! - **Runtime Reconfigurable**: Graphs can be modified and validated at runtime
//!
//! ## Node Types
//!
//! ### Input Nodes
//! - `input`: Entry point for audio frames from acquisition system
//!
//! ### Filter Nodes
//! - `filter` with `type: "bandpass"`: Bandpass filter with center frequency, bandwidth, and optional order (default: 4th order = 24dB/octave)
//! - `filter` with `type: "lowpass"`: Lowpass filter with cutoff frequency and optional order (default: 1st order = 6dB/octave)
//! - `filter` with `type: "highpass"`: Highpass filter with cutoff frequency and optional order (default: 1st order = 6dB/octave)
//!
//! All filters support an `order` parameter that controls the steepness of the roll-off:
//! - Order 1: 6dB/octave roll-off (gentle)
//! - Order 2: 12dB/octave roll-off (moderate)
//! - Order 3: 18dB/octave roll-off (steep)
//! - Order 4: 24dB/octave roll-off (very steep)
//!
//! ### Channel Operations
//! - `channel_selector`: Selects ChannelA, ChannelB, or Both channels
//! - `channel_mixer`: Mixes channels using Add, Subtract, Average, or Weighted strategies
//! - `differential`: Calculates differential between channels
//!
//! ### Output Nodes
//! - `photoacoustic_output`: Final analysis node with configurable detection threshold
//!
//! ## Configuration-Based Setup
//!
//! ```rust
//! use rust_photoacoustic::config::processing::*;
//! use rust_photoacoustic::processing::ProcessingGraph;
//!
//! // Create graph configuration
//! let config = ProcessingGraphConfig {
//!     id: "example_graph".to_string(),
//!     nodes: vec![
//!         NodeConfig {
//!             id: "input".to_string(),
//!             node_type: "input".to_string(),
//!             parameters: serde_json::Value::Null,
//!         },
//!         NodeConfig {
//!             id: "bandpass".to_string(),
//!             node_type: "filter".to_string(),
//!             parameters: serde_json::json!({
//!                 "type": "bandpass",
//!                 "center_frequency": 1000.0,
//!                 "bandwidth": 100.0,
//!                 "order": 2, // 2nd order = 12dB/octave
//!                 "target_channel": "Both"
//!             }),
//!         },
//!         NodeConfig {
//!             id: "photoacoustic".to_string(),
//!             node_type: "photoacoustic_output".to_string(),
//!             parameters: serde_json::json!({
//!                 "detection_threshold": 0.1,
//!                 "analysis_window_size": 1024
//!             }),
//!         },
//!     ],
//!     connections: vec![
//!         ConnectionConfig {
//!             from: "input".to_string(),
//!             to: "bandpass".to_string(),
//!         },
//!         ConnectionConfig {
//!             from: "bandpass".to_string(),
//!             to: "photoacoustic".to_string(),
//!         },
//!     ],
//!     output_node: Some("photoacoustic".to_string()),
//! };
//!
//! // Create graph from configuration
//! let graph = ProcessingGraph::from_config(&config)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Example Usage
//!
//! ```rust
//! use rust_photoacoustic::processing::*;
//! use rust_photoacoustic::processing::nodes::*;
//! use rust_photoacoustic::preprocessing::filters::{BandpassFilter, HighpassFilter, LowpassFilter};
//! use rust_photoacoustic::preprocessing::differential::SimpleDifferential;
//! use rust_photoacoustic::acquisition::AudioFrame;
//!
//! // Create processing graph
//! let mut graph = ProcessingGraph::new();
//!
//! // Add processing nodes
//! let input_node = Box::new(InputNode::new("input".to_string()));
//! let filter_node = Box::new(FilterNode::new(
//!     "bandpass".to_string(),
//!     Box::new(BandpassFilter::new(1000.0, 100.0).with_order(4)), // 4th order = 24dB/octave
//!     ChannelTarget::ChannelA,
//! ));
//! let diff_node = Box::new(DifferentialNode::new(
//!     "diff".to_string(),
//!     Box::new(SimpleDifferential::new()),
//! ));
//! let output_node = Box::new(PhotoacousticOutputNode::new("photoacoustic".to_string()));
//!
//! graph.add_node(input_node)?;
//! graph.add_node(filter_node)?;
//! graph.add_node(diff_node)?;
//! graph.add_node(output_node)?;
//!
//! // Connect nodes in sequence
//! graph.connect("input", "bandpass")?;
//! graph.connect("bandpass", "diff")?;
//! graph.connect("diff", "photoacoustic")?;
//!
//! // Set output node
//! graph.set_output_node("photoacoustic")?;
//!
//! // Create some example audio data
//! let audio_frame = AudioFrame {
//!     channel_a: vec![0.1, 0.2, 0.3, 0.4],
//!     channel_b: vec![0.05, 0.15, 0.25, 0.35],
//!     sample_rate: 44100,
//!     timestamp: 1000,
//!     frame_number: 1,
//! };
//!
//! // Execute processing with input data
//! let input_data = ProcessingData::from_audio_frame(audio_frame);
//! let results = graph.execute(input_data)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod computing_nodes;
pub mod consumer;
pub mod graph;
pub mod nodes;
pub mod result;

pub use consumer::ProcessingConsumer;
pub use graph::{
    PerformanceSummary, ProcessingGraph, ProcessingGraphError, SerializableConnection,
    SerializableNode, SerializableProcessingGraph,
};
pub use nodes::{
    ChannelMixerNode, ChannelSelectorNode, ChannelTarget, DifferentialNode, FilterNode, InputNode,
    MixStrategy, NodeId, PhotoacousticOutputNode, ProcessingData, ProcessingNode, RecordNode,
};
pub use result::{PhotoacousticAnalysis, ProcessingResult};
