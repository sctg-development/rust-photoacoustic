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
}
