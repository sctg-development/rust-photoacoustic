// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Action drivers for UniversalActionNode
//!
//! This module provides a pluggable driver architecture for action outputs in the
//! UniversalActionNode. Drivers abstract different action technologies and
//! communication protocols, allowing the same ActionNode to output to various endpoints.
//!
//! # Architecture
//!
//! ```text
//! UniversalActionNode
//!           ↓
//!    ActionDriver trait
//!           ↓
//! ┌─────────────┬─────────────┬─────────────┬─────────────┬─────────────┐
//! │   HTTPS     │    Redis    │    Kafka    │   Python    │  Physical   │
//! │  Callback   │   Driver    │   Driver    │   Driver    │   Drivers   │
//! │   Driver    │             │             │             │             │
//! └─────────────┴─────────────┴─────────────┴─────────────┴─────────────┘
//! ```

// Core modules containing driver implementations
mod http;
mod kafka;
mod redis;
// Python driver (feature-gated)
#[cfg(feature = "python-driver")]
mod python;

// Re-export driver implementations
pub use self::http::HttpsCallbackActionDriver;
pub use self::kafka::KafkaActionDriver;
pub use self::redis::{RedisActionDriver, RedisDriverMode};

#[cfg(feature = "python-driver")]
pub use self::python::{PythonActionDriver, PythonDriverConfig};

use anyhow::Result;
use async_trait::async_trait;
use rocket_okapi::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::SystemTime;

/// Core action data passed to drivers
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MeasurementData {
    /// Current concentration value in ppm
    pub concentration_ppm: f64,
    /// Source node ID that generated this data
    pub source_node_id: String,
    /// Peak amplitude value (0.0-1.0)
    pub peak_amplitude: f32,
    /// Peak frequency in Hz
    pub peak_frequency: f32,
    /// Timestamp of the measurement
    pub timestamp: SystemTime,
    /// Additional metadata for the action
    pub metadata: HashMap<String, Value>,
}

/// Alert/alarm data for special action states
#[derive(Debug, Clone, serde::Serialize)]
pub struct AlertData {
    /// Type of alert (concentration, amplitude, timeout, etc.)
    pub alert_type: String,
    /// Alert severity (info, warning, critical)
    pub severity: String,
    /// Human-readable alert message
    pub message: String,
    /// Alert-specific data
    pub data: HashMap<String, Value>,
    /// Timestamp when alert was triggered
    pub timestamp: SystemTime,
}

/// Trait for all action drivers
///
/// This trait abstracts different action technologies and communication protocols.
/// Each driver implements the specific logic for updating its action type while
/// providing a common interface to the UniversalActionNode.
#[async_trait]
pub trait ActionDriver: Send + Sync + std::fmt::Debug {
    /// Initialize the driver and establish connection
    ///
    /// This method is called when the driver is first configured.
    /// Use it to establish network connections, initialize hardware, etc.
    ///
    /// # Returns
    /// * `Ok(())` - Driver initialized successfully
    /// * `Err(anyhow::Error)` - Initialization failed
    async fn initialize(&mut self) -> Result<()>;

    /// Update action with current concentration data
    ///
    /// This is the primary method called when new concentration data is available.
    /// The driver should format and action the data according to its capabilities.
    ///
    /// # Arguments
    /// * `data` - Current action data with concentration, amplitude, etc.
    ///
    /// # Returns
    /// * `Ok(())` - Display updated successfully
    /// * `Err(anyhow::Error)` - Display update failed
    async fn update_action(&mut self, data: &MeasurementData) -> Result<()>;

    /// Flash/alert action for alarm conditions
    ///
    /// Called when threshold conditions are met and an alert needs to be displayed.
    /// The driver should implement appropriate visual/audio alerts for its action type.
    ///
    /// # Arguments
    /// * `alert` - Alert data with type, severity, and message
    ///
    /// # Returns
    /// * `Ok(())` - Alert actioned successfully
    /// * `Err(anyhow::Error)` - Alert action failed
    async fn show_alert(&mut self, alert: &AlertData) -> Result<()>;

    /// Clear action and return to idle state
    ///
    /// Called when the system is shutting down or resetting.
    /// The driver should clear any active actions and return to a safe state.
    ///
    /// # Returns
    /// * `Ok(())` - Display cleared successfully
    /// * `Err(anyhow::Error)` - Clear operation failed
    async fn clear_action(&mut self) -> Result<()>;

    /// Get driver status and health information
    ///
    /// Returns diagnostic information about the driver's current state.
    /// Used for monitoring and debugging.
    ///
    /// # Returns
    /// * `Ok(Value)` - Status information as JSON
    /// * `Err(anyhow::Error)` - Failed to get status
    async fn get_status(&self) -> Result<Value>;

    /// Get driver type identifier
    ///
    /// Returns a string identifying the driver type for logging and debugging.
    ///
    /// # Returns
    /// Driver type string (e.g., "https_callback", "redis", "kafka")
    fn driver_type(&self) -> &str;

    /// Check if driver supports real-time updates
    ///
    /// Some drivers (like physical actions) support real-time updates,
    /// while others (like batch data export) may only support periodic updates.
    ///
    /// # Returns
    /// * `true` - Driver supports real-time updates
    /// * `false` - Driver only supports periodic/batch updates
    fn supports_realtime(&self) -> bool {
        true // Most drivers support real-time by default
    }

    /// Shutdown the driver gracefully
    ///
    /// Called when the ActionNode is being destroyed or reconfigured.
    /// The driver should clean up resources, close connections, etc.
    ///
    /// # Returns
    /// * `Ok(())` - Driver shutdown successfully
    /// * `Err(anyhow::Error)` - Shutdown failed
    async fn shutdown(&mut self) -> Result<()> {
        // Default implementation does nothing
        Ok(())
    }

    /// Get recent history entries from the driver's buffer
    ///
    /// This method allows external systems (like REST APIs) to retrieve
    /// historical measurement data stored by the ActionNode. Each driver
    /// can implement its own history retention policy.
    ///
    /// # Arguments
    /// * `limit` - Maximum number of entries to return (None = all entries)
    ///
    /// # Returns
    /// * `Ok(Vec<MeasurementData>)` - Vector of historical data, newest first
    /// * `Err(anyhow::Error)` - Failed to retrieve history
    ///
    /// # Default Implementation
    /// Returns empty vector - drivers should override if they support history
    async fn get_history(&self, _limit: Option<usize>) -> Result<Vec<MeasurementData>> {
        Ok(Vec::new()) // Default: no history
    }

    /// Get history statistics and buffer information
    ///
    /// Returns metadata about the driver's history buffer including
    /// capacity, current size, oldest/newest timestamps, etc.
    ///
    /// # Returns
    /// * `Ok(Value)` - JSON object with buffer statistics
    /// * `Err(anyhow::Error)` - Failed to get statistics
    ///
    /// # Default Implementation
    /// Returns basic statistics - drivers should override for detailed info
    async fn get_history_stats(&self) -> Result<Value> {
        Ok(serde_json::json!({
            "driver_type": self.driver_type(),
            "history_supported": false,
            "buffer_capacity": 0,
            "buffer_size": 0,
            "oldest_entry": null,
            "newest_entry": null
        }))
    }
}
