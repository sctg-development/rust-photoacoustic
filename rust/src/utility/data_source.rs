// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Data source for photoacoustic measurements
//!
//! This module provides a central data repository that can be shared between
//! different components of the application, such as the web API, visualization,
//! and Modbus server.

use std::sync::{Arc, Mutex};

/// Represents a single photoacoustic measurement
///
/// This struct contains the key measurement data that is produced by the
/// photoacoustic analysis and is useful for external clients.
#[derive(Debug, Clone, Copy)]
pub struct PhotoacousticMeasurement {
    /// Resonance frequency in Hz
    pub frequency: f32,

    /// Signal amplitude (dimensionless)
    pub amplitude: f32,

    /// Water vapor concentration in ppm
    pub concentration: f32,

    /// Timestamp of the measurement (UNIX timestamp in seconds)
    pub timestamp: u64,
}

impl Default for PhotoacousticMeasurement {
    fn default() -> Self {
        Self {
            frequency: 0.0,
            amplitude: 0.0,
            concentration: 0.0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

/// A thread-safe repository of photoacoustic measurement data
///
/// This struct provides methods to store and retrieve the latest measurement data,
/// which can be shared between different components of the application.
#[derive(Clone)]
pub struct PhotoacousticDataSource {
    /// The latest measurement data
    latest_data: Arc<Mutex<Option<PhotoacousticMeasurement>>>,
}

impl PhotoacousticDataSource {
    /// Create a new, empty data source
    pub fn new() -> Self {
        Self {
            latest_data: Arc::new(Mutex::new(None)),
        }
    }

    /// Update the data source with new measurement data
    ///
    /// ### Parameters
    ///
    /// * `frequency` - The resonance frequency in Hz
    /// * `amplitude` - The signal amplitude (dimensionless)
    /// * `concentration` - The water vapor concentration in ppm
    pub fn update_data(&self, frequency: f32, amplitude: f32, concentration: f32) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let measurement = PhotoacousticMeasurement {
            frequency,
            amplitude,
            concentration,
            timestamp,
        };

        let mut latest = self.latest_data.lock().unwrap();
        *latest = Some(measurement);
    }

    /// Get the latest measurement data, if available
    ///
    /// ### Returns
    ///
    /// * `Some(PhotoacousticMeasurement)` if data is available
    /// * `None` if no data has been collected yet
    pub fn get_latest_data(&self) -> Option<PhotoacousticMeasurement> {
        *self.latest_data.lock().unwrap()
    }
}

impl Default for PhotoacousticDataSource {
    fn default() -> Self {
        Self::new()
    }
}
