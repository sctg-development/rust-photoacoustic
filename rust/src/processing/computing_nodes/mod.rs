// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

pub mod peak_finder;

// Re-export for easier access
pub use peak_finder::PeakFinderNode;

/// Shared data structure for computing nodes
///
/// This structure holds the results of analytical computations performed by computing nodes.
/// It's designed to be shared between nodes via Arc<RwLock<ComputingSharedData>> for
/// thread-safe access in a real-time processing environment.
///
/// # Fields
///
/// - `peak_frequency`: Detected resonance frequency in Hz
/// - `peak_amplitude`: Normalized amplitude of the detected peak (0.0 to 1.0)
/// - `concentration_ppm`: Calculated gas concentration in parts per million
/// - `polynomial_coefficients`: Coefficients for 4th-degree polynomial concentration calculation
/// - `last_update`: Timestamp of the last update for data validation
#[derive(Debug, Clone)]
pub struct ComputingSharedData {
    pub peak_frequency: Option<f32>,
    pub peak_amplitude: Option<f32>,
    pub concentration_ppm: Option<f32>,
    pub polynomial_coefficients: [f64; 5], // a₀ + a₁x + a₂x² + a₃x³ + a₄x⁴
    pub last_update: SystemTime,
}

impl Default for ComputingSharedData {
    fn default() -> Self {
        Self {
            peak_frequency: None,
            peak_amplitude: None,
            concentration_ppm: None,
            polynomial_coefficients: [0.0; 5],
            last_update: SystemTime::now(),
        }
    }
}

/// Type alias for thread-safe access to computing shared data
pub type SharedComputingState = Arc<RwLock<ComputingSharedData>>;
