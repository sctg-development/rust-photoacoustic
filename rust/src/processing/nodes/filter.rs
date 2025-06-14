// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Filter node implementation and channel targeting
//!
//! This module provides filtering capabilities for audio signals, supporting
//! both single and dual-channel processing with flexible channel targeting.

use super::data::ProcessingData;
use super::traits::ProcessingNode;
use crate::preprocessing::Filter;
use anyhow::Result;

/// Filter node that applies a digital filter to audio channels
///
/// The filter node applies digital signal processing filters to either individual
/// channels or both channels of audio data. It supports both single and dual-channel
/// input data.
///
/// ### Supported Operations
///
/// - Apply filter to Channel A only
/// - Apply filter to Channel B only  
/// - Apply filter to both channels
/// - Process single-channel data
///
/// ### Examples
///
/// Using with a bandpass filter:
///
/// ```no_run
/// use rust_photoacoustic::processing::{FilterNode, ChannelTarget, ProcessingNode, ProcessingData};
/// use rust_photoacoustic::preprocessing::BandpassFilter;
///
/// // Create a bandpass filter for both channels (2nd order = 12dB/octave)
/// let filter = Box::new(BandpassFilter::new(1000.0, 100.0).with_order(2)); // 1kHz center, 100Hz bandwidth
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
/// Ok::<(), anyhow::Error>(())
/// ```
///
/// Channel-specific filtering:
///
/// ```no_run
/// use rust_photoacoustic::processing::{FilterNode, ChannelTarget, ProcessingNode};
/// use rust_photoacoustic::preprocessing::BandpassFilter;
///
/// // Create a bandpass filter for channel A only (4th order = 24dB/octave)
/// let filter = Box::new(BandpassFilter::new(2000.0, 200.0).with_order(4)); // 2kHz center, 200Hz bandwidth
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
/// ### Variants
///
/// - [`ChannelA`](ChannelTarget::ChannelA) - Target only the first audio channel
/// - [`ChannelB`](ChannelTarget::ChannelB) - Target only the second audio channel
/// - [`Both`](ChannelTarget::Both) - Target both audio channels
///
/// ### Examples
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
    /// ### Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `filter` - The digital filter to apply (must implement [`Filter`] trait)
    /// * `target_channel` - Which channel(s) to apply the filter to
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::{FilterNode, ChannelTarget};
    /// use rust_photoacoustic::preprocessing::BandpassFilter;
    ///
    /// // Create a 3rd-order bandpass filter (18dB/octave roll-off)
    /// let filter = Box::new(BandpassFilter::new(1000.0, 100.0).with_order(3)); // 1kHz center, 100Hz bandwidth
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
