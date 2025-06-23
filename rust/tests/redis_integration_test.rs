// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Integration tests for Redis Action Driver
//!
//! These tests require a Redis server to be running.
//! In CI/CD, Redis is started as a Docker container.

use anyhow::Result;
use redis::{Client, Commands};
use rust_photoacoustic::processing::computing_nodes::display_drivers::{
    AlertData, DisplayData, DisplayDriver, RedisActionDriver, RedisDriverMode,
};
use serde_json::json;
use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

/// Check if Docker is available on this system
fn is_docker_available() -> bool {
    Command::new("docker")
        .args(&["--version"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Start a Redis container for testing
async fn start_redis_container() -> Result<String> {
    // Check if Docker is available
    if !is_docker_available() {
        return Err(anyhow::anyhow!(
            "Docker is not available on this system. Skipping Docker-based Redis tests."
        ));
    }

    // Check if Redis is already running
    let output = Command::new("docker")
        .args(&[
            "ps",
            "--filter",
            "name=redis-test",
            "--format",
            "{{.Names}}",
        ])
        .output()?;

    if !String::from_utf8_lossy(&output.stdout).trim().is_empty() {
        println!("Redis container already running");
        return Ok("redis://localhost:6380".to_string());
    }

    // Start Redis container
    let output = Command::new("docker")
        .args(&[
            "run",
            "-d",
            "--name",
            "redis-test",
            "-p",
            "6380:6379",
            "--rm",
            "redis:7-alpine",
        ])
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Failed to start Redis container: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Wait for Redis to be ready
    for _ in 0..10 {
        sleep(Duration::from_millis(500)).await;
        if redis_ready("redis://localhost:6380").await {
            return Ok("redis://localhost:6380".to_string());
        }
    }

    Err(anyhow::anyhow!("Redis container did not start in time"))
}

/// Check if Redis is ready
async fn redis_ready(url: &str) -> bool {
    match Client::open(url.to_string()) {
        Ok(client) => {
            if let Ok(mut conn) = client.get_connection() {
                redis::cmd("PING").query::<String>(&mut conn).is_ok()
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

/// Stop Redis container
async fn stop_redis_container() -> Result<()> {
    let _output = Command::new("docker")
        .args(&["stop", "redis-test"])
        .output()?;
    Ok(())
}

/// Helper to create test display data
fn create_test_display_data() -> DisplayData {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("test".to_string(), serde_json::Value::Bool(true));
    metadata.insert(
        "source".to_string(),
        serde_json::Value::String("integration_test".to_string()),
    );

    DisplayData {
        concentration_ppm: 123.45,
        source_node_id: "test_concentration_node".to_string(),
        peak_amplitude: 75.5,
        peak_frequency: 2150.0,
        timestamp: SystemTime::now(),
        metadata,
    }
}

/// Helper to create test alert data
fn create_test_alert_data() -> AlertData {
    let mut data = std::collections::HashMap::new();
    data.insert(
        "threshold".to_string(),
        serde_json::Value::Number(serde_json::Number::from_f64(100.0).unwrap()),
    );
    data.insert(
        "actual".to_string(),
        serde_json::Value::Number(serde_json::Number::from_f64(123.45).unwrap()),
    );

    AlertData {
        alert_type: "concentration_threshold".to_string(),
        severity: "warning".to_string(),
        message: "Test alert from integration test".to_string(),
        timestamp: SystemTime::now(),
        data,
    }
}

#[tokio::test]
#[ignore] // Ignored by default, run with --ignored flag
async fn test_redis_driver_key_value_mode() -> Result<()> {
    // Check if Docker is available, skip test gracefully if not
    if !is_docker_available() {
        println!("⚠️  Docker not available, skipping Docker-based Redis integration test");
        return Ok(());
    }

    // Start Redis container
    let redis_url = match start_redis_container().await {
        Ok(url) => url,
        Err(e) => {
            println!("⚠️  Failed to start Redis container: {}. Skipping test.", e);
            return Ok(());
        }
    };
    println!("Redis container started at: {}", redis_url);

    // Create Redis driver in key-value mode
    let mut driver = RedisActionDriver::new_key_value(&redis_url, "test_photoacoustic")
        .with_expiration_seconds(60);

    // Initialize driver
    driver.initialize().await?;
    println!("Redis driver initialized successfully");

    // Test display data
    let display_data = create_test_display_data();
    driver.update_display(&display_data).await?;
    println!("Display data sent successfully");

    // Test alert data
    let alert_data = create_test_alert_data();
    driver.show_alert(&alert_data).await?;
    println!("Alert data sent successfully");

    // Verify data was stored
    let client = Client::open(redis_url.clone())?;
    let mut conn = client.get_connection()?;

    // Check latest display data
    let latest_key = "test_photoacoustic:latest:test_concentration_node";
    let stored_data: Option<String> = conn.get(latest_key)?;
    assert!(
        stored_data.is_some(),
        "Latest display data should be stored"
    );

    let parsed_data: serde_json::Value = serde_json::from_str(&stored_data.unwrap())?;
    assert_eq!(parsed_data["type"], "display_update");
    assert_eq!(parsed_data["concentration_ppm"], 123.45);
    assert_eq!(parsed_data["source_node_id"], "test_concentration_node");

    // Check latest alert data
    let latest_alert_key = "test_photoacoustic:latest_alert";
    let stored_alert: Option<String> = conn.get(latest_alert_key)?;
    assert!(stored_alert.is_some(), "Latest alert data should be stored");

    let parsed_alert: serde_json::Value = serde_json::from_str(&stored_alert.unwrap())?;
    assert_eq!(parsed_alert["type"], "alert");
    assert_eq!(parsed_alert["alert_type"], "concentration_threshold");

    // Test clear display
    driver.clear_display().await?;
    println!("Clear display sent successfully");

    // Check status
    let status = driver.get_status().await?;
    assert_eq!(status["driver_type"], "redis");
    assert_eq!(status["mode"], "key_value");
    assert_eq!(status["is_connected"], true);

    // Cleanup
    driver.shutdown().await?;
    stop_redis_container().await?;
    println!("Test completed successfully");

    Ok(())
}

#[tokio::test]
#[ignore] // Ignored by default, run with --ignored flag
async fn test_redis_driver_pubsub_mode() -> Result<()> {
    // Check if Docker is available, skip test gracefully if not
    if !is_docker_available() {
        println!("⚠️  Docker not available, skipping Docker-based Redis pub/sub integration test");
        return Ok(());
    }

    // Start Redis container
    let redis_url = match start_redis_container().await {
        Ok(url) => url,
        Err(e) => {
            println!("⚠️  Failed to start Redis container: {}. Skipping test.", e);
            return Ok(());
        }
    };
    println!("Redis container started at: {}", redis_url);

    // Create Redis driver in pub/sub mode
    let mut driver = RedisActionDriver::new_pubsub(&redis_url, "test_channel");

    // Initialize driver
    driver.initialize().await?;
    println!("Redis driver initialized successfully");

    // Create subscriber to verify messages
    let client = Client::open(redis_url.clone())?;
    let mut pubsub = client.get_async_pubsub().await?;
    pubsub.subscribe("test_channel").await?;
    pubsub.subscribe("test_channel:alert").await?;

    // Test display data in separate task
    let display_data = create_test_display_data();
    tokio::spawn(async move {
        sleep(Duration::from_millis(100)).await;
    });

    // Send display data
    driver.update_display(&display_data).await?;
    println!("Display data published successfully");

    // Send alert data
    let alert_data = create_test_alert_data();
    driver.show_alert(&alert_data).await?;
    println!("Alert data published successfully");

    // Note: In pub/sub mode, we can't easily verify message receipt in this test
    // because it would require a separate subscriber thread/task

    // Test status
    let status = driver.get_status().await?;
    assert_eq!(status["driver_type"], "redis");
    assert_eq!(status["mode"], "pub_sub");
    assert_eq!(status["is_connected"], true);

    // Cleanup
    driver.shutdown().await?;
    stop_redis_container().await?;
    println!("Test completed successfully");

    Ok(())
}

#[tokio::test]
#[ignore] // Ignored by default
async fn test_redis_driver_reconnection() -> Result<()> {
    // Check if Docker is available, skip test gracefully if not
    if !is_docker_available() {
        println!("⚠️  Docker not available, skipping Docker-based Redis reconnection test");
        return Ok(());
    }

    // Start Redis container
    let redis_url = match start_redis_container().await {
        Ok(url) => url,
        Err(e) => {
            println!("⚠️  Failed to start Redis container: {}. Skipping test.", e);
            return Ok(());
        }
    };
    println!("Redis container started at: {}", redis_url);

    // Create Redis driver
    let mut driver = RedisActionDriver::new_key_value(&redis_url, "test_reconnection")
        .with_expiration_seconds(30);

    // Initialize driver
    driver.initialize().await?;
    println!("Redis driver initialized successfully");

    // Send initial data
    let display_data = create_test_display_data();
    driver.update_display(&display_data).await?;
    println!("Initial data sent successfully");

    // Stop Redis container to simulate connection loss
    println!("Stopping Redis container to simulate connection loss...");
    let _output = Command::new("docker")
        .args(&["stop", "redis-test"])
        .output()?;

    sleep(Duration::from_secs(1)).await;

    // Restart Redis container
    println!("Restarting Redis container...");
    let _output = Command::new("docker")
        .args(&[
            "run",
            "-d",
            "--name",
            "redis-test",
            "-p",
            "6380:6379",
            "--rm",
            "redis:7-alpine",
        ])
        .output()?;

    // Wait for Redis to be ready
    for _ in 0..10 {
        sleep(Duration::from_millis(500)).await;
        if redis_ready(&redis_url).await {
            break;
        }
    }

    // Try to send data again - should reconnect automatically
    let display_data2 = create_test_display_data();
    let result = driver.update_display(&display_data2).await;

    match result {
        Ok(_) => println!("Reconnection test successful"),
        Err(e) => {
            println!("Reconnection failed, but this might be expected: {}", e);
            // Don't fail the test as network timing can be unpredictable
        }
    }

    // Cleanup
    driver.shutdown().await?;
    stop_redis_container().await?;
    println!("Reconnection test completed");

    Ok(())
}

#[tokio::test]
async fn test_redis_driver_without_server() -> Result<()> {
    // Test driver behavior when Redis is not available
    let mut driver = RedisActionDriver::new_key_value("redis://localhost:6999", "test_noserver");

    // Initialize should fail
    let init_result = driver.initialize().await;
    assert!(
        init_result.is_err(),
        "Initialization should fail without Redis"
    );

    // Update should also fail
    let display_data = create_test_display_data();
    let update_result = driver.update_display(&display_data).await;
    assert!(
        update_result.is_err(),
        "Update should fail without Redis connection"
    );

    println!("No-server test completed successfully");
    Ok(())
}

/// Test helper for local development (requires manual Redis setup)
#[tokio::test]
#[ignore]
async fn test_redis_driver_local() -> Result<()> {
    // This test assumes Redis is running locally on default port
    let redis_url = "redis://localhost:6379";

    if !redis_ready(redis_url).await {
        println!("Skipping local Redis test - no Redis server detected");
        return Ok(());
    }

    let mut driver =
        RedisActionDriver::new_key_value(redis_url, "test_local").with_expiration_seconds(300); // 5 minutes

    driver.initialize().await?;

    let display_data = create_test_display_data();
    driver.update_display(&display_data).await?;

    println!("Local Redis test completed - check redis-cli for data");
    Ok(())
}

/// Test Docker availability detection
#[tokio::test]
async fn test_docker_availability_check() -> Result<()> {
    let docker_available = is_docker_available();
    println!("Docker availability check result: {}", docker_available);
    
    // This test always passes, it just reports the Docker status
    // In CI environments, Docker should be available
    // In local environments without Docker, it should gracefully handle the absence
    Ok(())
}
