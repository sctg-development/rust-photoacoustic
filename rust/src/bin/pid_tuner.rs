// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! PID Tuner Binary
//!
//! This binary provides automatic PID parameter tuning for thermal regulation
//! systems using various algorithms including Ziegler-Nichols and Cohen-Coon.
//! It generates HTML reports with SVG charts showing the tuning process.

use anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};
use log::info;
use rust_photoacoustic::config::{
    thermal_regulation::{I2CBusType, ThermalRegulatorConfig},
    Config,
};
use std::path::PathBuf;

mod pid_tuner_helper;
use crate::pid_tuner_helper::html_report::HtmlReportGenerator;
use crate::pid_tuner_helper::step_response::StepResponseTest;
use crate::pid_tuner_helper::tuning_algorithms::*;

/// PID Tuning algorithms supported
#[derive(Debug, Clone, ValueEnum)]
pub enum TuningAlgorithm {
    /// Ziegler-Nichols step response method
    ZieglerNichols,
    /// Cohen-Coon step response method
    CohenCoon,
    /// Manual tuning (interactive - TODO)
    Manual,
}

/// PID Tuner CLI arguments
#[derive(Parser, Debug)]
#[command(name = "pid_tuner")]
#[command(about = "PID parameter tuning for thermal regulation systems")]
#[command(version = "1.0.0")]
pub struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config.yaml")]
    pub config: PathBuf,

    /// Thermal regulator ID to tune
    #[arg(short, long)]
    pub regulator_id: String,

    /// Tuning algorithm to use
    #[arg(short = 'm', long, value_enum, default_value = "ziegler-nichols")]
    pub method: TuningAlgorithm,

    /// Output HTML report file
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Step input amplitude (in temperature units)
    #[arg(short, long, default_value = "5.0")]
    pub step_amplitude: f64,

    /// Test duration in seconds
    #[arg(short = 'd', long, default_value = "300")]
    pub duration: u64,

    /// Interactive mode (for manual tuning)
    #[arg(short, long)]
    pub interactive: bool,

    /// Enable tuning real driver (default is mock driver only)
    #[arg(long, default_value = "false")]
    pub real_regulator: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,
}

/// Main tuning result structure
#[derive(Debug, Clone)]
pub struct TuningResult {
    pub kp: f64,
    pub ki: f64,
    pub kd: f64,
    pub algorithm: String,
    pub step_response: StepResponseData,
    pub performance_metrics: PerformanceMetrics,
}

/// Performance metrics for the tuning
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub rise_time: f64,
    pub settling_time: f64,
    pub overshoot: f64,
    pub steady_state_error: f64,
    pub process_gain: f64,
    pub time_constant: f64,
    pub dead_time: f64,
}

/// Step response data for plotting
#[derive(Debug, Clone)]
pub struct StepResponseData {
    pub time: Vec<f64>,
    pub temperature: Vec<f64>,
    pub setpoint: Vec<f64>,
    pub control_output: Vec<f64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    std::env::set_var("RUST_LOG", log_level);
    env_logger::init();

    info!("Starting PID Tuner v1.0.0");
    info!("Configuration file: {:?}", args.config);
    info!("Regulator ID: {}", args.regulator_id);
    info!("Tuning method: {:?}", args.method);

    // Load configuration
    let config = load_config(&args.config).await?;

    // Find the specified regulator
    let regulator_config = find_regulator_config(&config, &args.regulator_id)?;

    if !args.real_regulator {
        // Verify it's using a mock driver for safety
        verify_mock_driver(&config, regulator_config)?;
    } else {
        info!("Real regulator tuning enabled - ensure safety precautions are followed");
    }

    info!(
        "Found regulator '{}' with driver {}",
        args.regulator_id, regulator_config.i2c_bus
    );

    // Perform the tuning
    let tuning_result = match args.method {
        TuningAlgorithm::ZieglerNichols => {
            info!("Running Ziegler-Nichols tuning...");
            perform_ziegler_nichols_tuning(&config, regulator_config, &args).await?
        }
        TuningAlgorithm::CohenCoon => {
            info!("Running Cohen-Coon tuning...");
            perform_cohen_coon_tuning(&config, regulator_config, &args).await?
        }
        TuningAlgorithm::Manual => {
            if args.interactive {
                info!("Starting interactive manual tuning...");
                return Err(anyhow!("Manual tuning not yet implemented - TODO"));
            } else {
                return Err(anyhow!("Manual tuning requires --interactive flag"));
            }
        }
    };

    // Display results
    display_tuning_results(&tuning_result);

    // Generate HTML report if requested
    if let Some(output_path) = args.output {
        info!("Generating HTML report: {:?}", output_path);
        generate_html_report(&tuning_result, &output_path)?;
        info!("HTML report generated successfully");
    }

    info!("PID tuning completed successfully");
    Ok(())
}

/// Load configuration from file
async fn load_config(config_path: &PathBuf) -> Result<Config> {
    let config_content = tokio::fs::read_to_string(config_path)
        .await
        .map_err(|e| anyhow!("Failed to read config file: {}", e))?;

    let config: Config = serde_yml::from_str(&config_content)
        .map_err(|e| anyhow!("Failed to parse config file: {}", e))?;

    if !config.thermal_regulation.enabled {
        return Err(anyhow!("Thermal regulation is disabled in configuration"));
    }

    Ok(config)
}

/// Find the specified regulator configuration
fn find_regulator_config<'a>(
    config: &'a Config,
    regulator_id: &str,
) -> Result<&'a ThermalRegulatorConfig> {
    let thermal_config = &config.thermal_regulation;

    thermal_config
        .regulators
        .iter()
        .find(|r| r.id == regulator_id)
        .ok_or_else(|| anyhow!("Regulator '{}' not found in configuration", regulator_id))
}

/// Verify that the regulator is using a mock driver for safety
fn verify_mock_driver(config: &Config, regulator_config: &ThermalRegulatorConfig) -> Result<()> {
    let thermal_config = &config.thermal_regulation;

    let bus_config = thermal_config
        .i2c_buses
        .get(&regulator_config.i2c_bus)
        .ok_or_else(|| anyhow!("I2C bus '{}' not found", regulator_config.i2c_bus))?;

    if !matches!(bus_config.bus_type, I2CBusType::Mock) {
        return Err(anyhow!(
            "Safety check failed: Regulator '{}' is not using a mock driver. \
            PID tuning should only be performed with mock drivers for safety.",
            regulator_config.id
        ));
    }

    Ok(())
}

/// Perform Ziegler-Nichols tuning
async fn perform_ziegler_nichols_tuning(
    config: &Config,
    regulator_config: &ThermalRegulatorConfig,
    args: &Args,
) -> Result<TuningResult> {
    let mut step_test = StepResponseTest::new(config, regulator_config).await?;

    info!("Performing step response test...");
    let step_response = step_test
        .perform_step_response(args.step_amplitude, args.duration)
        .await?;

    info!("Analyzing step response...");
    let metrics = analyze_step_response(&step_response)?;

    info!("Calculating Ziegler-Nichols parameters...");
    let zn_params = ZieglerNicholsCalculator::calculate_pid_parameters(&metrics)?;

    Ok(TuningResult {
        kp: zn_params.kp,
        ki: zn_params.ki,
        kd: zn_params.kd,
        algorithm: "Ziegler-Nichols".to_string(),
        step_response,
        performance_metrics: metrics,
    })
}

/// Perform Cohen-Coon tuning
async fn perform_cohen_coon_tuning(
    config: &Config,
    regulator_config: &ThermalRegulatorConfig,
    args: &Args,
) -> Result<TuningResult> {
    let mut step_test = StepResponseTest::new(config, regulator_config).await?;

    info!("Performing step response test...");
    let step_response = step_test
        .perform_step_response(args.step_amplitude, args.duration)
        .await?;

    info!("Analyzing step response...");
    let metrics = analyze_step_response(&step_response)?;

    info!("Calculating Cohen-Coon parameters...");
    let cc_params = CohenCoonCalculator::calculate_pid_parameters(&metrics)?;

    Ok(TuningResult {
        kp: cc_params.kp,
        ki: cc_params.ki,
        kd: cc_params.kd,
        algorithm: "Cohen-Coon".to_string(),
        step_response,
        performance_metrics: metrics,
    })
}

/// Analyze step response to extract key metrics
fn analyze_step_response(data: &StepResponseData) -> Result<PerformanceMetrics> {
    if data.time.is_empty() || data.temperature.is_empty() {
        return Err(anyhow!("Empty step response data"));
    }

    // Find steady-state values
    let final_temp = data.temperature.last().unwrap();
    let initial_temp = data.temperature.first().unwrap();
    let setpoint = data.setpoint.last().unwrap();

    // Calculate process gain
    let process_gain = (final_temp - initial_temp) / (setpoint - initial_temp);

    // Find 63.2% of steady-state response for time constant
    let target_63_2 = initial_temp + 0.632 * (final_temp - initial_temp);
    let time_constant =
        find_time_for_value(&data.time, &data.temperature, target_63_2).unwrap_or(60.0); // Default fallback

    // Find dead time (time to reach 5% of final value)
    let target_5_percent = initial_temp + 0.05 * (final_temp - initial_temp);
    let dead_time =
        find_time_for_value(&data.time, &data.temperature, target_5_percent).unwrap_or(0.0);

    // Calculate rise time (10% to 90%)
    let target_10 = initial_temp + 0.1 * (final_temp - initial_temp);
    let target_90 = initial_temp + 0.9 * (final_temp - initial_temp);
    let t10 = find_time_for_value(&data.time, &data.temperature, target_10).unwrap_or(0.0);
    let t90 = find_time_for_value(&data.time, &data.temperature, target_90).unwrap_or(60.0);
    let rise_time = t90 - t10;

    // Calculate overshoot
    let max_temp = data
        .temperature
        .iter()
        .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let overshoot = if max_temp > *final_temp {
        100.0 * (max_temp - final_temp) / (final_temp - initial_temp)
    } else {
        0.0
    };

    // Calculate settling time (time to stay within 2% of final value)
    let settling_time = find_settling_time(&data.time, &data.temperature, *final_temp, 0.02)
        .unwrap_or(data.time.last().unwrap() * 0.8);

    // Calculate steady-state error
    let steady_state_error = (setpoint - final_temp).abs() / setpoint * 100.0;

    Ok(PerformanceMetrics {
        rise_time,
        settling_time,
        overshoot,
        steady_state_error,
        process_gain,
        time_constant,
        dead_time,
    })
}

/// Find the time when temperature reaches a specific value
fn find_time_for_value(time: &[f64], temperature: &[f64], target: f64) -> Option<f64> {
    for (i, &temp) in temperature.iter().enumerate() {
        if temp >= target {
            return Some(time[i]);
        }
    }
    None
}

/// Find settling time (time to stay within tolerance of final value)
fn find_settling_time(
    time: &[f64],
    temperature: &[f64],
    final_value: f64,
    tolerance: f64,
) -> Option<f64> {
    let tolerance_band = final_value * tolerance;

    // Look backwards from the end to find when it last left the tolerance band
    for (i, &temp) in temperature.iter().enumerate().rev() {
        if (temp - final_value).abs() > tolerance_band {
            return Some(time.get(i + 1).copied().unwrap_or(*time.last().unwrap()));
        }
    }

    // If never outside tolerance, settling time is the time to first reach it
    for (i, &temp) in temperature.iter().enumerate() {
        if (temp - final_value).abs() <= tolerance_band {
            return Some(time[i]);
        }
    }

    None
}

/// Display tuning results to console
fn display_tuning_results(result: &TuningResult) {
    println!("\n🎯 PID Tuning Results ({}):", result.algorithm);
    println!("   ╭─────────────────────────────────────────╮");
    println!("   │  Kp = {:.6}                       │", result.kp);
    println!("   │  Ki = {:.6}                       │", result.ki);
    println!("   │  Kd = {:.6}                       │", result.kd);
    println!("   ╰─────────────────────────────────────────╯");

    println!("\n📊 Performance Metrics:");
    println!(
        "   • Process Gain:       {:.3}",
        result.performance_metrics.process_gain
    );
    println!(
        "   • Time Constant:      {:.1} s",
        result.performance_metrics.time_constant
    );
    println!(
        "   • Dead Time:          {:.1} s",
        result.performance_metrics.dead_time
    );
    println!(
        "   • Rise Time:          {:.1} s",
        result.performance_metrics.rise_time
    );
    println!(
        "   • Settling Time:      {:.1} s",
        result.performance_metrics.settling_time
    );
    println!(
        "   • Overshoot:          {:.1} %",
        result.performance_metrics.overshoot
    );
    println!(
        "   • Steady-State Error: {:.2} %",
        result.performance_metrics.steady_state_error
    );

    // Provide recommendations
    println!("\n💡 Recommendations:");
    if result.performance_metrics.overshoot > 20.0 {
        println!("   ⚠️  High overshoot detected. Consider reducing Kp or increasing Kd.");
    }
    if result.performance_metrics.settling_time > 180.0 {
        println!("   ⚠️  Slow settling time. Consider increasing Ki for faster response.");
    }
    if result.performance_metrics.steady_state_error > 5.0 {
        println!("   ⚠️  High steady-state error. Consider increasing Ki.");
    }
    if result.performance_metrics.overshoot < 5.0
        && result.performance_metrics.settling_time < 120.0
    {
        println!("   ✅ Good balance between stability and response time.");
    }
}

/// Generate HTML report with SVG charts
fn generate_html_report(result: &TuningResult, output_path: &PathBuf) -> Result<()> {
    let report_generator = HtmlReportGenerator::new()?;
    report_generator.generate_report(result, output_path)
}
