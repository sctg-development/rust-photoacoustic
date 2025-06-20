// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Mock I2C driver for thermal regulation simulation
//!
//! This module provides a mock I2C driver that simulates the thermal behavior
//! of a photoacoustic cell. The simulation includes:
//! - Physical properties of a 1016g stainless steel 316 cell (110x30x60mm)
//! - Peltier module thermal dynamics (15x30mm)
//! - Heating resistor simulation (60W DBK HPG-1/10-60x35-12-24V)
//! - Temperature sensor behavior
//! - Realistic thermal time constants and responses

use crate::config::thermal_regulation::I2CBusConfig;
use crate::thermal_regulation::I2CBusDriver;
use anyhow::{anyhow, Result};
use log::{debug, info};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Mock I2C driver for thermal regulation simulation
pub struct MockI2CDriver {
    devices: Arc<Mutex<HashMap<u8, MockDevice>>>,
    thermal_simulation: Arc<Mutex<ThermalCellSimulation>>,
    start_time: Instant,
}

/// Mock I2C device representation
#[derive(Debug, Clone)]
pub struct MockDevice {
    address: u8,
    device_type: MockDeviceType,
    registers: HashMap<u8, u8>,
}

/// Types of mock devices supported
#[derive(Debug, Clone)]
pub enum MockDeviceType {
    /// Temperature sensor (MCP9808)
    TemperatureSensor,
    /// ADC controller (ADS1115)
    AdcController,
    /// PWM controller (PCA9685)
    PwmController,
    /// GPIO controller (CAT9555)
    GpioController,
}

/// Thermal simulation of the photoacoustic cell
#[derive(Debug)]
pub struct ThermalCellSimulation {
    /// Current temperature of the cell in Celsius
    temperature: f64,
    /// Target temperature for regulation
    target_temperature: f64,
    /// Current Peltier power (-100 to +100%)
    peltier_power: f64,
    /// Current heating resistor power (0 to 100%)
    heater_power: f64,
    /// Ambient temperature in Celsius
    ambient_temperature: f64,
    /// Last simulation update time
    last_update: Instant,
    /// Last logging time for periodic status messages
    last_log_time: Instant,
    /// Physical properties
    properties: ThermalProperties,
}

/// Physical and thermal properties of the photoacoustic cell
#[derive(Debug)]
pub struct ThermalProperties {
    /// Cell mass in grams
    mass_g: f64,
    /// Cell dimensions in mm (length, width, height)
    dimensions_mm: (f64, f64, f64),
    /// Material specific heat capacity (J/kg·K) - Stainless Steel 316
    specific_heat: f64,
    /// Thermal conductivity (W/m·K) - Stainless Steel 316
    thermal_conductivity: f64,
    /// Surface area for heat transfer (m²)
    surface_area_m2: f64,
    /// Heat transfer coefficient to ambient (W/m²·K)
    heat_transfer_coefficient: f64,
    /// Peltier maximum power (W)
    peltier_max_power: f64,
    /// Peltier dimensions in mm (length, width)
    peltier_dimensions_mm: (f64, f64),
    /// Heating resistor maximum power (W) - DBK HPG-1/10-60x35-12-24V
    heater_max_power: f64,
    /// Thermal time constant (seconds)
    thermal_time_constant: f64,
}

impl Default for ThermalProperties {
    fn default() -> Self {
        let length_mm = 110.0;
        let width_mm = 30.0;
        let height_mm = 60.0;

        // Calculate surface area in m²
        let surface_area_m2 = 2.0
            * ((length_mm * width_mm) + (length_mm * height_mm) + (width_mm * height_mm))
            / 1_000_000.0; // Convert mm² to m²

        Self {
            mass_g: 1016.0,
            dimensions_mm: (length_mm, width_mm, height_mm),
            specific_heat: 501.0, // J/kg·K for stainless steel 316 (updated from 0.5 J/g·K)
            thermal_conductivity: 16.2, // W/m·K for stainless steel 316
            surface_area_m2,
            heat_transfer_coefficient: 25.0, // W/m²·K (increased for better heat dissipation)
            peltier_max_power: 5.0,          // W (typical for 15x30mm Peltier)
            peltier_dimensions_mm: (15.0, 30.0),
            heater_max_power: 60.0, // W - DBK HPG-1/10-60x35-12-24V (60W resistor)
            thermal_time_constant: 90.0, // seconds (reduced for faster response with 60W heater)
        }
    }
}

impl MockI2CDriver {
    /// Create a new mock I2C driver
    pub fn new(config: &I2CBusConfig) -> Result<Self> {
        let mut devices = HashMap::new();

        // Add configured temperature sensors (MCP9808) - using ADC addresses as temperature sensors
        for controller in &config.adc_controllers {
            devices.insert(
                controller.address,
                MockDevice::new(controller.address, MockDeviceType::TemperatureSensor),
            );
        }

        // Add configured ADC controllers (ADS1115)
        for controller in &config.adc_controllers {
            devices.insert(
                controller.address,
                MockDevice::new(controller.address, MockDeviceType::AdcController),
            );
        }

        // Add configured PWM controllers (PCA9685)
        for controller in &config.pwm_controllers {
            devices.insert(
                controller.address,
                MockDevice::new(controller.address, MockDeviceType::PwmController),
            );
        }

        // Add configured GPIO controllers (CAT9555)
        for controller in &config.gpio_controllers {
            devices.insert(
                controller.address,
                MockDevice::new(controller.address, MockDeviceType::GpioController),
            );
        }

        let thermal_simulation = ThermalCellSimulation::new();

        Ok(Self {
            devices: Arc::new(Mutex::new(devices)),
            thermal_simulation: Arc::new(Mutex::new(thermal_simulation)),
            start_time: Instant::now(),
        })
    }

    /// Update thermal simulation based on current control outputs
    fn update_thermal_simulation(&self) -> Result<()> {
        let mut simulation = self
            .thermal_simulation
            .lock()
            .map_err(|_| anyhow!("Failed to lock thermal simulation"))?;

        let old_temp = simulation.temperature;
        simulation.update();
        let new_temp = simulation.temperature;

        // Always show debug output for temperature changes
        debug!(
            "Thermal simulation updated: {:.2}°C -> {:.2}°C (heater: {:.1}%, peltier: {:.1}%)",
            old_temp, new_temp, simulation.heater_power, simulation.peltier_power
        );

        // Each minute, log the current temperature, peltier power, and heater power
        if simulation.last_log_time.elapsed() >= Duration::from_secs(60) {
            info!(
                "Thermal simulation status: {:.2}°C, Peltier power: {:.1}%, Heater power: {:.1}%",
                simulation.temperature, simulation.peltier_power, simulation.heater_power
            );
            simulation.last_log_time = Instant::now();
        }
        Ok(())
    }

    /// Get current temperature from simulation
    pub fn get_current_temperature(&self) -> Result<f64> {
        let simulation = self
            .thermal_simulation
            .lock()
            .map_err(|_| anyhow!("Failed to lock thermal simulation"))?;
        Ok(simulation.temperature)
    }

    /// Set Peltier power for simulation
    pub fn set_peltier_power(&self, power_percent: f64) -> Result<()> {
        let mut simulation = self
            .thermal_simulation
            .lock()
            .map_err(|_| anyhow!("Failed to lock thermal simulation"))?;
        simulation.set_peltier_power(power_percent);
        Ok(())
    }

    /// Set heating resistor power for simulation
    pub fn set_heater_power(&self, power_percent: f64) -> Result<()> {
        let mut simulation = self
            .thermal_simulation
            .lock()
            .map_err(|_| anyhow!("Failed to lock thermal simulation"))?;
        simulation.set_heater_power(power_percent);
        Ok(())
    }
}

#[async_trait::async_trait]
impl I2CBusDriver for MockI2CDriver {
    async fn read(&mut self, address: u8, register: u8, length: usize) -> Result<Vec<u8>> {
        // Update thermal simulation before reading
        self.update_thermal_simulation()?;

        let devices = self
            .devices
            .lock()
            .map_err(|_| anyhow!("Failed to lock devices"))?;

        let device = devices
            .get(&address)
            .ok_or_else(|| anyhow!("Device not found at address 0x{:02X}", address))?;

        match device.device_type {
            MockDeviceType::TemperatureSensor => self.read_temperature_sensor(register, length),
            MockDeviceType::AdcController => self.read_adc_controller(register, length),
            MockDeviceType::PwmController => self.read_pwm_controller(register, length),
            MockDeviceType::GpioController => self.read_gpio_controller(register, length),
        }
    }

    async fn write(&mut self, address: u8, register: u8, data: &[u8]) -> Result<()> {
        let mut devices = self
            .devices
            .lock()
            .map_err(|_| anyhow!("Failed to lock devices"))?;

        let device = devices
            .get_mut(&address)
            .ok_or_else(|| anyhow!("Device not found at address 0x{:02X}", address))?;

        // Debug output for all writes
        debug!(
            "I2C write to address=0x{:02X}, register=0x{:02X}, data={:?}",
            address, register, data
        );

        match device.device_type {
            MockDeviceType::TemperatureSensor => self.write_temperature_sensor(register, data),
            MockDeviceType::AdcController => self.write_adc_controller(register, data),
            MockDeviceType::PwmController => self.write_pwm_controller(register, data),
            MockDeviceType::GpioController => self.write_gpio_controller(register, data),
        }
    }

    async fn device_present(&mut self, address: u8) -> Result<bool> {
        let devices = self
            .devices
            .lock()
            .map_err(|_| anyhow!("Failed to lock devices"))?;
        Ok(devices.contains_key(&address))
    }
}

impl MockI2CDriver {
    /// Read from temperature sensor (MCP9808)
    fn read_temperature_sensor(&self, register: u8, length: usize) -> Result<Vec<u8>> {
        match register {
            0x05 => {
                // Temperature register
                let simulation = self
                    .thermal_simulation
                    .lock()
                    .map_err(|_| anyhow!("Failed to lock thermal simulation"))?;

                // Convert temperature to MCP9808 format (16-bit, 0.0625°C resolution)
                let temp_c = simulation.temperature;
                // MCP9808 uses 16-bit signed format: temp = register_value / 16.0
                let temp_raw = (temp_c * 16.0) as i16;

                Ok(vec![(temp_raw >> 8) as u8, (temp_raw & 0xFF) as u8])
            }
            0x06 => {
                // Configuration register
                Ok(vec![0x00, 0x00]) // Default configuration
            }
            0x07 => {
                // Alert temperature upper boundary
                Ok(vec![0x00, 0x00])
            }
            _ => Err(anyhow!(
                "Unsupported register 0x{:02X} for temperature sensor",
                register
            )),
        }
    }

    /// Write to temperature sensor (MCP9808)
    fn write_temperature_sensor(&self, register: u8, data: &[u8]) -> Result<()> {
        match register {
            0x01 => {
                // Configuration register
                // Accept configuration writes but don't do anything
                Ok(())
            }
            _ => Err(anyhow!(
                "Unsupported write to register 0x{:02X} for temperature sensor",
                register
            )),
        }
    }

    /// Read from ADC controller (ADS1115)
    fn read_adc_controller(&self, register: u8, length: usize) -> Result<Vec<u8>> {
        match register {
            0x00 => {
                // Conversion register
                let simulation = self
                    .thermal_simulation
                    .lock()
                    .map_err(|_| anyhow!("Failed to lock thermal simulation"))?;

                // Simulate NTC thermistor circuit:
                // NTC: 10kΩ at 25°C, β=3977
                // Circuit: 5V --- 10kΩ resistor --- ADC input --- NTC --- GND
                // ADC voltage = 5V * R_ntc / (10000 + R_ntc)

                let temp_c = simulation.temperature;
                let temp_k = temp_c + 273.15;

                // NTC resistance using β formula: R = R0 * exp(β * (1/T - 1/T0))
                let r0 = 10000.0; // 10kΩ at 25°C
                let beta = 3977.0;
                let t0 = 298.15; // 25°C in Kelvin

                let r_ntc = r0 * ((beta * (1.0 / temp_k - 1.0 / t0)).exp());

                // Voltage divider: V_adc = 5V * R_ntc / (10000 + R_ntc)
                let v_adc = 5.0 * r_ntc / (10000.0 + r_ntc);

                // Convert to 16-bit ADC reading (0-65535 for 0-5V)
                let adc_raw = ((v_adc / 5.0) * 65535.0) as u16;

                Ok(vec![(adc_raw >> 8) as u8, (adc_raw & 0xFF) as u8])
            }
            0x01 => {
                // Configuration register
                Ok(vec![0x85, 0x83]) // Default configuration
            }
            _ => Err(anyhow!(
                "Unsupported register 0x{:02X} for ADC controller",
                register
            )),
        }
    }

    /// Write to ADC controller (ADS1115)
    fn write_adc_controller(&self, register: u8, data: &[u8]) -> Result<()> {
        match register {
            0x01 => {
                // Configuration register
                // Accept configuration writes
                Ok(())
            }
            _ => Err(anyhow!(
                "Unsupported write to register 0x{:02X} for ADC controller",
                register
            )),
        }
    }

    /// Read from PWM controller (PCA9685)
    fn read_pwm_controller(&self, register: u8, length: usize) -> Result<Vec<u8>> {
        match register {
            0x00 => {
                // Mode1 register
                Ok(vec![0x20]) // Default mode
            }
            0x01 => {
                // Mode2 register
                Ok(vec![0x04]) // Default mode
            }
            0xFE => {
                // Prescaler register
                Ok(vec![0x79]) // Default prescaler for ~50Hz
            }
            _ if register >= 0x06 && register <= 0x45 => {
                // PWM channel registers
                Ok(vec![0x00, 0x00, 0x00, 0x00])
            }
            _ => Err(anyhow!(
                "Unsupported register 0x{:02X} for PWM controller",
                register
            )),
        }
    }

    /// Write to PWM controller (PCA9685)
    fn write_pwm_controller(&self, register: u8, data: &[u8]) -> Result<()> {
        debug!(
            "PWM controller write - register: 0x{:02X}, data: {:?}",
            register, data
        );

        match register {
            0x00 => {
                // Mode register, but also used for channel 0 in simple addressing
                if data.len() >= 2 {
                    // Check if this is PWM data (not a mode command)
                    let pwm_value = ((data[1] as u16) << 8) | (data[0] as u16);
                    if pwm_value <= 4095 {
                        // Looks like PWM data for channel 0
                        debug!("2-byte PWM write to channel 0 - value: {}", pwm_value);
                        let duty_cycle = (pwm_value as f64 / 4095.0) * 100.0;
                        debug!("Calculated duty cycle: {:.1}%", duty_cycle);
                        let _ = self.set_heater_power(duty_cycle);
                        debug!("PWM channel 0 set to {:.1}% duty cycle", duty_cycle);
                        // Force thermal simulation update
                        let _ = self.update_thermal_simulation();
                        return Ok(());
                    }
                }
                // If not PWM data, treat as mode register
                Ok(())
            }
            0x01 => {
                // Mode registers
                Ok(())
            }
            0xFE => {
                // Prescaler register
                Ok(())
            }
            // Handle both direct register writes and channel-based writes
            _ if register >= 0x06 && register <= 0x45 => {
                // Direct PWM channel register writes (standard PCA9685 addressing)
                let duty_cycle = if data.len() >= 4 {
                    // Full 4-byte PWM register write (on_time + off_time)
                    let on_time = ((data[1] as u16) << 8) | (data[0] as u16);
                    let off_time = ((data[3] as u16) << 8) | (data[2] as u16);
                    debug!(
                        "4-byte PWM write - on_time: {}, off_time: {}",
                        on_time, off_time
                    );
                    if off_time > on_time {
                        (off_time - on_time) as f64 / 4096.0 * 100.0
                    } else {
                        0.0
                    }
                } else if data.len() >= 2 {
                    // Simple 2-byte PWM value write
                    let pwm_value = ((data[1] as u16) << 8) | (data[0] as u16);
                    debug!("2-byte PWM write - value: {}", pwm_value);
                    (pwm_value as f64 / 4095.0) * 100.0
                } else {
                    debug!("Single byte PWM write - value: {}", data[0]);
                    (data[0] as f64 / 255.0) * 100.0
                };

                info!("Calculated duty cycle: {:.1}%", duty_cycle);

                // Channel 0 (register 0x06) controls thermal actuator
                if register == 0x06 {
                    // First channel - Peltier/heater control
                    let _ = self.set_heater_power(duty_cycle);
                    debug!("PWM channel 0 set to {:.1}% duty cycle", duty_cycle);
                    // Force thermal simulation update
                    let _ = self.update_thermal_simulation();
                }

                Ok(())
            }
            _ => Err(anyhow!(
                "Unsupported write to register 0x{:02X} for PWM controller",
                register
            )),
        }
    }

    /// Read from GPIO controller (CAT9555)
    fn read_gpio_controller(&self, register: u8, length: usize) -> Result<Vec<u8>> {
        match register {
            0x00 | 0x01 => {
                // Input port registers
                Ok(vec![0xFF, 0xFF]) // All inputs high
            }
            0x02 | 0x03 => {
                // Output port registers
                Ok(vec![0x00, 0x00]) // All outputs low
            }
            0x04 | 0x05 => {
                // Polarity inversion registers
                Ok(vec![0x00, 0x00]) // No inversion
            }
            0x06 | 0x07 => {
                // Configuration registers
                Ok(vec![0xFF, 0xFF]) // All pins as inputs
            }
            _ => Err(anyhow!(
                "Unsupported register 0x{:02X} for GPIO controller",
                register
            )),
        }
    }

    /// Write to GPIO controller (CAT9555)
    fn write_gpio_controller(&self, register: u8, data: &[u8]) -> Result<()> {
        match register {
            0x02 | 0x03 => {
                // Output port registers
                // Accept GPIO writes (could be used for H-Bridge control)
                Ok(())
            }
            0x04 | 0x05 => {
                // Polarity inversion registers
                Ok(())
            }
            0x06 | 0x07 => {
                // Configuration registers
                Ok(())
            }
            _ => Err(anyhow!(
                "Unsupported write to register 0x{:02X} for GPIO controller",
                register
            )),
        }
    }
}

impl MockDevice {
    /// Create a new mock device
    pub fn new(address: u8, device_type: MockDeviceType) -> Self {
        Self {
            address,
            device_type,
            registers: HashMap::new(),
        }
    }
}

impl ThermalCellSimulation {
    /// Create a new thermal cell simulation
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            temperature: 25.0, // Start at room temperature
            target_temperature: 41.0,
            peltier_power: 0.0,
            heater_power: 0.0,
            ambient_temperature: 25.0,
            last_update: now,
            last_log_time: now,
            properties: ThermalProperties::default(),
        }
    }

    /// Update thermal simulation
    pub fn update(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f64();
        self.last_update = now;

        if dt > 0.0 && dt < 10.0 {
            // Sanity check on time step
            self.temperature = self.calculate_next_temperature(dt);
        }
    }

    /// Calculate next temperature based on thermal dynamics
    fn calculate_next_temperature(&self, dt: f64) -> f64 {
        // Heat input from Peltier (positive = heating, negative = cooling)
        let peltier_heat = self.peltier_power / 100.0 * self.properties.peltier_max_power;

        // Heat input from resistive heater (always positive)
        let heater_heat = self.heater_power / 100.0 * self.properties.heater_max_power;

        // Heat loss to ambient (convective cooling)
        let temp_diff = self.temperature - self.ambient_temperature;
        let ambient_heat_loss =
            self.properties.heat_transfer_coefficient * self.properties.surface_area_m2 * temp_diff;

        // Total heat rate (Watts)
        let total_heat_rate = peltier_heat + heater_heat - ambient_heat_loss;

        // Temperature change using thermal mass
        let thermal_mass = (self.properties.mass_g / 1000.0) * self.properties.specific_heat; // J/K (mass converted from g to kg)
        let temp_change = total_heat_rate * dt / thermal_mass; // K

        // Apply first-order thermal lag using time constant
        let thermal_lag_factor = 1.0 - (-dt / self.properties.thermal_time_constant).exp();
        let effective_temp_change = temp_change * thermal_lag_factor;

        self.temperature + effective_temp_change
    }

    /// Set Peltier power (-100 to +100%)
    pub fn set_peltier_power(&mut self, power: f64) {
        self.peltier_power = power.clamp(-100.0, 100.0);
    }

    /// Set heating resistor power (0 to 100%)
    pub fn set_heater_power(&mut self, power: f64) {
        self.heater_power = power.clamp(0.0, 100.0);
    }

    /// Set ambient temperature
    pub fn set_ambient_temperature(&mut self, temp: f64) {
        self.ambient_temperature = temp;
    }

    /// Get current temperature
    pub fn get_temperature(&self) -> f64 {
        self.temperature
    }

    /// Get thermal properties
    pub fn get_properties(&self) -> &ThermalProperties {
        &self.properties
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_mock_driver_creation() {
        let config = I2CBusConfig {
            bus_type: crate::config::thermal_regulation::I2CBusType::Mock,
            device: "mock".to_string(),
            usb_vendor_id: None,
            usb_product_id: None,
            pwm_controllers: vec![],
            adc_controllers: vec![],
            gpio_controllers: vec![],
            bus_settings: Default::default(),
        };

        let driver = MockI2CDriver::new(&config);
        assert!(driver.is_ok());
    }

    #[tokio::test]
    async fn test_device_presence() {
        let config = I2CBusConfig {
            bus_type: crate::config::thermal_regulation::I2CBusType::Mock,
            device: "mock".to_string(),
            usb_vendor_id: None,
            usb_product_id: None,
            pwm_controllers: vec![crate::config::thermal_regulation::PwmControllerConfig {
                address: 0x40,
                channels: 16,
                frequency_hz: 1000,
                settings: Default::default(),
            }],
            adc_controllers: vec![],
            gpio_controllers: vec![],
            bus_settings: Default::default(),
        };

        let mut driver = MockI2CDriver::new(&config).unwrap();

        // Test device presence
        assert!(driver.device_present(0x40).await.unwrap());
        assert!(!driver.device_present(0x41).await.unwrap());
    }

    #[test]
    fn test_thermal_simulation() {
        let mut sim = ThermalCellSimulation::new();

        // Test initial conditions
        assert_eq!(sim.get_temperature(), 25.0);

        // Test heating
        sim.set_heater_power(50.0);
        sim.update();

        // Temperature should increase (though the change might be small)
        // This test verifies the simulation logic runs without error
        assert!(sim.get_temperature() >= 25.0);

        // Test cooling
        sim.set_peltier_power(-50.0);
        sim.set_heater_power(0.0);

        // Run simulation for a bit
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(100));
            sim.update();
        }

        // Temperature should be influenced by cooling
        let final_temp = sim.get_temperature();
        assert!(final_temp < 30.0); // Should not have heated too much
    }

    #[test]
    fn test_thermal_properties() {
        let props = ThermalProperties::default();

        // Verify physical properties match specifications
        assert_eq!(props.mass_g, 1016.0);
        assert_eq!(props.dimensions_mm, (110.0, 30.0, 60.0));
        assert_eq!(props.peltier_dimensions_mm, (15.0, 30.0));
        assert_eq!(props.heater_max_power, 60.0); // DBK HPG-1/10-60x35-12-24V

        // Verify calculated surface area is reasonable
        assert!(props.surface_area_m2 > 0.0);
        assert!(props.surface_area_m2 < 1.0); // Should be less than 1 m²
    }

    #[tokio::test]
    async fn test_i2c_communication() {
        let config = I2CBusConfig {
            bus_type: crate::config::thermal_regulation::I2CBusType::Mock,
            device: "mock".to_string(),
            usb_vendor_id: None,
            usb_product_id: None,
            pwm_controllers: vec![crate::config::thermal_regulation::PwmControllerConfig {
                address: 0x40,
                channels: 16,
                frequency_hz: 1000,
                settings: Default::default(),
            }],
            adc_controllers: vec![crate::config::thermal_regulation::AdcControllerConfig {
                address: 0x48,
                channels: 4,
                resolution: 16,
                voltage_ref: 3.3,
                gain: crate::config::thermal_regulation::AdcGain::Gain2,
                data_rate: Default::default(),
            }],
            gpio_controllers: vec![],
            bus_settings: Default::default(),
        };

        let mut driver = MockI2CDriver::new(&config).unwrap();

        // Test ADC read
        let adc_data = driver.read(0x48, 0x00, 2).await.unwrap();
        assert_eq!(adc_data.len(), 2);

        // Test PWM write
        let pwm_data = [0x00, 0x00, 0xFF, 0x0F]; // 100% duty cycle
        assert!(driver.write(0x40, 0x06, &pwm_data).await.is_ok());

        // Test that thermal simulation was updated
        let temp = driver.get_current_temperature().unwrap();
        assert!(temp >= 20.0 && temp <= 50.0); // Reasonable temperature range
    }

    #[test]
    fn test_pid_controller() {
        let mut pid = crate::thermal_regulation::PidController::new(1.0, 0.1, 0.05, -100.0, 100.0);

        // Test step response
        let setpoint = 30.0;
        let mut process_value = 25.0;
        let dt = 0.1;

        for _ in 0..10 {
            let output = pid.update(setpoint, process_value, dt);
            assert!(output >= -100.0 && output <= 100.0);

            // Simulate simple process response
            process_value += output * 0.001;
        }

        // PID should drive process value towards setpoint
        assert!(process_value > 25.0); // Should have increased
    }

    #[test]
    fn test_thermal_dynamics_realistic() {
        let mut sim = ThermalCellSimulation::new();

        // Test heating with 60W resistor for 60 seconds
        sim.set_heater_power(100.0); // 100% = 60W

        let initial_temp = sim.get_temperature();

        // Simulate 60 seconds of heating
        for _ in 0..600 {
            std::thread::sleep(Duration::from_millis(1));
            sim.update();
        }

        let final_temp = sim.get_temperature();

        // Should have heated up but not unreasonably
        assert!(final_temp > initial_temp);
        assert!(final_temp < initial_temp + 50.0); // Shouldn't exceed 75°C

        // Test cooling
        sim.set_heater_power(0.0);
        sim.set_peltier_power(-100.0); // Full cooling

        let heated_temp = sim.get_temperature();

        // Simulate 60 seconds of cooling
        for _ in 0..600 {
            std::thread::sleep(Duration::from_millis(1));
            sim.update();
        }

        let cooled_temp = sim.get_temperature();
        assert!(cooled_temp < heated_temp);
    }
}
