// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Core data types for the processing graph
//!
//! This module defines the fundamental data structures that flow through
//! the audio processing graph and associated metadata.

use crate::acquisition::AudioFrame;
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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
