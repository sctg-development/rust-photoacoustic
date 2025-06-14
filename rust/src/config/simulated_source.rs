// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Configuration for simulated photoacoustic sources
//!
//! This module defines configuration structures for both `MockSource` and the new
//! `SimulatedPhotoacousticRealtimeAudioSource` that uses the universal photoacoustic
//! generator function.

use rocket_okapi::JsonSchema;
use serde::{Deserialize, Serialize};
/// Configuration for simulated photoacoustic sources
///
/// This structure configures all parameters for generating synthetic photoacoustic signals
/// using either the simple mock approach or the comprehensive universal photoacoustic
/// simulation. It supports both the existing `MockSource` and the new
/// `SimulatedPhotoacousticRealtimeAudioSource`.
///
/// ### Physics Parameters
///
/// **For Physics PhD Specialists:**
/// The configuration models a Helmholtz resonance cell photoacoustic analyzer with:
/// - Resonance frequency typically around 2 kHz for optimal acoustic coupling
/// - Dual microphone differential configuration for noise rejection
/// - Laser modulation creating photoacoustic waves through molecular absorption
/// - Gas flow introducing 1/f noise characteristics
/// - Temperature effects causing frequency and phase drift
/// - SNR control for signal quality assessment
///
/// **For Rust Developers:**
/// This structure provides all parameters needed to configure the `generate_universal_photoacoustic_stereo`
/// function, with sensible defaults for typical photoacoustic measurement scenarios.
///
/// ### Examples
///
/// ```no_run
/// use rust_photoacoustic::config::SimulatedSourceConfig;
///
/// // Configuration for simple mock source
/// let mock_config = SimulatedSourceConfig {
///     source_type: "mock".to_string(),
///     correlation: 0.8,
///     ..Default::default()
/// };
///
/// // Configuration for comprehensive physics simulation
/// let universal_config = SimulatedSourceConfig {
///     source_type: "universal".to_string(),
///     background_noise_amplitude: 0.1,
///     resonance_frequency: 2100.0,
///     laser_modulation_depth: 0.9,
///     signal_amplitude: 0.7,
///     phase_opposition_degrees: 175.0,
///     temperature_drift_factor: 0.02,
///     gas_flow_noise_factor: 0.3,
///     snr_factor: 25.0,
///     modulation_mode: "amplitude".to_string(),
///     pulse_width_seconds: 0.005,
///     pulse_frequency_hz: 100.0,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SimulatedSourceConfig {
    /// Source type: "mock" for simple MockSource or "universal" for full physics simulation
    ///
    /// **Source Options:**
    /// - "mock": Uses the existing `MockSource` with simple correlation-based signal generation
    /// - "universal": Uses the new `SimulatedPhotoacousticRealtimeAudioSource` with comprehensive
    ///   physics simulation including Helmholtz resonance, gas flow noise, and thermal drift
    ///
    /// **For Physics PhD Specialists:**
    /// Use "mock" for quick testing and "universal" for realistic photoacoustic physics modeling.
    ///
    /// **For Rust Developers:**
    /// "mock" provides lightweight simulation, "universal" provides full feature simulation.
    #[serde(default = "default_source_type")]
    pub source_type: String,

    /// Correlation coefficient between channels for simple mock mode (0.0 to 1.0)
    ///
    /// This parameter is used by the existing `MockSource` for backward compatibility.
    /// It controls the statistical correlation between left and right channels:
    /// - 1.0: Perfectly correlated (identical channels)
    /// - 0.0: Uncorrelated (independent channels)
    /// Only used when source_type is "mock".
    #[serde(default = "default_correlation")]
    pub correlation: f32,

    /// Background noise amplitude for gas flow and environmental noise (0.0 to 1.0)
    ///
    /// Controls the amplitude of background noise including:
    /// - Gas flow turbulence (1/f characteristics)
    /// - Environmental acoustic interference
    /// - Electronic noise in the measurement system
    #[serde(default = "default_background_noise_amplitude")]
    pub background_noise_amplitude: f32,

    /// Resonance frequency of the Helmholtz cell in Hz
    ///
    /// **Physics Background:**
    /// The Helmholtz resonance frequency is determined by the cell geometry and gas properties.
    /// Typical values range from 1.5 to 3 kHz for optimal photoacoustic coupling.
    /// The resonance enhances the photoacoustic signal at this frequency.
    #[serde(default = "default_resonance_frequency")]
    pub resonance_frequency: f32,

    /// Laser modulation depth (0.0 to 1.0)
    ///
    /// Controls the depth of laser intensity modulation creating the photoacoustic effect.
    /// Higher values produce stronger photoacoustic signals but may introduce nonlinearities.
    #[serde(default = "default_laser_modulation_depth")]
    pub laser_modulation_depth: f32,

    /// Photoacoustic signal amplitude (0.0 to 1.0)
    ///
    /// Sets the overall amplitude of the photoacoustic signal relative to background noise.
    /// This represents the signal strength that would be observed with a given analyte concentration.
    #[serde(default = "default_signal_amplitude")]
    pub signal_amplitude: f32,

    /// Phase opposition between microphones in degrees
    ///
    /// **Physics Background:**
    /// In a differential photoacoustic cell, microphones are positioned to be in approximate
    /// phase opposition (180°). Small deviations (170-185°) are realistic due to:
    /// - Manufacturing tolerances in microphone positioning
    /// - Temperature-induced changes in gas properties
    /// - Cell geometry variations
    #[serde(default = "default_phase_opposition_degrees")]
    pub phase_opposition_degrees: f32,

    /// Temperature drift factor affecting phase and frequency stability (0.0 to 0.1)
    ///
    /// **Physics Background:**
    /// Temperature variations affect:
    /// - Gas density and sound velocity (frequency drift)
    /// - Thermal expansion of cell components (phase drift)
    /// - Molecular absorption characteristics
    /// Typical values: 0.01-0.05 for laboratory conditions
    #[serde(default = "default_temperature_drift_factor")]
    pub temperature_drift_factor: f32,

    /// Gas flow noise factor for 1/f characteristics (0.0 to 1.0)
    ///
    /// **Physics Background:**
    /// Gas circulation in photoacoustic cells introduces characteristic 1/f noise due to:
    /// - Turbulent flow patterns
    /// - Pressure fluctuations
    /// - Flow-induced vibrations
    /// Higher values simulate more turbulent conditions.
    #[serde(default = "default_gas_flow_noise_factor")]
    pub gas_flow_noise_factor: f32,

    /// Signal-to-noise ratio factor in dB
    ///
    /// Controls the overall signal quality by setting the ratio between the
    /// photoacoustic signal and background noise. Typical values:
    /// - 10-20 dB: Poor conditions (high noise environment)
    /// - 20-30 dB: Normal operation
    /// - 30+ dB: Excellent conditions (low noise, high concentration)
    #[serde(default = "default_snr_factor")]
    pub snr_factor: f32,

    /// Laser modulation mode: "amplitude" or "pulsed"
    ///
    /// **Physics Background:**
    /// - "amplitude": Continuous amplitude modulation at resonance frequency
    /// - "pulsed": Periodic pulsed operation with configurable pulse width and frequency
    ///
    /// Pulsed mode allows for different measurement techniques and can provide
    /// better temporal resolution for concentration measurements.
    #[serde(default = "default_modulation_mode")]
    pub modulation_mode: String,

    /// Pulse width in seconds (for pulsed mode)
    ///
    /// Duration of each laser pulse when using pulsed modulation mode.
    /// Typical values: 0.001-0.01 seconds (1-10 ms)
    #[serde(default = "default_pulse_width_seconds")]
    pub pulse_width_seconds: f32,

    /// Pulse frequency in Hz (for pulsed mode)
    ///
    /// Repetition rate of laser pulses when using pulsed modulation mode.
    /// Should be much lower than the resonance frequency.
    /// Typical values: 10-1000 Hz
    #[serde(default = "default_pulse_frequency_hz")]
    pub pulse_frequency_hz: f32,
}

impl Default for SimulatedSourceConfig {
    fn default() -> Self {
        Self {
            source_type: default_source_type(),
            correlation: default_correlation(),
            background_noise_amplitude: default_background_noise_amplitude(),
            resonance_frequency: default_resonance_frequency(),
            laser_modulation_depth: default_laser_modulation_depth(),
            signal_amplitude: default_signal_amplitude(),
            phase_opposition_degrees: default_phase_opposition_degrees(),
            temperature_drift_factor: default_temperature_drift_factor(),
            gas_flow_noise_factor: default_gas_flow_noise_factor(),
            snr_factor: default_snr_factor(),
            modulation_mode: default_modulation_mode(),
            pulse_width_seconds: default_pulse_width_seconds(),
            pulse_frequency_hz: default_pulse_frequency_hz(),
        }
    }
}

// Default value functions for serde
fn default_source_type() -> String {
    "mock".to_string() // Default to simple mock for backward compatibility
}

fn default_correlation() -> f32 {
    0.7 // 70% correlation between channels for mock mode
}

fn default_background_noise_amplitude() -> f32 {
    0.15 // 15% background noise amplitude
}

fn default_resonance_frequency() -> f32 {
    2100.0 // 2.1 kHz typical Helmholtz resonance
}

fn default_laser_modulation_depth() -> f32 {
    0.8 // 80% modulation depth
}

fn default_signal_amplitude() -> f32 {
    0.6 // 60% signal amplitude
}

fn default_phase_opposition_degrees() -> f32 {
    175.0 // 5° off perfect opposition (realistic)
}

fn default_temperature_drift_factor() -> f32 {
    0.02 // 2% temperature variation factor
}

fn default_gas_flow_noise_factor() -> f32 {
    0.3 // 30% gas flow noise contribution
}

fn default_snr_factor() -> f32 {
    25.0 // 25 dB SNR (good conditions)
}

fn default_modulation_mode() -> String {
    "amplitude".to_string() // Default to amplitude modulation
}

fn default_pulse_width_seconds() -> f32 {
    0.005 // 5 ms pulse width
}

fn default_pulse_frequency_hz() -> f32 {
    100.0 // 100 Hz pulse frequency
}
