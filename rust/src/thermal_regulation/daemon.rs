// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Thermal regulation daemon
//!
//! This module provides the thermal regulation daemon that manages multiple
//! thermal regulators, each running in its own thread with individual PID control loops.

use anyhow::Result;
use log::{debug, error, info};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time;

use crate::config::thermal_regulation::{ThermalRegulationConfig, ThermalRegulatorConfig};
use crate::thermal_regulation::shared_state::{
    CurrentPidParams, PidComponents, RegulatorStatus, SharedThermalState,
};
use crate::thermal_regulation::{create_thermal_regulation_driver, ThermalRegulationDriver};

/// Commands that can be sent to a thermal regulator thread
#[derive(Debug, Clone)]
pub enum ThermalRegulatorCommand {
    /// Update PID parameters
    UpdatePidParameters { kp: f64, ki: f64, kd: f64 },
    /// Update setpoint temperature
    UpdateSetpoint { setpoint_celsius: f64 },
    /// Request current status
    GetStatus,
    /// Stop the regulator
    Stop,
}

/// Response from a thermal regulator thread
#[derive(Debug, Clone)]
pub enum ThermalRegulatorResponse {
    /// PID parameters updated successfully
    PidParametersUpdated,
    /// Setpoint updated successfully
    SetpointUpdated,
    /// Current status information
    Status {
        temperature: f64,
        control_output: f64,
        setpoint: f64,
    },
    /// Regulator stopped
    Stopped,
    /// Error occurred
    Error { message: String },
}

/// Individual thermal regulator daemon running in its own thread
pub struct ThermalRegulatorDaemon {
    /// Regulator configuration
    config: ThermalRegulatorConfig,
    /// I2C bus configuration
    bus_config: crate::config::thermal_regulation::I2CBusConfig,
    /// Shared state for data exchange
    shared_state: SharedThermalState,
    /// Running flag shared across the system
    running: Arc<AtomicBool>,
    /// Thread handle for this regulator
    thread_handle: Option<JoinHandle<Result<()>>>,
    /// Command sender to communicate with the regulator thread
    command_sender: Option<mpsc::UnboundedSender<ThermalRegulatorCommand>>,
}

/// PID controller implementation for thermal regulation
#[derive(Debug, Clone)]
pub struct PidController {
    /// Proportional gain
    kp: f64,
    /// Integral gain
    ki: f64,
    /// Derivative gain
    kd: f64,
    /// Current setpoint in Celsius
    setpoint_celsius: f64,
    /// Integral term accumulator
    integral: f64,
    /// Previous error for derivative calculation
    previous_error: f64,
    /// Maximum integral value (anti-windup)
    integral_max: f64,
    /// Output limits
    output_min: f64,
    output_max: f64,
    /// Last update time for dt calculation
    last_update: Option<Instant>,
}

/// Thermal regulation system daemon managing multiple regulators
pub struct ThermalRegulationSystemDaemon {
    /// System configuration
    config: ThermalRegulationConfig,
    /// Individual regulator daemons
    regulator_daemons: Vec<ThermalRegulatorDaemon>,
    /// Shared state across all regulators
    shared_state: SharedThermalState,
    /// System running flag
    running: Arc<AtomicBool>,
    /// System thread handles
    thread_handles: Vec<JoinHandle<Result<()>>>,
}

impl PidController {
    /// Create a new PID controller with the given parameters
    pub fn new(
        kp: f64,
        ki: f64,
        kd: f64,
        setpoint_celsius: f64,
        integral_max: f64,
        output_min: f64,
        output_max: f64,
    ) -> Self {
        Self {
            kp,
            ki,
            kd,
            setpoint_celsius,
            integral: 0.0,
            previous_error: 0.0,
            integral_max,
            output_min,
            output_max,
            last_update: None,
        }
    }

    /// Update PID controller and return control output
    ///
    /// # Arguments
    /// * `process_variable` - Current temperature in Celsius
    ///
    /// # Returns
    /// * Control output percentage (-100.0 to +100.0)
    pub fn update(&mut self, process_variable: f64) -> PidOutput {
        let now = Instant::now();

        // Calculate time delta
        let dt = if let Some(last_update) = self.last_update {
            now.duration_since(last_update).as_secs_f64()
        } else {
            // First update, use a reasonable default dt
            0.1 // 100ms
        };
        self.last_update = Some(now);

        // Calculate error
        let error = self.setpoint_celsius - process_variable;

        // Proportional term
        let proportional = self.kp * error;

        // Integral term with anti-windup
        self.integral += error * dt;
        // Clamp integral to prevent windup
        self.integral = self.integral.clamp(-self.integral_max, self.integral_max);
        let integral = self.ki * self.integral;

        // Derivative term
        let derivative = if dt > 0.0 {
            self.kd * (error - self.previous_error) / dt
        } else {
            0.0
        };
        self.previous_error = error;

        // Calculate total output
        let output = proportional + integral + derivative;
        let clamped_output = output.clamp(self.output_min, self.output_max);

        // Reset integral if output is saturated (additional anti-windup)
        if (output > self.output_max || output < self.output_min)
            && ((error > 0.0 && self.integral > 0.0) || (error < 0.0 && self.integral < 0.0))
        {
            self.integral *= 0.9; // Gradually reduce integral when saturated
        }

        PidOutput {
            control_output: clamped_output,
            components: PidComponents {
                proportional,
                integral,
                derivative,
                error,
            },
        }
    }

    /// Update PID parameters without resetting state
    pub fn update_parameters(&mut self, kp: f64, ki: f64, kd: f64) {
        self.kp = kp;
        self.ki = ki;
        self.kd = kd;
    }

    /// Update setpoint
    pub fn set_setpoint(&mut self, setpoint_celsius: f64) {
        self.setpoint_celsius = setpoint_celsius;
    }

    /// Reset PID controller state
    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.previous_error = 0.0;
        self.last_update = None;
    }

    /// Get current PID parameters
    pub fn get_current_params(&self) -> CurrentPidParams {
        CurrentPidParams {
            kp: self.kp,
            ki: self.ki,
            kd: self.kd,
            setpoint_celsius: self.setpoint_celsius,
            output_min: self.output_min,
            output_max: self.output_max,
        }
    }
}

/// PID controller output including components for debugging
#[derive(Debug, Clone)]
pub struct PidOutput {
    /// Final control output percentage
    pub control_output: f64,
    /// Individual PID components
    pub components: PidComponents,
}

impl ThermalRegulatorDaemon {
    /// Create a new thermal regulator daemon
    pub async fn new(
        config: ThermalRegulatorConfig,
        bus_config: crate::config::thermal_regulation::I2CBusConfig,
        shared_state: SharedThermalState,
        running: Arc<AtomicBool>,
    ) -> Result<Self> {
        // Debug: Print the bus config being used
        debug!(
            "Creating ThermalRegulatorDaemon with bus_config: {:?}",
            bus_config
        );
        debug!(
            "Bus type: {:?}, ADC address: 0x{:02X}",
            bus_config.bus_type, config.temperature_sensor.adc_address
        );

        // Initialize regulator in shared state
        let pid_controller = PidController::new(
            config.pid_parameters.kp as f64,
            config.pid_parameters.ki as f64,
            config.pid_parameters.kd as f64,
            (config.pid_parameters.setpoint - 273.15) as f64, // Convert K to C
            config.pid_parameters.integral_max as f64,
            config.pid_parameters.output_min as f64,
            config.pid_parameters.output_max as f64,
        );

        {
            let mut state = shared_state.write().await;
            state.initialize_regulator(
                config.id.clone(),
                config.name.clone(),
                pid_controller.get_current_params(),
            )?;
        }

        Ok(Self {
            config,
            bus_config,
            shared_state,
            running,
            thread_handle: None,
            command_sender: None,
        })
    }

    /// Start the thermal regulation loop in a separate thread
    pub fn start(&mut self) -> Result<()> {
        let regulator_id = self.config.id.clone();
        let sampling_frequency = self.config.control_parameters.sampling_frequency_hz;
        let interval_duration = Duration::from_secs_f64(1.0 / sampling_frequency as f64);

        info!(
            "Starting thermal regulator '{}' with sampling frequency {} Hz (interval: {:?})",
            regulator_id, sampling_frequency, interval_duration
        );

        // Create communication channel
        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<ThermalRegulatorCommand>();
        self.command_sender = Some(command_tx);

        // Clone necessary data for the async task
        let config = self.config.clone();
        let bus_config = self.bus_config.clone();
        let shared_state = self.shared_state.clone();
        let running = self.running.clone();

        let handle = tokio::spawn(async move {
            info!("Thermal regulator '{}' thread started", regulator_id);

            // Use the provided bus configuration instead of hardcoded mock
            let mut driver = match create_thermal_regulation_driver(&bus_config, &config) {
                Ok(driver) => driver,
                Err(e) => {
                    error!(
                        "Failed to create thermal regulation driver for '{}': {}",
                        regulator_id, e
                    );
                    return Err(e);
                }
            };

            // Initialize hardware
            if let Err(e) = driver.initialize().await {
                error!(
                    "Failed to initialize thermal regulation driver for '{}': {}",
                    regulator_id, e
                );
                return Err(e);
            }

            // Create PID controller
            let mut pid_controller = PidController::new(
                config.pid_parameters.kp as f64,
                config.pid_parameters.ki as f64,
                config.pid_parameters.kd as f64,
                (config.pid_parameters.setpoint - 273.15) as f64, // Convert K to C
                config.pid_parameters.integral_max as f64,
                config.pid_parameters.output_min as f64,
                config.pid_parameters.output_max as f64,
            );

            // Update status to running
            {
                let mut state = shared_state.write().await;
                state
                    .update_regulator_status(&regulator_id, RegulatorStatus::Running)
                    .ok();
            }

            let mut interval = time::interval(interval_duration);
            let mut iteration_count = 0u64;

            while running.load(Ordering::Relaxed) {
                tokio::select! {
                    // Handle incoming commands
                    command = command_rx.recv() => {
                        if let Some(cmd) = command {
                            match cmd {
                                ThermalRegulatorCommand::UpdatePidParameters { kp, ki, kd } => {
                                    pid_controller.update_parameters(kp, ki, kd);

                                    // Update shared state
                                    {
                                        let mut state = shared_state.write().await;
                                        state.update_regulator_pid_params(
                                            &regulator_id,
                                            pid_controller.get_current_params(),
                                        ).ok();
                                    }

                                    info!("Updated PID parameters for regulator '{}': Kp={}, Ki={}, Kd={}",
                                          regulator_id, kp, ki, kd);
                                }
                                ThermalRegulatorCommand::UpdateSetpoint { setpoint_celsius } => {
                                    pid_controller.set_setpoint(setpoint_celsius);

                                    // Update shared state
                                    {
                                        let mut state = shared_state.write().await;
                                        state.update_regulator_pid_params(
                                            &regulator_id,
                                            pid_controller.get_current_params(),
                                        ).ok();
                                    }

                                    info!("Updated setpoint for regulator '{}' to {} °C",
                                          regulator_id, setpoint_celsius);
                                }
                                ThermalRegulatorCommand::Stop => {
                                    info!("Received stop command for regulator '{}'", regulator_id);
                                    break;
                                }
                                _ => {
                                    // Other commands can be handled here
                                }
                            }
                        }
                    }

                    // Regular regulation cycle
                    _ = interval.tick() => {
                        iteration_count += 1;

                        // Execute regulation cycle inline to avoid Send issues
                        if let Err(e) = async {
                            // Read current temperature
                            let temperature_celsius = driver.read_temperature().await?;

                            // Calculate PID output
                            let pid_output = pid_controller.update(temperature_celsius);

                            // Apply control output to hardware
                            driver.apply_control_output(pid_output.control_output).await?;

                            // Update shared state with new data
                            {
                                let mut state = shared_state.write().await;
                                state.update_regulator_data(
                                    &regulator_id,
                                    temperature_celsius,
                                    pid_output.control_output,
                                    pid_controller.setpoint_celsius,
                                    pid_output.components,
                                )?;
                            }

                            Ok::<(), anyhow::Error>(())
                        }.await {
                            error!(
                                "Error in thermal regulator '{}' cycle: {:?}",
                                regulator_id, e
                            );

                            // Update status to error
                            {
                                let mut state = shared_state.write().await;
                                state.update_regulator_status(
                                    &regulator_id,
                                    RegulatorStatus::Error {
                                        message: e.to_string(),
                                    }
                                ).ok(); // Don't propagate error here
                            }

                            // Wait a bit before retrying
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        } else {
                            if iteration_count % (sampling_frequency as u64 * 60) == 0 {
                                // Log every minute
                                debug!(
                                    "Thermal regulator '{}' completed {} iterations",
                                    regulator_id, iteration_count
                                );
                            }
                        }
                    }
                }
            }

            info!("Thermal regulator '{}' thread stopping", regulator_id);

            // Update status to stopped
            {
                let mut state = shared_state.write().await;
                state
                    .update_regulator_status(&regulator_id, RegulatorStatus::Stopped)
                    .ok();
            }

            Ok(())
        });

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Update PID parameters for this regulator
    pub async fn update_pid_parameters(&mut self, kp: f64, ki: f64, kd: f64) -> Result<()> {
        if let Some(ref sender) = self.command_sender {
            sender
                .send(ThermalRegulatorCommand::UpdatePidParameters { kp, ki, kd })
                .map_err(|e| anyhow::anyhow!("Failed to send PID update command: {}", e))?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Regulator thread not started"))
        }
    }

    /// Update setpoint for this regulator
    pub async fn update_setpoint(&mut self, setpoint_celsius: f64) -> Result<()> {
        if let Some(ref sender) = self.command_sender {
            sender
                .send(ThermalRegulatorCommand::UpdateSetpoint { setpoint_celsius })
                .map_err(|e| anyhow::anyhow!("Failed to send setpoint update command: {}", e))?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Regulator thread not started"))
        }
    }

    /// Stop this regulator
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(ref sender) = self.command_sender {
            sender
                .send(ThermalRegulatorCommand::Stop)
                .map_err(|e| anyhow::anyhow!("Failed to send stop command: {}", e))?;
        }

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            handle.await??;
        }

        Ok(())
    }
}

impl ThermalRegulationSystemDaemon {
    /// Create a new thermal regulation system daemon
    pub fn new(
        config: ThermalRegulationConfig,
        shared_state: SharedThermalState,
        running: Arc<AtomicBool>,
    ) -> Self {
        Self {
            config,
            regulator_daemons: Vec::new(),
            shared_state,
            running,
            thread_handles: Vec::new(),
        }
    }

    /// Start all thermal regulators
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting thermal regulation system");

        // Initialize and start each regulator
        for regulator_config in &self.config.regulators {
            if !regulator_config.enabled {
                info!(
                    "Skipping disabled thermal regulator '{}'",
                    regulator_config.id
                );
                continue;
            }

            // Find the corresponding I2C bus configuration
            let bus_config = self
                .config
                .i2c_buses
                .iter()
                .find(|(bus_name, _)| *bus_name == &regulator_config.i2c_bus)
                .map(|(_, bus_config)| bus_config.clone())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "I2C bus '{}' not found for regulator '{}'",
                        regulator_config.i2c_bus,
                        regulator_config.id
                    )
                })?;

            let mut regulator_daemon = ThermalRegulatorDaemon::new(
                regulator_config.clone(),
                bus_config,
                self.shared_state.clone(),
                self.running.clone(),
            )
            .await?;

            regulator_daemon.start()?;
            self.regulator_daemons.push(regulator_daemon);

            info!(
                "Started thermal regulator '{}' ({})",
                regulator_config.id, regulator_config.name
            );
        }

        info!(
            "Thermal regulation system started with {} active regulators",
            self.regulator_daemons.len()
        );

        Ok(())
    }

    /// Stop all thermal regulators
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping thermal regulation system");

        // Stop all regulator daemons
        for regulator_daemon in &mut self.regulator_daemons {
            regulator_daemon.stop().await?;
        }

        self.regulator_daemons.clear();

        // Wait for any remaining system threads
        for handle in self.thread_handles.drain(..) {
            handle.await??;
        }

        info!("Thermal regulation system stopped");
        Ok(())
    }

    /// Get shared state reference
    pub fn get_shared_state(&self) -> &SharedThermalState {
        &self.shared_state
    }

    /// Update PID parameters for a specific regulator
    pub async fn update_regulator_pid_parameters(
        &mut self,
        regulator_id: &str,
        kp: f64,
        ki: f64,
        kd: f64,
    ) -> Result<()> {
        for regulator_daemon in &mut self.regulator_daemons {
            if regulator_daemon.config.id == regulator_id {
                return regulator_daemon.update_pid_parameters(kp, ki, kd).await;
            }
        }
        Err(anyhow::anyhow!("Regulator '{}' not found", regulator_id))
    }

    /// Update setpoint for a specific regulator
    pub async fn update_regulator_setpoint(
        &mut self,
        regulator_id: &str,
        setpoint_celsius: f64,
    ) -> Result<()> {
        for regulator_daemon in &mut self.regulator_daemons {
            if regulator_daemon.config.id == regulator_id {
                return regulator_daemon.update_setpoint(setpoint_celsius).await;
            }
        }
        Err(anyhow::anyhow!("Regulator '{}' not found", regulator_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermal_regulation::shared_state::create_shared_thermal_state;

    /// Test PID controller basic functionality
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::thermal_regulation::daemon::PidController;
    ///
    /// let mut pid = PidController::new(1.0, 0.1, 0.01, 25.0, 100.0, -100.0, 100.0);
    /// let output = pid.update(20.0); // Current temperature is 20°C, setpoint is 25°C
    /// assert!(output.control_output > 0.0); // Should heat up
    /// ```
    #[tokio::test]
    async fn test_pid_controller_basic() {
        let mut pid = PidController::new(1.0, 0.1, 0.01, 25.0, 100.0, -100.0, 100.0);

        // Test heating response
        let output = pid.update(20.0);
        assert!(
            output.control_output > 0.0,
            "PID should output positive control for heating"
        );
        assert!(
            output.components.error > 0.0,
            "Error should be positive when temp < setpoint"
        );

        // Test cooling response
        let output = pid.update(30.0);
        assert!(
            output.control_output < 0.0,
            "PID should output negative control for cooling"
        );
        assert!(
            output.components.error < 0.0,
            "Error should be negative when temp > setpoint"
        );
    }

    /// Test PID parameter updates
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::thermal_regulation::daemon::PidController;
    ///
    /// let mut pid = PidController::new(1.0, 0.1, 0.01, 25.0, 100.0, -100.0, 100.0);
    /// pid.update_parameters(2.0, 0.2, 0.02);
    /// let params = pid.get_current_params();
    /// assert_eq!(params.kp, 2.0);
    /// ```
    #[tokio::test]
    async fn test_pid_parameter_updates() {
        let mut pid = PidController::new(1.0, 0.1, 0.01, 25.0, 100.0, -100.0, 100.0);

        pid.update_parameters(2.0, 0.2, 0.02);
        let params = pid.get_current_params();

        assert_eq!(params.kp, 2.0);
        assert_eq!(params.ki, 0.2);
        assert_eq!(params.kd, 0.02);
    }

    /// Test thermal regulation system daemon creation
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::thermal_regulation::daemon::ThermalRegulationSystemDaemon;
    /// use rust_photoacoustic::config::thermal_regulation::ThermalRegulationConfig;
    /// use std::sync::{Arc, atomic::AtomicBool};
    ///
    /// let config = ThermalRegulationConfig::default();
    /// let shared_state = create_shared_thermal_state();
    /// let running = Arc::new(AtomicBool::new(true));
    /// let daemon = ThermalRegulationSystemDaemon::new(config, shared_state, running);
    /// ```
    #[tokio::test]
    async fn test_thermal_regulation_system_daemon_creation() {
        let config = ThermalRegulationConfig::default();
        let shared_state = create_shared_thermal_state();
        let running = Arc::new(AtomicBool::new(true));

        let daemon = ThermalRegulationSystemDaemon::new(config, shared_state, running);
        assert_eq!(daemon.regulator_daemons.len(), 0);
        assert!(daemon.thread_handles.is_empty());
    }

    /// Test PID anti-windup functionality
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::thermal_regulation::daemon::PidController;
    ///
    /// let mut pid = PidController::new(1.0, 10.0, 0.0, 25.0, 10.0, -50.0, 50.0);
    ///
    /// // Simulate sustained error that would cause integral windup
    /// for _ in 0..100 {
    ///     pid.update(0.0); // Large sustained error
    /// }
    ///
    /// // The integral should be limited
    /// let params = pid.get_current_params();
    /// assert!(pid.integral.abs() <= 10.0); // Within integral_max limit
    /// ```
    #[tokio::test]
    async fn test_pid_anti_windup() {
        let mut pid = PidController::new(1.0, 10.0, 0.0, 25.0, 10.0, -50.0, 50.0);

        // Simulate sustained error that would cause integral windup
        for _ in 0..100 {
            pid.update(0.0); // Large sustained error
        }

        // The integral should be limited
        assert!(
            pid.integral.abs() <= 10.0,
            "Integral should be limited by anti-windup"
        );
    }
}
