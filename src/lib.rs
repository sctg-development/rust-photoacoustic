//! Rust Photoacoustic library
//! 
//! This library provides functionality for photoacoustic analysis of water vapor.

pub mod acquisition;
pub mod preprocessing;
pub mod spectral;
pub mod utility;
pub mod visualization;

use chrono;
use serde::{Serialize, Deserialize};

/// Result of a photoacoustic analysis
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Detected frequency in Hz
    pub frequency: f32,
    /// Amplitude at the detected frequency
    pub amplitude: f32,
    /// Calculated concentration
    pub concentration: f32,
    /// Timestamp of the analysis
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
