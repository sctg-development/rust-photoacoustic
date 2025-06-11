// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Channel operation nodes
//!
//! This module provides nodes for channel selection and mixing operations,
//! allowing flexible routing and combination of audio channels.

use super::data::ProcessingData;
use super::filter::ChannelTarget;
use super::traits::ProcessingNode;
use anyhow::Result;

/// Channel selector node that extracts a specific channel from dual-channel data
///
/// The channel selector node extracts one channel from dual-channel audio data,
/// converting it to single-channel format. This is useful when you only need
/// to process one channel of a stereo signal or when splitting channels for
/// parallel processing paths.
///
/// ### Input/Output
///
/// - **Input**: [`ProcessingData::DualChannel`] with two audio channels
/// - **Output**: [`ProcessingData::SingleChannel`] with the selected channel
///
/// ### Channel Selection
///
/// The node can select:
/// - Channel A (left channel)
/// - Channel B (right channel)
/// - Note: [`ChannelTarget::Both`] is not valid for this node as it produces single-channel output
///
/// ### Examples
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
/// Ok::<(), anyhow::Error>(())
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
    /// ### Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `target_channel` - Which channel to select (ChannelA or ChannelB only)
    ///
    /// ### Panics
    ///
    /// This constructor does not validate the target_channel, but the process method
    /// will return an error if [`ChannelTarget::Both`] is used.
    ///
    /// ### Examples
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
/// ### Input/Output
///
/// - **Input**: [`ProcessingData::DualChannel`] with two audio channels
/// - **Output**: [`ProcessingData::SingleChannel`] with the mixed signal
///
/// ### Mixing Strategies
///
/// The node supports several mixing strategies via [`MixStrategy`]:
/// - **Add**: Simple addition (A + B)
/// - **Subtract**: Subtraction (A - B)
/// - **Average**: Mean of both channels ((A + B) / 2)
/// - **Weighted**: Custom weighted combination (A × weight_a + B × weight_b)
///
/// ### Examples
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
/// Ok::<(), anyhow::Error>(())
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
/// ### Variants
///
/// - [`Add`](MixStrategy::Add) - Simple addition: `output[i] = a[i] + b[i]`
/// - [`Subtract`](MixStrategy::Subtract) - Subtraction: `output[i] = a[i] - b[i]`
/// - [`Average`](MixStrategy::Average) - Average: `output[i] = (a[i] + b[i]) / 2`
/// - [`Weighted`](MixStrategy::Weighted) - Weighted sum: `output[i] = a[i] * weight_a + b[i] * weight_b`
///
/// ### Examples
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
    /// ### Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `mix_strategy` - The mixing strategy to use for combining channels
    ///
    /// ### Examples
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
