// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

pub mod action_trait;
pub mod concentration;
pub mod display_drivers;
pub mod peak_finder;
pub mod universal_action;

/// Result data from a peak finder node
#[derive(Debug, Clone)]
pub struct PeakResult {
    /// Detected peak frequency in Hz
    pub frequency: f32,
    /// Detected peak amplitude (normalized, 0.0 to 1.0)
    pub amplitude: f32,
    /// Concentration in parts per million (ppm) derived from frequency
    pub concentration_ppm: Option<f32>,
    /// Timestamp of when this peak was detected
    pub timestamp: SystemTime,
    /// Coherence score for this detection (0.0 to 1.0)
    pub coherence_score: f32,
    /// Additional metadata for this peak detection
    pub processing_metadata: HashMap<String, String>,
}

/// Result data from a concentration calculation node
#[derive(Debug, Clone)]
pub struct ConcentrationResult {
    /// Calculated concentration in parts per million (ppm)
    pub concentration_ppm: f64,
    /// Source PeakFinderNode ID that provided the amplitude data
    pub source_peak_finder_id: String,
    /// Spectral line identifier (e.g., "CO2_line", "CH4_line")
    pub spectral_line_id: Option<String>,
    /// Polynomial coefficients used for this calculation [a₀, a₁, a₂, a₃, a₄]
    pub polynomial_coefficients: [f64; 5],
    /// Source peak amplitude used for calculation
    pub source_amplitude: f32,
    /// Source peak frequency
    pub source_frequency: f32,
    /// Whether temperature compensation was applied
    pub temperature_compensated: bool,
    /// Timestamp of when this concentration was calculated
    pub timestamp: SystemTime,
    /// Additional metadata for this concentration calculation
    pub processing_metadata: HashMap<String, String>,
}

/// Shared data structure for computing nodes
///
/// This structure holds the results of analytical computations performed by computing nodes.
/// It's designed to be shared between nodes via Arc<RwLock<ComputingSharedData>> for
/// thread-safe access in a real-time processing environment.
///
/// # Fields
///
/// - `peak_results`: HashMap of peak detection results from multiple nodes, keyed by node ID
/// - `concentration_results`: HashMap of concentration calculation results from multiple nodes, keyed by node ID
/// - `peak_frequency`: Detected resonance frequency in Hz (legacy, use peak_results)
/// - `peak_amplitude`: Normalized amplitude of the detected peak (legacy, use peak_results)
/// - `concentration_ppm`: Calculated gas concentration in ppm (legacy, use concentration_results)
/// - `polynomial_coefficients`: Coefficients for 4th-degree polynomial concentration calculation (legacy)
/// - `last_update`: Timestamp of the last update for data validation
#[derive(Debug, Clone)]
pub struct ComputingSharedData {
    /// Peak detection results from multiple nodes, keyed by node ID
    pub peak_results: HashMap<String, PeakResult>,

    /// Concentration calculation results from multiple nodes, keyed by node ID
    pub concentration_results: HashMap<String, ConcentrationResult>,

    // Legacy fields for backward compatibility
    pub peak_frequency: Option<f32>,
    pub peak_amplitude: Option<f32>,
    pub concentration_ppm: Option<f32>,
    pub polynomial_coefficients: [f64; 5], // a₀ + a₁x + a₂x² + a₃x³ + a₄x⁴
    pub last_update: SystemTime,
}

impl Default for ComputingSharedData {
    fn default() -> Self {
        Self {
            peak_results: HashMap::new(),
            concentration_results: HashMap::new(),
            peak_frequency: None,
            peak_amplitude: None,
            concentration_ppm: None,
            polynomial_coefficients: [0.0; 5],
            last_update: SystemTime::now(),
        }
    }
}

impl ComputingSharedData {
    /// Get peak result for a specific node ID
    pub fn get_peak_result(&self, node_id: &str) -> Option<&PeakResult> {
        self.peak_results.get(node_id)
    }

    /// Update peak result for a specific node ID
    pub fn update_peak_result(&mut self, node_id: String, result: PeakResult) {
        // Update the HashMap
        self.peak_results.insert(node_id.clone(), result.clone());

        // Update legacy fields for backward compatibility
        // Use the most recent result (this one) for legacy fields
        self.peak_frequency = Some(result.frequency);
        self.peak_amplitude = Some(result.amplitude);
        self.concentration_ppm = result.concentration_ppm;
        self.last_update = result.timestamp;
    }

    /// Get concentration result for a specific node ID
    pub fn get_concentration_result(&self, node_id: &str) -> Option<&ConcentrationResult> {
        self.concentration_results.get(node_id)
    }

    /// Update concentration result for a specific node ID
    pub fn update_concentration_result(&mut self, node_id: String, result: ConcentrationResult) {
        // Update the HashMap
        self.concentration_results
            .insert(node_id.clone(), result.clone());

        // Update legacy fields for backward compatibility
        // Use the most recent result (this one) for legacy fields
        self.concentration_ppm = Some(result.concentration_ppm as f32);
        self.polynomial_coefficients = result.polynomial_coefficients;
        self.last_update = result.timestamp;
    }

    /// Get the most recent peak result across all nodes
    pub fn get_latest_peak_result(&self) -> Option<&PeakResult> {
        self.peak_results
            .values()
            .max_by_key(|result| result.timestamp)
    }

    /// Get the most recent concentration result across all nodes
    pub fn get_latest_concentration_result(&self) -> Option<&ConcentrationResult> {
        self.concentration_results
            .values()
            .max_by_key(|result| result.timestamp)
    }

    /// Get all node IDs that have peak results
    pub fn get_peak_finder_node_ids(&self) -> Vec<String> {
        self.peak_results.keys().cloned().collect()
    }

    /// Get all node IDs that have concentration results
    pub fn get_concentration_node_ids(&self) -> Vec<String> {
        self.concentration_results.keys().cloned().collect()
    }

    /// Check if a node has recent peak data (within last 30 seconds)
    pub fn has_recent_peak_data(&self, node_id: &str) -> bool {
        if let Some(result) = self.peak_results.get(node_id) {
            if let Ok(elapsed) = result.timestamp.elapsed() {
                elapsed.as_secs() < 30
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Check if a node has recent concentration data (within last 30 seconds)
    pub fn has_recent_concentration_data(&self, node_id: &str) -> bool {
        if let Some(result) = self.concentration_results.get(node_id) {
            if let Ok(elapsed) = result.timestamp.elapsed() {
                elapsed.as_secs() < 30
            } else {
                false
            }
        } else {
            false
        }
    }
}

/// Type alias for thread-safe access to computing shared data
pub type SharedComputingState = Arc<RwLock<ComputingSharedData>>;

// Re-export for easier access
pub use action_trait::{
    ActionHistoryEntry, ActionNode, ActionNodeHelper, ActionTrigger, CircularBuffer,
};
pub use concentration::ConcentrationNode;
pub use peak_finder::PeakFinderNode;
pub use universal_action::UniversalActionNode;
