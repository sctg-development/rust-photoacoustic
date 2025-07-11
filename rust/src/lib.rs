// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Rust Photoacoustic Library
//!
//! A comprehensive library for photoacoustic analysis of water vapor and other substances.
//! This library provides tools for signal acquisition, preprocessing, spectral analysis,
//! and visualization of photoacoustic data.
//!
//! ## Main Components
//!
//! - **Acquisition**: Components for capturing audio signals from devices or files
//! - **Preprocessing**: Signal processing tools like filters and differential analysis
//! - **Spectral**: Frequency domain analysis tools, including FFT implementations
//! - **Visualization**: Server components for visualizing photoacoustic data
//! - **Utility**: Helper functions and tools for various operations
//!
//! ## Usage
//!
//! This library can be used either:
//!
//! 1. As a dependency in other Rust projects for photoacoustic analysis
//! 2. Through the provided binaries for specific tasks
//!
//! ## Binaries
//!
//! The project includes several utility binaries:
//!
//! - **main**: The primary application for photoacoustic analysis
//! - **rs256keygen**: Tool for generating RSA key pairs for JWT token signing
//! - **debug_config**: Tool for debugging configuration files
//! - **differential**: Utility for differential signal analysis
//! - **filters**: Tool for testing signal filtering operations
//! - **modbus_client**: Tool for testing the Modbus slave included in **main**

/// Module for handling audio signal acquisition from various sources.
///
/// This includes interfaces for working with microphones and file-based audio sources.
pub mod acquisition;

/// Build and version information for maintenance and debugging.
///
/// Provides access to Git commit hashes, build timestamps, and other metadata
/// useful for tracking exact versions in deployed systems.
pub mod build_info;

/// Configuration handling for the photoacoustic application.
///
/// Provides functionality for loading, validating, and managing application settings
/// including visualization server configuration and authentication keys.
pub mod config;

/// Signal preprocessing tools for photoacoustic analysis.
///
/// Contains implementations of various filters and differential analysis methods
/// used in preparing raw signals for spectral analysis.
pub mod preprocessing;

/// Processing pipeline for modular audio analysis.
///
/// Provides a graph-based processing system for real-time audio processing
/// with configurable nodes and connections.
pub mod processing;

/// Spectral analysis tools for frequency domain operations.
///
/// Provides implementations of FFT and other spectral analysis methods for
/// extracting frequency information from time-domain signals.
pub mod spectral;

/// Utility functions and helper tools.
///
/// Includes various utilities like certificate handling, noise generation,
/// and other common operations used throughout the application.
pub mod utility;

/// Visualization server and components for displaying photoacoustic data.
///
/// Implements a web server with secure authentication for presenting
/// analysis results and real-time data visualization.
pub mod visualization;

/// Daemon process for background analysis and service management.
///
/// The daemon module provides functionality for running and managing background
/// services in the photoacoustic application. This includes the web server for
/// visualization, data acquisition tasks, and system monitoring.
pub mod daemon;

/// Modbus communication for interfacing with external devices.
///
/// This module provides functionality for Modbus communication, allowing
/// interaction with external devices and systems that support the Modbus protocol.
pub mod modbus;

/// Photoacoustic computations module.
/// This module contains the core computations and algorithms used in photoacoustic analysis.
pub mod photoacoustic;

/// Thermal regulation module.
/// This module handles thermal regulation tasks, ensuring that the system operates within safe temperature limits.
pub mod thermal_regulation;

use serde::{Deserialize, Serialize};

/// Result of a photoacoustic analysis operation.
///
/// This structure holds the key measurements and calculations from a
/// photoacoustic analysis session, including frequency, amplitude,
/// calculated concentration, and the timestamp of when the analysis was performed.
///
/// ### Examples
///
/// ```no_run
/// use rust_photoacoustic::AnalysisResult;
/// use chrono::Utc;
///
/// let result = AnalysisResult {
///     frequency: 1342.5,
///     amplitude: 0.85,
///     concentration: 456.2,
///     timestamp: Utc::now(),
/// };
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// The detected resonance frequency of the photoacoustic signal in Hertz (Hz).
    /// This is typically the frequency at which the maximum amplitude was observed.
    pub frequency: f32,

    /// The amplitude value at the detected frequency, representing the strength of
    /// the photoacoustic response. Units depend on the acquisition system calibration.
    pub amplitude: f32,

    /// The calculated concentration of the target substance (e.g., water vapor) in parts
    /// per million (ppm) or other appropriate units, derived from the amplitude and
    /// calibration data.
    pub concentration: f32,

    /// The UTC timestamp when the analysis was performed, allowing for temporal tracking
    /// of measurements in long-term monitoring scenarios.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Default for AnalysisResult {
    fn default() -> Self {
        Self {
            frequency: 0.0,
            amplitude: 0.0,
            concentration: 0.0,
            timestamp: chrono::Utc::now(),
        }
    }
}
