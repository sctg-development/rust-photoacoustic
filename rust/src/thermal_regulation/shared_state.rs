// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Shared state for thermal regulation system
//!
//! This module provides thread-safe shared state management for thermal regulation
//! including historical data storage and real-time status information.

use anyhow::Result;
use rocket::serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Maximum number of historical data points per regulator (1 hour at 1Hz)
pub const MAX_HISTORY_SIZE: usize = 3600;

/// Single data point in thermal regulation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalDataPoint {
    /// Timestamp in Unix seconds
    pub timestamp: u64,
    /// Temperature reading in degrees Celsius
    pub temperature_celsius: f64,
    /// Control output percentage (-100.0 to +100.0)
    pub control_output_percent: f64,
    /// PID setpoint temperature in degrees Celsius
    pub setpoint_celsius: f64,
    /// Individual PID components for debugging
    pub pid_components: PidComponents,
}

/// PID controller components for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PidComponents {
    /// Proportional term value
    pub proportional: f64,
    /// Integral term value
    pub integral: f64,
    /// Derivative term value
    pub derivative: f64,
    /// Error value (setpoint - process_variable)
    pub error: f64,
}

/// Historical data for a single thermal regulator
#[derive(Debug, Clone)]
pub struct ThermalRegulatorHistory {
    /// Regulator unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Whether the regulator is currently active
    pub enabled: bool,
    /// Current operational status
    pub status: RegulatorStatus,
    /// Rolling history of temperature and control data
    pub history: VecDeque<ThermalDataPoint>,
    /// Last update timestamp
    pub last_update: u64,
    /// Current PID parameters
    pub current_pid_params: CurrentPidParams,
}

/// Current status of a thermal regulator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegulatorStatus {
    /// Regulator is not initialized
    Uninitialized,
    /// Regulator is initializing hardware
    Initializing,
    /// Regulator is running normally
    Running,
    /// Regulator is in error state
    Error { message: String },
    /// Regulator is stopped
    Stopped,
}

/// Current PID parameters for a regulator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentPidParams {
    /// Proportional gain
    pub kp: f64,
    /// Integral gain
    pub ki: f64,
    /// Derivative gain
    pub kd: f64,
    /// Current setpoint in Celsius
    pub setpoint_celsius: f64,
    /// Output limits
    pub output_min: f64,
    pub output_max: f64,
}

/// Shared thermal regulation state across the entire system
#[derive(Debug)]
pub struct SharedThermalRegulationState {
    /// Map of regulator ID to its historical data
    regulators: HashMap<String, ThermalRegulatorHistory>,
    /// Global system status
    system_status: ThermalSystemStatus,
    /// Last global update timestamp
    last_system_update: u64,
}

/// Global thermal regulation system status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalSystemStatus {
    /// Total number of configured regulators
    pub total_regulators: usize,
    /// Number of active regulators
    pub active_regulators: usize,
    /// Number of regulators in error state
    pub error_regulators: usize,
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Whether the thermal system is globally enabled
    pub system_enabled: bool,
}

impl SharedThermalRegulationState {
    /// Create a new shared thermal regulation state
    pub fn new() -> Self {
        Self {
            regulators: HashMap::new(),
            system_status: ThermalSystemStatus {
                total_regulators: 0,
                active_regulators: 0,
                error_regulators: 0,
                uptime_seconds: 0,
                system_enabled: false,
            },
            last_system_update: current_timestamp(),
        }
    }

    /// Initialize a new regulator in the shared state
    pub fn initialize_regulator(
        &mut self,
        id: String,
        name: String,
        pid_params: CurrentPidParams,
    ) -> Result<()> {
        let regulator_history = ThermalRegulatorHistory {
            id: id.clone(),
            name,
            enabled: true,
            status: RegulatorStatus::Initializing,
            history: VecDeque::with_capacity(MAX_HISTORY_SIZE),
            last_update: current_timestamp(),
            current_pid_params: pid_params,
        };

        self.regulators.insert(id, regulator_history);
        self.update_system_status();
        Ok(())
    }

    /// Update regulator data with new temperature and control readings
    pub fn update_regulator_data(
        &mut self,
        regulator_id: &str,
        temperature_celsius: f64,
        control_output_percent: f64,
        setpoint_celsius: f64,
        pid_components: PidComponents,
    ) -> Result<()> {
        let regulator = self
            .regulators
            .get_mut(regulator_id)
            .ok_or_else(|| anyhow::anyhow!("Regulator '{}' not found", regulator_id))?;

        let data_point = ThermalDataPoint {
            timestamp: current_timestamp(),
            temperature_celsius,
            control_output_percent,
            setpoint_celsius,
            pid_components,
        };

        // Add new data point and maintain size limit
        regulator.history.push_back(data_point);
        if regulator.history.len() > MAX_HISTORY_SIZE {
            regulator.history.pop_front();
        }

        regulator.last_update = current_timestamp();
        regulator.status = RegulatorStatus::Running;

        Ok(())
    }

    /// Update regulator status
    pub fn update_regulator_status(
        &mut self,
        regulator_id: &str,
        status: RegulatorStatus,
    ) -> Result<()> {
        let regulator = self
            .regulators
            .get_mut(regulator_id)
            .ok_or_else(|| anyhow::anyhow!("Regulator '{}' not found", regulator_id))?;

        regulator.status = status;
        regulator.last_update = current_timestamp();
        self.update_system_status();

        Ok(())
    }

    /// Update PID parameters for a regulator
    pub fn update_regulator_pid_params(
        &mut self,
        regulator_id: &str,
        pid_params: CurrentPidParams,
    ) -> Result<()> {
        let regulator = self
            .regulators
            .get_mut(regulator_id)
            .ok_or_else(|| anyhow::anyhow!("Regulator '{}' not found", regulator_id))?;

        regulator.current_pid_params = pid_params;
        regulator.last_update = current_timestamp();

        Ok(())
    }

    /// Get historical data for a specific regulator
    pub fn get_regulator_history(&self, regulator_id: &str) -> Option<&ThermalRegulatorHistory> {
        self.regulators.get(regulator_id)
    }

    /// Get recent data points for a regulator (last N points)
    pub fn get_recent_data(
        &self,
        regulator_id: &str,
        count: usize,
    ) -> Option<Vec<ThermalDataPoint>> {
        let regulator = self.regulators.get(regulator_id)?;
        let start_index = if regulator.history.len() > count {
            regulator.history.len() - count
        } else {
            0
        };

        Some(regulator.history.range(start_index..).cloned().collect())
    }

    /// Get current system status
    pub fn get_system_status(&self) -> &ThermalSystemStatus {
        &self.system_status
    }

    /// Get all regulator IDs
    pub fn get_regulator_ids(&self) -> Vec<String> {
        self.regulators.keys().cloned().collect()
    }

    /// Get current status for all regulators
    pub fn get_all_regulator_status(&self) -> HashMap<String, (RegulatorStatus, u64)> {
        self.regulators
            .iter()
            .map(|(id, history)| (id.clone(), (history.status.clone(), history.last_update)))
            .collect()
    }

    /// Remove a regulator from the shared state
    pub fn remove_regulator(&mut self, regulator_id: &str) -> Result<()> {
        self.regulators
            .remove(regulator_id)
            .ok_or_else(|| anyhow::anyhow!("Regulator '{}' not found", regulator_id))?;
        self.update_system_status();
        Ok(())
    }

    /// Update global system status based on individual regulators
    fn update_system_status(&mut self) {
        let total = self.regulators.len();
        let mut active = 0;
        let mut errors = 0;

        for regulator in self.regulators.values() {
            match regulator.status {
                RegulatorStatus::Running => active += 1,
                RegulatorStatus::Error { .. } => errors += 1,
                _ => {}
            }
        }

        self.system_status = ThermalSystemStatus {
            total_regulators: total,
            active_regulators: active,
            error_regulators: errors,
            uptime_seconds: current_timestamp() - self.last_system_update,
            system_enabled: total > 0,
        };
    }
}

impl Default for SharedThermalRegulationState {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current Unix timestamp in seconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Type alias for the shared thermal state wrapped in Arc<RwLock<>>
pub type SharedThermalState = std::sync::Arc<RwLock<SharedThermalRegulationState>>;

/// Create a new shared thermal state instance
pub fn create_shared_thermal_state() -> SharedThermalState {
    std::sync::Arc::new(RwLock::new(SharedThermalRegulationState::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_thermal_state_creation() {
        let state = SharedThermalRegulationState::new();
        assert_eq!(state.regulators.len(), 0);
        assert_eq!(state.system_status.total_regulators, 0);
        assert!(!state.system_status.system_enabled);
    }

    #[test]
    fn test_regulator_initialization() {
        let mut state = SharedThermalRegulationState::new();
        let pid_params = CurrentPidParams {
            kp: 1.0,
            ki: 0.1,
            kd: 0.01,
            setpoint_celsius: 25.0,
            output_min: -100.0,
            output_max: 100.0,
        };

        state
            .initialize_regulator(
                "test_reg".to_string(),
                "Test Regulator".to_string(),
                pid_params,
            )
            .unwrap();

        assert_eq!(state.regulators.len(), 1);
        assert_eq!(state.system_status.total_regulators, 1);
        assert!(state.system_status.system_enabled);

        let regulator = state.regulators.get("test_reg").unwrap();
        assert_eq!(regulator.id, "test_reg");
        assert_eq!(regulator.name, "Test Regulator");
        assert!(matches!(regulator.status, RegulatorStatus::Initializing));
    }

    #[test]
    fn test_regulator_data_update() {
        let mut state = SharedThermalRegulationState::new();
        let pid_params = CurrentPidParams {
            kp: 1.0,
            ki: 0.1,
            kd: 0.01,
            setpoint_celsius: 25.0,
            output_min: -100.0,
            output_max: 100.0,
        };

        state
            .initialize_regulator(
                "test_reg".to_string(),
                "Test Regulator".to_string(),
                pid_params,
            )
            .unwrap();

        let pid_components = PidComponents {
            proportional: 1.0,
            integral: 0.5,
            derivative: 0.1,
            error: -1.0,
        };

        state
            .update_regulator_data("test_reg", 24.0, 10.5, 25.0, pid_components)
            .unwrap();

        let regulator = state.regulators.get("test_reg").unwrap();
        assert_eq!(regulator.history.len(), 1);
        assert!(matches!(regulator.status, RegulatorStatus::Running));

        let data_point = regulator.history.back().unwrap();
        assert_eq!(data_point.temperature_celsius, 24.0);
        assert_eq!(data_point.control_output_percent, 10.5);
        assert_eq!(data_point.setpoint_celsius, 25.0);
    }

    #[test]
    fn test_history_size_limit() {
        let mut state = SharedThermalRegulationState::new();
        let pid_params = CurrentPidParams {
            kp: 1.0,
            ki: 0.1,
            kd: 0.01,
            setpoint_celsius: 25.0,
            output_min: -100.0,
            output_max: 100.0,
        };

        state
            .initialize_regulator(
                "test_reg".to_string(),
                "Test Regulator".to_string(),
                pid_params,
            )
            .unwrap();

        let pid_components = PidComponents {
            proportional: 1.0,
            integral: 0.5,
            derivative: 0.1,
            error: -1.0,
        };

        // Add more than MAX_HISTORY_SIZE data points
        for i in 0..(MAX_HISTORY_SIZE + 100) {
            state
                .update_regulator_data(
                    "test_reg",
                    25.0 + i as f64 * 0.1,
                    i as f64 * 0.01,
                    25.0,
                    pid_components.clone(),
                )
                .unwrap();
        }

        let regulator = state.regulators.get("test_reg").unwrap();
        assert_eq!(regulator.history.len(), MAX_HISTORY_SIZE);

        // Verify that the oldest data was removed (FIFO behavior)
        let first_point = regulator.history.front().unwrap();
        assert!(first_point.temperature_celsius > 25.0); // Should not be the very first value
    }

    #[test]
    fn test_recent_data_retrieval() {
        let mut state = SharedThermalRegulationState::new();
        let pid_params = CurrentPidParams {
            kp: 1.0,
            ki: 0.1,
            kd: 0.01,
            setpoint_celsius: 25.0,
            output_min: -100.0,
            output_max: 100.0,
        };

        state
            .initialize_regulator(
                "test_reg".to_string(),
                "Test Regulator".to_string(),
                pid_params,
            )
            .unwrap();

        let pid_components = PidComponents {
            proportional: 1.0,
            integral: 0.5,
            derivative: 0.1,
            error: -1.0,
        };

        // Add 10 data points
        for i in 0..10 {
            state
                .update_regulator_data(
                    "test_reg",
                    25.0 + i as f64,
                    i as f64,
                    25.0,
                    pid_components.clone(),
                )
                .unwrap();
        }

        // Get last 5 data points
        let recent_data = state.get_recent_data("test_reg", 5).unwrap();
        assert_eq!(recent_data.len(), 5);

        // Verify we got the most recent 5 points
        for (i, point) in recent_data.iter().enumerate() {
            assert_eq!(point.temperature_celsius, 25.0 + (5 + i) as f64);
            assert_eq!(point.control_output_percent, (5 + i) as f64);
        }
    }

    #[tokio::test]
    async fn test_shared_thermal_state_thread_safety() {
        let shared_state = create_shared_thermal_state();
        let pid_params = CurrentPidParams {
            kp: 1.0,
            ki: 0.1,
            kd: 0.01,
            setpoint_celsius: 25.0,
            output_min: -100.0,
            output_max: 100.0,
        };

        // Initialize regulator
        {
            let mut state = shared_state.write().await;
            state
                .initialize_regulator(
                    "test_reg".to_string(),
                    "Test Regulator".to_string(),
                    pid_params,
                )
                .unwrap();
        }

        // Simulate concurrent access from multiple tasks
        let mut handles = vec![];

        for i in 0..10 {
            let shared_state_clone = shared_state.clone();
            let handle = tokio::spawn(async move {
                let pid_components = PidComponents {
                    proportional: i as f64,
                    integral: 0.5,
                    derivative: 0.1,
                    error: -1.0,
                };

                let mut state = shared_state_clone.write().await;
                state
                    .update_regulator_data(
                        "test_reg",
                        25.0 + i as f64,
                        i as f64,
                        25.0,
                        pid_components,
                    )
                    .unwrap();
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all data was added
        let state = shared_state.read().await;
        let regulator = state.regulators.get("test_reg").unwrap();
        assert_eq!(regulator.history.len(), 10);
    }
}
