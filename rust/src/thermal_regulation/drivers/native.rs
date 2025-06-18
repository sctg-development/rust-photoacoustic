// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Native I2C driver for Raspberry Pi hardware
//!
//! This module provides a native I2C driver that communicates directly
//! with Raspberry Pi I2C hardware through /dev/i2c-* devices.

use crate::thermal_regulation::I2CBusDriver;
use anyhow::{anyhow, Result};

/// Native I2C driver for Raspberry Pi
pub struct NativeI2CDriver {
    device_path: String,
}

impl NativeI2CDriver {
    /// Create a new native I2C driver
    pub fn new(device_path: &str) -> Result<Self> {
        // TODO: Implement actual native I2C driver
        // This is a stub implementation for compilation
        Ok(Self {
            device_path: device_path.to_string(),
        })
    }
}

#[async_trait::async_trait]
impl I2CBusDriver for NativeI2CDriver {
    async fn read(&mut self, address: u8, register: u8, length: usize) -> Result<Vec<u8>> {
        // TODO: Implement actual I2C read operation
        Err(anyhow!("Native I2C driver not yet implemented"))
    }

    async fn write(&mut self, address: u8, register: u8, data: &[u8]) -> Result<()> {
        // TODO: Implement actual I2C write operation
        Err(anyhow!("Native I2C driver not yet implemented"))
    }

    async fn device_present(&mut self, address: u8) -> Result<bool> {
        // TODO: Implement device detection
        Err(anyhow!("Native I2C driver not yet implemented"))
    }
}
