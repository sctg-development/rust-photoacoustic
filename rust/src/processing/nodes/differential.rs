//! # Differential Node
//!
//! This module provides the `DifferentialNode` for calculating differential signals
//! between two audio channels. Differential processing is essential in photoacoustic
//! signal processing to enhance signal-to-noise ratio and reject common-mode interference.
//!
//! ## Features
//!
//! - Calculates the difference between two audio channels (A - B)
//! - Uses pluggable differential calculator implementations
//! - Converts dual-channel input to single-channel output
//! - Supports various differential algorithms through trait interface
//!
//! ## Examples
//!
//! Basic usage:
//!
//! ```no_run
//! use rust_photoacoustic::processing::{DifferentialNode, ProcessingNode, ProcessingData};
//! use rust_photoacoustic::preprocessing::differential::SimpleDifferential;
//!
//! let calculator = Box::new(SimpleDifferential::new());
//! let mut diff_node = DifferentialNode::new("differential".to_string(), calculator);
//!
//! let input = ProcessingData::DualChannel {
//!     channel_a: vec![0.5, 0.3, 0.8],
//!     channel_b: vec![0.1, 0.2, 0.3],
//!     sample_rate: 44100,
//!     timestamp: 1000,
//!     frame_number: 1,
//! };
//!
//! let result = diff_node.process(input)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

use super::{ProcessingData, ProcessingNode};
use crate::preprocessing::DifferentialCalculator;
use anyhow::Result;

/// Differential node that calculates the difference between two channels
///
/// The differential node performs differential signal analysis by calculating
/// the difference between two audio channels. This is a common operation in
/// photoacoustic signal processing to enhance signal-to-noise ratio and
/// reject common-mode interference.
///
/// ### Input/Output
///
/// - **Input**: [`ProcessingData::DualChannel`] with two audio channels
/// - **Output**: [`ProcessingData::SingleChannel`] with the differential signal
///
/// ### Signal Processing
///
/// The node uses a [`DifferentialCalculator`] implementation to compute the
/// difference signal, which may include:
/// - Simple subtraction (A - B)
/// - Weighted differential
/// - Phase-corrected differential
/// - Adaptive differential algorithms
///
/// ### Examples
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
/// Ok::<(), anyhow::Error>(())
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
    /// ### Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `calculator` - The differential calculator implementation to use
    ///
    /// ### Examples
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

    fn supports_hot_reload(&self) -> bool {
        false // DifferentialNode has infrastructure but no configurable parameters yet
    }

    fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
        use serde_json::Value;

        // Parse the parameters and update compatible ones
        if let Value::Object(_params) = parameters {
            // Currently, the DifferentialNode doesn't have hot-reloadable parameters
            // because the DifferentialCalculator doesn't support runtime reconfiguration.
            //
            // Future enhancements could include:
            // - Different calculator types (simple, weighted, adaptive)
            // - Calculator-specific parameters (weights, thresholds, etc.)
            //
            // For now, return false to indicate no hot-reload support
            // This will trigger a node reconstruction when parameters change
        } else {
            anyhow::bail!("Parameters must be a JSON object");
        }

        Ok(false) // No hot-reload support currently - requires node reconstruction
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preprocessing::differential::SimpleDifferential;
    use serde_json::json;

    #[test]
    fn test_differential_node_creation() {
        let calculator = Box::new(SimpleDifferential::new());
        let node = DifferentialNode::new("test_diff".to_string(), calculator);

        assert_eq!(node.node_id(), "test_diff");
        assert_eq!(node.node_type(), "differential");
    }

    #[test]
    fn test_differential_node_accepts_input() {
        let calculator = Box::new(SimpleDifferential::new());
        let node = DifferentialNode::new("test".to_string(), calculator);

        let dual_channel = ProcessingData::DualChannel {
            channel_a: vec![1.0, 2.0],
            channel_b: vec![0.5, 1.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let single_channel = ProcessingData::SingleChannel {
            samples: vec![1.0, 2.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        assert!(node.accepts_input(&dual_channel));
        assert!(!node.accepts_input(&single_channel));
    }

    #[test]
    fn test_differential_node_output_type() {
        let calculator = Box::new(SimpleDifferential::new());
        let node = DifferentialNode::new("test".to_string(), calculator);

        let dual_channel = ProcessingData::DualChannel {
            channel_a: vec![1.0, 2.0],
            channel_b: vec![0.5, 1.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let single_channel = ProcessingData::SingleChannel {
            samples: vec![1.0, 2.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        assert_eq!(
            node.output_type(&dual_channel),
            Some("SingleChannel".to_string())
        );
        assert_eq!(node.output_type(&single_channel), None);
    }

    #[test]
    fn test_differential_node_process() {
        let calculator = Box::new(SimpleDifferential::new());
        let mut node = DifferentialNode::new("test".to_string(), calculator);

        let input = ProcessingData::DualChannel {
            channel_a: vec![1.0, 2.0, 3.0],
            channel_b: vec![0.5, 1.0, 1.5],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let result = node.process(input).unwrap();
        match result {
            ProcessingData::SingleChannel {
                samples,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                assert_eq!(samples, vec![0.5, 1.0, 1.5]); // A - B
                assert_eq!(sample_rate, 44100);
                assert_eq!(timestamp, 1000);
                assert_eq!(frame_number, 1);
            }
            _ => panic!("Expected SingleChannel output"),
        }
    }

    #[test]
    fn test_differential_node_process_invalid_input() {
        let calculator = Box::new(SimpleDifferential::new());
        let mut node = DifferentialNode::new("test".to_string(), calculator);

        let input = ProcessingData::SingleChannel {
            samples: vec![1.0, 2.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let result = node.process(input);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("DifferentialNode requires DualChannel input"));
    }

    #[test]
    fn test_differential_node_update_config_no_support() {
        let calculator = Box::new(SimpleDifferential::new());
        let mut node = DifferentialNode::new("test".to_string(), calculator);

        // Test with valid JSON object (but no hot-reload support)
        let params = json!({
            "some_parameter": "value"
        });

        let result = node.update_config(&params);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false); // No hot-reload support currently
    }

    #[test]
    fn test_differential_node_update_config_invalid_params() {
        let calculator = Box::new(SimpleDifferential::new());
        let mut node = DifferentialNode::new("test".to_string(), calculator);

        // Test with non-object JSON
        let params = json!("not_an_object");

        let result = node.update_config(&params);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Parameters must be a JSON object"));
    }

    #[test]
    fn test_differential_node_reset() {
        let calculator = Box::new(SimpleDifferential::new());
        let mut node = DifferentialNode::new("test".to_string(), calculator);

        // Reset should not panic
        node.reset();
    }
}
