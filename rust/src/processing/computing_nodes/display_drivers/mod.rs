// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Display drivers for UniversalActionNode
//!
//! This module provides a pluggable driver architecture for display outputs in the
//! UniversalActionNode. Drivers abstract different display technologies and
//! communication protocols, allowing the same ActionNode to output to various endpoints.
//!
//! # Architecture
//!
//! ```text
//! UniversalActionNode
//!           ↓
//!    DisplayDriver trait
//!           ↓
//! ┌─────────────┬─────────────┬─────────────┬─────────────┐
//! │   HTTPS     │    Redis    │    Kafka    │  Physical   │
//! │  Callback   │   Driver    │   Driver    │   Drivers   │
//! │   Driver    │             │             │             │
//! └─────────────┴─────────────┴─────────────┴─────────────┘
//! ```

// Core modules containing driver implementations
mod http;
mod kafka;
mod redis;

// Re-export driver implementations
pub use self::http::HttpsCallbackActionDriver;
pub use self::kafka::KafkaActionDriver;
pub use self::redis::{RedisActionDriver, RedisDriverMode};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::time::SystemTime;

/// Core display data passed to drivers
#[derive(Debug, Clone)]
pub struct DisplayData {
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
    /// Additional metadata for the display
    pub metadata: HashMap<String, Value>,
}

/// Alert/alarm data for special display states
#[derive(Debug, Clone)]
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

/// Trait for all display drivers
///
/// This trait abstracts different display technologies and communication protocols.
/// Each driver implements the specific logic for updating its display type while
/// providing a common interface to the UniversalActionNode.
#[async_trait]
pub trait DisplayDriver: Send + Sync + std::fmt::Debug {
    /// Initialize the driver and establish connection
    ///
    /// This method is called when the driver is first configured.
    /// Use it to establish network connections, initialize hardware, etc.
    ///
    /// # Returns
    /// * `Ok(())` - Driver initialized successfully
    /// * `Err(anyhow::Error)` - Initialization failed
    async fn initialize(&mut self) -> Result<()>;

    /// Update display with current concentration data
    ///
    /// This is the primary method called when new concentration data is available.
    /// The driver should format and display the data according to its capabilities.
    ///
    /// # Arguments
    /// * `data` - Current display data with concentration, amplitude, etc.
    ///
    /// # Returns
    /// * `Ok(())` - Display updated successfully
    /// * `Err(anyhow::Error)` - Display update failed
    async fn update_display(&mut self, data: &DisplayData) -> Result<()>;

    /// Flash/alert display for alarm conditions
    ///
    /// Called when threshold conditions are met and an alert needs to be displayed.
    /// The driver should implement appropriate visual/audio alerts for its display type.
    ///
    /// # Arguments
    /// * `alert` - Alert data with type, severity, and message
    ///
    /// # Returns
    /// * `Ok(())` - Alert displayed successfully
    /// * `Err(anyhow::Error)` - Alert display failed
    async fn show_alert(&mut self, alert: &AlertData) -> Result<()>;

    /// Clear display and return to idle state
    ///
    /// Called when the system is shutting down or resetting.
    /// The driver should clear any active displays and return to a safe state.
    ///
    /// # Returns
    /// * `Ok(())` - Display cleared successfully
    /// * `Err(anyhow::Error)` - Clear operation failed
    async fn clear_display(&mut self) -> Result<()>;

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
    /// Some drivers (like physical displays) support real-time updates,
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
}
