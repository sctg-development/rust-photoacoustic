// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! PID Tuning Algorithms
//!
//! This module implements classical PID tuning algorithms including
//! Ziegler-Nichols and Cohen-Coon methods based on step response analysis.

use crate::PerformanceMetrics;
use anyhow::{anyhow, Result};

/// PID parameters calculated by tuning algorithms
#[derive(Debug, Clone)]
pub struct PidParameters {
    pub kp: f64,
    pub ki: f64,
    pub kd: f64,
}

/// Ziegler-Nichols step response tuning algorithm
pub struct ZieglerNicholsCalculator;

impl ZieglerNicholsCalculator {
    /// Calculate PID parameters using Ziegler-Nichols step response method
    ///
    /// Based on the classical Ziegler-Nichols rules for step response:
    /// - Kp = 1.2 * (τ / (K * L))
    /// - Ki = Kp / (2 * L)  
    /// - Kd = Kp * L / 2
    ///
    /// Where:
    /// - K = process gain
    /// - τ = time constant  
    /// - L = dead time
    pub fn calculate_pid_parameters(metrics: &PerformanceMetrics) -> Result<PidParameters> {
        let k = metrics.process_gain;
        let tau = metrics.time_constant;
        let l = metrics.dead_time;

        // Validate inputs
        if k.abs() < 1e-6 {
            return Err(anyhow!("Process gain too small for reliable tuning"));
        }
        if tau <= 0.0 {
            return Err(anyhow!("Invalid time constant: {}", tau));
        }
        if l < 0.0 {
            return Err(anyhow!("Invalid dead time: {}", l));
        }

        // Handle case where dead time is very small
        let effective_l = if l < 1.0 { 1.0 } else { l };

        // Apply Ziegler-Nichols formulas
        let kp = 1.2 * tau / (k * effective_l);
        let ki = kp / (2.0 * effective_l);
        let kd = kp * effective_l / 2.0;

        // Apply reasonable limits to prevent extreme values
        let kp = kp.clamp(0.01, 100.0);
        let ki = ki.clamp(0.001, 10.0);
        let kd = kd.clamp(0.0, 50.0);

        log::info!("Ziegler-Nichols calculation:");
        log::info!("  Process gain (K): {:.3}", k);
        log::info!("  Time constant (τ): {:.1} s", tau);
        log::info!("  Dead time (L): {:.1} s", effective_l);
        log::info!("  Calculated Kp: {:.6}", kp);
        log::info!("  Calculated Ki: {:.6}", ki);
        log::info!("  Calculated Kd: {:.6}", kd);

        Ok(PidParameters { kp, ki, kd })
    }
}

/// Cohen-Coon step response tuning algorithm
pub struct CohenCoonCalculator;

impl CohenCoonCalculator {
    /// Calculate PID parameters using Cohen-Coon step response method
    ///
    /// Cohen-Coon method provides better performance for processes with
    /// significant dead time. The formulas are:
    ///
    /// Kp = (1/K) * (τ/L) * (16 + 3*(L/τ)) / (13 + 8*(L/τ))
    /// Ki = Kp / (L * (32 + 6*(L/τ)) / (13 + 8*(L/τ)))
    /// Kd = Kp * L * 4 / (11 + 2*(L/τ))
    ///
    /// Where:
    /// - K = process gain
    /// - τ = time constant
    /// - L = dead time
    pub fn calculate_pid_parameters(metrics: &PerformanceMetrics) -> Result<PidParameters> {
        let k = metrics.process_gain;
        let tau = metrics.time_constant;
        let l = metrics.dead_time;

        // Validate inputs
        if k.abs() < 1e-6 {
            return Err(anyhow!("Process gain too small for reliable tuning"));
        }
        if tau <= 0.0 {
            return Err(anyhow!("Invalid time constant: {}", tau));
        }
        if l < 0.0 {
            return Err(anyhow!("Invalid dead time: {}", l));
        }

        // Handle case where dead time is very small
        let effective_l = if l < 1.0 { 1.0 } else { l };

        // Calculate L/τ ratio
        let l_tau_ratio = effective_l / tau;

        // Apply Cohen-Coon formulas
        let kp = (1.0 / k) * (tau / effective_l) * (16.0 + 3.0 * l_tau_ratio)
            / (13.0 + 8.0 * l_tau_ratio);
        let ki_denominator = effective_l * (32.0 + 6.0 * l_tau_ratio) / (13.0 + 8.0 * l_tau_ratio);
        let ki = kp / ki_denominator;
        let kd = kp * effective_l * 4.0 / (11.0 + 2.0 * l_tau_ratio);

        // Apply reasonable limits to prevent extreme values
        let kp = kp.clamp(0.01, 100.0);
        let ki = ki.clamp(0.001, 10.0);
        let kd = kd.clamp(0.0, 50.0);

        log::info!("Cohen-Coon calculation:");
        log::info!("  Process gain (K): {:.3}", k);
        log::info!("  Time constant (τ): {:.1} s", tau);
        log::info!("  Dead time (L): {:.1} s", effective_l);
        log::info!("  L/τ ratio: {:.3}", l_tau_ratio);
        log::info!("  Calculated Kp: {:.6}", kp);
        log::info!("  Calculated Ki: {:.6}", ki);
        log::info!("  Calculated Kd: {:.6}", kd);

        Ok(PidParameters { kp, ki, kd })
    }
}

/// Manual tuning helper (TODO: implement interactive tuning)
pub struct ManualTuningCalculator;

impl ManualTuningCalculator {
    /// Interactive manual tuning - TODO: implement
    pub fn interactive_tuning() -> Result<PidParameters> {
        // TODO: Implement interactive tuning interface
        // This could include:
        // - Real-time plotting of system response
        // - Manual adjustment of parameters with live feedback
        // - Guided tuning with suggestions
        // - Save/load of tuning sessions

        Err(anyhow!("Manual tuning not yet implemented"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ziegler_nichols_calculation() {
        let metrics = PerformanceMetrics {
            rise_time: 30.0,
            settling_time: 120.0,
            overshoot: 10.0,
            steady_state_error: 1.0,
            process_gain: 0.8,
            time_constant: 60.0,
            dead_time: 10.0,
        };

        let result = ZieglerNicholsCalculator::calculate_pid_parameters(&metrics);
        assert!(result.is_ok());

        let params = result.unwrap();
        assert!(params.kp > 0.0);
        assert!(params.ki > 0.0);
        assert!(params.kd > 0.0);

        // Check that values are within reasonable ranges
        assert!(params.kp <= 100.0);
        assert!(params.ki <= 10.0);
        assert!(params.kd <= 50.0);
    }

    #[test]
    fn test_cohen_coon_calculation() {
        let metrics = PerformanceMetrics {
            rise_time: 30.0,
            settling_time: 120.0,
            overshoot: 10.0,
            steady_state_error: 1.0,
            process_gain: 0.8,
            time_constant: 60.0,
            dead_time: 20.0, // Higher dead time for Cohen-Coon
        };

        let result = CohenCoonCalculator::calculate_pid_parameters(&metrics);
        assert!(result.is_ok());

        let params = result.unwrap();
        assert!(params.kp > 0.0);
        assert!(params.ki > 0.0);
        assert!(params.kd > 0.0);

        // Check that values are within reasonable ranges
        assert!(params.kp <= 100.0);
        assert!(params.ki <= 10.0);
        assert!(params.kd <= 50.0);
    }

    #[test]
    fn test_invalid_process_gain() {
        let metrics = PerformanceMetrics {
            rise_time: 30.0,
            settling_time: 120.0,
            overshoot: 10.0,
            steady_state_error: 1.0,
            process_gain: 0.0, // Invalid
            time_constant: 60.0,
            dead_time: 10.0,
        };

        let result = ZieglerNicholsCalculator::calculate_pid_parameters(&metrics);
        assert!(result.is_err());
    }

    #[test]
    fn test_small_dead_time_handling() {
        let metrics = PerformanceMetrics {
            rise_time: 30.0,
            settling_time: 120.0,
            overshoot: 10.0,
            steady_state_error: 1.0,
            process_gain: 0.8,
            time_constant: 60.0,
            dead_time: 0.1, // Very small dead time
        };

        let result = ZieglerNicholsCalculator::calculate_pid_parameters(&metrics);
        assert!(result.is_ok());

        // Should still produce reasonable parameters
        let params = result.unwrap();
        assert!(params.kp > 0.0 && params.kp <= 100.0);
        assert!(params.ki > 0.0 && params.ki <= 10.0); // Allow clamping to maximum
        assert!(params.kd >= 0.0 && params.kd <= 50.0);

        // For very small dead time, effective_l is clamped to 1.0
        // This should result in reasonable but potentially clamped parameters
        println!(
            "Small dead time parameters: Kp={:.3}, Ki={:.3}, Kd={:.3}",
            params.kp, params.ki, params.kd
        );
    }
}
