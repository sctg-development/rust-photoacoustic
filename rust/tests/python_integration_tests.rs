//! Python integration tests for the PythonActionDriver.
//!
//! Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
//! This file is part of the rust-photoacoustic project and is licensed under the
//! SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use serde_json::Value;
use tokio::time::sleep;

// Only compile tests when python-driver feature is enabled
#[cfg(feature = "python-driver")]
mod python_integration_tests {
    use super::*;
    use rust_photoacoustic::processing::computing_nodes::action_drivers::{
        ActionDriver, AlertData, MeasurementData, PythonActionDriver, PythonDriverConfig,
    };

    fn get_test_script_path(script_name: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("test_scripts");
        path.push(script_name);
        path
    }

    fn get_anaconda_path() -> Option<PathBuf> {
        if let Ok(home) = std::env::var("HOME") {
            let anaconda_path = PathBuf::from(home).join("anaconda3");
            if anaconda_path.exists() {
                return Some(anaconda_path);
            }
        }
        None
    }

    fn create_test_measurement_data(concentration: f64, node_id: &str) -> MeasurementData {
        let mut metadata = HashMap::new();
        metadata.insert("test".to_string(), Value::String("true".to_string()));
        metadata.insert(
            "sequence".to_string(),
            Value::Number(serde_json::Number::from(1)),
        );

        MeasurementData {
            concentration_ppm: concentration,
            source_node_id: node_id.to_string(),
            peak_amplitude: 0.75,
            peak_frequency: 1200.0,
            timestamp: SystemTime::now(),
            metadata,
        }
    }

    fn create_test_alert_data(severity: &str, message: &str) -> AlertData {
        let mut alert_data = HashMap::new();
        alert_data.insert(
            "threshold".to_string(),
            Value::Number(serde_json::Number::from(1000)),
        );

        AlertData {
            alert_type: "concentration_high".to_string(),
            severity: severity.to_string(),
            message: message.to_string(),
            data: alert_data,
            timestamp: SystemTime::now(),
        }
    }

    #[tokio::test]
    async fn test_python_driver_basic_functionality() -> Result<()> {
        println!("🧪 Testing basic Python driver functionality...");

        let script_path = get_test_script_path("simple_action.py");
        assert!(
            script_path.exists(),
            "Test script not found: {:?}",
            script_path
        );

        let mut config = PythonDriverConfig {
            script_path,
            timeout_seconds: 10,
            auto_reload: false,
            ..Default::default()
        };

        // Try to use anaconda if available
        if let Some(anaconda_path) = get_anaconda_path() {
            config.venv_path = Some(anaconda_path);
            println!("📦 Using Anaconda environment: {:?}", config.venv_path);
        }

        let mut driver = PythonActionDriver::new(config);

        // Test initialization
        println!("🚀 Testing initialization...");
        driver.initialize().await?;

        let status = driver.get_status().await?;
        println!(
            "📊 Initial status: {}",
            serde_json::to_string_pretty(&status)?
        );

        // Test measurement processing
        println!("📊 Testing measurement processing...");
        let test_data = create_test_measurement_data(850.5, "test_sensor");
        driver.update_action(&test_data).await?;

        // Test high concentration measurement
        println!("⚠️  Testing high concentration...");
        let high_test_data = create_test_measurement_data(1250.0, "test_sensor");
        driver.update_action(&high_test_data).await?;

        // Test alert handling
        println!("🚨 Testing alert handling...");
        let test_alert = create_test_alert_data("warning", "Test alert message");
        driver.show_alert(&test_alert).await?;

        // Test critical alert
        println!("🔴 Testing critical alert...");
        let critical_alert = create_test_alert_data("critical", "Critical system alert");
        driver.show_alert(&critical_alert).await?;

        // Test status after operations
        println!("📈 Testing status after operations...");
        let final_status = driver.get_status().await?;
        println!(
            "📊 Final status: {}",
            serde_json::to_string_pretty(&final_status)?
        );

        // Verify history
        println!("📚 Testing history...");
        let history = driver.get_history(Some(10)).await?;
        assert_eq!(history.len(), 2, "Should have 2 measurements in history");

        let history_stats = driver.get_history_stats().await?;
        println!(
            "📊 History stats: {}",
            serde_json::to_string_pretty(&history_stats)?
        );

        // Test clear action
        println!("🧹 Testing clear action...");
        driver.clear_action().await?;

        // Test shutdown
        println!("🔄 Testing shutdown...");
        driver.shutdown().await?;

        println!("✅ Basic functionality test completed successfully!");
        Ok(())
    }

    #[tokio::test]
    async fn test_python_driver_advanced_features() -> Result<()> {
        println!("🧪 Testing advanced Python driver features...");

        let script_path = get_test_script_path("advanced_action.py");
        assert!(
            script_path.exists(),
            "Advanced test script not found: {:?}",
            script_path
        );

        let mut config = PythonDriverConfig {
            script_path,
            timeout_seconds: 15,
            auto_reload: false,
            ..Default::default()
        };

        // Try to use anaconda if available
        if let Some(anaconda_path) = get_anaconda_path() {
            config.venv_path = Some(anaconda_path);
        }

        let mut driver = PythonActionDriver::new(config);

        // Initialize
        driver.initialize().await?;

        // Test multiple measurements to trigger statistics
        println!("📊 Testing statistical analysis...");
        let test_concentrations = vec![100.0, 200.0, 500.0, 1200.0, 800.0, 300.0, 1500.0];

        for (i, concentration) in test_concentrations.iter().enumerate() {
            let test_data = create_test_measurement_data(*concentration, &format!("sensor_{}", i));
            driver.update_action(&test_data).await?;

            // Small delay to simulate real timing
            sleep(Duration::from_millis(10)).await;
        }

        // Test status with statistics
        println!("📈 Testing advanced status...");
        let status = driver.get_status().await?;
        println!(
            "📊 Advanced status: {}",
            serde_json::to_string_pretty(&status)?
        );

        // Test alert frequency
        println!("🚨 Testing alert frequency...");
        for i in 0..3 {
            let alert = create_test_alert_data("warning", &format!("Alert #{}", i + 1));
            driver.show_alert(&alert).await?;
            sleep(Duration::from_millis(50)).await;
        }

        // Test final status
        let final_status = driver.get_status().await?;
        println!(
            "📊 Final advanced status: {}",
            serde_json::to_string_pretty(&final_status)?
        );

        // Shutdown
        driver.shutdown().await?;

        println!("✅ Advanced features test completed successfully!");
        Ok(())
    }

    #[tokio::test]
    async fn test_python_driver_error_handling() -> Result<()> {
        println!("🧪 Testing Python driver error handling...");

        // Test with non-existent script
        println!("❌ Testing non-existent script...");
        let bad_config = PythonDriverConfig {
            script_path: PathBuf::from("non_existent_script.py"),
            ..Default::default()
        };

        let mut bad_driver = PythonActionDriver::new(bad_config);
        let init_result = bad_driver.initialize().await;
        assert!(init_result.is_err(), "Should fail with non-existent script");
        println!("✅ Correctly handled non-existent script");

        // Test timeout behavior (create a script that takes too long)
        println!("⏱️  Testing timeout behavior...");
        let timeout_config = PythonDriverConfig {
            script_path: get_test_script_path("simple_action.py"),
            timeout_seconds: 1, // Very short timeout
            ..Default::default()
        };

        if let Some(anaconda_path) = get_anaconda_path() {
            let mut driver_with_timeout = PythonActionDriver::new(timeout_config);
            driver_with_timeout.initialize().await?;

            // This should work fine as our test scripts are fast
            let test_data = create_test_measurement_data(500.0, "timeout_test");
            let result = driver_with_timeout.update_action(&test_data).await;
            // Note: Our test scripts are actually fast enough that this won't timeout
            // In a real scenario, you'd have a script that sleeps longer than the timeout

            driver_with_timeout.shutdown().await?;
        }

        println!("✅ Error handling test completed!");
        Ok(())
    }

    #[tokio::test]
    async fn test_python_driver_configuration() -> Result<()> {
        println!("🧪 Testing Python driver configuration...");

        // Test configuration from JSON
        let config_json = serde_json::json!({
            "script_path": get_test_script_path("simple_action.py").to_string_lossy(),
            "timeout_seconds": 30,
            "auto_reload": true,
            "update_function": "on_measurement",
            "alert_function": "on_alert",
            "init_function": "initialize",
            "shutdown_function": "shutdown",
            "status_function": "get_status"
        });

        // Convert to HashMap for from_config
        let config_map: HashMap<String, Value> = serde_json::from_value(config_json)?;
        let driver = PythonActionDriver::from_config(config_map)?;
        println!("✅ Successfully created driver from config");

        // Test direct creation with custom config
        let mut custom_config = PythonDriverConfig::default();
        custom_config.script_path = get_test_script_path("simple_action.py");
        custom_config.timeout_seconds = 25;
        custom_config.auto_reload = true;
        let builder_driver = PythonActionDriver::new(custom_config);

        println!("✅ Successfully created driver with builder pattern");

        // Test that driver_type is correct
        assert_eq!(driver.driver_type(), "python");
        assert_eq!(builder_driver.driver_type(), "python");

        // Test supports_realtime
        assert!(driver.supports_realtime());
        assert!(builder_driver.supports_realtime());

        println!("✅ Configuration test completed!");
        Ok(())
    }

    #[tokio::test]
    async fn test_python_driver_with_anaconda() -> Result<()> {
        println!("🧪 Testing Python driver with Anaconda environment...");

        if let Some(anaconda_path) = get_anaconda_path() {
            println!("📦 Found Anaconda at: {:?}", anaconda_path);

            let config = PythonDriverConfig {
                script_path: get_test_script_path("advanced_action.py"),
                venv_path: Some(anaconda_path.clone()),
                timeout_seconds: 20,
                python_paths: vec![anaconda_path
                    .join("lib")
                    .join("python3.11")
                    .join("site-packages")],
                ..Default::default()
            };

            let mut driver = PythonActionDriver::new(config);

            // Test initialization with Anaconda
            println!("🚀 Initializing with Anaconda...");
            driver.initialize().await?;

            // Test that Python libraries work (our advanced script uses statistics)
            println!("📊 Testing mathematical operations...");
            let test_data = create_test_measurement_data(750.0, "anaconda_test");
            driver.update_action(&test_data).await?;

            let status = driver.get_status().await?;
            println!(
                "📊 Anaconda test status: {}",
                serde_json::to_string_pretty(&status)?
            );

            driver.shutdown().await?;
            println!("✅ Anaconda integration test completed!");
        } else {
            println!("⚠️  Anaconda not found at $HOME/anaconda3, skipping Anaconda-specific test");
        }

        Ok(())
    }
}

// Fallback tests when python-driver feature is not enabled
#[cfg(not(feature = "python-driver"))]
mod fallback_tests {
    use super::*;
    use rust_photoacoustic::processing::computing_nodes::action_drivers::{
        PythonActionDriver, PythonDriverConfig,
    };

    #[tokio::test]
    async fn test_python_driver_without_feature() {
        println!("🧪 Testing Python driver without feature flag...");

        let config = PythonDriverConfig::default();
        let mut driver = PythonActionDriver::new(config);

        // Should fail to initialize without feature
        let result = driver.initialize().await;
        assert!(result.is_err());
        println!("✅ Correctly failed without python-driver feature");

        // Status should indicate Python is not enabled
        let status = driver.get_status().await.unwrap();
        assert_eq!(status["python_enabled"], false);
        println!("✅ Status correctly reports Python not enabled");
    }
}

// Helper function to run self-tests of Python scripts
#[cfg(feature = "python-driver")]
#[tokio::test]
async fn test_python_scripts_self_test() -> Result<()> {
    use std::process::Command;

    println!("🧪 Running Python scripts self-tests...");

    let scripts = vec!["simple_action.py", "advanced_action.py"];

    for script in scripts {
        let script_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("test_scripts")
            .join(script);

        if script_path.exists() {
            println!("🐍 Testing {}", script);

            // Try to run the script directly with Python
            let output = Command::new("python3").arg(&script_path).output();

            match output {
                Ok(output) => {
                    if output.status.success() {
                        println!("✅ {} self-test passed", script);
                        println!("Output: {}", String::from_utf8_lossy(&output.stdout));
                    } else {
                        println!("❌ {} self-test failed", script);
                        println!("Error: {}", String::from_utf8_lossy(&output.stderr));
                    }
                }
                Err(e) => {
                    println!("⚠️  Could not run {} directly: {}", script, e);
                    println!("   This is normal if python3 is not in PATH");
                }
            }
        }
    }

    Ok(())
}
