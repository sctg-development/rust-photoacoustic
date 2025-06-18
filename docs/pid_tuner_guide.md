# PID Tuner Binary - Comprehensive Guide

## Overview

The PID Tuner is a specialized binary tool designed for automatic tuning of PID (Proportional-Integral-Derivative) controllers in thermal regulation systems. It uses classical control theory algorithms to analyze system step responses and calculate optimal PID parameters for stable and responsive temperature control.

### Key Features

- **Automated PID Parameter Calculation**: Uses proven algorithms (Ziegler-Nichols, Cohen-Coon)
- **Step Response Analysis**: Performs controlled step tests to characterize system dynamics
- **Safety-First Design**: Only works with mock drivers to prevent damage to real hardware
- **Comprehensive Reporting**: Generates HTML reports with SVG charts and performance metrics
- **Multiple Tuning Algorithms**: Choose the best method for your specific application

### Target Audience

This guide is intended for:
- **Rust Developers**: Working on thermal control systems and PID implementations
- **System Integrators**: Configuring photoacoustic equipment for production use
- **Control Engineers**: Tuning PID parameters for optimal thermal regulation performance

## Quick Start

### Basic Usage

```bash
# Tune a regulator using default Ziegler-Nichols method
cargo run --bin pid_tuner -- --regulator-id dev_cell_temperature

# Tune with Cohen-Coon method and generate HTML report
cargo run --bin pid_tuner -- \
    --regulator-id dev_cell_temperature \
    --method cohen-coon \
    --output tuning_report.html

# Custom step amplitude and duration
cargo run --bin pid_tuner -- \
    --regulator-id dev_cell_temperature \
    --step-amplitude 8.0 \
    --duration 600 \
    --output detailed_report.html
```

### Prerequisites

1. **Configuration File**: Ensure your `config.yaml` contains:
   - Thermal regulation enabled
   - Mock I2C bus configuration
   - Regulator definition with proper sensor and actuator mappings

2. **Safety Check**: The tuner will only work with mock drivers to prevent hardware damage during testing.

## Command Line Interface

### Arguments

| Argument | Short | Default | Description |
|----------|-------|---------|-------------|
| `--config` | `-c` | `config.yaml` | Path to configuration file |
| `--regulator-id` | `-r` | *required* | ID of the thermal regulator to tune |
| `--method` | `-m` | `ziegler-nichols` | Tuning algorithm to use |
| `--output` | `-o` | *none* | Output HTML report file path |
| `--step-amplitude` | `-s` | `5.0` | Step input amplitude (°C) |
| `--duration` | `-d` | `300` | Test duration in seconds |
| `--interactive` | `-i` | `false` | Enable interactive mode (future feature) |
| `--verbose` | `-v` | `false` | Enable verbose logging |

### Tuning Methods

The tuner supports three algorithms:

- `ziegler-nichols`: Classical Ziegler-Nichols step response method
- `cohen-coon`: Cohen-Coon method for processes with significant dead time
- `manual`: Interactive manual tuning (not yet implemented)

## Tuning Algorithms

### Ziegler-Nichols Method

**Best for**: General-purpose applications with moderate dynamics.

The Ziegler-Nichols step response method is based on analyzing the system's reaction to a step input. It characterizes the process using three key parameters:

- **Process Gain (K)**: Ratio of output change to input change at steady state
- **Time Constant (τ)**: Time for the system to reach 63.2% of its final value
- **Dead Time (L)**: Time delay before the system begins to respond

#### Calculation Formulas

```
Kp = 1.2 × (τ / (K × L))
Ki = Kp / (2 × L)
Kd = Kp × L / 2
```

#### Characteristics

- **Pros**: 
  - Simple and well-established
  - Good general-purpose performance
  - Works well for first-order plus dead time systems
- **Cons**: 
  - Can be aggressive (high overshoot)
  - May not be optimal for processes with large dead times
  - Limited performance for complex dynamics

#### Typical Applications

- Temperature control with moderate time constants
- Systems with dead time < 25% of time constant
- General industrial process control

### Cohen-Coon Method

**Best for**: Processes with significant dead time relative to time constant.

The Cohen-Coon method provides improved performance for processes where dead time is a significant portion of the total response time. It uses more sophisticated formulas that account for the dead time to time constant ratio.

#### Calculation Formulas

```
L/τ ratio = Dead Time / Time Constant

Kp = (1/K) × (τ/L) × (16 + 3×(L/τ)) / (13 + 8×(L/τ))
Ki = Kp / (L × (32 + 6×(L/τ)) / (13 + 8×(L/τ)))
Kd = Kp × L × 4 / (11 + 2×(L/τ))
```

#### Characteristics

- **Pros**: 
  - Better performance for high dead time processes
  - More conservative than Ziegler-Nichols
  - Reduced overshoot and oscillation
- **Cons**: 
  - More complex calculations
  - May be slower responding than Ziegler-Nichols
  - Requires accurate dead time identification

#### Typical Applications

- Thermal systems with significant thermal mass
- Processes with dead time > 25% of time constant
- Systems requiring stable, non-oscillatory response

### Method Selection Guidelines

| Process Characteristics | Recommended Method | Reasoning |
|------------------------|-------------------|-----------|
| Fast thermal response, low dead time | Ziegler-Nichols | Simple, effective for responsive systems |
| Slow thermal response, high thermal mass | Cohen-Coon | Better handling of dead time effects |
| Critical stability requirements | Cohen-Coon | More conservative tuning |
| General-purpose applications | Ziegler-Nichols | Well-established, good starting point |

## Step Response Analysis

### Process Identification

The tuner performs a step response test to identify system characteristics:

1. **Pre-stabilization**: 10 seconds at initial conditions
2. **Step Application**: Sudden change in control output
3. **Response Recording**: 1 Hz sampling of temperature response
4. **Analysis**: Mathematical extraction of process parameters

### Key Metrics Extracted

| Metric | Description | Control Impact |
|--------|-------------|----------------|
| **Process Gain** | Steady-state output/input ratio | Determines proportional gain magnitude |
| **Time Constant** | Time to reach 63.2% of final value | Affects integral and derivative timing |
| **Dead Time** | Initial response delay | Critical for stability margins |
| **Rise Time** | Time from 10% to 90% of final value | Indicates system responsiveness |
| **Settling Time** | Time to reach ±2% of final value | Shows control effectiveness |
| **Overshoot** | Maximum excursion beyond final value | Indicates stability margins |

### Performance Evaluation

The tuner automatically evaluates controller performance:

- **Overshoot > 20%**: Warning about potential instability
- **Settling Time > 180s**: Suggestion to increase integral gain
- **Steady-State Error > 5%**: Recommendation to increase integral action
- **Good Balance**: Overshoot < 5% and settling time < 120s

## Configuration Requirements

### I2C Bus Configuration

```yaml
thermal_regulation:
  enabled: true
  i2c_buses:
    mock_bus:
      bus_type: Mock
      device: "mock"
      pwm_controllers:
        - address: 0x40
          channels: 16
          frequency_hz: 1000
      adc_controllers:
        - address: 0x48
          channels: 4
          resolution: 16
          voltage_ref: 5.0
```

### Regulator Configuration

```yaml
  regulators:
    - id: "dev_cell_temperature"
      name: "Development Cell Temperature"
      i2c_bus: "mock_bus"
      temperature_sensor:
        sensor_type: Ntc
        adc_address: 0x48
        adc_channel: 0
        # NTC thermistor parameters (10kΩ at 25°C, β=3977)
        ntc_beta: 3977.0
        ntc_r0: 10000.0
        ntc_t0: 25.0
        series_resistor: 10000.0
        supply_voltage: 5.0
      actuators:
        thermal_control:
          actuator_type: Resistive
          pwm_controller:
            address: 0x40
            channel: 0
          power_rating: 60.0  # DBK HPG-1/10-60x35-12-24V
```

## Output and Reporting

### Console Output

The tuner provides comprehensive console output:

```
🎯 PID Tuning Results (Ziegler-Nichols):
   ╭─────────────────────────────────────────╮
   │  Kp = 2.458620                          │
   │  Ki = 0.245862                          │
   │  Kd = 6.146551                          │
   ╰─────────────────────────────────────────╯

📊 Performance Metrics:
   • Process Gain:       0.850
   • Time Constant:      90.0 s
   • Dead Time:          5.0 s
   • Rise Time:          45.2 s
   • Settling Time:      120.8 s
   • Overshoot:          12.3 %
   • Steady-State Error: 1.45 %

💡 Recommendations:
   ✅ Good balance between stability and response time.
```

### HTML Report Generation

When `--output` is specified, the tuner generates a comprehensive HTML report:

- **Executive Summary**: Tuning parameters and method used
- **Step Response Chart**: SVG plot of temperature vs. time
- **Performance Metrics**: Detailed analysis of system characteristics
- **Parameter Comparison**: Side-by-side comparison if multiple methods are used
- **Recommendations**: Specific guidance for parameter adjustment

## Implementation for Developers

### Mock Driver Integration

The PID tuner integrates with the mock thermal driver to provide realistic simulation:

```rust
// Thermal properties for 1016g stainless steel 316 cell
mass_g: 1016.0,
specific_heat: 501.0,  // J/kg·K for stainless steel 316
thermal_conductivity: 16.2,  // W/m·K
heat_transfer_coefficient: 25.0,  // W/m²·K
```

### NTC Thermistor Simulation

The mock driver simulates a 10kΩ NTC thermistor (β=3977) with voltage divider:

```rust
let r_ntc = r0 * ((beta * (1.0 / temp_k - 1.0 / t0)).exp());
let v_adc = 5.0 * r_ntc / (10000.0 + r_ntc);
let adc_raw = ((v_adc / 5.0) * 65535.0) as u16;
```

### Heating Element Modeling

60W DBK HPG-1/10-60x35-12-24V resistive heater simulation:

```rust
let heater_heat = self.heater_power / 100.0 * 60.0;  // Watts
let temp_change = total_heat_rate * dt / thermal_mass;  // K
```

## Best Practices

### For Rust Developers

1. **Test with Mock First**: Always use mock drivers for initial tuning
2. **Validate Parameters**: Check that calculated parameters are within reasonable ranges
3. **Monitor Performance**: Use the HTML reports to verify tuning effectiveness
4. **Iterative Approach**: Start with conservative parameters and refine as needed

### For System Integrators

1. **Characterize Your System**: Understand your thermal mass, dead times, and response characteristics
2. **Choose Appropriate Method**: Use Cohen-Coon for high dead time systems, Ziegler-Nichols for general use
3. **Validate in Practice**: Test tuned parameters with real hardware under controlled conditions
4. **Document Settings**: Keep records of tuning parameters and system characteristics

### Safety Considerations

- **Mock Driver Only**: Never run tuning on real hardware without proper safety measures
- **Parameter Limits**: The tuner applies reasonable limits to prevent extreme values
- **Gradual Implementation**: When applying tuned parameters to real hardware, start with reduced gains
- **Monitor Stability**: Watch for oscillations or instability when implementing new parameters

## Troubleshooting

### Common Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| "Regulator not found" | Incorrect regulator ID | Verify ID matches config.yaml |
| "Not using mock driver" | Safety check failed | Ensure I2C bus is configured as Mock type |
| "Process gain too small" | Insufficient step response | Increase step amplitude or check sensor |
| "Invalid time constant" | Poor step response data | Increase test duration or check system |

### Debug Mode

Use `--verbose` flag for detailed logging:

```bash
cargo run --bin pid_tuner -- \
    --regulator-id dev_cell_temperature \
    --verbose
```

This provides detailed information about:
- Step response data collection
- Parameter calculation steps
- Internal algorithm decisions
- Thermal simulation updates

## Future Enhancements

### Planned Features

- **Interactive Manual Tuning**: Real-time parameter adjustment with live feedback
- **Multiple Algorithm Comparison**: Automatic testing of all methods with comparison
- **Advanced Algorithms**: Implementation of Lambda tuning, IMC, and other modern methods
- **Closed-Loop Tuning**: Relay feedback and ultimate cycle methods
- **Real Hardware Support**: Safe tuning procedures for production systems

### Extending the Tuner

The modular design allows easy extension:

```rust
// Add new tuning algorithm
pub struct MyCustomCalculator;

impl MyCustomCalculator {
    pub fn calculate_pid_parameters(metrics: &PerformanceMetrics) -> Result<PidParameters> {
        // Implement your algorithm
    }
}
```

## References

### Technical References

- Ziegler, J.G. and Nichols, N.B. (1942). "Optimum Settings for Automatic Controllers"
- Cohen, G.H. and Coon, G.A. (1953). "Theoretical Consideration of Retarded Control"
- Åström, Karl J. and Hägglund, Tore (2006). "Advanced PID Control"

### Related Documentation

- [Thermal Regulation Guide](regulation_thermique.md)
- [Mock Driver Documentation](mock_driver_guide.md)
- [Configuration Reference](config_reference.md)

## License

Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development

This documentation is part of the rust-photoacoustic project and is licensed under the SCTG Development Non-Commercial License v1.0.
