// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Configuration for thermal regulation system
//!
//! This module provides configuration structures for the thermal regulation system
//! including I2C bus configuration, hardware controllers, and individual regulators.

use rocket::serde::{Deserialize, Serialize};
use rocket_okapi::JsonSchema;
use std::collections::HashMap;

/// Main thermal regulation configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ThermalRegulationConfig {
    /// Enable or disable thermal regulation system
    #[serde(default)]
    pub enabled: bool,

    /// I2C bus configurations (primary and optional secondary buses)
    pub i2c_buses: HashMap<String, I2CBusConfig>,

    /// Individual thermal regulators configuration
    #[serde(default)]
    pub regulators: Vec<ThermalRegulatorConfig>,

    /// Global thermal regulation parameters
    #[serde(default)]
    pub global_settings: GlobalThermalSettings,
}

/// I2C bus configuration for hardware controllers
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct I2CBusConfig {
    /// Bus type: "native" for Raspberry Pi I2C or "cp2112" for USB-HID bridge
    #[serde(rename = "type")]
    pub bus_type: I2CBusType,

    /// Device path for native I2C (e.g., "/dev/i2c-1") or USB identifier for CP2112
    pub device: String,

    /// USB vendor ID for CP2112 (only used when bus_type is "cp2112")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usb_vendor_id: Option<u16>,

    /// USB product ID for CP2112 (only used when bus_type is "cp2112")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usb_product_id: Option<u16>,

    /// PWM controllers on this bus (up to 32 PCA9685)
    #[serde(default)]
    pub pwm_controllers: Vec<PwmControllerConfig>,

    /// ADC controllers on this bus (up to 4 ADS1115)
    #[serde(default)]
    pub adc_controllers: Vec<AdcControllerConfig>,

    /// GPIO controllers on this bus (up to 8 CAT9555)
    #[serde(default)]
    pub gpio_controllers: Vec<GpioControllerConfig>,

    /// Bus-specific settings
    #[serde(default)]
    pub bus_settings: I2CBusSettings,
}

/// I2C bus type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum I2CBusType {
    /// Native Raspberry Pi I2C bus
    Native,
    /// Silicon Labs CP2112 USB-to-I2C bridge
    Cp2112,
    /// Mock driver for thermal simulation testing
    Mock,
}

/// PWM controller configuration (PCA9685)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PwmControllerConfig {
    /// I2C address of the PCA9685 controller (0x40-0x7F)
    pub address: u8,

    /// Number of PWM channels (always 16 for PCA9685)
    #[serde(default = "default_pwm_channels")]
    pub channels: u8,

    /// PWM frequency in Hz (24-1526 Hz for PCA9685)
    #[serde(default = "default_pwm_frequency")]
    pub frequency_hz: u16,

    /// Controller-specific settings
    #[serde(default)]
    pub settings: PwmControllerSettings,
}

/// ADC controller configuration (ADS1115)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AdcControllerConfig {
    /// I2C address of the ADS1115 controller (0x48-0x4B)
    pub address: u8,

    /// Number of ADC channels (always 4 for ADS1115)
    #[serde(default = "default_adc_channels")]
    pub channels: u8,

    /// ADC resolution in bits (always 16 for ADS1115)
    #[serde(default = "default_adc_resolution")]
    pub resolution: u8,

    /// Voltage reference in volts
    #[serde(default = "default_voltage_ref")]
    pub voltage_ref: f32,

    /// ADC gain setting
    #[serde(default)]
    pub gain: AdcGain,

    /// Data rate in samples per second
    #[serde(default)]
    pub data_rate: AdcDataRate,
}

/// GPIO controller configuration (CAT9555)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GpioControllerConfig {
    /// I2C address of the CAT9555 controller (0x20-0x27)
    pub address: u8,

    /// Number of GPIO channels (always 16 for CAT9555)
    #[serde(default = "default_gpio_channels")]
    pub channels: u8,

    /// Controller type identifier
    #[serde(rename = "type", default = "default_gpio_type")]
    pub controller_type: String,

    /// Primary function of this GPIO controller
    #[serde(default)]
    pub function: GpioFunction,

    /// GPIO controller settings
    #[serde(default)]
    pub settings: GpioControllerSettings,
}

/// Individual thermal regulator configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ThermalRegulatorConfig {
    /// Unique identifier for this regulator
    pub id: String,

    /// Human-readable name for this regulator
    pub name: String,

    /// Enable or disable this specific regulator
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// I2C bus identifier (reference to i2c_buses key)
    pub i2c_bus: String,

    /// Temperature sensor configuration
    pub temperature_sensor: TemperatureSensorConfig,

    /// Thermal actuators configuration
    pub actuators: ThermalActuatorsConfig,

    /// Temperature conversion parameters
    pub temperature_conversion: TemperatureConversionConfig,

    /// PID controller parameters
    pub pid_parameters: PidParameters,

    /// Control system parameters
    pub control_parameters: ControlParameters,

    /// Safety limits and protections
    pub safety_limits: SafetyLimits,
}

/// Temperature sensor configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemperatureSensorConfig {
    /// ADC controller I2C address
    pub adc_address: u8,

    /// ADC channel number (0-3)
    pub adc_channel: u8,

    /// Sensor type for calibration
    #[serde(default)]
    pub sensor_type: TemperatureSensorType,
}

/// Thermal actuators configuration with H-Bridge control
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ThermalActuatorsConfig {
    /// Main thermal control configuration
    pub thermal_control: ThermalControlConfig,
}

/// Thermal control configuration for bidirectional control
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ThermalControlConfig {
    /// PWM controller configuration
    pub pwm_controller: PwmChannelConfig,

    /// Direction controller (GPIO) configuration
    pub direction_controller: DirectionControllerConfig,

    /// Available thermal modes
    pub thermal_modes: ThermalModesConfig,
}

/// PWM channel configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PwmChannelConfig {
    /// PCA9685 I2C address
    pub address: u8,

    /// PWM channel number (0-15)
    pub channel: u8,
}

/// Direction controller configuration for H-Bridge control
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DirectionControllerConfig {
    /// CAT9555 I2C address
    pub address: u8,

    /// GPIO pins mapping for H-Bridge control
    pub gpio_pins: HBridgeGpioPins,
}

/// H-Bridge GPIO pins configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HBridgeGpioPins {
    /// GPIO pin for H-Bridge IN1 (direction bit 1)
    pub h_bridge_in1: u8,

    /// GPIO pin for H-Bridge IN2 (direction bit 2)
    pub h_bridge_in2: u8,

    /// GPIO pin for H-Bridge enable
    pub h_bridge_enable: u8,
}

/// Thermal modes configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ThermalModesConfig {
    /// Heating via TEC (Peltier) mode
    pub heating_tec: ThermalModeConfig,

    /// Cooling via TEC (Peltier) mode
    pub cooling_tec: ThermalModeConfig,

    /// Heating via resistive element mode
    pub heating_resistive: ThermalModeConfig,
}

/// Individual thermal mode configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ThermalModeConfig {
    /// Human-readable description
    pub description: String,

    /// H-Bridge direction ("forward" or "reverse")
    pub h_bridge_direction: HBridgeDirection,

    /// Power range as string (e.g., "0-80%")
    pub power_range: String,

    /// Maximum power percentage for this mode
    #[serde(default = "default_max_power")]
    pub max_power_percent: f32,
}

/// Temperature conversion configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemperatureConversionConfig {
    /// Conversion formula (polynomial or lookup table)
    pub formula: String,

    /// ADC resolution in bits
    pub adc_resolution: u8,

    /// Voltage reference in volts
    pub voltage_reference: f32,

    /// Conversion type
    #[serde(default)]
    pub conversion_type: ConversionType,
}

/// PID controller parameters
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PidParameters {
    /// Proportional gain
    pub kp: f32,

    /// Integral gain
    pub ki: f32,

    /// Derivative gain
    pub kd: f32,

    /// Target temperature setpoint in Kelvin
    pub setpoint: f32,

    /// Minimum output value
    pub output_min: f32,

    /// Maximum output value
    pub output_max: f32,

    /// Maximum integral value (anti-windup)
    pub integral_max: f32,

    /// PID controller settings
    #[serde(default)]
    pub settings: PidSettings,
}

/// Control system parameters
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ControlParameters {
    /// Sampling frequency in Hz
    pub sampling_frequency_hz: f32,

    /// PWM frequency in Hz
    pub pwm_frequency_hz: f32,

    /// Control loop settings
    #[serde(default)]
    pub settings: ControlSettings,
}

/// Safety limits and protections
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SafetyLimits {
    /// Minimum allowed temperature in Kelvin
    pub min_temperature_k: f32,

    /// Maximum allowed temperature in Kelvin
    pub max_temperature_k: f32,

    /// Maximum heating duty cycle percentage
    pub max_heating_duty: f32,

    /// Maximum cooling duty cycle percentage
    pub max_cooling_duty: f32,

    /// Emergency shutdown settings
    #[serde(default)]
    pub emergency_settings: EmergencySettings,
}

// Supporting enums and structures

/// ADC gain settings for ADS1115
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum AdcGain {
    /// ±6.144V range
    #[serde(rename = "GAIN_TWOTHIRDS")]
    Gain23,
    /// ±4.096V range
    #[serde(rename = "GAIN_ONE")]
    Gain1,
    /// ±2.048V range (default)
    #[serde(rename = "GAIN_TWO")]
    Gain2,
    /// ±1.024V range
    #[serde(rename = "GAIN_FOUR")]
    Gain4,
    /// ±0.512V range
    #[serde(rename = "GAIN_EIGHT")]
    Gain8,
    /// ±0.256V range
    #[serde(rename = "GAIN_SIXTEEN")]
    Gain16,
}

/// ADC data rate settings for ADS1115
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum AdcDataRate {
    /// 8 samples per second
    Sps8,
    /// 16 samples per second
    Sps16,
    /// 32 samples per second
    Sps32,
    /// 64 samples per second
    Sps64,
    /// 128 samples per second (default)
    Sps128,
    /// 250 samples per second
    Sps250,
    /// 475 samples per second
    Sps475,
    /// 860 samples per second
    Sps860,
}

/// GPIO controller function
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GpioFunction {
    /// H-Bridge control for thermal regulation
    HBridgeControl,
    /// General purpose I/O
    GeneralPurpose,
    /// Status indication (LEDs, etc.)
    StatusIndication,
}

/// Temperature sensor types
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TemperatureSensorType {
    /// Thermocouple (Type K)
    ThermocoupleK,
    /// Thermocouple (Type J)  
    ThermocoupleJ,
    /// RTD (PT100)
    RtdPt100,
    /// RTD (PT1000)
    RtdPt1000,
    /// Thermistor (NTC)
    ThermistorNtc,
    /// Generic voltage sensor
    Generic,
}

/// H-Bridge direction
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HBridgeDirection {
    /// Forward direction (IN1=HIGH, IN2=LOW)
    Forward,
    /// Reverse direction (IN1=LOW, IN2=HIGH)
    Reverse,
}

/// Temperature conversion type
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConversionType {
    /// Polynomial conversion
    Polynomial,
    /// Linear conversion
    Linear,
    /// Lookup table
    LookupTable,
}

// Additional configuration structures

/// I2C bus settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct I2CBusSettings {
    /// I2C clock frequency in Hz
    #[serde(default = "default_i2c_frequency")]
    pub frequency_hz: u32,

    /// Bus timeout in milliseconds
    #[serde(default = "default_bus_timeout")]
    pub timeout_ms: u32,

    /// Maximum retry attempts for failed operations
    #[serde(default = "default_max_retries")]
    pub max_retries: u8,
}

/// PWM controller settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PwmControllerSettings {
    /// Enable auto-increment mode
    #[serde(default)]
    pub auto_increment: bool,

    /// Sleep mode configuration
    #[serde(default)]
    pub sleep_mode: bool,

    /// Restart mode configuration
    #[serde(default)]
    pub restart_mode: bool,
}

/// GPIO controller settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GpioControllerSettings {
    /// Input polarity inversion
    #[serde(default)]
    pub polarity_inversion: u16,

    /// Interrupt configuration
    #[serde(default)]
    pub interrupt_config: InterruptConfig,
}

/// Interrupt configuration for GPIO controllers
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InterruptConfig {
    /// Enable interrupts
    #[serde(default)]
    pub enabled: bool,

    /// Interrupt pin configuration
    #[serde(default)]
    pub pin_config: u16,
}

/// PID controller settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PidSettings {
    /// Derivative on measurement (instead of error)
    #[serde(default)]
    pub derivative_on_measurement: bool,

    /// Integral clamping enabled
    #[serde(default = "default_true")]
    pub integral_clamping: bool,

    /// Output rate limiting
    #[serde(default)]
    pub output_rate_limit: Option<f32>,
}

/// Control loop settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ControlSettings {
    /// Enable adaptive control
    #[serde(default)]
    pub adaptive_control: bool,

    /// Deadband around setpoint
    #[serde(default)]
    pub deadband_k: f32,

    /// Minimum control action
    #[serde(default)]
    pub min_control_action: f32,
}

/// Emergency settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EmergencySettings {
    /// Enable emergency shutdown
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Temperature threshold for emergency shutdown
    #[serde(default = "default_emergency_temp")]
    pub emergency_temp_k: f32,

    /// Timeout for emergency response in seconds
    #[serde(default = "default_emergency_timeout")]
    pub response_timeout_s: f32,
}

/// Global thermal regulation settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GlobalThermalSettings {
    /// Global sampling rate for all regulators
    #[serde(default = "default_global_sampling_rate")]
    pub global_sampling_rate_hz: f32,

    /// Maximum number of concurrent regulators
    #[serde(default = "default_max_regulators")]
    pub max_concurrent_regulators: u8,

    /// Resource sharing settings
    #[serde(default)]
    pub resource_sharing: ResourceSharingSettings,

    /// Logging and monitoring settings
    #[serde(default)]
    pub monitoring: MonitoringSettings,
}

/// Resource sharing settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResourceSharingSettings {
    /// I2C bus arbitration timeout in milliseconds
    #[serde(default = "default_arbitration_timeout")]
    pub i2c_arbitration_timeout_ms: u32,

    /// Maximum queue size for pending operations
    #[serde(default = "default_max_queue_size")]
    pub max_operation_queue_size: usize,

    /// Enable priority-based scheduling
    #[serde(default)]
    pub priority_scheduling: bool,
}

/// Monitoring settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonitoringSettings {
    /// Enable performance monitoring
    #[serde(default = "default_true")]
    pub enable_performance_monitoring: bool,

    /// Metrics collection interval in seconds
    #[serde(default = "default_metrics_interval")]
    pub metrics_interval_s: f32,

    /// Enable thermal history logging
    #[serde(default)]
    pub enable_thermal_history: bool,

    /// History buffer size
    #[serde(default = "default_history_buffer_size")]
    pub history_buffer_size: usize,
}

// Default value functions
fn default_pwm_channels() -> u8 {
    16
}
fn default_pwm_frequency() -> u16 {
    1000
}
fn default_adc_channels() -> u8 {
    4
}
fn default_adc_resolution() -> u8 {
    16
}
fn default_voltage_ref() -> f32 {
    3.3
}
fn default_gpio_channels() -> u8 {
    16
}
fn default_gpio_type() -> String {
    "CAT9555".to_string()
}
fn default_true() -> bool {
    true
}
fn default_max_power() -> f32 {
    100.0
}
fn default_i2c_frequency() -> u32 {
    100000
}
fn default_bus_timeout() -> u32 {
    1000
}
fn default_max_retries() -> u8 {
    3
}
fn default_global_sampling_rate() -> f32 {
    10.0
}
fn default_max_regulators() -> u8 {
    32
}
fn default_arbitration_timeout() -> u32 {
    100
}
fn default_max_queue_size() -> usize {
    100
}
fn default_metrics_interval() -> f32 {
    1.0
}
fn default_history_buffer_size() -> usize {
    1000
}
fn default_emergency_temp() -> f32 {
    373.15
}
fn default_emergency_timeout() -> f32 {
    5.0
}

// Default implementations
impl Default for ThermalRegulationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            i2c_buses: HashMap::new(),
            regulators: Vec::new(),
            global_settings: GlobalThermalSettings::default(),
        }
    }
}

impl Default for AdcGain {
    fn default() -> Self {
        AdcGain::Gain2
    }
}

impl Default for AdcDataRate {
    fn default() -> Self {
        AdcDataRate::Sps128
    }
}

impl Default for GpioFunction {
    fn default() -> Self {
        GpioFunction::HBridgeControl
    }
}

impl Default for TemperatureSensorType {
    fn default() -> Self {
        TemperatureSensorType::Generic
    }
}

impl Default for ConversionType {
    fn default() -> Self {
        ConversionType::Polynomial
    }
}

impl Default for I2CBusSettings {
    fn default() -> Self {
        Self {
            frequency_hz: default_i2c_frequency(),
            timeout_ms: default_bus_timeout(),
            max_retries: default_max_retries(),
        }
    }
}

impl Default for PwmControllerSettings {
    fn default() -> Self {
        Self {
            auto_increment: false,
            sleep_mode: false,
            restart_mode: false,
        }
    }
}

impl Default for GpioControllerSettings {
    fn default() -> Self {
        Self {
            polarity_inversion: 0,
            interrupt_config: InterruptConfig::default(),
        }
    }
}

impl Default for InterruptConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            pin_config: 0,
        }
    }
}

impl Default for PidSettings {
    fn default() -> Self {
        Self {
            derivative_on_measurement: false,
            integral_clamping: default_true(),
            output_rate_limit: None,
        }
    }
}

impl Default for ControlSettings {
    fn default() -> Self {
        Self {
            adaptive_control: false,
            deadband_k: 0.1,
            min_control_action: 0.1,
        }
    }
}

impl Default for EmergencySettings {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            emergency_temp_k: default_emergency_temp(),
            response_timeout_s: default_emergency_timeout(),
        }
    }
}

impl Default for GlobalThermalSettings {
    fn default() -> Self {
        Self {
            global_sampling_rate_hz: default_global_sampling_rate(),
            max_concurrent_regulators: default_max_regulators(),
            resource_sharing: ResourceSharingSettings::default(),
            monitoring: MonitoringSettings::default(),
        }
    }
}

impl Default for ResourceSharingSettings {
    fn default() -> Self {
        Self {
            i2c_arbitration_timeout_ms: default_arbitration_timeout(),
            max_operation_queue_size: default_max_queue_size(),
            priority_scheduling: false,
        }
    }
}

impl Default for MonitoringSettings {
    fn default() -> Self {
        Self {
            enable_performance_monitoring: default_true(),
            metrics_interval_s: default_metrics_interval(),
            enable_thermal_history: false,
            history_buffer_size: default_history_buffer_size(),
        }
    }
}
