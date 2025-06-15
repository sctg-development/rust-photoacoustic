// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Gain processing node implementation
//!
//! This module provides the `GainNode` which applies amplification or attenuation
//! to audio signals. The gain is specified in decibels (dB) and can be applied
//! to both single and dual-channel audio data.

use super::data::ProcessingData;
use super::traits::ProcessingNode;
use anyhow::Result;
use log::debug;

/// A processing node that applies gain (amplification/attenuation) to audio signals.
///
/// The `GainNode` multiplies audio samples by a linear gain factor derived from
/// a decibel value. It supports both single and dual-channel audio processing,
/// preserving the input data format while applying the gain transformation.
///
/// ### Gain Calculation
///
/// The linear gain factor is calculated from the decibel value using:
/// ```text
/// linear_gain = 10^(gain_db / 20)
/// ```
///
/// ### Examples
///
/// ```no_run
/// use rust_photoacoustic::processing::nodes::{GainNode, ProcessingNode, ProcessingData};
///
/// // Create a gain node with +6 dB amplification
/// let mut gain_node = GainNode::new("amplifier".to_string(), 6.0);
///
/// let input = ProcessingData::SingleChannel {
///     samples: vec![0.1, 0.2, -0.1, -0.2],
///     sample_rate: 44100,
///     timestamp: 1000,
///     frame_number: 1,
/// };
///
/// let result = gain_node.process(input)?;
/// match result {
///     ProcessingData::SingleChannel { samples, .. } => {
///         // Samples are amplified by ~2x (6 dB ≈ 1.995x)
///         assert!(samples[0] > 0.15 && samples[0] < 0.25);
///     }
///     _ => panic!("Expected SingleChannel output"),
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// Dual-channel processing:
///
/// ```no_run
/// use rust_photoacoustic::processing::nodes::{GainNode, ProcessingNode, ProcessingData};
///
/// // Create a gain node with -3 dB attenuation
/// let mut gain_node = GainNode::new("attenuator".to_string(), -3.0);
///
/// let input = ProcessingData::DualChannel {
///     channel_a: vec![1.0, 0.5],
///     channel_b: vec![-1.0, -0.5],
///     sample_rate: 44100,
///     timestamp: 1000,
///     frame_number: 1,
/// };
///
/// let result = gain_node.process(input)?;
/// match result {
///     ProcessingData::DualChannel { channel_a, channel_b, .. } => {
///         // Both channels are attenuated by ~0.707x (-3 dB ≈ 0.708x)
///         assert!(channel_a[0] > 0.7 && channel_a[0] < 0.71);
///         assert!(channel_b[0] < -0.7 && channel_b[0] > -0.71);
///     }
///     _ => panic!("Expected DualChannel output"),
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// Zero gain (unity):
///
/// ```no_run
/// use rust_photoacoustic::processing::nodes::{GainNode, ProcessingNode};
///
/// // Create a unity gain node (0 dB = no change)
/// let gain_node = GainNode::new("unity".to_string(), 0.0);
/// assert_eq!(gain_node.node_type(), "gain");
/// assert_eq!(gain_node.get_gain_db(), 0.0);
/// ```
#[derive(Debug, Clone)]
pub struct GainNode {
    /// Unique identifier for this node
    id: String,
    /// Gain value in decibels
    gain_db: f32,
    /// Cached linear gain factor
    linear_gain: f32,
}

impl GainNode {
    /// Create a new gain node with the specified gain in decibels.
    ///
    /// ### Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `gain_db` - Gain value in decibels (positive = amplification, negative = attenuation)
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::GainNode;
    ///
    /// // +12 dB amplification (4x linear gain)
    /// let amplifier = GainNode::new("amp".to_string(), 12.0);
    ///
    /// // -6 dB attenuation (~0.5x linear gain)
    /// let attenuator = GainNode::new("att".to_string(), -6.0);
    ///
    /// // Unity gain (no change)
    /// let unity = GainNode::new("unity".to_string(), 0.0);
    /// ```
    pub fn new(id: String, gain_db: f32) -> Self {
        let linear_gain = Self::db_to_linear(gain_db);
        Self {
            id,
            gain_db,
            linear_gain,
        }
    }

    /// Set the gain value in decibels.
    ///
    /// This method updates both the dB value and the cached linear gain factor.
    ///
    /// ### Arguments
    ///
    /// * `gain_db` - New gain value in decibels
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::GainNode;
    ///
    /// let mut gain_node = GainNode::new("variable".to_string(), 0.0);
    /// gain_node.set_gain_db(6.0); // Change to +6 dB
    /// assert_eq!(gain_node.get_gain_db(), 6.0);
    /// ```
    pub fn set_gain_db(&mut self, gain_db: f32) {
        self.gain_db = gain_db;
        self.linear_gain = Self::db_to_linear(gain_db);
    }

    /// Get the current gain value in decibels.
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::GainNode;
    ///
    /// let gain_node = GainNode::new("test".to_string(), -3.0);
    /// assert_eq!(gain_node.get_gain_db(), -3.0);
    /// ```
    pub fn get_gain_db(&self) -> f32 {
        self.gain_db
    }

    /// Get the current linear gain factor.
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::GainNode;
    ///
    /// let gain_node = GainNode::new("test".to_string(), 20.0); // +20 dB = 10x
    /// let linear = gain_node.get_linear_gain();
    /// assert!((linear - 10.0).abs() < 0.01);
    /// ```
    pub fn get_linear_gain(&self) -> f32 {
        self.linear_gain
    }

    /// Convert decibels to linear gain factor.
    ///
    /// Uses the formula: linear_gain = 10^(gain_db / 20)
    ///
    /// ### Arguments
    ///
    /// * `gain_db` - Gain in decibels
    ///
    /// ### Returns
    ///
    /// Linear gain factor
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::GainNode;
    ///
    /// // Test known conversions
    /// assert!((GainNode::db_to_linear(0.0) - 1.0).abs() < 0.001);   // 0 dB = 1x
    /// assert!((GainNode::db_to_linear(20.0) - 10.0).abs() < 0.001); // 20 dB = 10x
    /// assert!((GainNode::db_to_linear(-20.0) - 0.1).abs() < 0.001); // -20 dB = 0.1x
    /// ```
    pub fn db_to_linear(gain_db: f32) -> f32 {
        10.0_f32.powf(gain_db / 20.0)
    }

    /// Convert linear gain factor to decibels.
    ///
    /// Uses the formula: gain_db = 20 * log10(linear_gain)
    ///
    /// ### Arguments
    ///
    /// * `linear_gain` - Linear gain factor (must be positive)
    ///
    /// ### Returns
    ///
    /// Gain in decibels, or negative infinity for zero/negative input
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::GainNode;
    ///
    /// // Test known conversions
    /// assert!((GainNode::linear_to_db(1.0) - 0.0).abs() < 0.001);   // 1x = 0 dB
    /// assert!((GainNode::linear_to_db(10.0) - 20.0).abs() < 0.001); // 10x = 20 dB
    /// assert!((GainNode::linear_to_db(0.1) - (-20.0)).abs() < 0.001); // 0.1x = -20 dB
    /// ```
    pub fn linear_to_db(linear_gain: f32) -> f32 {
        if linear_gain <= 0.0 {
            f32::NEG_INFINITY
        } else {
            20.0 * linear_gain.log10()
        }
    }

    /// Apply gain to a vector of samples.
    ///
    /// ### Arguments
    ///
    /// * `samples` - Audio samples to process
    ///
    /// ### Returns
    ///
    /// Vector of samples with gain applied
    fn apply_gain(&self, samples: &[f32]) -> Vec<f32> {
        samples
            .iter()
            .map(|&sample| sample * self.linear_gain)
            .collect()
    }
}

impl ProcessingNode for GainNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        match input {
            ProcessingData::SingleChannel {
                samples,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                let processed_samples = self.apply_gain(&samples);
                Ok(ProcessingData::SingleChannel {
                    samples: processed_samples,
                    sample_rate,
                    timestamp,
                    frame_number,
                })
            }
            ProcessingData::DualChannel {
                channel_a,
                channel_b,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                let processed_channel_a = self.apply_gain(&channel_a);
                let processed_channel_b = self.apply_gain(&channel_b);
                Ok(ProcessingData::DualChannel {
                    channel_a: processed_channel_a,
                    channel_b: processed_channel_b,
                    sample_rate,
                    timestamp,
                    frame_number,
                })
            }
            ProcessingData::AudioFrame(frame) => {
                let processed_channel_a = self.apply_gain(&frame.channel_a);
                let processed_channel_b = self.apply_gain(&frame.channel_b);
                let mut processed_frame = frame;
                processed_frame.channel_a = processed_channel_a;
                processed_frame.channel_b = processed_channel_b;
                Ok(ProcessingData::AudioFrame(processed_frame))
            }
            ProcessingData::PhotoacousticResult { .. } => {
                anyhow::bail!("GainNode cannot process PhotoacousticResult data")
            }
        }
    }

    fn node_id(&self) -> &str {
        &self.id
    }

    fn node_type(&self) -> &str {
        "gain"
    }

    fn accepts_input(&self, input: &ProcessingData) -> bool {
        matches!(
            input,
            ProcessingData::SingleChannel { .. }
                | ProcessingData::DualChannel { .. }
                | ProcessingData::AudioFrame(_)
        )
    }

    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        match input {
            ProcessingData::SingleChannel { .. } => Some("SingleChannel".to_string()),
            ProcessingData::DualChannel { .. } => Some("DualChannel".to_string()),
            ProcessingData::AudioFrame(_) => Some("AudioFrame".to_string()),
            ProcessingData::PhotoacousticResult { .. } => None,
        }
    }

    fn reset(&mut self) {
        // No internal state to reset for gain processing
    }

    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        Box::new(self.clone())
    }

    fn supports_hot_reload(&self) -> bool {
        true // GainNode supports hot-reload for gain_db parameter
    }

    fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
        use serde_json::Value;

        // Parse the parameters and update compatible ones
        if let Value::Object(params) = parameters {
            let mut updated = false;

            // Check for gain_db parameter (hot-reloadable)
            if let Some(gain_value) = params.get("gain_db") {
                match gain_value {
                    Value::Number(num) => {
                        if let Some(gain_db) = num.as_f64() {
                            debug!(
                                "GainNode '{}': Updating gain_db from {:.2} to {:.2} dB",
                                self.id, self.gain_db, gain_db
                            );
                            self.set_gain_db(gain_db as f32);
                            updated = true;
                        } else {
                            anyhow::bail!("gain_db parameter must be a valid number");
                        }
                    }
                    _ => anyhow::bail!("gain_db parameter must be a number"),
                }
            }

            // Check for any non-hot-reloadable parameters
            // For GainNode, all current parameters are hot-reloadable

            if updated {
                debug!(
                    "GainNode '{}': Configuration updated successfully (hot-reload)",
                    self.id
                );
                Ok(true) // Hot-reload successful
            } else {
                debug!(
                    "GainNode '{}': No compatible parameters found for update",
                    self.id
                );
                Ok(false) // No relevant parameters found, but not an error
            }
        } else {
            anyhow::bail!("Parameters must be a JSON object");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::AudioFrame;

    #[test]
    fn test_db_to_linear_conversion() {
        // Test known conversions
        assert!((GainNode::db_to_linear(0.0) - 1.0).abs() < 0.001); // 0 dB = 1x
        assert!((GainNode::db_to_linear(20.0) - 10.0).abs() < 0.001); // 20 dB = 10x
        assert!((GainNode::db_to_linear(-20.0) - 0.1).abs() < 0.001); // -20 dB = 0.1x
        assert!((GainNode::db_to_linear(6.0) - 1.995).abs() < 0.01); // 6 dB ≈ 2x
        assert!((GainNode::db_to_linear(-6.0) - 0.501).abs() < 0.01); // -6 dB ≈ 0.5x
    }

    #[test]
    fn test_linear_to_db_conversion() {
        // Test known conversions
        assert!((GainNode::linear_to_db(1.0) - 0.0).abs() < 0.001); // 1x = 0 dB
        assert!((GainNode::linear_to_db(10.0) - 20.0).abs() < 0.001); // 10x = 20 dB
        assert!((GainNode::linear_to_db(0.1) - (-20.0)).abs() < 0.001); // 0.1x = -20 dB
        assert!((GainNode::linear_to_db(2.0) - 6.02).abs() < 0.1); // 2x ≈ 6 dB
        assert!((GainNode::linear_to_db(0.5) - (-6.02)).abs() < 0.1); // 0.5x ≈ -6 dB

        // Test edge cases
        assert!(GainNode::linear_to_db(0.0).is_infinite());
        assert!(GainNode::linear_to_db(-1.0).is_infinite());
    }

    #[test]
    fn test_roundtrip_conversion() {
        let test_values = vec![0.0, 6.0, -6.0, 12.0, -12.0, 20.0, -20.0];
        for db_value in test_values {
            let linear = GainNode::db_to_linear(db_value);
            let back_to_db = GainNode::linear_to_db(linear);
            assert!(
                (db_value - back_to_db).abs() < 0.001,
                "Roundtrip failed for {} dB",
                db_value
            );
        }
    }

    #[test]
    fn test_gain_node_creation() {
        let gain_node = GainNode::new("test".to_string(), 6.0);
        assert_eq!(gain_node.node_id(), "test");
        assert_eq!(gain_node.node_type(), "gain");
        assert_eq!(gain_node.get_gain_db(), 6.0);
        assert!((gain_node.get_linear_gain() - 1.995).abs() < 0.01);
    }

    #[test]
    fn test_set_gain_db() {
        let mut gain_node = GainNode::new("test".to_string(), 0.0);
        assert_eq!(gain_node.get_gain_db(), 0.0);
        assert!((gain_node.get_linear_gain() - 1.0).abs() < 0.001);

        gain_node.set_gain_db(6.0);
        assert_eq!(gain_node.get_gain_db(), 6.0);
        assert!((gain_node.get_linear_gain() - 1.995).abs() < 0.01);
    }

    #[test]
    fn test_process_single_channel() {
        let mut gain_node = GainNode::new("test".to_string(), 6.0); // ~2x gain

        let input = ProcessingData::SingleChannel {
            samples: vec![0.1, 0.2, -0.1, -0.2],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let result = gain_node.process(input).unwrap();
        match result {
            ProcessingData::SingleChannel {
                samples,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                assert_eq!(sample_rate, 44100);
                assert_eq!(timestamp, 1000);
                assert_eq!(frame_number, 1);
                assert_eq!(samples.len(), 4);

                // Check that gain was applied (~2x amplification)
                assert!((samples[0] - 0.2).abs() < 0.01); // 0.1 * ~2
                assert!((samples[1] - 0.4).abs() < 0.01); // 0.2 * ~2
                assert!((samples[2] - (-0.2)).abs() < 0.01); // -0.1 * ~2
                assert!((samples[3] - (-0.4)).abs() < 0.01); // -0.2 * ~2
            }
            _ => panic!("Expected SingleChannel output"),
        }
    }

    #[test]
    fn test_process_dual_channel() {
        let mut gain_node = GainNode::new("test".to_string(), -6.0); // ~0.5x gain

        let input = ProcessingData::DualChannel {
            channel_a: vec![1.0, 0.5],
            channel_b: vec![-1.0, -0.5],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let result = gain_node.process(input).unwrap();
        match result {
            ProcessingData::DualChannel {
                channel_a,
                channel_b,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                assert_eq!(sample_rate, 44100);
                assert_eq!(timestamp, 1000);
                assert_eq!(frame_number, 1);

                // Check that gain was applied (~0.5x attenuation)
                assert!((channel_a[0] - 0.5).abs() < 0.01); // 1.0 * ~0.5
                assert!((channel_a[1] - 0.25).abs() < 0.01); // 0.5 * ~0.5
                assert!((channel_b[0] - (-0.5)).abs() < 0.01); // -1.0 * ~0.5
                assert!((channel_b[1] - (-0.25)).abs() < 0.01); // -0.5 * ~0.5
            }
            _ => panic!("Expected DualChannel output"),
        }
    }

    #[test]
    fn test_process_audio_frame() {
        let mut gain_node = GainNode::new("test".to_string(), 0.0); // Unity gain

        let frame = AudioFrame {
            channel_a: vec![0.1, 0.2],
            channel_b: vec![0.3, 0.4],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let input = ProcessingData::AudioFrame(frame);
        let result = gain_node.process(input).unwrap();

        match result {
            ProcessingData::AudioFrame(processed_frame) => {
                // Unity gain should not change values
                assert_eq!(processed_frame.channel_a, vec![0.1, 0.2]);
                assert_eq!(processed_frame.channel_b, vec![0.3, 0.4]);
                assert_eq!(processed_frame.sample_rate, 44100);
                assert_eq!(processed_frame.timestamp, 1000);
                assert_eq!(processed_frame.frame_number, 1);
            }
            _ => panic!("Expected AudioFrame output"),
        }
    }

    #[test]
    fn test_process_photoacoustic_result_fails() {
        let mut gain_node = GainNode::new("test".to_string(), 0.0);

        let input = ProcessingData::PhotoacousticResult {
            signal: vec![1.0, 2.0],
            metadata: crate::processing::nodes::ProcessingMetadata {
                original_frame_number: 1,
                original_timestamp: 1000,
                sample_rate: 44100,
                processing_steps: vec!["test".to_string()],
                processing_latency_us: 100,
            },
        };

        let result = gain_node.process(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_accepts_input() {
        let gain_node = GainNode::new("test".to_string(), 0.0);

        let single_channel = ProcessingData::SingleChannel {
            samples: vec![1.0, 2.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };
        assert!(gain_node.accepts_input(&single_channel));

        let dual_channel = ProcessingData::DualChannel {
            channel_a: vec![1.0, 2.0],
            channel_b: vec![3.0, 4.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };
        assert!(gain_node.accepts_input(&dual_channel));

        let audio_frame = ProcessingData::AudioFrame(AudioFrame {
            channel_a: vec![1.0, 2.0],
            channel_b: vec![3.0, 4.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        });
        assert!(gain_node.accepts_input(&audio_frame));

        let photoacoustic = ProcessingData::PhotoacousticResult {
            signal: vec![1.0, 2.0],
            metadata: crate::processing::nodes::ProcessingMetadata {
                original_frame_number: 1,
                original_timestamp: 1000,
                sample_rate: 44100,
                processing_steps: vec!["test".to_string()],
                processing_latency_us: 100,
            },
        };
        assert!(!gain_node.accepts_input(&photoacoustic));
    }

    #[test]
    fn test_output_type() {
        let gain_node = GainNode::new("test".to_string(), 0.0);

        let single_channel = ProcessingData::SingleChannel {
            samples: vec![1.0, 2.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };
        assert_eq!(
            gain_node.output_type(&single_channel),
            Some("SingleChannel".to_string())
        );

        let dual_channel = ProcessingData::DualChannel {
            channel_a: vec![1.0, 2.0],
            channel_b: vec![3.0, 4.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };
        assert_eq!(
            gain_node.output_type(&dual_channel),
            Some("DualChannel".to_string())
        );

        let audio_frame = ProcessingData::AudioFrame(AudioFrame {
            channel_a: vec![1.0, 2.0],
            channel_b: vec![3.0, 4.0],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        });
        assert_eq!(
            gain_node.output_type(&audio_frame),
            Some("AudioFrame".to_string())
        );
    }

    #[test]
    fn test_reset() {
        let mut gain_node = GainNode::new("test".to_string(), 6.0);
        gain_node.reset(); // Should not change anything
        assert_eq!(gain_node.get_gain_db(), 6.0);
    }

    #[test]
    fn test_clone_node() {
        let gain_node = GainNode::new("test".to_string(), 6.0);
        let cloned = gain_node.clone_node();

        assert_eq!(cloned.node_id(), "test");
        assert_eq!(cloned.node_type(), "gain");
    }

    #[test]
    fn test_extreme_gain_values() {
        // Test very high gain
        let high_gain = GainNode::new("high".to_string(), 60.0); // 1000x
        assert!((high_gain.get_linear_gain() - 1000.0).abs() < 1.0);

        // Test very low gain
        let low_gain = GainNode::new("low".to_string(), -60.0); // 0.001x
        assert!((low_gain.get_linear_gain() - 0.001).abs() < 0.0001);

        // Test zero dB
        let unity_gain = GainNode::new("unity".to_string(), 0.0);
        assert!((unity_gain.get_linear_gain() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_dynamic_config_update() {
        let mut gain_node = GainNode::new("dynamic".to_string(), 0.0);
        assert_eq!(gain_node.get_gain_db(), 0.0);

        // Test successful hot-reload of gain_db
        let config = serde_json::json!({
            "gain_db": 6.0
        });

        let result = gain_node.update_config(&config);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should return true for successful hot-reload
        assert_eq!(gain_node.get_gain_db(), 6.0);
        assert!((gain_node.get_linear_gain() - 1.995).abs() < 0.01);

        // Test update with negative gain
        let config = serde_json::json!({
            "gain_db": -12.0
        });

        let result = gain_node.update_config(&config);
        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(gain_node.get_gain_db(), -12.0);
        assert!((gain_node.get_linear_gain() - 0.251).abs() < 0.01);

        // Test update with no relevant parameters
        let config = serde_json::json!({
            "irrelevant_param": "value"
        });

        let result = gain_node.update_config(&config);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false (no relevant params)
        assert_eq!(gain_node.get_gain_db(), -12.0); // Should remain unchanged
    }

    #[test]
    fn test_dynamic_config_update_invalid() {
        let mut gain_node = GainNode::new("test".to_string(), 0.0);

        // Test invalid parameter type
        let config = serde_json::json!({
            "gain_db": "not_a_number"
        });

        let result = gain_node.update_config(&config);
        assert!(result.is_err());
        assert_eq!(gain_node.get_gain_db(), 0.0); // Should remain unchanged

        // Test invalid JSON structure
        let config = serde_json::json!("not_an_object");

        let result = gain_node.update_config(&config);
        assert!(result.is_err());
        assert_eq!(gain_node.get_gain_db(), 0.0); // Should remain unchanged
    }

    #[test]
    fn test_gain_node_integration_with_graph() {
        use crate::processing::ProcessingGraph;

        let mut graph = ProcessingGraph::new();
        let gain_node = Box::new(GainNode::new("test_gain".to_string(), 0.0));

        // Add node to graph and test configuration update
        graph.add_node(gain_node).unwrap();

        // Test hot-reload configuration update
        let config = serde_json::json!({
            "gain_db": 12.0
        });

        let result = graph.update_node_config("test_gain", &config);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should return true for successful hot-reload
    }
}
