// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

pub mod peak_finder;

// Re-export for easier access
pub use peak_finder::PeakFinderNode;

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
/// - `peak_frequency`: Detected resonance frequency in Hz (legacy, use peak_results)
/// - `peak_amplitude`: Normalized amplitude of the detected peak (legacy, use peak_results)
/// - `concentration_ppm`: Calculated gas concentration in ppm (legacy, use peak_results)
/// - `polynomial_coefficients`: Coefficients for 4th-degree polynomial concentration calculation
/// - `last_update`: Timestamp of the last update for data validation
#[derive(Debug, Clone)]
pub struct ComputingSharedData {
    /// Peak detection results from multiple nodes, keyed by node ID
    pub peak_results: HashMap<String, PeakResult>,

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

    /// Get the most recent peak result across all nodes
    pub fn get_latest_peak_result(&self) -> Option<&PeakResult> {
        self.peak_results
            .values()
            .max_by_key(|result| result.timestamp)
    }

    /// Get all node IDs that have peak results
    pub fn get_peak_finder_node_ids(&self) -> Vec<String> {
        self.peak_results.keys().cloned().collect()
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
}

/// Type alias for thread-safe access to computing shared data
pub type SharedComputingState = Arc<RwLock<ComputingSharedData>>;
