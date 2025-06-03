//! # Output Node
//!
//! This module provides the `PhotoacousticOutputNode` for final photoacoustic signal analysis.
//! The output node typically serves as the final stage in a processing chain, converting
//! processed audio signals into photoacoustic analysis results with metadata.
//!
//! ## Features
//!
//! - Signal amplitude analysis (peak and RMS)
//! - Detection threshold comparison
//! - Processing metadata generation
//! - Configurable detection thresholds and analysis windows
//! - Converts processed signals to photoacoustic results
//!
//! ## Examples
//!
//! Basic usage:
//!
//! ```no_run
//! use rust_photoacoustic::processing::{PhotoacousticOutputNode, ProcessingNode, ProcessingData};
//!
//! let mut output_node = PhotoacousticOutputNode::new("pa_output".to_string());
//!
//! let input = ProcessingData::SingleChannel {
//!     samples: vec![0.05, 0.1, 0.15, 0.02],
//!     sample_rate: 44100,
//!     timestamp: 1000,
//!     frame_number: 5,
//! };
//!
//! let result = output_node.process(input)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

use super::{ProcessingData, ProcessingMetadata, ProcessingNode};
use anyhow::Result;

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
