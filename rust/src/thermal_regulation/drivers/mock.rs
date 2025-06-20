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

const AMBIENT_ROOM_TEMP_C: f64 = 25.0; // Ambient room temperature in Celsius

// Physical dimensions and properties of the photoacoustic cell
/// Cell mass in grams (stainless steel 316, 110x30x60mm)
const CELL_MASS_G: f64 = 1016.0;
/// Cell length in mm
const CELL_LENGTH_MM: f64 = 110.0;
/// Cell width in mm  
const CELL_WIDTH_MM: f64 = 30.0;
/// Cell height in mm
const CELL_HEIGHT_MM: f64 = 60.0;
/// Peltier module length in mm
const PELTIER_LENGTH_MM: f64 = 15.0;
/// Peltier module width in mm
const PELTIER_WIDTH_MM: f64 = 30.0;
/// Peltier module maximum power in Watts (typical for 15x30mm Peltier)
const PELTIER_MAX_POWER_W: f64 = 32.0;
/// Heating resistor maximum power in Watts (DBK HPG-1/10-60x35-12-24V)
const HEATER_MAX_POWER_W: f64 = 60.0;

/// Mock I2C driver for thermal regulation simulation with L298N H-Bridge control
///
/// # Reference Implementation for Real Hardware Drivers
///
/// This mock driver serves as a comprehensive reference implementation for developing
/// real hardware drivers for thermal regulation systems using L298N H-Bridge motor
/// drivers for Peltier module control. The architecture demonstrates:
///
/// ## Key Design Patterns:
/// - **Multi-device management**: Handles multiple I2C devices (ADC, PWM, GPIO controllers)
/// - **Hardware abstraction**: Separates logical operations from physical I2C transactions
/// - **State tracking**: Maintains H-Bridge direction and PWM state for proper thermal control
/// - **Thread-safe operations**: Uses Arc<Mutex<>> for concurrent access to shared state
///
/// ## Real Hardware Mapping:
/// - **Temperature Sensor**: MCP9808 or ADS1115 ADC with NTC thermistor
/// - **PWM Controller**: PCA9685 for H-Bridge enable (ENA/ENB) power control
/// - **GPIO Controller**: CAT9555 for H-Bridge direction (IN1/IN2/IN3/IN4) control
/// - **Thermal Actuator**: L298N H-Bridge driving Peltier modules and heating resistors
///
/// ## For Real Hardware Implementation:
/// 1. Replace `ThermalCellSimulation` with actual temperature readings
/// 2. Replace mock I2C transactions with real hardware communication
/// 3. Keep the H-Bridge state management logic intact
/// 4. Maintain the separation between direction control (GPIO) and power control (PWM)
///
/// ## Critical Implementation Notes:
/// - **Direction before Power**: Always set H-Bridge direction via GPIO before applying PWM
/// - **State Consistency**: Track H-Bridge state to ensure proper heating/cooling operation
/// - **Error Handling**: Implement robust error handling for I2C communication failures
/// - **Thread Safety**: Maintain thread-safe access patterns for concurrent thermal control
pub struct MockI2CL298NDriver {
    /// Collection of emulated I2C devices mapped by their bus addresses
    /// In real hardware: Replace with actual I2C bus communication handles
    devices: Arc<Mutex<HashMap<u8, MockDevice>>>,

    /// Thermal physics simulation replacing real temperature sensor readings
    /// In real hardware: Remove this and implement actual sensor reading logic
    thermal_simulation: Arc<Mutex<ThermalCellSimulation>>,

    /// Current H-Bridge direction and PWM state tracking
    /// In real hardware: KEEP THIS - essential for proper L298N control coordination
    h_bridge_state: Arc<Mutex<HBridgeState>>,

    /// Driver initialization timestamp for debugging and diagnostics
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
        // Calculate surface area in m²
        let surface_area_m2 = 2.0
            * ((CELL_LENGTH_MM * CELL_WIDTH_MM)
                + (CELL_LENGTH_MM * CELL_HEIGHT_MM)
                + (CELL_WIDTH_MM * CELL_HEIGHT_MM))
            / 1_000_000.0; // Convert mm² to m²

        Self {
            mass_g: CELL_MASS_G,
            dimensions_mm: (CELL_LENGTH_MM, CELL_WIDTH_MM, CELL_HEIGHT_MM),
            specific_heat: 501.0, // J/kg·K for stainless steel 316 (updated from 0.5 J/g·K)
            thermal_conductivity: 16.2, // W/m·K for stainless steel 316
            surface_area_m2,
            heat_transfer_coefficient: 25.0, // W/m²·K (increased for better heat dissipation)
            peltier_max_power: PELTIER_MAX_POWER_W,
            peltier_dimensions_mm: (PELTIER_LENGTH_MM, PELTIER_WIDTH_MM),
            heater_max_power: HEATER_MAX_POWER_W,
            thermal_time_constant: 90.0, // seconds (reduced for faster response with 60W heater)
        }
    }
}

impl MockI2CL298NDriver {
    /// Create a new mock I2C driver for L298N thermal regulation simulation
    ///
    /// # Real Hardware Implementation Reference
    ///
    /// This constructor demonstrates the complete initialization pattern for
    /// thermal regulation drivers using L298N H-Bridge controllers. The same
    /// structure and initialization sequence should be used for real hardware.
    ///
    /// ## Initialization Sequence:
    /// 1. **Device Discovery**: Scan configured I2C addresses for hardware devices
    /// 2. **Device Registration**: Create device handles for each detected controller
    /// 3. **State Initialization**: Initialize H-Bridge state to safe defaults
    /// 4. **Thermal Simulation**: Initialize thermal physics simulation (mock only)
    ///
    /// ## Hardware Device Types:
    /// - **Temperature Sensors (MCP9808)**: Direct temperature reading devices
    /// - **ADC Controllers (ADS1115)**: For thermistor-based temperature sensors
    /// - **PWM Controllers (PCA9685)**: For H-Bridge ENA/ENB power control
    /// - **GPIO Controllers (CAT9555)**: For H-Bridge IN1/IN2/IN3/IN4 direction control
    ///
    /// ## Real Hardware Adaptation:
    /// 1. **Replace MockDevice**: Use actual hardware device handles
    /// 2. **Add Device Detection**: Implement I2C device presence verification
    /// 3. **Hardware Initialization**: Add device-specific initialization sequences
    /// 4. **Remove Thermal Simulation**: Replace with actual sensor reading logic
    /// 5. **Add Safety Systems**: Implement emergency shutdown and fault detection
    ///
    /// ## Critical Implementation Notes:
    /// - **Thread Safety**: All shared state uses Arc<Mutex<>> for concurrent access
    /// - **Configuration Driven**: All hardware addresses from configuration file
    /// - **State Tracking**: H-Bridge state initialized to safe defaults (disabled)
    /// - **Error Handling**: Propagate initialization errors to caller
    ///
    /// ## Safety Considerations for Real Hardware:
    /// - Verify all configured devices are present before enabling thermal control
    /// - Initialize all H-Bridge outputs to safe state (disabled) before operations
    /// - Implement hardware watchdog for thermal runaway protection
    /// - Add power supply monitoring and undervoltage protection
    /// - Validate thermal actuator specifications against control parameters
    pub fn new(config: &I2CBusConfig) -> Result<Self> {
        let mut devices = HashMap::new();

        // Add configured temperature sensors (MCP9808) - Direct temperature reading
        // Real hardware: Initialize actual MCP9808 devices with proper configuration
        for controller in &config.adc_controllers {
            devices.insert(
                controller.address,
                MockDevice::new(controller.address, MockDeviceType::TemperatureSensor),
            );
        }

        // Add configured ADC controllers (ADS1115) - For NTC thermistor temperature sensing
        // Real hardware: Configure ADC resolution, sampling rate, and reference voltage
        for controller in &config.adc_controllers {
            devices.insert(
                controller.address,
                MockDevice::new(controller.address, MockDeviceType::AdcController),
            );
        }

        // Add configured PWM controllers (PCA9685) - For H-Bridge ENA/ENB power control
        // Real hardware: Set PWM frequency to match thermal actuator specifications
        for controller in &config.pwm_controllers {
            devices.insert(
                controller.address,
                MockDevice::new(controller.address, MockDeviceType::PwmController),
            );
        }

        // Add configured GPIO controllers (CAT9555) - For H-Bridge IN1/IN2/IN3/IN4 direction control
        // Real hardware: Configure GPIO pins as outputs with proper drive strength
        for controller in &config.gpio_controllers {
            devices.insert(
                controller.address,
                MockDevice::new(controller.address, MockDeviceType::GpioController),
            );
        }

        // Initialize thermal simulation (mock only - remove for real hardware)
        let thermal_simulation = ThermalCellSimulation::new();

        Ok(Self {
            devices: Arc::new(Mutex::new(devices)),
            thermal_simulation: Arc::new(Mutex::new(thermal_simulation)),
            h_bridge_state: Arc::new(Mutex::new(HBridgeState::default())), // Safe initial state
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
impl I2CBusDriver for MockI2CL298NDriver {
    /// Read data from I2C device with thermal simulation integration
    ///
    /// # Real Hardware Implementation Reference
    ///
    /// This method demonstrates the complete I2C read pattern for thermal regulation
    /// systems. The pre-read thermal simulation update ensures temperature readings
    /// reflect current thermal actuator state.
    ///
    /// ## Read Sequence:
    /// 1. **Update Thermal State**: Ensure thermal simulation reflects current actuator state
    /// 2. **Device Lookup**: Verify device exists at specified I2C address
    /// 3. **Device-Specific Read**: Route to appropriate device handler
    /// 4. **Return Data**: Provide formatted data matching real hardware behavior
    ///
    /// ## Device-Specific Handlers:
    /// - **Temperature Sensor**: Direct temperature reading from MCP9808
    /// - **ADC Controller**: Thermistor voltage conversion for temperature
    /// - **PWM Controller**: PWM configuration and status registers
    /// - **GPIO Controller**: H-Bridge direction control status
    ///
    /// ## Real Hardware Adaptation:
    /// - Replace thermal simulation update with actual sensor reading
    /// - Replace device lookup with real I2C device communication
    /// - Keep device-specific read logic for different controller types
    /// - Add I2C communication error handling and retry logic
    ///
    /// ## Critical Implementation Notes:
    /// - **Thermal Consistency**: Update thermal state before temperature readings
    /// - **Device Abstraction**: Maintain device-specific read behavior
    /// - **Error Propagation**: Propagate I2C errors to caller for proper handling
    /// - **Data Format**: Match real hardware data formats and endianness
    async fn read(&mut self, address: u8, register: u8, length: usize) -> Result<Vec<u8>> {
        // Update thermal simulation before reading (remove for real hardware)
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

    /// Write data to I2C device with thermal control integration
    ///
    /// # Real Hardware Implementation Reference
    ///
    /// This method demonstrates the complete I2C write pattern for thermal regulation
    /// systems. All writes are logged for debugging and routed to device-specific
    /// handlers that coordinate H-Bridge state and thermal control.
    ///
    /// ## Write Sequence:
    /// 1. **Device Lookup**: Verify device exists at specified I2C address
    /// 2. **Write Logging**: Log all I2C transactions for debugging and verification
    /// 3. **Device-Specific Write**: Route to appropriate device write handler
    /// 4. **State Coordination**: Update H-Bridge state and apply thermal effects
    ///
    /// ## Device-Specific Handlers:
    /// - **Temperature Sensor**: Configuration and calibration writes
    /// - **ADC Controller**: ADC configuration and channel selection
    /// - **PWM Controller**: H-Bridge ENA/ENB power control (CRITICAL)
    /// - **GPIO Controller**: H-Bridge IN1/IN2/IN3/IN4 direction control (CRITICAL)
    ///
    /// ## Real Hardware Adaptation:
    /// - Replace device lookup with real I2C device communication
    /// - Keep comprehensive write logging for thermal system debugging
    /// - Maintain device-specific write handlers and state coordination
    /// - Add I2C write verification and retry logic
    ///
    /// ## Critical Implementation Notes:
    /// - **Write Logging**: All thermal control writes must be logged for debugging
    /// - **Device Routing**: Maintain strict device-type-specific write routing
    /// - **State Coordination**: GPIO and PWM writes must update H-Bridge state
    /// - **Error Handling**: Propagate write failures to caller for safety
    ///
    /// ## Safety Considerations for Real Hardware:
    /// - Verify all thermal control writes complete successfully
    /// - Implement write verification readback for critical control registers
    /// - Add thermal actuator monitoring after control writes
    /// - Implement emergency shutdown on repeated write failures
    async fn write(&mut self, address: u8, register: u8, data: &[u8]) -> Result<()> {
        let mut devices = self
            .devices
            .lock()
            .map_err(|_| anyhow!("Failed to lock devices"))?;

        let device = devices
            .get_mut(&address)
            .ok_or_else(|| anyhow!("Device not found at address 0x{:02X}", address))?;

        // Debug output for all writes - ESSENTIAL for thermal system debugging
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

    /// Check if I2C device is present on the bus
    ///
    /// # Real Hardware Implementation Reference
    ///
    /// This method demonstrates device presence detection for thermal regulation
    /// systems. Proper device detection is critical for safe thermal control
    /// initialization and fault detection.
    ///
    /// ## Device Detection Strategy:
    /// 1. **Address Verification**: Check if device responds at specified I2C address
    /// 2. **Configuration Validation**: Verify device is properly configured
    /// 3. **Capability Check**: Ensure device supports required thermal control features
    ///
    /// ## Real Hardware Adaptation:
    /// - Replace device lookup with actual I2C device ping/detection
    /// - Add device identification register reading for verification
    /// - Implement device-specific initialization status checking
    /// - Add retry logic for intermittent device communication issues
    ///
    /// ## Critical Implementation Notes:
    /// - **Initialization Safety**: Never initialize thermal control with missing devices
    /// - **Fault Detection**: Use for ongoing device health monitoring
    /// - **Error Recovery**: Implement device re-initialization on detection failure
    /// - **Configuration Validation**: Verify device configuration matches expectations
    ///
    /// ## Safety Considerations for Real Hardware:
    /// - Disable thermal control immediately if critical devices are missing
    /// - Implement device watchdog monitoring during thermal operations
    /// - Add device redundancy checking for safety-critical thermal systems
    /// - Log all device presence changes for thermal system diagnostics
    async fn device_present(&mut self, address: u8) -> Result<bool> {
        let devices = self
            .devices
            .lock()
            .map_err(|_| anyhow!("Failed to lock devices"))?;
        Ok(devices.contains_key(&address))
    }
}

impl MockI2CL298NDriver {
    /// Read from temperature sensor (MCP9808) - Direct Temperature Reading
    ///
    /// # Real Hardware Implementation Reference
    ///
    /// This method demonstrates reading from MCP9808 digital temperature sensors
    /// commonly used in precision thermal regulation systems. The MCP9808 provides
    /// high-accuracy temperature readings with 0.0625°C resolution.
    ///
    /// ## MCP9808 Register Map:
    /// - **0x05**: Temperature register (16-bit, signed, 0.0625°C resolution)
    /// - **0x06**: Configuration register (operating mode, alert settings)
    /// - **0x07**: Alert temperature upper boundary register
    /// - **0x08**: Alert temperature lower boundary register
    ///
    /// ## Temperature Data Format:
    /// - **16-bit signed integer**: Temperature × 16 (0.0625°C resolution)
    /// - **Endianness**: Big-endian (MSB first)
    /// - **Range**: -40°C to +125°C
    /// - **Accuracy**: ±0.25°C (typical)
    ///
    /// ## Real Hardware Adaptation:
    /// - Replace thermal simulation with actual MCP9808 I2C communication
    /// - Keep exact same register mapping and data format conversion
    /// - Add temperature sensor fault detection and error handling
    /// - Implement sensor calibration offset correction if needed
    ///
    /// ## Critical Implementation Notes:
    /// - **Data Format**: Maintain exact MCP9808 16-bit signed format
    /// - **Resolution**: 0.0625°C resolution must be preserved for PID accuracy
    /// - **Error Handling**: Invalid register access must return proper errors
    /// - **Thread Safety**: Temperature reading must be atomic for PID stability
    fn read_temperature_sensor(&self, register: u8, length: usize) -> Result<Vec<u8>> {
        match register {
            0x05 => {
                // Temperature register - PRIMARY temperature reading for thermal control
                let simulation = self
                    .thermal_simulation
                    .lock()
                    .map_err(|_| anyhow!("Failed to lock thermal simulation"))?;

                // Convert temperature to MCP9808 format (16-bit, 0.0625°C resolution)
                let temp_c = simulation.temperature;
                // MCP9808 uses 16-bit signed format: temp = register_value / 16.0
                let temp_raw = (temp_c * 16.0) as i16;

                // Return as big-endian bytes (MSB first)
                Ok(vec![(temp_raw >> 8) as u8, (temp_raw & 0xFF) as u8])
            }
            0x06 => {
                // Configuration register - Sensor operating mode and alert settings
                Ok(vec![0x00, 0x00]) // Default configuration (continuous conversion)
            }
            0x07 => {
                // Alert temperature upper boundary - For thermal runaway protection
                Ok(vec![0x00, 0x00]) // Default: no alert threshold set
            }
            _ => Err(anyhow!(
                "Unsupported register 0x{:02X} for temperature sensor",
                register
            )),
        }
    }

    /// Write to temperature sensor (MCP9808) - Configuration and Calibration
    ///
    /// # Real Hardware Implementation Reference
    ///
    /// This method demonstrates writing configuration data to MCP9808 temperature
    /// sensors for thermal regulation applications. Configuration writes are used
    /// to set operating mode, alert thresholds, and calibration parameters.
    ///
    /// ## MCP9808 Writable Registers:
    /// - **0x01**: Configuration register (operating mode, shutdown, alerts)
    /// - **0x02**: Alert temperature upper boundary register
    /// - **0x03**: Alert temperature lower boundary register
    /// - **0x04**: Critical temperature register
    ///
    /// ## Configuration Register (0x01) Bits:
    /// - **Bit 8**: Shutdown mode (0 = continuous conversion, 1 = shutdown)
    /// - **Bit 7**: Critical temperature lock
    /// - **Bit 6**: Temperature upper/lower boundary lock
    /// - **Bit 5-3**: Alert output configuration
    /// - **Bit 2-0**: Alert temperature hysteresis
    ///
    /// ## Real Hardware Adaptation:
    /// - Replace mock implementation with actual MCP9808 register writes
    /// - Add configuration validation and readback verification
    /// - Implement alert threshold configuration for thermal protection
    /// - Add sensor calibration offset writing if supported
    ///
    /// ## Critical Implementation Notes:
    /// - **Configuration Validation**: Validate configuration parameters before writing
    /// - **Write Verification**: Read back configuration to ensure proper setting
    /// - **Alert Configuration**: Set appropriate thermal alert thresholds
    /// - **Error Handling**: Report configuration write failures to caller
    fn write_temperature_sensor(&self, register: u8, data: &[u8]) -> Result<()> {
        match register {
            0x01 => {
                // Configuration register - Operating mode and alert configuration
                // Accept configuration writes but don't do anything in mock
                // Real hardware: Validate configuration data and write to MCP9808
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

    /// Write to PWM controller (PCA9685) - H-Bridge Power Control
    ///
    /// # Real Hardware Implementation Reference
    ///
    /// This method implements the critical PWM control pattern for L298N H-Bridge
    /// power management in thermal regulation systems. The PWM duty cycle controls
    /// the effective power delivered to thermal actuators (Peltier modules, heating resistors).
    ///
    /// ## Hardware Mapping (PCA9685 PWM to L298N H-Bridge):
    /// ```text
    /// PWM Channel 0 (register 0x06) -> H-Bridge #1 ENA (primary thermal control)
    /// PWM Channel 1 (register 0x0A) -> H-Bridge #2 ENB (secondary thermal control)
    /// ```
    ///
    /// ## PWM Control Logic:
    /// - **0% Duty Cycle**: No power, thermal actuator disabled
    /// - **1-100% Duty Cycle**: Proportional power control for thermal actuator
    /// - **Direction**: Power level only, direction controlled separately via GPIO
    ///
    /// ## Critical Implementation Notes:
    /// 1. **Power After Direction**: Only apply PWM after GPIO direction is set
    /// 2. **State Coordination**: Update H-Bridge state and apply thermal effects immediately
    /// 3. **Multiple Formats**: Support different PWM data formats (2-byte, 4-byte)
    /// 4. **Channel Mapping**: Channel 0 for primary, Channel 1 for secondary control
    ///
    /// ## Real Hardware Adaptation:
    /// - Replace thermal simulation with actual actuator control verification
    /// - Keep the exact same PWM calculation and channel mapping logic
    /// - Maintain the state update and thermal power application sequence
    /// - Add power level validation and safety limits
    ///
    /// ## PCA9685 Register Format:
    /// - **2-byte mode**: Direct PWM value (0-4095)
    /// - **4-byte mode**: ON_time + OFF_time for precise timing control
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

                // Channel 0 (register 0x06) controls H-Bridge 1 ENA (primary thermal control)
                if register == 0x06 {
                    // Channel 0 - H-Bridge 1 ENA (primary thermal actuator)
                    debug!(
                        "PWM channel 0 (H-Bridge 1 ENA) set to {:.1}% duty cycle",
                        duty_cycle
                    );

                    // Update H-Bridge state with new duty cycle and apply thermal power
                    if let Ok(mut state) = self.h_bridge_state.lock() {
                        state.h1_duty_cycle = duty_cycle;
                        // Apply thermal power based on direction and duty cycle
                        let _ = self.apply_thermal_power_based_on_state(&state);
                    }

                    // Force thermal simulation update
                    let _ = self.update_thermal_simulation();
                }
                // Channel 1 (register 0x0A) controls H-Bridge 2 ENB (secondary thermal control)
                else if register == 0x0A {
                    // Channel 1 - H-Bridge 2 ENB (secondary thermal actuator for future expansion)
                    debug!(
                        "PWM channel 1 (H-Bridge 2 ENB) set to {:.1}% duty cycle",
                        duty_cycle
                    );

                    // Update H-Bridge 2 state (future expansion)
                    if let Ok(mut state) = self.h_bridge_state.lock() {
                        state.h2_duty_cycle = duty_cycle;
                        // Note: Secondary thermal control not yet implemented in simulation
                        // Future: apply_h2_thermal_power_based_on_state(&state);
                    }
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

    /// Write to GPIO controller (CAT9555) - H-Bridge Direction Control
    ///
    /// # Real Hardware Implementation Reference
    ///
    /// This method demonstrates the critical GPIO control pattern for L298N H-Bridge
    /// direction management in thermal regulation systems. This logic should be
    /// preserved exactly in real hardware implementations.
    ///
    /// ## Hardware Mapping (CAT9555 GPIO to L298N H-Bridge):
    /// ```text
    /// GPIO 0 (bit 0) -> H-Bridge #1 IN1 (primary thermal control)
    /// GPIO 1 (bit 1) -> H-Bridge #1 IN2 (primary thermal control)  
    /// GPIO 2 (bit 2) -> H-Bridge #2 IN3 (secondary thermal control)
    /// GPIO 3 (bit 3) -> H-Bridge #2 IN4 (secondary thermal control)
    /// ```
    ///
    /// ## L298N Control Logic:
    /// - **Heating Mode**: IN1=HIGH, IN2=LOW (Forward direction)
    /// - **Cooling Mode**: IN1=LOW, IN2=HIGH (Reverse direction)
    /// - **Coast Mode**: IN1=LOW, IN2=LOW (Disabled, high impedance)
    /// - **Brake Mode**: IN1=HIGH, IN2=HIGH (Disabled, active braking)
    ///
    /// ## Critical Implementation Notes:
    /// 1. **Direction First**: Always set direction before applying PWM power
    /// 2. **State Tracking**: Update H-Bridge state immediately after GPIO changes
    /// 3. **Safety**: Never leave direction in undefined state during transitions
    /// 4. **Error Handling**: Validate GPIO operations complete successfully
    ///
    /// ## Real Hardware Adaptation:
    /// - Replace mock thermal simulation with actual hardware verification
    /// - Keep the exact same GPIO bit mapping and direction logic
    /// - Maintain the state update and thermal power application sequence
    /// - Add hardware-specific error recovery mechanisms
    fn write_gpio_controller(&self, register: u8, data: &[u8]) -> Result<()> {
        match register {
            0x02 | 0x03 => {
                // Output port registers - H-Bridge control
                if data.len() > 0 {
                    let gpio_value = data[0];
                    debug!(
                        "GPIO controller write - register: 0x{:02X}, value: 0x{:02X}",
                        register, gpio_value
                    );

                    // Decode H-Bridge signals based on GPIO pin assignment:
                    // GPIO 0 (bit 0) = H-Bridge 1 IN1
                    // GPIO 1 (bit 1) = H-Bridge 1 IN2
                    // GPIO 2 (bit 2) = H-Bridge 2 IN3
                    // GPIO 3 (bit 3) = H-Bridge 2 IN4
                    // Note: ENA (H-Bridge 1) controlled by PWM Channel 0 (PCA9685)
                    // Note: ENB (H-Bridge 2) controlled by PWM Channel 1 (PCA9685)

                    let h1_in1 = (gpio_value & 0x01) != 0; // GPIO 0 - H-Bridge #1 IN1
                    let h1_in2 = (gpio_value & 0x02) != 0; // GPIO 1 - H-Bridge #1 IN2
                    let h2_in3 = (gpio_value & 0x04) != 0; // GPIO 2 - H-Bridge #2 IN3
                    let h2_in4 = (gpio_value & 0x08) != 0; // GPIO 3 - H-Bridge #2 IN4
                                                           // Note: ENA (PWM0) and ENB (PWM1) are controlled by PCA9685, not GPIO

                    debug!(
                        "H-Bridge 1: IN1={}, IN2={} | H-Bridge 2: IN3={}, IN4={} (ENA/ENB controlled by PWM)",
                        h1_in1, h1_in2, h2_in3, h2_in4
                    );

                    // Update thermal simulation based on H-Bridge 1 direction (primary thermal control)
                    // Note: ENA power level is controlled by PWM channel 0
                    let h1_direction = if h1_in1 && !h1_in2 {
                        debug!("H-Bridge 1: Forward direction (heating mode) - power controlled by PWM ENA");
                        HBridgeDirection::Forward
                    } else if !h1_in1 && h1_in2 {
                        debug!("H-Bridge 1: Reverse direction (cooling mode) - power controlled by PWM ENA");
                        HBridgeDirection::Reverse
                    } else if h1_in1 && h1_in2 {
                        debug!("H-Bridge 1: Brake state - both inputs high");
                        HBridgeDirection::Disabled
                    } else {
                        debug!("H-Bridge 1: Coast state - both inputs low");
                        HBridgeDirection::Disabled
                    };

                    // Update H-Bridge state
                    if let Ok(mut state) = self.h_bridge_state.lock() {
                        state.h1_direction = h1_direction;
                        // Apply current thermal power based on direction and duty cycle
                        self.apply_thermal_power_based_on_state(&mut state)?;
                    }

                    // H-Bridge 2 control for future expansion (secondary thermal control)
                    if h2_in3 || h2_in4 {
                        debug!("H-Bridge 2: Direction signals detected (future expansion)");
                    }
                }
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
            temperature: AMBIENT_ROOM_TEMP_C, // Start at room temperature
            target_temperature: 41.0,
            peltier_power: 0.0,
            heater_power: 0.0,
            ambient_temperature: AMBIENT_ROOM_TEMP_C,
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

/// H-Bridge direction control for L298N thermal regulation systems
///
/// # Real Hardware Implementation Reference
///
/// This enum provides the essential control logic for L298N H-Bridge direction
/// control in thermal regulation applications. Each direction maps directly to
/// specific GPIO pin combinations and thermal control modes.
///
/// ## L298N H-Bridge Control Logic:
/// The L298N H-Bridge requires two direction inputs (IN1/IN2) and one enable input (ENA)
/// for each motor channel. In thermal regulation applications:
/// - **Direction**: Controlled by GPIO pins (CAT9555 controller)
/// - **Power**: Controlled by PWM duty cycle (PCA9685 controller)
///
/// ## Direction to Thermal Mode Mapping:
///
/// ### Forward Direction (Heating Mode):
/// - **GPIO Control**: IN1=HIGH, IN2=LOW
/// - **Thermal Effect**: Resistive heating or Peltier heating
/// - **Use Case**: Raising temperature above ambient
/// - **Power Source**: Heating resistor (DBK HPG-1/10) or Peltier forward bias
///
/// ### Reverse Direction (Cooling Mode):
/// - **GPIO Control**: IN1=LOW, IN2=HIGH  
/// - **Thermal Effect**: Peltier cooling (heat pump mode)
/// - **Use Case**: Lowering temperature below ambient
/// - **Power Source**: Peltier reverse bias for thermoelectric cooling
///
/// ### Disabled State (Coast Mode):
/// - **GPIO Control**: IN1=LOW, IN2=LOW
/// - **Thermal Effect**: No active thermal control
/// - **Use Case**: Thermal system idle, natural equilibrium
/// - **Power Source**: All thermal actuators disabled
///
/// ## Critical Safety Considerations:
///
/// ### NEVER USE BRAKE MODE (IN1=HIGH, IN2=HIGH):
/// - **Hardware Risk**: Creates short circuit in L298N H-Bridge
/// - **Thermal Risk**: Can damage thermal actuators and power supply
/// - **Safety Rule**: Always use coast mode (both LOW) for disable state
///
/// ## Real Hardware Implementation Notes:
/// - **Direction Before Power**: Always set direction before applying PWM
/// - **State Consistency**: Keep this enum synchronized with actual GPIO state
/// - **Error Recovery**: Use Disabled state for safe error recovery
/// - **Thread Safety**: Direction changes must be atomic with power changes
///
/// ## Thermal Control Strategy:
/// - **Heating**: Use Forward direction with appropriate PWM duty cycle
/// - **Cooling**: Use Reverse direction with appropriate PWM duty cycle
/// - **Hold**: Use previous direction with minimal PWM to maintain temperature
/// - **Emergency**: Use Disabled state to immediately stop all thermal control
#[derive(Debug, Clone, Copy)]
pub enum HBridgeDirection {
    /// Forward direction (heating mode for Peltier, or resistive heating)
    /// L298N Control: IN1=HIGH, IN2=LOW
    Forward,
    /// Reverse direction (cooling mode for Peltier)
    /// L298N Control: IN1=LOW, IN2=HIGH
    Reverse,
    /// Disabled (no thermal control)
    /// L298N Control: IN1=LOW, IN2=LOW (coast mode)
    Disabled,
}

/// H-Bridge state tracking for L298N thermal control coordination
///
/// # Real Hardware Implementation Guide:
/// This structure is ESSENTIAL for real hardware implementations as it ensures
/// proper coordination between GPIO direction control and PWM power control.
///
/// ## Hardware Control Sequence:
/// 1. **Set Direction First**: Configure H-Bridge direction via GPIO (IN1/IN2)
/// 2. **Apply Power Second**: Set PWM duty cycle for enable pins (ENA/ENB)
/// 3. **Update State**: Keep this structure synchronized with hardware state
///
/// ## Threading Safety:
/// This structure is accessed via Arc<Mutex<>> to ensure thread-safe operations
/// when multiple thermal regulation threads are controlling different H-Bridges.
///
/// ## Dual H-Bridge Support:
/// Designed to support two independent L298N H-Bridges for complex thermal systems:
/// - **H-Bridge 1**: Primary thermal control (Peltier + heating resistor)
/// - **H-Bridge 2**: Secondary thermal control (future expansion)
#[derive(Debug)]
pub struct HBridgeState {
    /// H-Bridge 1 direction (primary thermal control)
    /// Controls: GPIO 0 (IN1) and GPIO 1 (IN2) on CAT9555
    h1_direction: HBridgeDirection,

    /// H-Bridge 2 direction (secondary thermal control)
    /// Controls: GPIO 2 (IN3) and GPIO 3 (IN4) on CAT9555
    h2_direction: HBridgeDirection,

    /// Current PWM duty cycle for H-Bridge 1 ENA (0.0 to 100.0%)
    /// Controls: PWM Channel 0 on PCA9685 (register 0x06)
    h1_duty_cycle: f64,

    /// Current PWM duty cycle for H-Bridge 2 ENB (0.0 to 100.0%)
    /// Controls: PWM Channel 1 on PCA9685 (register 0x0A)
    h2_duty_cycle: f64,
}

impl Default for HBridgeState {
    fn default() -> Self {
        Self {
            h1_direction: HBridgeDirection::Disabled,
            h2_direction: HBridgeDirection::Disabled,
            h1_duty_cycle: 0.0,
            h2_duty_cycle: 0.0,
        }
    }
}

impl MockI2CL298NDriver {
    /// Apply thermal power based on H-Bridge state and PWM duty cycle
    ///
    /// # Real Hardware Implementation Reference
    ///
    /// This method demonstrates the critical coordination between H-Bridge direction
    /// control (GPIO) and power control (PWM) for thermal regulation systems.
    /// This logic is ESSENTIAL for real hardware implementations.
    ///
    /// ## Thermal Control Strategy:
    ///
    /// ### Forward Direction (Heating Mode):
    /// - **Resistive Heating**: Apply PWM duty cycle to heating resistor
    /// - **Peltier Disabled**: Ensure Peltier is not fighting resistive heating
    /// - **Use Case**: Rapid heating, high power thermal input
    ///
    /// ### Reverse Direction (Cooling Mode):
    /// - **Peltier Cooling**: Apply PWM duty cycle as negative Peltier power
    /// - **Resistive Heating Disabled**: Ensure no conflicting thermal input
    /// - **Use Case**: Active cooling below ambient temperature
    ///
    /// ### Disabled State (Coast/Brake):
    /// - **All Thermal Disabled**: Both Peltier and resistive heating off
    /// - **Use Case**: Thermal system idle, natural thermal equilibrium
    ///
    /// ## Critical Implementation Notes:
    /// 1. **Mutual Exclusion**: Never enable both heating and cooling simultaneously
    /// 2. **Power Scaling**: PWM duty cycle (0-100%) maps directly to thermal power
    /// 3. **Immediate Effect**: Apply thermal changes immediately after state update
    /// 4. **Error Handling**: Validate thermal actuator responses in real hardware
    ///
    /// ## Real Hardware Adaptation:
    /// - Replace `set_heater_power()` with actual heating resistor control
    /// - Replace `set_peltier_power()` with actual Peltier module control  
    /// - Add thermal feedback verification (current sensing, temperature monitoring)
    /// - Implement safety limits and thermal runaway protection
    /// - Add power supply monitoring and overcurrent protection
    ///
    /// ## Safety Considerations for Real Hardware:
    /// - Monitor thermal actuator current draw
    /// - Implement thermal runaway detection
    /// - Add emergency thermal shutdown capabilities
    /// - Validate power supply capacity before high-power operations
    fn apply_thermal_power_based_on_state(&self, state: &HBridgeState) -> Result<()> {
        match state.h1_direction {
            HBridgeDirection::Forward => {
                // Forward direction: Heating mode (resistive heating or Peltier heating)
                // Use duty cycle as heating power
                let _ = self.set_heater_power(state.h1_duty_cycle);
                let _ = self.set_peltier_power(0.0); // Disable Peltier in resistive heating mode
                debug!("Applied heating power: {:.1}%", state.h1_duty_cycle);
            }
            HBridgeDirection::Reverse => {
                // Reverse direction: Cooling mode (Peltier cooling)
                // Use duty cycle as cooling power (negative for Peltier)
                let _ = self.set_heater_power(0.0); // Disable resistive heating in cooling mode
                let _ = self.set_peltier_power(-state.h1_duty_cycle); // Negative for cooling
                debug!("Applied cooling power: -{:.1}%", state.h1_duty_cycle);
            }
            HBridgeDirection::Disabled => {
                // Disabled: No thermal control
                let _ = self.set_heater_power(0.0);
                let _ = self.set_peltier_power(0.0);
                debug!("Thermal control disabled");
            }
        }
        Ok(())
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

        let driver = MockI2CL298NDriver::new(&config);
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

        let mut driver = MockI2CL298NDriver::new(&config).unwrap();

        // Test device presence
        assert!(driver.device_present(0x40).await.unwrap());
        assert!(!driver.device_present(0x41).await.unwrap());
    }

    #[test]
    fn test_thermal_simulation() {
        let mut sim = ThermalCellSimulation::new();

        // Test initial conditions
        assert_eq!(sim.get_temperature(), AMBIENT_ROOM_TEMP_C);

        // Test heating
        sim.set_heater_power(50.0);
        sim.update();

        // Temperature should increase (though the change might be small)
        // This test verifies the simulation logic runs without error
        assert!(sim.get_temperature() >= AMBIENT_ROOM_TEMP_C);

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
        assert_eq!(props.mass_g, CELL_MASS_G);
        assert_eq!(
            props.dimensions_mm,
            (CELL_LENGTH_MM, CELL_WIDTH_MM, CELL_HEIGHT_MM)
        );
        assert_eq!(
            props.peltier_dimensions_mm,
            (PELTIER_LENGTH_MM, PELTIER_WIDTH_MM)
        );
        assert_eq!(props.heater_max_power, HEATER_MAX_POWER_W);
        assert_eq!(props.peltier_max_power, PELTIER_MAX_POWER_W);

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

        let mut driver = MockI2CL298NDriver::new(&config).unwrap();

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
        let mut process_value = AMBIENT_ROOM_TEMP_C;
        let dt = 0.1;

        for _ in 0..10 {
            let output = pid.update(setpoint, process_value, dt);
            assert!(output >= -100.0 && output <= 100.0);

            // Simulate simple process response
            process_value += output * 0.001;
        }

        // PID should drive process value towards setpoint
        assert!(process_value > AMBIENT_ROOM_TEMP_C); // Should have increased
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
