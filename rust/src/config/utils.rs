// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Configuration utilities
//!
//! This module provides utility functions for working with configuration
//! settings, including validation and schema management.

use anyhow::{Context, Result};
use base64::Engine;
use log::debug;

use super::{Config, USER_SESSION_SEPARATOR};
use crate::utility::temperature_conversion::convert_voltage_to_temperature;

/// Output the embedded JSON schema to the console.
///
/// This function is called when the `--show-config-schema` flag is provided
/// on the command line. It outputs the full JSON schema for the configuration
/// to stdout, formatted for readability.
///
/// ### Example
///
/// ```bash
/// ./rust_photoacoustic --show-config-schema > config_schema.json
/// ```
pub fn output_config_schema() -> Result<()> {
    // Load the schema from the embedded string
    let schema_str = include_str!("../../resources/config.schema.json");

    // Parse the schema to a JSON Value to pretty-format it
    let schema: serde_json::Value =
        serde_json::from_str(schema_str).context("Failed to parse JSON schema")?;

    // Pretty-print the schema
    let formatted_schema =
        serde_json::to_string_pretty(&schema).context("Failed to format JSON schema")?;

    // Output to stdout
    println!("{}", formatted_schema);

    Ok(())
}

/// Check if a string is a valid IP address
///
/// Validates that a string represents a valid IPv4 or IPv6 address,
/// or is one of the special values like "localhost" or "0.0.0.0".
///
/// ### Arguments
///
/// * `addr` - The address string to validate
///
/// ### Returns
///
/// `true` if the address is valid, `false` otherwise
pub fn is_valid_ip_address(addr: &str) -> bool {
    if addr.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }

    // Special cases
    matches!(addr, "localhost" | "::" | "::0" | "0.0.0.0")
}

/// Validates the configuration against additional rules that aren't covered by the JSON schema.
///
/// This function performs deeper validation checks that can't be easily expressed in a JSON schema,
/// such as verifying that certificate and key pairs are both present, validating base64 encoding
/// of cryptographic material, checking user password hashes, and testing temperature conversion formulas.
///
/// ### Arguments
///
/// * `config` - The configuration object to validate
///
/// ### Returns
///
/// * `Ok(())` if all validations pass
/// * `Err(anyhow::Error)` with descriptive message if any validation fails
///
/// ### Validation Rules
///
/// This function validates:
///
/// - **SSL Configuration**: Ensures that if a certificate is provided, a key is also provided (and vice versa)
/// - **Base64 Encoding**: Validates that certificates, keys, and RS256 keys are valid base64-encoded strings
/// - **Port Range**: Ensures the visualization port is within a valid range (1-65534)
/// - **IP Address Format**: Checks if the provided address is a valid IP address or special value
/// - **User Credentials**: Validates that user password hashes are properly base64-encoded and follow
///   the expected format from `openssl passwd`
/// - **Temperature Formulas**: Tests temperature conversion formulas with sample voltages to ensure they
///   work correctly with the `convert_voltage_to_temperature` function
pub fn validate_specific_rules(config: &Config) -> Result<()> {
    debug!("Performing additional validation checks");

    // Validate SSL certificates
    if let Some(cert) = &config.visualization.cert {
        if config.visualization.key.is_none() {
            anyhow::bail!("SSL certificate provided without a key");
        }

        // Validate the cert is valid base64
        let _ = base64::engine::general_purpose::STANDARD
            .decode(cert)
            .context("SSL certificate is not valid base64")?;
    }

    if let Some(key) = &config.visualization.key {
        if config.visualization.cert.is_none() {
            anyhow::bail!("SSL key provided without a certificate");
        }

        // Validate the key is valid base64
        let _ = base64::engine::general_purpose::STANDARD
            .decode(key)
            .context("SSL key is not valid base64")?;
    }

    // Check value ranges for certain fields
    if config.visualization.port < 1 || config.visualization.port > 65534 {
        anyhow::bail!("Invalid port number: {}", config.visualization.port);
    }

    // Check if the address is in a valid format
    if !is_valid_ip_address(&config.visualization.address) {
        debug!(
            "Potentially invalid address format: {}",
            config.visualization.address
        );
        // Just issue a warning but don't block
    }

    // Validate the rs256_private_key and rs256_public_key they should some valid base64 encoded strings
    let _ = base64::engine::general_purpose::STANDARD
        .decode(&config.visualization.rs256_private_key)
        .context("RS256 private key is not valid base64")?;
    let _ = base64::engine::general_purpose::STANDARD
        .decode(&config.visualization.rs256_public_key)
        .context("RS256 public key is not valid base64")?;

    // if AccessConfig contains users, validate their credentials
    // User password should be a valid base64 string
    // the decoded string should be a valid password hash conforming to the openssl passwd -1 format
    // permissions should not contain the char USER_SESSION_SEPARATOR
    for user in &config.access.users {
        if !user.pass.is_empty() {
            let decoded_pass = base64::engine::general_purpose::STANDARD
                .decode(&user.pass)
                .context("User password is not valid base64")?;
            // Check if the decoded password is a valid hash
            // Password hash should start with $1$, $5$, $6$, $apr1$
            // Next contains the salt
            // The rest is the hash
            if !decoded_pass.starts_with(b"$1$")
                && !decoded_pass.starts_with(b"$5$")
                && !decoded_pass.starts_with(b"$6$")
                && !decoded_pass.starts_with(b"$apr1$")
            {
                anyhow::bail!("User password is not a valid hash, you should use openssl passwd -5 <password> | base64 -w0");
            }
        }
        for permission in &user.permissions {
            if permission.contains(USER_SESSION_SEPARATOR) {
                anyhow::bail!(
                    "User permission contains invalid character: {}",
                    USER_SESSION_SEPARATOR
                );
            }
        }
    }

    // Validate temperature conversion formulas
    debug!("Validating temperature conversion formulas");

    for regulator in &config.thermal_regulation.regulators {
        let formula = &regulator.temperature_conversion.formula;
        debug!(
            "Validating formula for regulator '{}': {}",
            regulator.name, formula
        );

        // Test the formula with a few test voltages to ensure it works correctly
        let test_voltages = [1.0, 2.5, 4.0]; // Test voltages in volts

        for &test_voltage in &test_voltages {
            match convert_voltage_to_temperature(formula.clone(), test_voltage) {
                Ok(temperature_k) => {
                    debug!(
                        "Formula validation for '{}': {}V -> {:.2}K ({:.2}°C)",
                        regulator.name,
                        test_voltage,
                        temperature_k,
                        temperature_k - 273.15
                    );
                }
                Err(e) => {
                    anyhow::bail!(
                        "Temperature conversion formula validation failed for regulator '{}' at {}V: {}. Formula: '{}'",
                        regulator.name,
                        test_voltage,
                        e,
                        formula
                    );
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::thermal_regulation::*;

    /// Créer une configuration minimale pour tester la validation des formules
    fn create_test_config_with_formula(formula: &str) -> Config {
        use crate::config::*;
        use std::collections::HashMap;

        let mut config = Config::default();

        // Créer juste le minimum nécessaire pour le test
        let temp_conversion = TemperatureConversionConfig {
            formula: formula.to_string(),
            adc_resolution: 16,
            voltage_reference: 5.0,
            conversion_type: ConversionType::NtcThermistor,
        };

        let temp_sensor = TemperatureSensorConfig {
            adc_address: 0x48,
            adc_channel: 0,
            sensor_type: TemperatureSensorType::ThermistorNtc,
        };

        // Configuration minimale pour les actuateurs
        let thermal_control = ThermalControlConfig {
            pwm_controller: PwmChannelConfig {
                address: 0x40,
                channel: 0,
            },
            direction_controller: DirectionControllerConfig {
                address: 0x20,
                gpio_pins: HBridgeGpioPins {
                    h_bridge_in1: 0,
                    h_bridge_in2: 1,
                    h_bridge_enable: 2,
                },
            },
            thermal_modes: ThermalModesConfig {
                heating_tec: ThermalModeConfig {
                    description: "Heating via TEC".to_string(),
                    h_bridge_direction: HBridgeDirection::Forward,
                    power_range: "0-80%".to_string(),
                    max_power_percent: 80.0,
                },
                cooling_tec: ThermalModeConfig {
                    description: "Cooling via TEC".to_string(),
                    h_bridge_direction: HBridgeDirection::Reverse,
                    power_range: "0-80%".to_string(),
                    max_power_percent: 80.0,
                },
                heating_resistive: ThermalModeConfig {
                    description: "Heating via resistive element".to_string(),
                    h_bridge_direction: HBridgeDirection::Forward,
                    power_range: "0-100%".to_string(),
                    max_power_percent: 100.0,
                },
            },
        };

        let regulator = ThermalRegulatorConfig {
            id: "test_regulator".to_string(),
            name: "Test Regulator".to_string(),
            enabled: true,
            i2c_bus: "primary".to_string(),
            temperature_sensor: temp_sensor,
            actuators: ThermalActuatorsConfig { thermal_control },
            temperature_conversion: temp_conversion,
            pid_parameters: PidParameters {
                kp: 1.0,
                ki: 0.1,
                kd: 0.01,
                setpoint: 298.15,
                output_min: -100.0,
                output_max: 100.0,
                integral_max: 1000.0,
                settings: PidSettings::default(),
            },
            control_parameters: ControlParameters {
                sampling_frequency_hz: 1.0,
                pwm_frequency_hz: 1000.0,
                settings: ControlSettings::default(),
            },
            safety_limits: SafetyLimits {
                min_temperature_k: 273.15,
                max_temperature_k: 373.15,
                max_heating_duty: 80.0,
                max_cooling_duty: 80.0,
                emergency_settings: EmergencySettings::default(),
            },
        };

        let mut i2c_buses = HashMap::new();
        i2c_buses.insert(
            "primary".to_string(),
            I2CBusConfig {
                bus_type: I2CBusType::Mock,
                device: "/dev/i2c-1".to_string(),
                usb_vendor_id: None,
                usb_product_id: None,
                pwm_controllers: vec![],
                adc_controllers: vec![],
                gpio_controllers: vec![],
                bus_settings: I2CBusSettings::default(),
            },
        );

        config.thermal_regulation = ThermalRegulationConfig {
            enabled: true,
            i2c_buses,
            regulators: vec![regulator],
            global_settings: GlobalThermalSettings::default(),
        };

        config
    }

    #[test]
    fn test_validate_temperature_formula_valid() {
        // Test avec une formule valide NTC
        let formula = "1.0 / (1.0 / 298.15 + math::ln(10000.0 * voltage / (5.0 - voltage) / 10000.0) / 3977.0)";
        let config = create_test_config_with_formula(formula);

        // La validation devrait réussir
        assert!(validate_specific_rules(&config).is_ok());
    }

    #[test]
    fn test_validate_temperature_formula_invalid() {
        // Test avec une formule invalide
        let formula = "invalid_formula_without_voltage";
        let config = create_test_config_with_formula(formula);

        // La validation devrait échouer
        assert!(validate_specific_rules(&config).is_err());
    }

    #[test]
    fn test_validate_temperature_formula_no_voltage() {
        // Test avec une formule qui ne contient pas 'voltage'
        let formula = "273.15 + 10.0"; // Formule sans variable voltage
        let config = create_test_config_with_formula(formula);

        // La validation devrait échouer
        let result = validate_specific_rules(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("voltage"));
    }

    #[test]
    fn test_validate_temperature_formula_malformed() {
        // Test avec une formule malformée
        let formula = "1.0 / (1.0 / 298.15 + math::ln(10000.0 * voltage / (5.0 - voltage) / 10000.0) / 3977.0"; // Parenthèse manquante
        let config = create_test_config_with_formula(formula);

        // La validation devrait échouer
        let result = validate_specific_rules(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_temperature_formula_simple_linear() {
        // Test avec une formule linéaire simple
        let formula = "273.15 + voltage * 10.0";
        let config = create_test_config_with_formula(formula);

        // La validation devrait réussir
        assert!(validate_specific_rules(&config).is_ok());
    }
}
