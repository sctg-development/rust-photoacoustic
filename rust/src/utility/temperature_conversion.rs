use anyhow::{anyhow, Result};
use evalexpr::{
    eval_with_context, Context, ContextWithMutableVariables, DefaultNumericTypes, HashMapContext,
    Value,
};

/// Convert voltage to temperature using a configurable mathematical formula
///
/// This function evaluates a mathematical expression where 'voltage' is available as a variable.
/// The formula should return temperature in Kelvin.
///
/// # Arguments
/// * `formula` - Mathematical expression as string (e.g., "1.0 / (1.0 / 298.15 + ln(10000.0 * voltage / (5.0 - voltage) / 10000.0) / 3977.0)")
/// * `voltage` - Input voltage in volts
///
/// # Returns
/// * `Result<f64>` - Temperature in Kelvin, or error if formula evaluation fails
///
/// # Example
/// ```rust
/// use rust_photoacoustic::utility::temperature_conversion::convert_voltage_to_temperature;
///
/// // NTC formula for 10kΩ NTC with β=3977, 10kΩ voltage divider, 5V supply
///        let formula = "1.0 / (1.0 / 298.15 + math::ln(10000.0 * voltage / (5.0 - voltage) / 10000.0) / 3977.0)".to_string();
/// let temp_k = convert_voltage_to_temperature(formula, 2.5).unwrap();
/// assert!((temp_k - 298.15).abs() < 1.0); // Should be close to 25°C (298.15K)
/// ```
pub fn convert_voltage_to_temperature(formula: String, voltage: f32) -> Result<f64> {
    // Validate input voltage
    if voltage < 0.0 || voltage > 10.0 {
        return Err(anyhow!(
            "Invalid voltage: {:.3}V (must be between 0V and 10V)",
            voltage
        ));
    }

    // Validate formula contains 'voltage' variable
    if !formula.contains("voltage") {
        return Err(anyhow!(
            "Formula must contain 'voltage' variable, got: '{}'",
            formula
        ));
    }

    // Create evaluation context with voltage variable
    let mut context = HashMapContext::<DefaultNumericTypes>::new();
    context.set_builtin_functions_disabled(false).unwrap();
    context.set_value("voltage".into(), Value::Float(voltage as f64))?;

    // Evaluate the formula
    let result = eval_with_context(&formula, &context).map_err(|e| {
        anyhow!(
            "Failed to evaluate temperature formula '{}': {}",
            formula,
            e
        )
    })?;

    // Convert result to f64
    let temperature_k = result.as_float().or_else(|_| {
        Err(anyhow!(
            "Formula did not return a numeric value: '{}'",
            formula
        ))
    })?;

    // Validate result (reasonable temperature range in Kelvin: -50°C to 100°C)
    if temperature_k < 223.15 || temperature_k > 373.15 {
        return Err(anyhow!(
            "Calculated temperature {:.2}K ({:.2}°C) is outside reasonable range (-50°C to 100°C)",
            temperature_k,
            temperature_k - 273.15
        ));
    }

    Ok(temperature_k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_convert_voltage_to_temperature_ntc_formula() {
        // Formula for 10kΩ NTC with β=3977, 10kΩ voltage divider, 5V supply
        let formula =
            "1.0 / (1.0 / 298.15 + math::ln(10000.0 * voltage / (5.0 - voltage) / 10000.0) / 3977.0)"
                .to_string();

        // Test at 2.5V (should be close to 25°C = 298.15K for balanced voltage divider)
        let temp_k = convert_voltage_to_temperature(formula.clone(), 2.5).unwrap();
        assert_relative_eq!(temp_k, 298.15, epsilon = 1.0);

        // Test at other voltages
        let temp_k_low = convert_voltage_to_temperature(formula.clone(), 1.0).unwrap();
        let temp_k_high = convert_voltage_to_temperature(formula.clone(), 4.0).unwrap();

        // Lower voltage should mean higher temperature (NTC characteristic)
        assert!(temp_k_low > temp_k_high);
    }

    #[test]
    fn test_convert_voltage_to_temperature_simple_linear() {
        // Simple linear formula: temperature = 273.15 + voltage * 10 (10°C per volt)
        let formula = "273.15 + voltage * 10.0".to_string();

        let temp_k = convert_voltage_to_temperature(formula, 2.5).unwrap();
        assert_relative_eq!(temp_k, 298.15, epsilon = 0.001); // 273.15 + 2.5 * 10 = 298.15K
    }

    #[test]
    fn test_convert_voltage_to_temperature_invalid_voltage() {
        let formula = "273.15 + voltage * 10.0".to_string();

        // Test negative voltage
        assert!(convert_voltage_to_temperature(formula.clone(), -1.0).is_err());

        // Test excessive voltage
        assert!(convert_voltage_to_temperature(formula, 15.0).is_err());
    }

    #[test]
    fn test_convert_voltage_to_temperature_invalid_formula() {
        // Test malformed formula
        assert!(convert_voltage_to_temperature("invalid formula".to_string(), 2.5).is_err());

        // Test formula with undefined variable
        assert!(
            convert_voltage_to_temperature("273.15 + unknown_var * 10.0".to_string(), 2.5).is_err()
        );
    }

    #[test]
    fn test_convert_voltage_to_temperature_unreasonable_result() {
        // Formula that would produce unreasonable temperature
        let formula = "1000.0".to_string(); // 1000K = 726.85°C (too hot)
        assert!(convert_voltage_to_temperature(formula, 2.5).is_err());

        let formula_cold = "100.0".to_string(); // 100K = -173.15°C (too cold)
        assert!(convert_voltage_to_temperature(formula_cold, 2.5).is_err());
    }

    #[test]
    fn test_convert_voltage_to_temperature_non_numeric_result() {
        // Formula that returns string (should fail)
        let formula = "\"not a number\"".to_string();
        assert!(convert_voltage_to_temperature(formula, 2.5).is_err());
    }
}
