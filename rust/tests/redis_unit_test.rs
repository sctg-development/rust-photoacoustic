// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Unit tests for Redis Action Driver
//!
//! These tests focus on testing the driver logic without requiring a Redis server.

use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ActionDriver, AlertData, MeasurementData, RedisActionDriver, RedisDriverMode,
};
use serde_json::json;
use std::time::SystemTime;

#[tokio::test]
async fn test_redis_driver_creation() {
    // Test pub/sub mode creation
    let pubsub_driver = RedisActionDriver::new_pubsub("redis://localhost:6379", "test_channel");
    let status = pubsub_driver.get_status().await.unwrap();

    assert_eq!(status["driver_type"], "redis");
    assert_eq!(status["mode"], "pub_sub");
    assert_eq!(status["channel_or_prefix"], "test_channel");
    assert_eq!(status["url"], "redis://localhost:6379");
    assert_eq!(status["is_connected"], false); // Not connected yet

    // Test key-value mode creation
    let kv_driver = RedisActionDriver::new_key_value("redis://localhost:6379", "test_prefix")
        .with_expiration_seconds(3600);
    let status = kv_driver.get_status().await.unwrap();

    assert_eq!(status["driver_type"], "redis");
    assert_eq!(status["mode"], "key_value");
    assert_eq!(status["channel_or_prefix"], "test_prefix");
    assert_eq!(status["expiration_seconds"], 3600);
}

#[tokio::test]
async fn test_redis_driver_expiration_settings() {
    // Test with expiration
    let driver_with_exp = RedisActionDriver::new_key_value("redis://localhost:6379", "test")
        .with_expiration_seconds(7200);
    let status = driver_with_exp.get_status().await.unwrap();
    assert_eq!(status["expiration_seconds"], 7200);

    // Test with zero expiration (should be None)
    let driver_no_exp = RedisActionDriver::new_key_value("redis://localhost:6379", "test")
        .with_expiration_seconds(0);
    let status = driver_no_exp.get_status().await.unwrap();
    assert!(status["expiration_seconds"].is_null());
}

#[tokio::test]
async fn test_redis_driver_connection_failure() {
    // Test connection to non-existent Redis server
    let mut driver = RedisActionDriver::new_key_value("redis://localhost:9999", "test");

    // Initialize should fail gracefully
    let result = driver.initialize().await;
    assert!(result.is_err());

    // Status should show connection error
    let status = driver.get_status().await.unwrap();
    assert_eq!(status["is_connected"], false);
    assert!(
        status["connection_status"]
            .as_str()
            .unwrap()
            .contains("error")
            || status["connection_status"]
                .as_str()
                .unwrap()
                .contains("Error")
            || status["connection_status"]
                .as_str()
                .unwrap()
                .contains("Initializing")
    );
}

#[test]
fn test_display_data_creation() {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("test".to_string(), serde_json::Value::Bool(true));

    let display_data = MeasurementData {
        concentration_ppm: 42.5,
        source_node_id: "test_node".to_string(),
        peak_amplitude: 65.0,
        peak_frequency: 2100.0,
        timestamp: SystemTime::now(),
        metadata,
    };

    assert_eq!(display_data.concentration_ppm, 42.5);
    assert_eq!(display_data.source_node_id, "test_node");
    assert_eq!(display_data.peak_amplitude, 65.0);
    assert_eq!(display_data.peak_frequency, 2100.0);
}

#[test]
fn test_alert_data_creation() {
    let mut data = std::collections::HashMap::new();
    data.insert(
        "threshold".to_string(),
        serde_json::Value::Number(serde_json::Number::from_f64(100.0).unwrap()),
    );
    data.insert(
        "actual".to_string(),
        serde_json::Value::Number(serde_json::Number::from_f64(150.5).unwrap()),
    );

    let alert_data = AlertData {
        alert_type: "threshold_exceeded".to_string(),
        severity: "critical".to_string(),
        message: "Concentration threshold exceeded".to_string(),
        timestamp: SystemTime::now(),
        data,
    };

    assert_eq!(alert_data.alert_type, "threshold_exceeded");
    assert_eq!(alert_data.severity, "critical");
    assert_eq!(alert_data.message, "Concentration threshold exceeded");
}

#[test]
fn test_redis_driver_mode_clone() {
    let mode1 = RedisDriverMode::PubSub;
    let mode2 = mode1.clone();

    // Verify enum can be cloned and compared
    match (mode1, mode2) {
        (RedisDriverMode::PubSub, RedisDriverMode::PubSub) => {}
        _ => panic!("Mode cloning failed"),
    }

    let mode3 = RedisDriverMode::KeyValue;
    let mode4 = mode3.clone();

    match (mode3, mode4) {
        (RedisDriverMode::KeyValue, RedisDriverMode::KeyValue) => {}
        _ => panic!("Mode cloning failed"),
    }
}

#[tokio::test]
async fn test_redis_driver_shutdown() {
    let mut driver = RedisActionDriver::new_pubsub("redis://localhost:6379", "test");

    // Shutdown should succeed even without connection
    let result = driver.shutdown().await;
    assert!(result.is_ok());

    // Status should show not connected after shutdown
    let status = driver.get_status().await.unwrap();
    assert_eq!(status["is_connected"], false);
}
