// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Step Response Test Implementation
//!
//! This module implements step response testing for PID tuning using the
//! thermal regulation driver abstraction. It provides controlled step inputs
//! and measures the system response for parameter identification.

use crate::StepResponseData;
use anyhow::{anyhow, Result};
use log::{debug, info};
use rust_photoacoustic::config::{thermal_regulation::ThermalRegulatorConfig, Config};
use rust_photoacoustic::thermal_regulation::{
    create_thermal_regulation_driver, ThermalRegulationDriver,
};
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Step response test controller
pub struct StepResponseTest {
    driver: Box<dyn ThermalRegulationDriver + Send + Sync>,
    regulator_config: ThermalRegulatorConfig,
    initial_temperature: f64,
}

impl StepResponseTest {
    /// Create a new step response test
    pub async fn new(config: &Config, regulator_config: &ThermalRegulatorConfig) -> Result<Self> {
        let thermal_config = &config.thermal_regulation;

        let bus_config = thermal_config
            .i2c_buses
            .get(&regulator_config.i2c_bus)
            .ok_or_else(|| anyhow!("I2C bus '{}' not found", regulator_config.i2c_bus))?;

        let mut driver = create_thermal_regulation_driver(bus_config, regulator_config)
            .map_err(|e| anyhow!("Failed to create thermal regulation driver: {}", e))?;

        // Initialize the driver
        driver
            .initialize()
            .await
            .map_err(|e| anyhow!("Failed to initialize thermal regulation driver: {}", e))?;

        // Get initial temperature
        let initial_temperature = driver
            .read_temperature()
            .await
            .map_err(|e| anyhow!("Failed to read initial temperature: {}", e))?;

        info!("Step response test initialized");
        info!("  Initial temperature: {:.1}°C", initial_temperature);
        info!(
            "  Driver status: {}",
            driver
                .get_status()
                .await
                .unwrap_or_else(|_| "Unknown".to_string())
        );

        Ok(Self {
            driver,
            regulator_config: regulator_config.clone(),
            initial_temperature,
        })
    }

    /// Perform a step response test
    ///
    /// This method applies a step input to the system and records the response
    /// over the specified duration. The step is applied as a sudden change in
    /// the control output (PWM duty cycle).
    pub async fn perform_step_response(
        &mut self,
        step_amplitude: f64,
        duration_seconds: u64,
    ) -> Result<StepResponseData> {
        info!("Starting step response test");
        info!("  Step amplitude: {:.1}°C", step_amplitude);
        info!("  Duration: {} seconds", duration_seconds);

        let mut time_data = Vec::new();
        let mut temperature_data = Vec::new();
        let mut setpoint_data = Vec::new();
        let mut control_output_data = Vec::new();

        let start_time = Instant::now();
        let sampling_interval = Duration::from_millis(1000); // 1 Hz sampling
        let total_samples = duration_seconds as usize;

        // Pre-stabilization period (10 seconds at initial conditions)
        info!("Pre-stabilization period...");
        for i in 0..10 {
            let elapsed = start_time.elapsed().as_secs_f64() - 10.0 + i as f64;
            let temp = self.driver.read_temperature().await?;

            time_data.push(elapsed);
            temperature_data.push(temp);
            setpoint_data.push(self.initial_temperature);
            control_output_data.push(0.0);

            sleep(sampling_interval).await;
        }

        // Calculate target temperature and required control output
        let target_temperature = self.initial_temperature + step_amplitude;
        let step_control_output = calculate_step_control_output(step_amplitude);

        info!(
            "Applying step input: {:.1}% control output",
            step_control_output
        );
        info!("Target temperature: {:.1}°C", target_temperature);

        // Apply step input and record response
        for i in 0..total_samples {
            let elapsed = start_time.elapsed().as_secs_f64();

            // Apply control output (step input)
            self.driver
                .apply_control_output(step_control_output)
                .await?;

            // Read temperature response
            let temp = self.driver.read_temperature().await?;

            // Record data point
            time_data.push(elapsed);
            temperature_data.push(temp);
            setpoint_data.push(target_temperature);
            control_output_data.push(step_control_output);

            debug!(
                "t={:.1}s, T={:.2}°C, u={:.1}%",
                elapsed, temp, step_control_output
            );

            // Progress indication
            if i % 30 == 0 {
                let progress = 100.0 * i as f64 / total_samples as f64;
                info!(
                    "Progress: {:.1}% - Current temperature: {:.2}°C",
                    progress, temp
                );
            }

            sleep(sampling_interval).await;
        }

        // Turn off control output
        self.driver.apply_control_output(0.0).await?;

        info!("Step response test completed");
        info!("  Samples collected: {}", time_data.len());
        info!(
            "  Final temperature: {:.2}°C",
            temperature_data.last().unwrap_or(&0.0)
        );

        Ok(StepResponseData {
            time: time_data,
            temperature: temperature_data,
            setpoint: setpoint_data,
            control_output: control_output_data,
        })
    }
}

/// Calculate appropriate control output for a given temperature step
fn calculate_step_control_output(step_amplitude: f64) -> f64 {
    // Simple heuristic: larger steps need more control effort
    // This is a rough approximation - in practice this would be based on
    // system identification or prior knowledge
    let base_output = step_amplitude * 10.0; // 10% output per degree
    base_output.clamp(-80.0, 80.0) // Limit to safe range
}
