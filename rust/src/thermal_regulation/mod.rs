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
pub mod daemon;
pub mod drivers;
pub mod shared_state;
pub mod simulation;

// Re-export main types for easier access
pub use daemon::ThermalRegulationSystemDaemon;
pub use shared_state::{create_shared_thermal_state, SharedThermalState};

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

/// I2C bus driver trait for low-level hardware communication
#[async_trait::async_trait]
pub trait I2CBusDriver {
    /// Read data from I2C device
    async fn read(&mut self, address: u8, register: u8, length: usize) -> Result<Vec<u8>>;

    /// Write data to I2C device
    async fn write(&mut self, address: u8, register: u8, data: &[u8]) -> Result<()>;

    /// Check if device is present on the bus
    async fn device_present(&mut self, address: u8) -> Result<bool>;
}

/// High-level thermal regulation driver trait for complete hardware abstraction
///
/// This trait provides a complete abstraction for thermal regulation operations,
/// encapsulating temperature reading, control output, and thermal direction control.
/// It allows the PID tuner to be completely hardware-independent.
#[async_trait::async_trait]
pub trait ThermalRegulationDriver: Send + Sync {
    /// Read the current temperature from the thermal sensor
    ///
    /// Returns the temperature in degrees Celsius.
    async fn read_temperature(&mut self) -> Result<f64>;

    /// Apply thermal control output
    ///
    /// # Arguments
    /// * `control_output` - Control output in percentage (-100.0 to +100.0)
    ///   - Positive values indicate heating
    ///   - Negative values indicate cooling  
    ///   - Zero indicates no thermal control
    async fn apply_control_output(&mut self, control_output: f64) -> Result<()>;

    /// Get the current control output value
    ///
    /// Returns the last applied control output percentage.
    fn get_current_control_output(&self) -> f64;

    /// Initialize the thermal regulation hardware
    ///
    /// This method should be called before any thermal operations.
    async fn initialize(&mut self) -> Result<()>;

    /// Get thermal regulation status information
    ///
    /// Returns a status string with hardware-specific information.
    async fn get_status(&mut self) -> Result<String>;
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

/// Mock thermal regulation driver for testing and simulation
pub struct MockThermalRegulationDriver {
    i2c_driver: drivers::mock::MockI2CDriver,
    regulator_config: crate::config::thermal_regulation::ThermalRegulatorConfig,
    current_control_output: f64,
}

impl MockThermalRegulationDriver {
    /// Create a new mock thermal regulation driver
    pub fn new(
        bus_config: &I2CBusConfig,
        regulator_config: &crate::config::thermal_regulation::ThermalRegulatorConfig,
    ) -> Result<Self> {
        let i2c_driver = drivers::mock::MockI2CDriver::new(bus_config)?;

        Ok(Self {
            i2c_driver,
            regulator_config: regulator_config.clone(),
            current_control_output: 0.0,
        })
    }
}

#[async_trait::async_trait]
impl ThermalRegulationDriver for MockThermalRegulationDriver {
    async fn read_temperature(&mut self) -> Result<f64> {
        use crate::utility::convert_voltage_to_temperature;
        use anyhow::anyhow;
        use log::debug;

        // Read from ADC (simulated temperature sensor)
        let adc_address = self.regulator_config.temperature_sensor.adc_address;
        let adc_channel = self.regulator_config.temperature_sensor.adc_channel;

        // Read raw ADC value
        let adc_data = self
            .i2c_driver
            .read(adc_address, adc_channel, 2)
            .await
            .map_err(|e| anyhow!("Failed to read ADC: {}", e))?;

        if adc_data.len() < 2 {
            return Err(anyhow!("Insufficient ADC data"));
        }

        // Convert raw ADC to temperature using the configured formula
        let raw_value = ((adc_data[0] as u16) << 8) | (adc_data[1] as u16);
        debug!("ADC raw value: {} (0x{:04X})", raw_value, raw_value);

        // Get conversion parameters from configuration
        let temp_conversion = &self.regulator_config.temperature_conversion;
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

    async fn apply_control_output(&mut self, control_output: f64) -> Result<()> {
        use anyhow::anyhow;
        use log::debug;

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
        let duty_clamped = control_output.clamp(-100.0, 100.0);
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

        self.i2c_driver
            .write(pwm_address, pwm_channel, &pwm_data)
            .await
            .map_err(|e| anyhow!("Failed to write PWM: {}", e))?;

        // Handle H-Bridge direction control
        let direction_controller = &self
            .regulator_config
            .actuators
            .thermal_control
            .direction_controller;

        let gpio_address = direction_controller.address;

        // Determine H-Bridge control signals
        let (in1, in2, enable) = if duty_clamped > 0.0 {
            // Positive (heating)
            (true, false, true)
        } else if duty_clamped < 0.0 {
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
        self.i2c_driver
            .write(gpio_address, 0x02, &[gpio_value])
            .await // 0x02 = output register
            .map_err(|e| anyhow!("Failed to write GPIO: {}", e))?;

        self.current_control_output = duty_clamped;
        Ok(())
    }

    fn get_current_control_output(&self) -> f64 {
        self.current_control_output
    }

    async fn initialize(&mut self) -> Result<()> {
        // Initialize hardware (mock implementation)
        log::info!("Initializing mock thermal regulation driver");
        Ok(())
    }

    async fn get_status(&mut self) -> Result<String> {
        let temp = self.i2c_driver.get_current_temperature()?;
        Ok(format!(
            "Mock Driver - Temperature: {:.2}°C, Control Output: {:.1}%",
            temp, self.current_control_output
        ))
    }
}

/// Native thermal regulation driver for Raspberry Pi
pub struct NativeThermalRegulationDriver {
    i2c_driver: drivers::native::NativeI2CDriver,
    regulator_config: crate::config::thermal_regulation::ThermalRegulatorConfig,
    current_control_output: f64,
}

impl NativeThermalRegulationDriver {
    /// Create a new native thermal regulation driver
    pub fn new(
        bus_config: &I2CBusConfig,
        regulator_config: &crate::config::thermal_regulation::ThermalRegulatorConfig,
    ) -> Result<Self> {
        let i2c_driver = drivers::native::NativeI2CDriver::new(&bus_config.device)?;

        Ok(Self {
            i2c_driver,
            regulator_config: regulator_config.clone(),
            current_control_output: 0.0,
        })
    }
}

#[async_trait::async_trait]
impl ThermalRegulationDriver for NativeThermalRegulationDriver {
    async fn read_temperature(&mut self) -> Result<f64> {
        use crate::utility::convert_voltage_to_temperature;
        use anyhow::anyhow;
        use log::debug;

        // Read from ADC
        let adc_address = self.regulator_config.temperature_sensor.adc_address;
        let adc_channel = self.regulator_config.temperature_sensor.adc_channel;

        let adc_data = self
            .i2c_driver
            .read(adc_address, adc_channel, 2)
            .await
            .map_err(|e| anyhow!("Failed to read ADC: {}", e))?;

        if adc_data.len() < 2 {
            return Err(anyhow!("Insufficient ADC data"));
        }

        let raw_value = ((adc_data[0] as u16) << 8) | (adc_data[1] as u16);
        debug!("ADC raw value: {} (0x{:04X})", raw_value, raw_value);

        let temp_conversion = &self.regulator_config.temperature_conversion;
        let adc_resolution = temp_conversion.adc_resolution;
        let voltage_reference = temp_conversion.voltage_reference;
        let formula = &temp_conversion.formula;

        let max_adc_value = (1_u32 << adc_resolution) - 1;
        let voltage = (raw_value as f64 / max_adc_value as f64) * voltage_reference as f64;

        let temperature_k = convert_voltage_to_temperature(formula.clone(), voltage as f32)?;
        let temperature_c = temperature_k - 273.15;

        debug!(
            "Temperature: {:.2}°C (formula: '{}')",
            temperature_c, formula
        );
        Ok(temperature_c)
    }

    async fn apply_control_output(&mut self, control_output: f64) -> Result<()> {
        use anyhow::anyhow;
        use log::debug;

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

        let duty_clamped = control_output.clamp(-100.0, 100.0);
        let pwm_value = ((duty_clamped.abs() / 100.0) * 4095.0) as u16;

        let pwm_data = [(pwm_value & 0xFF) as u8, ((pwm_value >> 8) & 0xFF) as u8];

        debug!(
            "Native PWM write: address=0x{:02X}, channel={}, duty={:.1}%",
            pwm_address, pwm_channel, duty_clamped
        );

        self.i2c_driver
            .write(pwm_address, pwm_channel, &pwm_data)
            .await
            .map_err(|e| anyhow!("Failed to write PWM: {}", e))?;

        // Handle H-Bridge direction control
        let direction_controller = &self
            .regulator_config
            .actuators
            .thermal_control
            .direction_controller;

        let gpio_address = direction_controller.address;
        let (in1, in2, enable) = if duty_clamped > 0.0 {
            (true, false, true)
        } else if duty_clamped < 0.0 {
            (false, true, true)
        } else {
            (false, false, false)
        };

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

        self.i2c_driver
            .write(gpio_address, 0x02, &[gpio_value])
            .await
            .map_err(|e| anyhow!("Failed to write GPIO: {}", e))?;

        self.current_control_output = duty_clamped;
        Ok(())
    }

    fn get_current_control_output(&self) -> f64 {
        self.current_control_output
    }

    async fn initialize(&mut self) -> Result<()> {
        log::info!("Initializing native thermal regulation driver");
        // Perform hardware initialization if needed
        Ok(())
    }

    async fn get_status(&mut self) -> Result<String> {
        Ok(format!(
            "Native Driver - Control Output: {:.1}%",
            self.current_control_output
        ))
    }
}

/// CP2112 thermal regulation driver for USB-based I2C
pub struct Cp2112ThermalRegulationDriver {
    i2c_driver: drivers::cp2112::Cp2112Driver,
    regulator_config: crate::config::thermal_regulation::ThermalRegulatorConfig,
    current_control_output: f64,
}

impl Cp2112ThermalRegulationDriver {
    /// Create a new CP2112 thermal regulation driver
    pub fn new(
        bus_config: &I2CBusConfig,
        regulator_config: &crate::config::thermal_regulation::ThermalRegulatorConfig,
    ) -> Result<Self> {
        let i2c_driver = drivers::cp2112::Cp2112Driver::new(
            bus_config.usb_vendor_id.unwrap_or(0x10c4),
            bus_config.usb_product_id.unwrap_or(0xea90),
        )?;

        Ok(Self {
            i2c_driver,
            regulator_config: regulator_config.clone(),
            current_control_output: 0.0,
        })
    }
}

#[async_trait::async_trait]
impl ThermalRegulationDriver for Cp2112ThermalRegulationDriver {
    async fn read_temperature(&mut self) -> Result<f64> {
        use crate::utility::convert_voltage_to_temperature;
        use anyhow::anyhow;
        use log::debug;

        let adc_address = self.regulator_config.temperature_sensor.adc_address;
        let adc_channel = self.regulator_config.temperature_sensor.adc_channel;

        let adc_data = self
            .i2c_driver
            .read(adc_address, adc_channel, 2)
            .await
            .map_err(|e| anyhow!("Failed to read ADC: {}", e))?;

        if adc_data.len() < 2 {
            return Err(anyhow!("Insufficient ADC data"));
        }

        let raw_value = ((adc_data[0] as u16) << 8) | (adc_data[1] as u16);
        debug!("ADC raw value: {} (0x{:04X})", raw_value, raw_value);

        let temp_conversion = &self.regulator_config.temperature_conversion;
        let adc_resolution = temp_conversion.adc_resolution;
        let voltage_reference = temp_conversion.voltage_reference;
        let formula = &temp_conversion.formula;

        let max_adc_value = (1_u32 << adc_resolution) - 1;
        let voltage = (raw_value as f64 / max_adc_value as f64) * voltage_reference as f64;

        let temperature_k = convert_voltage_to_temperature(formula.clone(), voltage as f32)?;
        let temperature_c = temperature_k - 273.15;

        debug!(
            "Temperature: {:.2}°C (formula: '{}')",
            temperature_c, formula
        );
        Ok(temperature_c)
    }

    async fn apply_control_output(&mut self, control_output: f64) -> Result<()> {
        use anyhow::anyhow;
        use log::debug;

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

        let duty_clamped = control_output.clamp(-100.0, 100.0);
        let pwm_value = ((duty_clamped.abs() / 100.0) * 4095.0) as u16;

        let pwm_data = [(pwm_value & 0xFF) as u8, ((pwm_value >> 8) & 0xFF) as u8];

        debug!(
            "CP2112 PWM write: address=0x{:02X}, channel={}, duty={:.1}%",
            pwm_address, pwm_channel, duty_clamped
        );

        self.i2c_driver
            .write(pwm_address, pwm_channel, &pwm_data)
            .await
            .map_err(|e| anyhow!("Failed to write PWM: {}", e))?;

        // Handle H-Bridge direction control
        let direction_controller = &self
            .regulator_config
            .actuators
            .thermal_control
            .direction_controller;

        let gpio_address = direction_controller.address;
        let (in1, in2, enable) = if duty_clamped > 0.0 {
            (true, false, true)
        } else if duty_clamped < 0.0 {
            (false, true, true)
        } else {
            (false, false, false)
        };

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

        self.i2c_driver
            .write(gpio_address, 0x02, &[gpio_value])
            .await
            .map_err(|e| anyhow!("Failed to write GPIO: {}", e))?;

        self.current_control_output = duty_clamped;
        Ok(())
    }

    fn get_current_control_output(&self) -> f64 {
        self.current_control_output
    }

    async fn initialize(&mut self) -> Result<()> {
        log::info!("Initializing CP2112 thermal regulation driver");
        Ok(())
    }

    async fn get_status(&mut self) -> Result<String> {
        Ok(format!(
            "CP2112 Driver - Control Output: {:.1}%",
            self.current_control_output
        ))
    }
}

/// Factory function to create appropriate thermal regulation driver
pub fn create_thermal_regulation_driver(
    bus_config: &I2CBusConfig,
    regulator_config: &crate::config::thermal_regulation::ThermalRegulatorConfig,
) -> Result<Box<dyn ThermalRegulationDriver + Send + Sync>> {
    // Add debug logging to see what driver type we're creating
    log::info!(
        "Creating thermal regulation driver with bus type: {:?}, ADC address: 0x{:02x}",
        bus_config.bus_type,
        regulator_config.temperature_sensor.adc_address
    );

    match bus_config.bus_type {
        I2CBusType::Native => {
            log::info!("Using native thermal regulation driver");
            Ok(Box::new(NativeThermalRegulationDriver::new(
                bus_config,
                regulator_config,
            )?))
        }
        I2CBusType::Cp2112 => {
            log::info!("Using CP2112 thermal regulation driver");
            Ok(Box::new(Cp2112ThermalRegulationDriver::new(
                bus_config,
                regulator_config,
            )?))
        }
        I2CBusType::Mock => {
            log::info!("Using mock thermal regulation driver");
            Ok(Box::new(MockThermalRegulationDriver::new(
                bus_config,
                regulator_config,
            )?))
        }
    }
}
