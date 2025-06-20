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
            I2CBusType::Mock => Ok(Box::new(drivers::mock::MockI2CL298NDriver::new(config)?)),
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

/// Mock thermal regulation driver for testing and simulation with L298N H-Bridge control
///
/// # Complete Reference Implementation for Real Hardware Drivers
///
/// This driver provides a comprehensive, production-ready reference implementation
/// for developing real hardware thermal regulation drivers using L298N H-Bridge
/// motor drivers for Peltier module and heating resistor control.
///
/// ## Architecture Overview:
///
/// ### Hardware Abstraction Layers:
/// 1. **High-Level Interface**: Implements `ThermalRegulationDriver` trait
/// 2. **I2C Communication**: Uses `MockI2CL298NDriver` for device coordination
/// 3. **Thermal Control**: Coordinates GPIO direction + PWM power control
/// 4. **State Management**: Tracks current control output and hardware state
///
/// ### Key Design Principles:
/// - **Complete Hardware Abstraction**: PID tuner is hardware-independent
/// - **Thread-Safe Operations**: Safe for concurrent thermal regulation
/// - **Error Handling**: Comprehensive error propagation and recovery
/// - **Configuration-Driven**: All hardware parameters from config files
///
/// ## Real Hardware Implementation Guide:
///
/// ### 1. Temperature Reading (`read_temperature`):
/// - **Current**: Simulated NTC thermistor via ADC
/// - **Real Hardware**: Replace with actual ADS1115 ADC + NTC thermistor
/// - **Keep**: Voltage-to-temperature conversion logic (NTC β-parameter formula)
/// - **Add**: ADC calibration, noise filtering, multiple sensor averaging
///
/// ### 2. Control Output (`apply_control_output`):
/// - **Current**: Coordinated GPIO direction + PWM power control
/// - **Real Hardware**: KEEP EXACT SAME LOGIC - this is production-ready
/// - **Critical**: Direction (GPIO) before power (PWM) sequence
/// - **Add**: Hardware verification, current sensing, safety interlocks
///
/// ### 3. Hardware Initialization (`initialize`):
/// - **Current**: Mock initialization
/// - **Real Hardware**: I2C device detection, hardware configuration validation
/// - **Add**: Power supply validation, thermal sensor calibration
///
/// ## Hardware Configuration Requirements:
/// ```yaml
/// # Essential configuration sections for real hardware:
/// temperature_sensor:
///   adc_address: 0x48        # ADS1115 I2C address
///   adc_channel: 0x00        # ADC input channel
///
/// actuators:
///   thermal_control:
///     direction_controller:   # CAT9555 GPIO for H-Bridge direction
///       address: 0x20
///     pwm_controller:         # PCA9685 PWM for H-Bridge power  
///       address: 0x40
///
/// temperature_conversion:
///   formula: "NTC_10K_3977"  # NTC thermistor parameters
///   voltage_reference: 5.0   # ADC reference voltage
/// ```
///
/// ## Critical Implementation Notes:
/// 1. **State Consistency**: Always track control output for PID feedback
/// 2. **Error Propagation**: Bubble up I2C errors to PID controller
/// 3. **Thread Safety**: Driver must be Send + Sync for multi-regulator systems
/// 4. **Configuration Validation**: Verify all hardware addresses during initialization
///
/// ## Safety Considerations for Real Hardware:
/// - Implement thermal runaway protection
/// - Add overcurrent detection and shutdown
/// - Monitor power supply voltage stability  
/// - Implement emergency thermal shutdown sequences
/// - Add hardware watchdog for control loop monitoring
///
/// ## Testing and Validation:
/// - Use this mock driver for PID tuning and algorithm development
/// - Validate control algorithms before real hardware deployment
/// - Compare thermal response between simulation and real hardware
/// - Use for thermal system modeling and performance prediction
pub struct MockL298NThermalRegulationDriver {
    /// Low-level I2C driver handling all hardware device communication
    /// In real hardware: Replace MockI2CL298NDriver with actual I2C implementation
    /// CRITICAL: Keep the same interface and H-Bridge state coordination logic
    i2c_driver: drivers::mock::MockI2CL298NDriver,

    /// Complete thermal regulator configuration from config file
    /// Contains all hardware addresses, conversion formulas, and thermal parameters
    /// In real hardware: KEEP UNCHANGED - this drives all hardware communication
    regulator_config: crate::config::thermal_regulation::ThermalRegulatorConfig,

    /// Current control output value for PID feedback (-100.0 to +100.0)
    /// ESSENTIAL: Must track actual applied control for proper PID operation
    /// In real hardware: KEEP UNCHANGED - critical for PID controller stability
    current_control_output: f64,
}

impl MockL298NThermalRegulationDriver {
    /// Create a new mock thermal regulation driver
    ///
    /// # Real Hardware Implementation Guide
    ///
    /// This constructor demonstrates the essential initialization pattern for
    /// thermal regulation drivers. The same structure should be used for real hardware.
    ///
    /// ## Initialization Sequence:
    /// 1. **I2C Driver Creation**: Initialize low-level hardware communication
    /// 2. **Configuration Validation**: Verify all hardware addresses and parameters
    /// 3. **State Initialization**: Set driver to safe initial state (0% output)
    ///
    /// ## Parameters:
    /// - `bus_config`: I2C bus configuration (addresses, device types, settings)
    /// - `regulator_config`: Complete thermal regulator configuration
    ///
    /// ## Real Hardware Adaptation:
    /// - Replace `MockI2CL298NDriver::new()` with actual hardware I2C driver
    /// - Add hardware device detection and verification
    /// - Implement hardware-specific initialization sequences
    /// - Add safety checks and emergency shutdown capabilities
    ///
    /// ## Error Handling:
    /// - Propagate I2C initialization errors to caller
    /// - Validate configuration parameters before hardware access
    /// - Implement graceful degradation for partial hardware failures
    pub fn new(
        bus_config: &I2CBusConfig,
        regulator_config: &crate::config::thermal_regulation::ThermalRegulatorConfig,
    ) -> Result<Self> {
        let i2c_driver = drivers::mock::MockI2CL298NDriver::new(bus_config)?;

        Ok(Self {
            i2c_driver,
            regulator_config: regulator_config.clone(),
            current_control_output: 0.0,
        })
    }
}

#[async_trait::async_trait]
impl ThermalRegulationDriver for MockL298NThermalRegulationDriver {
    /// Read current temperature from thermistor-based sensor via ADC
    ///
    /// # Complete Reference Implementation for Real Hardware
    ///
    /// This method provides a production-ready implementation of thermistor-based
    /// temperature sensing commonly used in thermal regulation systems. The conversion
    /// process demonstrates the complete chain from ADC reading to calibrated temperature.
    ///
    /// ## Temperature Sensing Chain:
    /// ```text
    /// NTC Thermistor -> Voltage Divider -> ADC -> Raw Digital -> Voltage -> Temperature
    /// ```
    ///
    /// ## Implementation Steps:
    /// 1. **ADC Reading**: Read raw digital value from ADS1115 ADC controller
    /// 2. **Voltage Conversion**: Convert raw ADC to voltage using reference and resolution
    /// 3. **Temperature Conversion**: Apply thermistor formula to calculate temperature
    /// 4. **Unit Conversion**: Convert from Kelvin to Celsius for PID controller
    ///
    /// ## Hardware Configuration Requirements:
    /// - **ADC Controller**: ADS1115 configured for appropriate resolution and reference
    /// - **Thermistor Circuit**: NTC thermistor in voltage divider configuration
    /// - **Conversion Formula**: Steinhart-Hart or NTC Beta formula in configuration
    /// - **Calibration**: Optional offset and scaling factors in configuration
    ///
    /// ## Real Hardware Adaptation:
    /// - **Keep Exact Logic**: This conversion chain works for real thermistor circuits
    /// - **ADC Configuration**: Ensure ADC resolution and reference match configuration
    /// - **Formula Validation**: Verify thermistor formula matches actual component
    /// - **Calibration**: Add temperature calibration offset correction if needed
    /// - **Error Handling**: Add ADC communication retry logic and fault detection
    ///
    /// ## Critical Implementation Notes:
    /// - **Configuration Driven**: All parameters come from configuration file
    /// - **Formula Flexibility**: Supports different thermistor equations via configuration
    /// - **Precision**: Maintains high precision throughout conversion chain
    /// - **Error Propagation**: ADC errors propagate to caller for PID error handling
    ///
    /// ## Temperature Sensor Types Supported:
    /// - **NTC Thermistors**: With Beta parameter or Steinhart-Hart coefficients
    /// - **RTD Sensors**: With linear or polynomial conversion formulas
    /// - **Direct Digital**: MCP9808 or similar with direct temperature reading
    ///
    /// ## Safety Considerations for Real Hardware:
    /// - Monitor for thermistor disconnection (open circuit detection)
    /// - Validate temperature readings are within expected physical range
    /// - Implement temperature sensor redundancy for safety-critical applications
    /// - Add temperature reading timeout and fallback mechanisms
    async fn read_temperature(&mut self) -> Result<f64> {
        use crate::utility::convert_voltage_to_temperature;
        use anyhow::anyhow;
        use log::debug;

        // Step 1: Read raw ADC value from thermistor sensor circuit
        let adc_address = self.regulator_config.temperature_sensor.adc_address;
        let adc_channel = self.regulator_config.temperature_sensor.adc_channel;

        // Read raw ADC value (typically 16-bit from ADS1115)
        let adc_data = self
            .i2c_driver
            .read(adc_address, adc_channel, 2)
            .await
            .map_err(|e| anyhow!("Failed to read ADC: {}", e))?;

        if adc_data.len() < 2 {
            return Err(anyhow!("Insufficient ADC data"));
        }

        // Step 2: Convert raw ADC bytes to digital value
        let raw_value = ((adc_data[0] as u16) << 8) | (adc_data[1] as u16);
        debug!("ADC raw value: {} (0x{:04X})", raw_value, raw_value);

        // Step 3: Get conversion parameters from configuration
        let temp_conversion = &self.regulator_config.temperature_conversion;
        let adc_resolution = temp_conversion.adc_resolution;
        let voltage_reference = temp_conversion.voltage_reference;
        let formula = &temp_conversion.formula;

        // Step 4: Convert ADC digital value to voltage
        let max_adc_value = (1_u32 << adc_resolution) - 1; // e.g., 65535 for 16-bit
        let voltage = (raw_value as f64 / max_adc_value as f64) * voltage_reference as f64;

        debug!(
            "ADC conversion: raw={}, voltage={:.3}V, formula='{}'",
            raw_value, voltage, formula
        );

        // Step 5: Convert voltage to temperature using configured formula
        let temperature_k = convert_voltage_to_temperature(formula.clone(), voltage as f32)?;

        // Step 6: Convert from Kelvin to Celsius for PID controller compatibility
        let temperature_c = temperature_k - 273.15;

        debug!(
            "Temperature conversion: {:.2}K = {:.2}°C",
            temperature_k, temperature_c
        );

        Ok(temperature_c)
    }

    /// Apply thermal control output to L298N H-Bridge system
    ///
    /// # Complete Reference Implementation for Real Hardware
    ///
    /// This method provides a production-ready implementation of the critical
    /// two-step thermal control sequence required for L298N H-Bridge operation.
    /// This exact logic should be preserved in real hardware implementations.
    ///
    /// ## Control Flow Overview:
    /// ```text
    /// 1. Validate and clamp control output (-100% to +100%)
    /// 2. Configure H-Bridge direction via GPIO (CAT9555)
    /// 3. Apply power level via PWM (PCA9685)  
    /// 4. Update driver state for PID feedback
    /// ```
    ///
    /// ## Step 1: H-Bridge Direction Control (GPIO)
    ///
    /// ### Hardware: CAT9555 GPIO Controller
    /// - **Address**: From `regulator_config.actuators.thermal_control.direction_controller`
    /// - **Register**: 0x02 (output register)
    /// - **Mapping**: GPIO 0 = IN1, GPIO 1 = IN2 (for H-Bridge #1)
    ///
    /// ### Control Logic:
    /// - **Positive Output (Heating)**: IN1=HIGH, IN2=LOW (Forward direction)
    /// - **Negative Output (Cooling)**: IN1=LOW, IN2=HIGH (Reverse direction)  
    /// - **Zero Output (Disabled)**: IN1=LOW, IN2=LOW (Coast mode)
    ///
    /// ## Step 2: H-Bridge Power Control (PWM)
    ///
    /// ### Hardware: PCA9685 PWM Controller
    /// - **Address**: From `regulator_config.actuators.thermal_control.pwm_controller`
    /// - **Channel**: 0 (register 0x06) for H-Bridge #1 ENA
    /// - **Resolution**: 12-bit (0-4095) mapped from percentage (0-100%)
    ///
    /// ### Power Calculation:
    /// ```text
    /// pwm_value = (abs(control_output) / 100.0) * 4095.0
    /// pwm_data = [pwm_value_low, pwm_value_high]
    /// ```
    ///
    /// ## Critical Implementation Requirements:
    /// 1. **Direction Before Power**: ALWAYS set GPIO direction before applying PWM
    /// 2. **State Tracking**: Update `current_control_output` for PID feedback
    /// 3. **Error Handling**: Propagate I2C errors to caller for PID error handling
    /// 4. **Range Validation**: Clamp control output to prevent hardware damage
    ///
    /// ## Real Hardware Adaptation:
    /// - **I2C Communication**: Replace mock I2C with actual hardware drivers
    /// - **Error Recovery**: Add I2C retry logic and device reset capabilities
    /// - **Safety Monitoring**: Add current sensing and thermal monitoring
    /// - **Validation**: Verify GPIO and PWM writes completed successfully
    /// - **Calibration**: Add PWM output calibration for actual power delivery
    ///
    /// ## Hardware Safety Considerations:
    /// - Never set IN1=HIGH and IN2=HIGH simultaneously (short circuit)
    /// - Monitor thermal actuator current draw during operation
    /// - Implement thermal runaway detection and emergency shutdown
    /// - Add power supply voltage monitoring and undervoltage protection
    /// - Validate PWM frequency matches thermal actuator specifications
    ///
    /// ## PID Controller Integration:
    /// - Input: `control_output` from PID controller (-100.0 to +100.0)
    /// - Processing: Convert to H-Bridge direction + PWM power commands
    /// - Output: Update `current_control_output` for next PID iteration
    /// - Feedback: Thermal effect visible in next temperature reading
    async fn apply_control_output(&mut self, control_output: f64) -> Result<()> {
        use anyhow::anyhow;
        use log::debug;

        // Clamp control output to valid range
        let duty_clamped = control_output.clamp(-100.0, 100.0);

        debug!(
            "Applying thermal control output: {:.1}% (clamped from {:.1}%)",
            duty_clamped, control_output
        );

        // Step 1: Configure H-Bridge direction via GPIO (CAT9555)
        let direction_controller = &self
            .regulator_config
            .actuators
            .thermal_control
            .direction_controller;
        let gpio_address = direction_controller.address;

        // Determine H-Bridge #1 direction control signals based on new GPIO mapping:
        // GPIO 0 = IN1, GPIO 1 = IN2 (ENA controlled separately by PWM)
        let (h1_in1, h1_in2) = if duty_clamped > 0.0 {
            // Positive: heating mode (forward direction)
            (true, false)
        } else if duty_clamped < 0.0 {
            // Negative: cooling mode (reverse direction)
            (false, true)
        } else {
            // Zero: coast mode (both low)
            (false, false)
        };

        // Construct GPIO register value for H-Bridge #1 control
        // GPIO 0 (bit 0) = H-Bridge 1 IN1
        // GPIO 1 (bit 1) = H-Bridge 1 IN2
        // GPIO 2,3 reserved for H-Bridge #2 (future expansion)
        let mut gpio_value = 0u8;
        if h1_in1 {
            gpio_value |= 0x01; // GPIO 0 = IN1
        }
        if h1_in2 {
            gpio_value |= 0x02; // GPIO 1 = IN2
        }

        debug!(
            "H-Bridge 1 direction: IN1={}, IN2={}, GPIO register=0x{:02X}",
            h1_in1, h1_in2, gpio_value
        );

        // Write GPIO direction control
        self.i2c_driver
            .write(gpio_address, 0x02, &[gpio_value])
            .await // 0x02 = output register
            .map_err(|e| anyhow!("Failed to write H-Bridge direction GPIO: {}", e))?;

        // Step 2: Configure H-Bridge power via PWM (PCA9685)
        let pwm_address = self
            .regulator_config
            .actuators
            .thermal_control
            .pwm_controller
            .address;

        // H-Bridge #1 uses PWM Channel 0 (register 0x06) for ENA control
        let pwm_channel = 0x06; // Channel 0 register address

        // Convert duty percentage to PWM value (12-bit PWM: 0-4095)
        let pwm_value = ((duty_clamped.abs() / 100.0) * 4095.0) as u16;

        // Write PWM duty cycle for ENA (H-Bridge #1 enable/power control)
        let pwm_data = [
            (pwm_value & 0xFF) as u8,        // Low byte
            ((pwm_value >> 8) & 0xFF) as u8, // High byte
        ];

        debug!(
            "H-Bridge 1 ENA (PWM Ch0): address=0x{:02X}, register=0x{:02X}, duty={:.1}%, pwm_value={}",
            pwm_address, pwm_channel, duty_clamped.abs(), pwm_value
        );

        self.i2c_driver
            .write(pwm_address, pwm_channel, &pwm_data)
            .await
            .map_err(|e| anyhow!("Failed to write H-Bridge ENA PWM: {}", e))?;

        self.current_control_output = duty_clamped;
        Ok(())
    }

    /// Get the current control output percentage
    ///
    /// # Real Hardware Implementation Reference
    ///
    /// This method provides essential feedback to the PID controller by returning
    /// the actual control output value that was last applied to the thermal system.
    /// This value is critical for PID stability and control loop verification.
    ///
    /// ## PID Controller Integration:
    /// - **Feedback Loop**: PID controller uses this value to verify control output
    /// - **Control Verification**: Ensures applied control matches PID calculation
    /// - **State Tracking**: Maintains control output history for PID tuning
    /// - **Error Detection**: Discrepancies indicate hardware control failures
    ///
    /// ## Real Hardware Adaptation:
    /// - **Keep Unchanged**: This method is hardware-independent
    /// - **State Consistency**: Ensure value reflects actual hardware state
    /// - **Range Validation**: Value should always be within -100.0 to +100.0
    /// - **Thread Safety**: Access must be thread-safe for concurrent PID operation
    ///
    /// ## Return Value:
    /// - **Range**: -100.0 to +100.0 (percentage)
    /// - **Positive**: Heating mode control output
    /// - **Negative**: Cooling mode control output
    /// - **Zero**: No thermal control (disabled state)
    fn get_current_control_output(&self) -> f64 {
        self.current_control_output
    }

    /// Initialize thermal regulation hardware
    ///
    /// # Real Hardware Implementation Reference
    ///
    /// This method demonstrates the complete hardware initialization sequence
    /// required for safe thermal regulation operation. Real hardware implementations
    /// must perform comprehensive device initialization and safety checks.
    ///
    /// ## Initialization Sequence for Real Hardware:
    /// 1. **Device Detection**: Verify all I2C devices are present and responding
    /// 2. **Hardware Reset**: Reset all thermal control devices to known state
    /// 3. **Configuration**: Apply hardware-specific configuration settings
    /// 4. **Safety Setup**: Configure thermal protection and emergency shutdown
    /// 5. **Calibration**: Load thermal sensor calibration and scaling factors
    /// 6. **Verification**: Verify all devices are properly configured
    ///
    /// ## Critical Safety Initialization:
    /// - **H-Bridge Safety**: Initialize all H-Bridge outputs to disabled state
    /// - **PWM Configuration**: Set PWM frequency to match thermal actuator specs
    /// - **GPIO Configuration**: Configure all direction control pins as outputs
    /// - **Temperature Monitoring**: Enable thermal runaway protection
    /// - **Power Monitoring**: Configure supply voltage and current monitoring
    ///
    /// ## Real Hardware Adaptation:
    /// - **Replace Mock**: Remove mock-specific initialization
    /// - **Device Initialization**: Add I2C device initialization sequences
    /// - **Safety Systems**: Initialize thermal protection and monitoring
    /// - **Calibration Loading**: Load sensor calibration from configuration
    /// - **Error Handling**: Implement comprehensive initialization error handling
    ///
    /// ## Initialization Failure Handling:
    /// - **Critical Failures**: Refuse to start thermal control if devices missing
    /// - **Partial Failures**: Degrade functionality gracefully where possible
    /// - **Error Reporting**: Provide detailed error information for diagnostics
    /// - **Recovery**: Implement initialization retry logic for transient failures
    async fn initialize(&mut self) -> Result<()> {
        // Initialize hardware (mock implementation)
        // Real hardware: Implement comprehensive device initialization sequence
        log::info!("Initializing mock thermal regulation driver");
        Ok(())
    }

    /// Get thermal regulation system status
    ///
    /// # Real Hardware Implementation Reference
    ///
    /// This method provides comprehensive status information for thermal regulation
    /// system monitoring, diagnostics, and troubleshooting. Real hardware implementations
    /// should provide detailed hardware status and health information.
    ///
    /// ## Status Information Categories:
    /// - **Temperature**: Current temperature readings from all sensors
    /// - **Control Output**: Current thermal control output percentage
    /// - **Hardware Status**: I2C device communication status and health
    /// - **Thermal Actuators**: Status of heating resistors and Peltier modules
    /// - **Power Systems**: Supply voltage, current draw, and power consumption
    /// - **Safety Systems**: Thermal protection status and fault conditions
    ///
    /// ## Real Hardware Status Information:
    /// ```text
    /// Temperature: 25.3°C (Target: 30.0°C)
    /// Control Output: +45.2% (Heating Mode)
    /// H-Bridge 1: Forward, ENA=45.2%, 2.1A
    /// Power Supply: 12.0V, 3.2A total
    /// Thermal Protection: Active, No Faults
    /// I2C Devices: All Present (ADC: OK, PWM: OK, GPIO: OK)
    /// ```
    ///
    /// ## Real Hardware Adaptation:
    /// - **Replace Temperature Source**: Get temperature from actual sensor reading
    /// - **Hardware Monitoring**: Add I2C device status and communication health
    /// - **Power Monitoring**: Include supply voltage and current consumption
    /// - **Thermal Actuator Status**: Report actual heating/cooling actuator status
    /// - **Safety Status**: Report thermal protection and fault conditions
    /// - **Performance Metrics**: Include PID performance and regulation accuracy
    ///
    /// ## Status Update Frequency:
    /// - **Real-time**: Temperature and control output (every PID cycle)
    /// - **Periodic**: Hardware status and power consumption (every 1-5 seconds)
    /// - **Event-driven**: Fault conditions and safety system status (immediate)
    /// - **Diagnostic**: Detailed hardware diagnostics (on request)
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

        // Clamp control output to valid range
        let duty_clamped = control_output.clamp(-100.0, 100.0);

        debug!(
            "Native: Applying thermal control output: {:.1}%",
            duty_clamped
        );

        // Step 1: Configure H-Bridge direction via GPIO (CAT9555)
        let direction_controller = &self
            .regulator_config
            .actuators
            .thermal_control
            .direction_controller;
        let gpio_address = direction_controller.address;

        // H-Bridge #1 direction control (GPIO 0=IN1, GPIO 1=IN2)
        let (h1_in1, h1_in2) = if duty_clamped > 0.0 {
            (true, false) // Heating mode
        } else if duty_clamped < 0.0 {
            (false, true) // Cooling mode
        } else {
            (false, false) // Coast mode
        };

        let mut gpio_value = 0u8;
        if h1_in1 {
            gpio_value |= 0x01; // GPIO 0 = IN1
        }
        if h1_in2 {
            gpio_value |= 0x02; // GPIO 1 = IN2
        }

        debug!(
            "Native H-Bridge 1 direction: IN1={}, IN2={}, GPIO=0x{:02X}",
            h1_in1, h1_in2, gpio_value
        );

        self.i2c_driver
            .write(gpio_address, 0x02, &[gpio_value])
            .await
            .map_err(|e| anyhow!("Failed to write H-Bridge direction GPIO: {}", e))?;

        // Step 2: Configure H-Bridge power via PWM (PCA9685)
        let pwm_address = self
            .regulator_config
            .actuators
            .thermal_control
            .pwm_controller
            .address;

        // H-Bridge #1 ENA uses PWM Channel 0 (register 0x06)
        let pwm_channel = 0x06; // Channel 0 register address
        let pwm_value = ((duty_clamped.abs() / 100.0) * 4095.0) as u16;
        let pwm_data = [(pwm_value & 0xFF) as u8, ((pwm_value >> 8) & 0xFF) as u8];

        debug!(
            "Native H-Bridge 1 ENA (PWM Ch0): duty={:.1}%, pwm_value={}",
            duty_clamped.abs(),
            pwm_value
        );

        self.i2c_driver
            .write(pwm_address, pwm_channel, &pwm_data)
            .await
            .map_err(|e| anyhow!("Failed to write H-Bridge ENA PWM: {}", e))?;

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

        // Clamp control output to valid range
        let duty_clamped = control_output.clamp(-100.0, 100.0);

        debug!(
            "CP2112: Applying thermal control output: {:.1}%",
            duty_clamped
        );

        // Step 1: Configure H-Bridge direction via GPIO (CAT9555)
        let direction_controller = &self
            .regulator_config
            .actuators
            .thermal_control
            .direction_controller;
        let gpio_address = direction_controller.address;

        // H-Bridge #1 direction control (GPIO 0=IN1, GPIO 1=IN2)
        let (h1_in1, h1_in2) = if duty_clamped > 0.0 {
            (true, false) // Heating mode
        } else if duty_clamped < 0.0 {
            (false, true) // Cooling mode
        } else {
            (false, false) // Coast mode
        };

        let mut gpio_value = 0u8;
        if h1_in1 {
            gpio_value |= 0x01; // GPIO 0 = IN1
        }
        if h1_in2 {
            gpio_value |= 0x02; // GPIO 1 = IN2
        }

        debug!(
            "CP2112 H-Bridge 1 direction: IN1={}, IN2={}, GPIO=0x{:02X}",
            h1_in1, h1_in2, gpio_value
        );

        self.i2c_driver
            .write(gpio_address, 0x02, &[gpio_value])
            .await
            .map_err(|e| anyhow!("Failed to write H-Bridge direction GPIO: {}", e))?;

        // Step 2: Configure H-Bridge power via PWM (PCA9685)
        let pwm_address = self
            .regulator_config
            .actuators
            .thermal_control
            .pwm_controller
            .address;

        // H-Bridge #1 ENA uses PWM Channel 0 (register 0x06)
        let pwm_channel = 0x06; // Channel 0 register address
        let pwm_value = ((duty_clamped.abs() / 100.0) * 4095.0) as u16;
        let pwm_data = [(pwm_value & 0xFF) as u8, ((pwm_value >> 8) & 0xFF) as u8];

        debug!(
            "CP2112 H-Bridge 1 ENA (PWM Ch0): duty={:.1}%, pwm_value={}",
            duty_clamped.abs(),
            pwm_value
        );

        self.i2c_driver
            .write(pwm_address, pwm_channel, &pwm_data)
            .await
            .map_err(|e| anyhow!("Failed to write H-Bridge ENA PWM: {}", e))?;

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
            Ok(Box::new(MockL298NThermalRegulationDriver::new(
                bus_config,
                regulator_config,
            )?))
        }
    }
}
