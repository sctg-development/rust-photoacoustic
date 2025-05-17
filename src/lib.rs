// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
//! Rust Photoacoustic library
//!
//! This library provides functionality for photoacoustic analysis of water vapor.

pub mod acquisition;
pub mod preprocessing;
pub mod spectral;
pub mod utility;
pub mod visualization;
pub mod config;

use serde::{Deserialize, Serialize};

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
