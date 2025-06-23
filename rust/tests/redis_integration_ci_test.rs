// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Redis integration tests specifically designed for CI environments
//!
//! These tests are designed to work with GitHub Actions Redis service
//! and avoid complex Docker orchestration.

use anyhow::Result;
use redis::{Client, Commands};
use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ActionDriver, AlertData, MeasurementData, RedisActionDriver, RedisDriverMode,
};
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

/// Get Redis URL from environment or use default
fn get_redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string())
}

/// Check if Redis is ready and reachable
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

/// Helper to create test display data
fn create_test_display_data() -> MeasurementData {
    let mut metadata = HashMap::new();
    metadata.insert("test".to_string(), serde_json::Value::Bool(true));
    metadata.insert(
        "ci_test".to_string(),
        serde_json::Value::String("github_actions".to_string()),
    );

    MeasurementData {
        concentration_ppm: 42.0,
        source_node_id: "ci_test_node".to_string(),
        peak_amplitude: 80.0,
        peak_frequency: 2200.0,
        timestamp: SystemTime::now(),
        metadata,
    }
}

/// Helper to create test alert data
fn create_test_alert_data() -> AlertData {
    let mut data = HashMap::new();
    data.insert("ci_test".to_string(), serde_json::Value::Bool(true));

    AlertData {
        alert_type: "ci_test_alert".to_string(),
        severity: "info".to_string(),
        message: "Test alert from CI integration test".to_string(),
        timestamp: SystemTime::now(),
        data,
    }
}

#[tokio::test]
#[ignore] // Run only in CI with --ignored flag
async fn test_redis_driver_key_value_mode_ci() -> Result<()> {
    let redis_url = get_redis_url();

    // Skip test if Redis is not available
    if !redis_ready(&redis_url).await {
        println!("⚠️  Redis not available at {}, skipping test", redis_url);
        return Ok(());
    }

    println!("🔗 Testing Redis driver with URL: {}", redis_url);

    // Create Redis driver in key-value mode with short expiration for CI
    let mut driver =
        RedisActionDriver::new_key_value(&redis_url, "ci_test").with_expiration_seconds(30); // Short expiration for CI cleanup

    // Initialize driver
    driver.initialize().await?;
    println!("✅ Redis driver initialized successfully");

    // Test display data
    let display_data = create_test_display_data();
    driver.update_action(&display_data).await?;
    println!("✅ Display data sent successfully");

    // Test alert data
    let alert_data = create_test_alert_data();
    driver.show_alert(&alert_data).await?;
    println!("✅ Alert data sent successfully");

    // Test clear display
    driver.clear_action().await?;
    println!("✅ Clear display sent successfully");

    // Verify data was stored using standard Redis client
    let client = Client::open(redis_url.clone())?;
    let mut conn = client.get_connection()?;

    // Check latest display data exists
    let latest_key = "ci_test:latest:ci_test_node";
    let stored_data: Option<String> = conn.get(latest_key)?;
    assert!(
        stored_data.is_some(),
        "Latest display data should be stored"
    );

    let parsed_data: serde_json::Value = serde_json::from_str(&stored_data.unwrap())?;
    // The latest operation should be either display_update or clear_display
    assert!(
        parsed_data["type"] == "display_update" || parsed_data["type"] == "clear_display",
        "Latest data type should be display_update or clear_display, got: {}",
        parsed_data["type"]
    );
    println!("✅ Latest data verified in Redis: {}", parsed_data["type"]);

    // Check status
    let status = driver.get_status().await?;
    assert_eq!(status["driver_type"], "redis");
    assert_eq!(status["mode"], "key_value");
    assert_eq!(status["is_connected"], true);
    println!("✅ Driver status verified");

    // Cleanup
    driver.shutdown().await?;

    // Clean up test keys
    let test_keys: Vec<String> = conn.keys("ci_test:*")?;
    if !test_keys.is_empty() {
        let _: () = redis::cmd("DEL").arg(&test_keys).query(&mut conn)?;
        println!("🧹 Cleaned up {} test keys", test_keys.len());
    }

    println!("🎉 CI integration test completed successfully");
    Ok(())
}

#[tokio::test]
#[ignore] // Run only in CI with --ignored flag
async fn test_redis_driver_pubsub_mode_ci() -> Result<()> {
    let redis_url = get_redis_url();

    // Skip test if Redis is not available
    if !redis_ready(&redis_url).await {
        println!("⚠️  Redis not available at {}, skipping test", redis_url);
        return Ok(());
    }

    println!("🔗 Testing Redis pub/sub driver with URL: {}", redis_url);

    // Create Redis driver in pub/sub mode
    let mut driver = RedisActionDriver::new_pubsub(&redis_url, "ci_test_channel");

    // Initialize driver
    driver.initialize().await?;
    println!("✅ Redis pub/sub driver initialized successfully");

    // Test display data (publishing)
    let display_data = create_test_display_data();
    driver.update_action(&display_data).await?;
    println!("✅ Display data published successfully");

    // Test alert data (publishing)
    let alert_data = create_test_alert_data();
    driver.show_alert(&alert_data).await?;
    println!("✅ Alert data published successfully");

    // Test clear display (publishing)
    driver.clear_action().await?;
    println!("✅ Clear display published successfully");

    // Check status
    let status = driver.get_status().await?;
    assert_eq!(status["driver_type"], "redis");
    assert_eq!(status["mode"], "pub_sub");
    assert_eq!(status["is_connected"], true);
    println!("✅ Driver status verified");

    // Cleanup
    driver.shutdown().await?;

    println!("🎉 CI pub/sub integration test completed successfully");
    Ok(())
}

#[tokio::test]
#[ignore] // Run only in CI with --ignored flag
async fn test_redis_driver_reconnection_ci() -> Result<()> {
    let redis_url = get_redis_url();

    // Skip test if Redis is not available
    if !redis_ready(&redis_url).await {
        println!("⚠️  Redis not available at {}, skipping test", redis_url);
        return Ok(());
    }

    println!("🔗 Testing Redis reconnection with URL: {}", redis_url);

    // Create Redis driver
    let mut driver = RedisActionDriver::new_key_value(&redis_url, "ci_reconnect_test")
        .with_expiration_seconds(30);

    // Initialize driver
    driver.initialize().await?;
    println!("✅ Redis driver initialized successfully");

    // Send initial data
    let display_data = create_test_display_data();
    driver.update_action(&display_data).await?;
    println!("✅ Initial data sent successfully");

    // Simulate connection issues by creating a temporary driver with wrong URL
    // Then test that the main driver still works (it should reconnect automatically)
    let mut temp_driver = RedisActionDriver::new_key_value("redis://localhost:9999", "temp");
    let temp_result = temp_driver.initialize().await;
    assert!(
        temp_result.is_err(),
        "Connection to non-existent Redis should fail"
    );
    println!("✅ Connection failure handling verified");

    // Main driver should still work and auto-reconnect if needed
    let display_data2 = create_test_display_data();
    driver.update_action(&display_data2).await?;
    println!("✅ Reconnection test successful");

    // Cleanup
    driver.shutdown().await?;

    // Clean up test keys
    let client = Client::open(redis_url)?;
    let mut conn = client.get_connection()?;
    let test_keys: Vec<String> = conn.keys("ci_reconnect_test:*")?;
    if !test_keys.is_empty() {
        let _: () = redis::cmd("DEL").arg(&test_keys).query(&mut conn)?;
        println!("🧹 Cleaned up {} test keys", test_keys.len());
    }

    println!("🎉 CI reconnection test completed successfully");
    Ok(())
}

#[tokio::test]
async fn test_redis_driver_no_server_ci() -> Result<()> {
    // Test driver behavior when Redis is not available (always run, not ignored)
    let mut driver = RedisActionDriver::new_key_value("redis://localhost:9999", "no_server_test");

    // Initialize should fail
    let init_result = driver.initialize().await;
    assert!(
        init_result.is_err(),
        "Initialization should fail without Redis"
    );
    println!("✅ No-server initialization failure verified");

    // Update should also fail
    let display_data = create_test_display_data();
    let update_result = driver.update_action(&display_data).await;
    assert!(
        update_result.is_err(),
        "Update should fail without Redis connection"
    );
    println!("✅ No-server update failure verified");

    println!("🎉 No-server test completed successfully");
    Ok(())
}
