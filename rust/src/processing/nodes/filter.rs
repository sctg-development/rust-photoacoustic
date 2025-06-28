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
use log;

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

    fn supports_hot_reload(&self) -> bool {
        true // FilterNode supports hot-reload for target_channel parameter
    }

    fn update_config(&mut self, parameters: &serde_json::Value) -> anyhow::Result<bool> {
        let mut updated = false;

        // Update target channel if provided
        if let Some(target) = parameters.get("target_channel") {
            if let Some(target_str) = target.as_str() {
                match target_str {
                    "channel_a" | "ChannelA" => {
                        self.target_channel = ChannelTarget::ChannelA;
                        updated = true;
                    }
                    "channel_b" | "ChannelB" => {
                        self.target_channel = ChannelTarget::ChannelB;
                        updated = true;
                    }
                    "both" | "Both" => {
                        self.target_channel = ChannelTarget::Both;
                        updated = true;
                    }
                    _ => {
                        anyhow::bail!("Invalid target_channel value. Must be 'channel_a', 'channel_b', or 'both'");
                    }
                }
            } else {
                anyhow::bail!("target_channel must be a string");
            }
        }

        // Update the underlying filter's parameters if provided
        // Extract filter-specific parameters from the main parameters object
        let mut filter_params = serde_json::Map::new();

        // Common filter parameters
        if let Some(sample_rate) = parameters.get("sample_rate") {
            filter_params.insert("sample_rate".to_string(), sample_rate.clone());
        }
        if let Some(order) = parameters.get("order") {
            filter_params.insert("order".to_string(), order.clone());
        }

        // BandpassFilter specific parameters
        if let Some(center_freq) = parameters.get("center_freq") {
            filter_params.insert("center_freq".to_string(), center_freq.clone());
        }
        if let Some(bandwidth) = parameters.get("bandwidth") {
            filter_params.insert("bandwidth".to_string(), bandwidth.clone());
        }

        // LowpassFilter and HighpassFilter specific parameters
        if let Some(cutoff_freq) = parameters.get("cutoff_freq") {
            filter_params.insert("cutoff_freq".to_string(), cutoff_freq.clone());
        }

        // If we have filter parameters to update, try to update the underlying filter
        if !filter_params.is_empty() {
            let filter_value = serde_json::Value::Object(filter_params);
            match self.filter.update_config(&filter_value) {
                Ok(filter_updated) => {
                    if filter_updated {
                        updated = true;
                    }
                }
                Err(e) => {
                    // Log the error but don't fail the entire update if only filter update failed
                    log::warn!("Failed to update underlying filter configuration: {}", e);
                    // Re-throw the error to inform the caller
                    anyhow::bail!("Filter configuration update failed: {}", e);
                }
            }
        }

        Ok(updated)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preprocessing::filter::LowpassFilter;
    use crate::processing::nodes::data::ProcessingData;
    use serde_json::json;

    #[test]
    fn test_filter_node_creation() {
        let filter = Box::new(LowpassFilter::new(1000.0));
        let node = FilterNode::new("test".to_string(), filter, ChannelTarget::Both);

        assert_eq!(node.node_id(), "test");
        assert_eq!(node.node_type(), "filter");
    }

    #[test]
    fn test_filter_node_update_config_target_channel() {
        let filter = Box::new(LowpassFilter::new(1000.0));
        let mut node = FilterNode::new("test".to_string(), filter, ChannelTarget::Both);

        // Update to channel A
        let params = json!({"target_channel": "channel_a"});
        let result = node.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert!(matches!(node.target_channel, ChannelTarget::ChannelA));

        // Update to channel B
        let params = json!({"target_channel": "channel_b"});
        let result = node.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert!(matches!(node.target_channel, ChannelTarget::ChannelB));

        // Update to both
        let params = json!({"target_channel": "both"});
        let result = node.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert!(matches!(node.target_channel, ChannelTarget::Both));
    }

    #[test]
    fn test_filter_node_update_config_target_channel_case_insensitive() {
        let filter = Box::new(LowpassFilter::new(1000.0));
        let mut node = FilterNode::new("test".to_string(), filter, ChannelTarget::Both);

        // Test uppercase variants
        let params = json!({"target_channel": "ChannelA"});
        let result = node.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert!(matches!(node.target_channel, ChannelTarget::ChannelA));

        let params = json!({"target_channel": "ChannelB"});
        let result = node.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert!(matches!(node.target_channel, ChannelTarget::ChannelB));

        let params = json!({"target_channel": "Both"});
        let result = node.update_config(&params);

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert!(matches!(node.target_channel, ChannelTarget::Both));
    }

    #[test]
    fn test_filter_node_update_config_invalid_target_channel() {
        let filter = Box::new(LowpassFilter::new(1000.0));
        let mut node = FilterNode::new("test".to_string(), filter, ChannelTarget::Both);

        let params = json!({"target_channel": "invalid"});
        let result = node.update_config(&params);

        assert!(result.is_err());
    }

    #[test]
    fn test_filter_node_update_config_no_params() {
        let filter = Box::new(LowpassFilter::new(1000.0));
        let mut node = FilterNode::new("test".to_string(), filter, ChannelTarget::Both);

        let params = json!({});
        let result = node.update_config(&params);

        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false for no updates
    }
}
