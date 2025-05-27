// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Modbus TCP server configuration
//!
//! This module defines the structures for configuring the Modbus TCP server
//! component of the photoacoustic application.

use serde::{Deserialize, Serialize};

/// Configuration for the Modbus TCP server component.
///
/// This structure contains settings that control the Modbus TCP server functionality,
/// including network binding parameters and whether the server is enabled.
///
/// # Fields
///
/// * `enabled` - Flag to enable or disable the Modbus server
/// * `port` - TCP port number for the Modbus server (default: 502)
/// * `address` - Network address for the Modbus server to bind to (default: 127.0.0.1)
///
/// # Example
///
/// ```
/// use rust_photoacoustic::config::ModbusConfig;
///
/// let modbus_config = ModbusConfig {
///     enabled: true,
///     port: 503,
///     address: "0.0.0.0".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusConfig {
    /// Flag to enable or disable the Modbus server.
    ///
    /// When enabled, the server will start and respond to Modbus TCP requests.
    /// When disabled, no server will be started and no resources will be used.
    pub enabled: bool,

    /// The TCP port the Modbus server will listen on.
    ///
    /// Valid range is 1-65534. Default value is 502, which is the standard Modbus TCP port.
    pub port: u16,

    /// The network address the Modbus server will bind to.
    ///
    /// Can be an IPv4/IPv6 address or a hostname. Default is "127.0.0.1".
    /// Use "0.0.0.0" to bind to all IPv4 interfaces.
    pub address: String,
}

impl Default for ModbusConfig {
    fn default() -> Self {
        Self {
            enabled: false,                   // Disabled by default for safety
            port: 502,                        // Standard Modbus TCP port
            address: "127.0.0.1".to_string(), // Localhost for security
        }
    }
}
