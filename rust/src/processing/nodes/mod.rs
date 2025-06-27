// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Processing nodes module
//!
//! This module contains all processing nodes for the audio processing graph.
//! Each file contains related functionality organized for better maintainability.
//!
//! # Module Organization
//!
//! - [`data`] - Core data types (`ProcessingData`, `ProcessingMetadata`, `NodeId`)
//! - [`traits`] - Core traits (`ProcessingNode`)
//! - [`input`] - Input nodes (`InputNode`)
//! - [`filter`] - Filter nodes (`FilterNode`, `ChannelTarget`)
//! - [`channel`] - Channel operation nodes (`ChannelSelectorNode`, `ChannelMixerNode`, `MixStrategy`)
//! - [`differential`] - Differential calculation nodes (`DifferentialNode`)
//! - [`output`] - Output nodes (`PhotoacousticOutputNode`)
//! - [`record`] - Recording nodes (`RecordNode`)
//! - [`streaming`] - Real-time streaming nodes (`StreamingNode`)
//! - [`streaming_registry`] - Centralized registry for managing streaming nodes (`StreamingNodeRegistry`)
//!
//! # Examples
//!
//! Basic node usage:
//!
//! ```no_run
//! use rust_photoacoustic::processing::nodes::{
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

pub mod channel;
pub mod data;
pub mod differential;
pub mod filter;
pub mod gain;
pub mod input;
pub mod output;
pub mod python;
pub mod record;
pub mod streaming;
pub mod streaming_registry;
pub mod traits;

// Re-export all public types for backward compatibility
pub use channel::{ChannelMixerNode, ChannelSelectorNode, MixStrategy};
pub use data::{NodeId, ProcessingData, ProcessingMetadata};
pub use differential::DifferentialNode;
pub use filter::{ChannelTarget, FilterNode};
pub use gain::GainNode;
pub use input::InputNode;
pub use output::PhotoacousticOutputNode;
pub use python::{PythonNode, PythonNodeConfig};
pub use record::RecordNode;
pub use streaming::StreamingNode;
pub use streaming_registry::StreamingNodeRegistry;
pub use traits::ProcessingNode;
