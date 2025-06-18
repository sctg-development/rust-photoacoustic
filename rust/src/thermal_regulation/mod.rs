// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Thermal regulation system for photoacoustic applications
//!
//! This module provides thermal regulation capabilities including:
//! - I2C device communication (native, CP2112, and mock drivers)
//! - PID controller implementation for precise temperature control
//! - Thermal cell simulation for testing and development
//! - Hardware abstraction for different thermal control systems

pub mod controller;
pub mod drivers;
pub mod simulation;

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::thermal_regulation::{I2CBusConfig, I2CBusType, ThermalRegulationConfig};

/// Main thermal regulation manager
pub struct ThermalRegulationManager {
    config: ThermalRegulationConfig,
    buses: HashMap<String, Arc<RwLock<Box<dyn I2CBusDriver + Send + Sync>>>>,
    controllers: Vec<ThermalController>,
}

/// I2C bus driver trait for hardware abstraction
#[async_trait::async_trait]
pub trait I2CBusDriver {
    /// Read data from I2C device
    async fn read(&mut self, address: u8, register: u8, length: usize) -> Result<Vec<u8>>;

    /// Write data to I2C device
    async fn write(&mut self, address: u8, register: u8, data: &[u8]) -> Result<()>;

    /// Check if device is present on the bus
    async fn device_present(&mut self, address: u8) -> Result<bool>;
}

/// Thermal controller for managing individual regulators
pub struct ThermalController {
    id: String,
    pid: PidController,
    target_temperature: f64,
    current_temperature: f64,
    output_power: f64,
}

/// PID controller implementation
pub struct PidController {
    kp: f64,
    ki: f64,
    kd: f64,
    integral: f64,
    previous_error: f64,
    max_output: f64,
    min_output: f64,
}

impl ThermalRegulationManager {
    /// Create a new thermal regulation manager
    pub fn new(config: ThermalRegulationConfig) -> Result<Self> {
        let mut buses = HashMap::new();

        // Initialize I2C buses based on configuration
        for (bus_name, bus_config) in &config.i2c_buses {
            let driver = Self::create_bus_driver(bus_config)?;
            buses.insert(bus_name.clone(), Arc::new(RwLock::new(driver)));
        }

        // Initialize thermal controllers
        let controllers = config
            .regulators
            .iter()
            .map(|reg_config| ThermalController::new(reg_config))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            config,
            buses,
            controllers,
        })
    }

    /// Create appropriate I2C bus driver based on configuration
    fn create_bus_driver(config: &I2CBusConfig) -> Result<Box<dyn I2CBusDriver + Send + Sync>> {
        match config.bus_type {
            I2CBusType::Native => Ok(Box::new(drivers::native::NativeI2CDriver::new(
                &config.device,
            )?)),
            I2CBusType::Cp2112 => Ok(Box::new(drivers::cp2112::Cp2112Driver::new(
                config.usb_vendor_id.unwrap_or(0x10c4),
                config.usb_product_id.unwrap_or(0xea90),
            )?)),
            I2CBusType::Mock => Ok(Box::new(drivers::mock::MockI2CDriver::new(config)?)),
        }
    }

    /// Start thermal regulation process
    pub async fn start(&mut self) -> Result<()> {
        // Implementation for starting regulation loops
        Ok(())
    }

    /// Stop thermal regulation process
    pub async fn stop(&mut self) -> Result<()> {
        // Implementation for stopping regulation loops
        Ok(())
    }
}

impl ThermalController {
    /// Create a new thermal controller
    pub fn new(config: &crate::config::thermal_regulation::ThermalRegulatorConfig) -> Result<Self> {
        Ok(Self {
            id: config.id.clone(),
            pid: PidController::new(
                config.pid_parameters.kp as f64,
                config.pid_parameters.ki as f64,
                config.pid_parameters.kd as f64,
                config.pid_parameters.output_min as f64,
                config.pid_parameters.output_max as f64,
            ),
            target_temperature: config.pid_parameters.setpoint as f64,
            current_temperature: 25.0, // ambient temperature
            output_power: 0.0,
        })
    }

    /// Update controller with new temperature reading
    pub fn update(&mut self, current_temp: f64, dt: f64) -> f64 {
        self.current_temperature = current_temp;
        self.output_power = self.pid.update(self.target_temperature, current_temp, dt);
        self.output_power
    }

    /// Set new target temperature
    pub fn set_target(&mut self, target: f64) {
        self.target_temperature = target;
    }
}

impl PidController {
    /// Create a new PID controller
    pub fn new(kp: f64, ki: f64, kd: f64, min_output: f64, max_output: f64) -> Self {
        Self {
            kp,
            ki,
            kd,
            integral: 0.0,
            previous_error: 0.0,
            max_output,
            min_output,
        }
    }

    /// Update PID controller and return control output
    pub fn update(&mut self, setpoint: f64, process_variable: f64, dt: f64) -> f64 {
        let error = setpoint - process_variable;

        // Proportional term
        let proportional = self.kp * error;

        // Integral term
        self.integral += error * dt;
        let integral = self.ki * self.integral;

        // Derivative term
        let derivative = self.kd * (error - self.previous_error) / dt;
        self.previous_error = error;

        // Combine terms
        let output = proportional + integral + derivative;

        // Clamp output to limits
        output.clamp(self.min_output, self.max_output)
    }

    /// Reset PID controller state
    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.previous_error = 0.0;
    }
}
