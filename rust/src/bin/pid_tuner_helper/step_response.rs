// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Step Response Test Implementation
//!
//! This module implements step response testing for PID tuning using the mock
//! thermal regulation driver. It provides controlled step inputs and measures
//! the system response for parameter identification.

use crate::StepResponseData;
use anyhow::{anyhow, Result};
use log::{debug, info};
use rust_photoacoustic::config::{thermal_regulation::ThermalRegulatorConfig, Config};
use rust_photoacoustic::thermal_regulation::drivers::MockI2CDriver;
use rust_photoacoustic::thermal_regulation::I2CBusDriver;
use rust_photoacoustic::utility::convert_voltage_to_temperature;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Step response test controller
pub struct StepResponseTest {
    driver: MockI2CDriver,
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

        let driver = MockI2CDriver::new(bus_config)
            .map_err(|e| anyhow!("Failed to create mock driver: {}", e))?;

        // Get initial temperature
        let initial_temperature = 25.0; // Mock driver starts at 25°C

        info!("Step response test initialized");
        info!("  Initial temperature: {:.1}°C", initial_temperature);
        info!(
            "  ADC address: 0x{:02X}",
            regulator_config.temperature_sensor.adc_address
        );
        info!(
            "  PWM address: 0x{:02X}",
            regulator_config
                .actuators
                .thermal_control
                .pwm_controller
                .address
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
            let temp = self.read_temperature().await?;

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
            self.apply_control_output(step_control_output).await?;

            // Read temperature response
            let temp = self.read_temperature().await?;

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
        self.apply_control_output(0.0).await?;

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

    /// Read current temperature from the mock sensor
    async fn read_temperature(&mut self) -> Result<f64> {
        // Read from ADC (simulated temperature sensor)
        let adc_address = self.regulator_config.temperature_sensor.adc_address;
        let adc_channel = self.regulator_config.temperature_sensor.adc_channel;

        // Read raw ADC value
        let adc_data = self
            .driver
            .read(adc_address, adc_channel, 2)
            .await
            .map_err(|e| anyhow!("Failed to read ADC: {}", e))?;

        if adc_data.len() < 2 {
            return Err(anyhow!("Insufficient ADC data"));
        }

        // Convert raw ADC to temperature using the configured formula
        let raw_value = ((adc_data[0] as u16) << 8) | (adc_data[1] as u16);
        debug!("ADC raw value: {} (0x{:04X})", raw_value, raw_value);
        let temperature = convert_adc_to_temperature(raw_value, &self.regulator_config)?;
        debug!("Converted temperature: {:.2}°C", temperature);

        Ok(temperature)
    }

    /// Apply control output (PWM duty cycle)
    async fn apply_control_output(&mut self, duty_percent: f64) -> Result<()> {
        let pwm_address = self
            .regulator_config
            .actuators
            .thermal_control
            .pwm_controller
            .address;
        let pwm_channel = self
            .regulator_config
            .actuators
            .thermal_control
            .pwm_controller
            .channel;

        // Convert duty percentage to PWM register values
        let duty_clamped = duty_percent.clamp(-100.0, 100.0);
        let pwm_value = ((duty_clamped.abs() / 100.0) * 4095.0) as u16; // 12-bit PWM

        // Write PWM duty cycle
        let pwm_data = [
            (pwm_value & 0xFF) as u8,        // Low byte
            ((pwm_value >> 8) & 0xFF) as u8, // High byte
        ];

        debug!(
            "PWM write: address=0x{:02X}, channel={}, duty={:.1}%, pwm_value={}",
            pwm_address, pwm_channel, duty_clamped, pwm_value
        );

        self.driver
            .write(pwm_address, pwm_channel, &pwm_data)
            .await
            .map_err(|e| anyhow!("Failed to write PWM: {}", e))?;

        // Handle H-Bridge direction control
        let direction_controller = self
            .regulator_config
            .actuators
            .thermal_control
            .direction_controller
            .clone();
        self.set_thermal_direction(duty_clamped, &direction_controller)
            .await?;

        Ok(())
    }

    /// Set thermal direction using GPIO controller for H-Bridge
    async fn set_thermal_direction(
        &mut self,
        duty_percent: f64,
        direction_controller: &rust_photoacoustic::config::thermal_regulation::DirectionControllerConfig,
    ) -> Result<()> {
        let gpio_address = direction_controller.address;

        // Determine H-Bridge control signals
        let (in1, in2, enable) = if duty_percent > 0.0 {
            // Positive (heating)
            (true, false, true)
        } else if duty_percent < 0.0 {
            // Negative (cooling)
            (false, true, true)
        } else {
            // Off
            (false, false, false)
        };

        // Construct GPIO register value
        let mut gpio_value = 0u8;
        if in1 {
            gpio_value |= 1 << direction_controller.gpio_pins.h_bridge_in1;
        }
        if in2 {
            gpio_value |= 1 << direction_controller.gpio_pins.h_bridge_in2;
        }
        if enable {
            gpio_value |= 1 << direction_controller.gpio_pins.h_bridge_enable;
        }

        // Write GPIO register
        self.driver
            .write(gpio_address, 0x02, &[gpio_value])
            .await // 0x02 = output register
            .map_err(|e| anyhow!("Failed to write GPIO: {}", e))?;

        Ok(())
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

/// Convert raw ADC value to temperature using configurable formula
///
/// This function converts a raw ADC reading to temperature using the formula
/// specified in the thermal regulator configuration. It supports various
/// conversion types including NTC thermistors with β formula.
fn convert_adc_to_temperature(
    raw_value: u16,
    regulator_config: &ThermalRegulatorConfig,
) -> Result<f64> {
    // Get conversion parameters from configuration
    let temp_conversion = &regulator_config.temperature_conversion;
    let adc_resolution = temp_conversion.adc_resolution;
    let voltage_reference = temp_conversion.voltage_reference;
    let formula = &temp_conversion.formula;

    // Convert ADC reading to voltage based on configuration
    let max_adc_value = (1_u32 << adc_resolution) - 1; // e.g., 65535 for 16-bit
    let voltage = (raw_value as f64 / max_adc_value as f64) * voltage_reference as f64;

    debug!(
        "ADC conversion: raw={}, voltage={:.3}V, formula='{}'",
        raw_value, voltage, formula
    );

    // Use the utility function to convert voltage to temperature
    let temperature_k = convert_voltage_to_temperature(formula.clone(), voltage as f32)?;

    // Convert from Kelvin to Celsius for return value
    let temperature_c = temperature_k - 273.15;

    debug!(
        "Temperature conversion: {:.2}K = {:.2}°C",
        temperature_k, temperature_c
    );

    Ok(temperature_c)
}
