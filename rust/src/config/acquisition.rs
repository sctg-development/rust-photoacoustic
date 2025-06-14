// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Data acquisition configuration
//!
//! This module defines the structures for configuring the data acquisition
//! process in the photoacoustic application.

use rocket_okapi::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for the data acquisition process.
///
/// This structure contains settings that control how data is acquired
/// from the photoacoustic sensor, including timing parameters and
/// whether the acquisition system is enabled.
#[derive(Debug, Deserialize, Serialize, Clone, JsonSchema)]
pub struct AcquisitionConfig {
    /// Flag to enable or disable data acquisition.
    ///
    /// When enabled, the system will acquire data from the configured source
    /// at the specified interval. When disabled, no data is acquired.
    pub enabled: bool,

    /// Time interval in milliseconds between acquisitions.
    ///
    /// This parameter controls how frequently the system will acquire new data.
    /// Lower values provide more frequent updates but may increase system load.
    /// Must be greater than zero.
    pub interval_ms: u64,
}

// implement Default for AcquisitionConfig
impl Default for AcquisitionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_ms: 1000, // Default to 1 second (1000ms) between acquisitions
        }
    }
}
