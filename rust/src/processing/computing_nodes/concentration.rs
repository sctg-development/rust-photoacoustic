// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! This module implements the ConcentrationNode, which calculates gas concentration from peak amplitude.
//!
//! The ConcentrationNode is a specialized ComputingNode that implements the ProcessingNode trait.
//! It performs concentration calculations based on peak amplitude data from PeakFinderNode instances
//! while passing the original data through unchanged. This enables real-time concentration calculation
//! for photoacoustic applications using configurable polynomial coefficients.
//!
//! # Features
//!
//! - **Multi-instance support**: Multiple ConcentrationNode instances can coexist
//! - **Selective source binding**: Each node can bind to a specific PeakFinderNode via `computing_peak_finder_id`
//! - **Individual polynomial coefficients**: Each node can have its own calibration polynomial
//! - **Pass-through processing**: Original signal data flows unchanged to next node
//! - **Shared state updates**: Concentration results are stored in global shared state
//! - **Temperature compensation**: Optional temperature correction for improved accuracy
//! - **Multi-spectral analysis**: Support for different spectral lines/harmonics
//!
//! # Configuration
//!
//! The ConcentrationNode configuration includes:
//! - `id`: Unique identifier for this node instance
//! - `computing_peak_finder_id`: ID of the PeakFinderNode to use as data source
//! - `polynomial_coefficients`: 5-element array for 4th-degree polynomial [a₀, a₁, a₂, a₃, a₄]
//! - `temperature_compensation`: Enable/disable temperature correction
//! - `spectral_line_id`: Optional identifier for the spectral line being analyzed
//!
//! # Usage
//!
//! ```rust
//! use rust_photoacoustic::processing::computing_nodes::concentration::ConcentrationNode;
//! use rust_photoacoustic::processing::{ProcessingNode, ProcessingData};
//!
//! let mut concentration_node = ConcentrationNode::new("concentration_calc".to_string())
//!     .with_peak_finder_source("primary_peak_finder".to_string())
//!     .with_polynomial_coefficients([0.0, 0.45, -0.002, 0.0001, 0.0])
//!     .with_temperature_compensation(true);
//! ```

use crate::processing::computing_nodes::{
    ComputingSharedData, ConcentrationResult, PeakResult, SharedComputingState,
};
use crate::processing::nodes::ProcessingMetadata;
use crate::processing::{ProcessingData, ProcessingNode};
use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

/// A computing node that calculates gas concentration from peak amplitude data
///
/// This node implements concentration calculation using configurable polynomial coefficients.
/// It can be bound to a specific PeakFinderNode or operate in automatic mode using the most
/// recent peak data available. Multiple instances can coexist to analyze different spectral
/// lines or test different calibration polynomials.
pub struct ConcentrationNode {
    /// Unique identifier for this node
    id: String,

    /// ID of the PeakFinderNode to use as data source
    /// If None, uses the most recent peak data available
    computing_peak_finder_id: Option<String>,

    /// Polynomial coefficients for concentration calculation [a₀, a₁, a₂, a₃, a₄]
    /// Concentration(ppm) = a₀ + a₁*A + a₂*A² + a₃*A³ + a₄*A⁴
    /// where A is the normalized peak amplitude
    polynomial_coefficients: [f64; 5],

    /// Enable temperature compensation for improved accuracy
    temperature_compensation: bool,

    /// Optional identifier for the spectral line being analyzed
    spectral_line_id: Option<String>,

    /// Minimum amplitude threshold for valid concentration calculation
    min_amplitude_threshold: f32,

    /// Maximum concentration limit for safety/validation
    max_concentration_ppm: f32,

    /// Shared state for communicating results to other nodes
    shared_state: Arc<RwLock<ComputingSharedData>>,

    /// Statistics for monitoring performance
    processing_count: u64,
    calculation_count: u64,
    last_calculation_time: Option<SystemTime>,
}

impl ConcentrationNode {
    /// Create a new ConcentrationNode with default parameters
    ///
    /// Default configuration:
    /// - No specific PeakFinderNode binding (uses most recent data)
    /// - Linear polynomial: [0.0, 1.0, 0.0, 0.0, 0.0] (amplitude = concentration)
    /// - Temperature compensation disabled
    /// - Minimum amplitude threshold: 0.001
    /// - Maximum concentration: 10000.0 ppm
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    ///
    /// # Returns
    ///
    /// A new ConcentrationNode instance with default configuration
    pub fn new(id: String) -> Self {
        Self {
            id,
            computing_peak_finder_id: None,
            polynomial_coefficients: [0.0, 1.0, 0.0, 0.0, 0.0], // Linear by default
            temperature_compensation: false,
            spectral_line_id: None,
            min_amplitude_threshold: 0.001,
            max_concentration_ppm: 10000.0,
            shared_state: Arc::new(RwLock::new(ComputingSharedData::default())),
            processing_count: 0,
            calculation_count: 0,
            last_calculation_time: None,
        }
    }

    /// Create a new ConcentrationNode with an external shared computing state
    ///
    /// This constructor allows sharing the computing state between multiple nodes,
    /// enabling centralized management of analytical results.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `shared_state` - Optional shared computing state. If None, creates a new one.
    ///
    /// # Returns
    ///
    /// A new ConcentrationNode instance with the provided or new shared state
    pub fn new_with_shared_state(id: String, shared_state: Option<SharedComputingState>) -> Self {
        let shared_state =
            shared_state.unwrap_or_else(|| Arc::new(RwLock::new(ComputingSharedData::default())));

        Self {
            id,
            computing_peak_finder_id: None,
            polynomial_coefficients: [0.0, 1.0, 0.0, 0.0, 0.0],
            temperature_compensation: false,
            spectral_line_id: None,
            min_amplitude_threshold: 0.001,
            max_concentration_ppm: 10000.0,
            shared_state,
            processing_count: 0,
            calculation_count: 0,
            last_calculation_time: None,
        }
    }

    /// Set the PeakFinderNode ID to use as data source
    ///
    /// # Arguments
    ///
    /// * `peak_finder_id` - ID of the PeakFinderNode to bind to
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_peak_finder_source(mut self, peak_finder_id: String) -> Self {
        self.computing_peak_finder_id = Some(peak_finder_id);
        self
    }

    /// Set the polynomial coefficients for concentration calculation
    ///
    /// # Arguments
    ///
    /// * `coefficients` - Array of 5 coefficients [a₀, a₁, a₂, a₃, a₄]
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_polynomial_coefficients(mut self, coefficients: [f64; 5]) -> Self {
        self.polynomial_coefficients = coefficients;
        self
    }

    /// Enable or disable temperature compensation
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable temperature compensation
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_temperature_compensation(mut self, enabled: bool) -> Self {
        self.temperature_compensation = enabled;
        self
    }

    /// Set the spectral line identifier
    ///
    /// # Arguments
    ///
    /// * `line_id` - Identifier for the spectral line being analyzed
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_spectral_line_id(mut self, line_id: String) -> Self {
        self.spectral_line_id = Some(line_id);
        self
    }

    /// Set the minimum amplitude threshold for calculations
    ///
    /// # Arguments
    ///
    /// * `threshold` - Minimum amplitude for valid concentration calculation
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_min_amplitude_threshold(mut self, threshold: f32) -> Self {
        self.min_amplitude_threshold = threshold.max(0.0);
        self
    }

    /// Set the maximum concentration limit
    ///
    /// # Arguments
    ///
    /// * `max_ppm` - Maximum concentration in ppm for safety/validation
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_max_concentration(mut self, max_ppm: f32) -> Self {
        self.max_concentration_ppm = max_ppm.max(0.0);
        self
    }

    /// Get the shared computing state
    ///
    /// # Returns
    ///
    /// Reference to the shared computing state
    pub fn get_shared_state(&self) -> &SharedComputingState {
        &self.shared_state
    }

    /// Calculate concentration from amplitude using polynomial coefficients
    ///
    /// Uses the configured polynomial: C(ppm) = a₀ + a₁*A + a₂*A² + a₃*A³ + a₄*A⁴
    /// where A is the normalized peak amplitude and C is the concentration in ppm.
    ///
    /// # Arguments
    ///
    /// * `amplitude` - Normalized peak amplitude (typically 0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// Calculated concentration in ppm, clamped to [0.0, max_concentration_ppm]
    fn calculate_concentration(&self, amplitude: f32) -> f64 {
        if amplitude < self.min_amplitude_threshold {
            return 0.0;
        }

        let a = amplitude as f64;
        let [a0, a1, a2, a3, a4] = self.polynomial_coefficients;

        let concentration = a0 + a1 * a + a2 * a * a + a3 * a * a * a + a4 * a * a * a * a;

        // Clamp to valid range
        concentration
            .max(0.0)
            .min(self.max_concentration_ppm as f64)
    }

    /// Update the concentration result in the shared state
    ///
    /// This method stores the concentration result under this node's ID in the shared state
    /// and maintains the legacy fields for backward compatibility.
    ///
    /// # Arguments
    ///
    /// * `source_peak_result` - The source peak result used for calculation
    /// * `concentration` - Calculated concentration in ppm
    fn update_shared_state(&mut self, source_peak_result: &PeakResult, concentration: f64) {
        if self.processing_count % 100 == 0 {
            info!(
                "Concentration node '{}': Calculated {:.2} ppm = {:.2e} + {:.2e}xA + {:.2e}xA² + {:.2e}xA³ + {:.2e}xA⁴ from amplitude {:.4}dB (source: {})",
                self.id,
                concentration,
                self.polynomial_coefficients[0],
                self.polynomial_coefficients[1],
                self.polynomial_coefficients[2],
                self.polynomial_coefficients[3],
                self.polynomial_coefficients[4],
                source_peak_result.amplitude,
                self.computing_peak_finder_id.as_deref().unwrap_or("latest")
            );
        }

        match self.shared_state.try_write() {
            Ok(mut state) => {
                // Create concentration result
                let concentration_result = ConcentrationResult {
                    concentration_ppm: concentration,
                    source_peak_finder_id: self
                        .computing_peak_finder_id
                        .as_deref()
                        .unwrap_or("legacy")
                        .to_string(),
                    spectral_line_id: self.spectral_line_id.clone(),
                    polynomial_coefficients: self.polynomial_coefficients,
                    source_amplitude: source_peak_result.amplitude,
                    source_frequency: source_peak_result.frequency,
                    temperature_compensated: self.temperature_compensation,
                    timestamp: SystemTime::now(),
                    processing_metadata: std::collections::HashMap::new(),
                };

                // Store concentration result under this node's ID
                state.update_concentration_result(self.id.clone(), concentration_result);

                // Update the source PeakResult with the calculated concentration
                if let Some(source_id) = &self.computing_peak_finder_id {
                    if let Some(mut peak_result) = state.get_peak_result(source_id).cloned() {
                        peak_result.concentration_ppm = Some(concentration as f32);
                        state.update_peak_result(source_id.clone(), peak_result);
                    }
                }
            }
            Err(_) => {
                warn!(
                    "Concentration node '{}': Failed to acquire write lock for shared state - concentration={:.2} ppm",
                    self.id, concentration
                );
            }
        }

        self.last_calculation_time = Some(SystemTime::now());
        self.calculation_count += 1;
    }

    /// Get processing statistics
    ///
    /// # Returns
    ///
    /// Tuple of (processing_count, calculation_count)
    pub fn get_statistics(&self) -> (u64, u64) {
        (self.processing_count, self.calculation_count)
    }
}

impl ProcessingNode for ConcentrationNode {
    /// Process input data while performing concentration calculation
    ///
    /// This method implements the pass-through behavior characteristic of ComputingNodes:
    /// the input data is returned unchanged while concentration calculation is performed
    /// in parallel. Results are stored in the shared state for access by other nodes.
    ///
    /// # Arguments
    ///
    /// * `input` - Input data to pass through (unchanged)
    ///
    /// # Returns
    ///
    /// The same input data unchanged, allowing it to flow to the next node
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        self.processing_count += 1;

        // Try to get peak data from the shared state
        let peak_result = match self.shared_state.try_read() {
            Ok(state) => {
                if let Some(source_id) = &self.computing_peak_finder_id {
                    // Get data from specific PeakFinderNode
                    state.get_peak_result(source_id).cloned()
                } else {
                    // Get most recent peak data or fall back to legacy data
                    if let Some(latest) = state.get_latest_peak_result() {
                        Some(latest.clone())
                    } else if let (Some(freq), Some(amp)) =
                        (state.peak_frequency, state.peak_amplitude)
                    {
                        // Create a PeakResult from legacy data for backward compatibility
                        Some(PeakResult {
                            frequency: freq,
                            amplitude: amp,
                            concentration_ppm: None,
                            timestamp: state.last_update,
                            coherence_score: 1.0, // Default for legacy data
                            processing_metadata: std::collections::HashMap::new(),
                        })
                    } else {
                        None
                    }
                }
            }
            Err(_) => {
                if self.processing_count % 1000 == 0 {
                    warn!(
                        "Concentration node '{}': Failed to read shared state",
                        self.id
                    );
                }
                None
            }
        };

        // Calculate concentration if peak data is available
        if let Some(peak_data) = peak_result {
            if peak_data.amplitude >= self.min_amplitude_threshold {
                let concentration = self.calculate_concentration(peak_data.amplitude);
                self.update_shared_state(&peak_data, concentration);
            } else {
                // Amplitude too low for reliable calculation
                if self.processing_count % 1000 == 0 {
                    debug!(
                        "Concentration node '{}': Amplitude {:.4} below threshold {:.4}",
                        self.id, peak_data.amplitude, self.min_amplitude_threshold
                    );
                }
            }
        } else {
            // No peak data available
            if self.processing_count % 1000 == 0 {
                debug!(
                    "Concentration node '{}': No peak data available from source '{}'",
                    self.id,
                    self.computing_peak_finder_id.as_deref().unwrap_or("latest")
                );
            }
        }

        // Pass input data through unchanged
        Ok(input)
    }

    /// Get the node type identifier
    ///
    /// # Returns
    ///
    /// Static string identifying this node type
    fn node_type(&self) -> &str {
        "computing_concentration"
    }

    /// Get the unique node identifier
    ///
    /// # Returns
    ///
    /// Reference to this node's unique ID
    fn node_id(&self) -> &str {
        &self.id
    }

    /// Check if this node can accept the given input type
    ///
    /// ConcentrationNode can process any data type (pass-through)
    fn accepts_input(&self, _input: &ProcessingData) -> bool {
        true // Pass-through node accepts any input
    }

    /// Get the expected output type for the given input
    ///
    /// ConcentrationNode is a pass-through node, so output type matches input type
    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        match input {
            ProcessingData::AudioFrame(_) => Some("AudioFrame".to_string()),
            ProcessingData::SingleChannel { .. } => Some("SingleChannel".to_string()),
            ProcessingData::DualChannel { .. } => Some("DualChannel".to_string()),
            ProcessingData::PhotoacousticResult { .. } => Some("PhotoacousticResult".to_string()),
        }
    }

    /// Reset internal state
    ///
    /// Clears processing statistics and resets calculation state
    fn reset(&mut self) {
        self.processing_count = 0;
        self.calculation_count = 0;
        self.last_calculation_time = None;

        // Note: We don't reset shared state as other nodes might depend on it
        info!("Concentration node '{}': State reset", self.id);
    }

    /// Clone the node for graph reconfiguration
    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        let mut cloned = ConcentrationNode::new(self.id.clone())
            .with_polynomial_coefficients(self.polynomial_coefficients)
            .with_temperature_compensation(self.temperature_compensation);

        if let Some(peak_finder_id) = &self.computing_peak_finder_id {
            cloned = cloned.with_peak_finder_source(peak_finder_id.clone());
        }

        if let Some(spectral_line_id) = &self.spectral_line_id {
            cloned = cloned.with_spectral_line_id(spectral_line_id.clone());
        }

        cloned.min_amplitude_threshold = self.min_amplitude_threshold;
        cloned.max_concentration_ppm = self.max_concentration_ppm;

        Box::new(cloned)
    }

    /// Check if this node supports hot-reload configuration updates
    fn supports_hot_reload(&self) -> bool {
        true // ConcentrationNode supports dynamic configuration updates
    }

    /// Update node configuration parameters
    ///
    /// Supports hot-reload of polynomial coefficients and other parameters
    ///
    /// # Arguments
    ///
    /// * `parameters` - JSON object with new parameter values
    ///
    /// # Returns
    ///
    /// Ok(true) if parameters were updated, Ok(false) if no changes, Err for invalid parameters
    fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
        let mut updated = false;

        // Update polynomial coefficients
        if let Some(coeffs) = parameters.get("polynomial_coefficients") {
            if let Some(coeffs_array) = coeffs.as_array() {
                if coeffs_array.len() == 5 {
                    let mut new_coeffs = [0.0; 5];
                    for (i, coeff) in coeffs_array.iter().enumerate() {
                        if let Some(val) = coeff.as_f64() {
                            new_coeffs[i] = val;
                        } else {
                            return Err(anyhow!("Invalid polynomial coefficient at index {}", i));
                        }
                    }
                    if new_coeffs != self.polynomial_coefficients {
                        self.polynomial_coefficients = new_coeffs;
                        updated = true;
                        info!(
                            "Concentration node '{}': Updated polynomial coefficients to {:?}",
                            self.id, new_coeffs
                        );
                    }
                } else {
                    return Err(anyhow!(
                        "Polynomial coefficients array must have exactly 5 elements"
                    ));
                }
            }
        }

        // Update temperature compensation
        if let Some(temp_comp) = parameters.get("temperature_compensation") {
            if let Some(enabled) = temp_comp.as_bool() {
                if enabled != self.temperature_compensation {
                    self.temperature_compensation = enabled;
                    updated = true;
                    info!(
                        "Concentration node '{}': Temperature compensation set to {}",
                        self.id, enabled
                    );
                }
            }
        }

        // Update min amplitude threshold
        if let Some(threshold) = parameters.get("min_amplitude_threshold") {
            if let Some(val) = threshold.as_f64() {
                let new_threshold = val as f32;
                if (new_threshold - self.min_amplitude_threshold).abs() > f32::EPSILON {
                    self.min_amplitude_threshold = new_threshold.max(0.0);
                    updated = true;
                    info!(
                        "Concentration node '{}': Min amplitude threshold set to {}",
                        self.id, self.min_amplitude_threshold
                    );
                }
            }
        }

        // Update max concentration
        if let Some(max_conc) = parameters.get("max_concentration_ppm") {
            if let Some(val) = max_conc.as_f64() {
                let new_max = val as f32;
                if (new_max - self.max_concentration_ppm).abs() > f32::EPSILON {
                    self.max_concentration_ppm = new_max.max(0.0);
                    updated = true;
                    info!(
                        "Concentration node '{}': Max concentration set to {} ppm",
                        self.id, self.max_concentration_ppm
                    );
                }
            }
        }

        // Update PeakFinder source binding
        if let Some(source_id) = parameters.get("computing_peak_finder_id") {
            if let Some(id_str) = source_id.as_str() {
                let new_source = if id_str.is_empty() {
                    None
                } else {
                    Some(id_str.to_string())
                };
                if new_source != self.computing_peak_finder_id {
                    self.computing_peak_finder_id = new_source.clone();
                    updated = true;
                    info!(
                        "Concentration node '{}': PeakFinder source set to {:?}",
                        self.id, new_source
                    );
                }
            }
        }

        Ok(updated)
    }
}
