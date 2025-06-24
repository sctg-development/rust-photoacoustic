// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Integration tests for Action Node History API endpoints
//!
//! These tests validate the REST API endpoints for accessing historical data
//! from UniversalActionNode instances, including authentication and authorization.
//! Uses a real daemon instance with the example configuration to test with actual
//! action nodes and processing graph.

use anyhow::Result;
use rust_photoacoustic::{
    config::Config,
    daemon::launch_daemon::Daemon,
    utility::jwt_token::{ConfigLoader, JwtAlgorithm, TokenCreationParams, TokenCreator},
};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{sync::RwLock, time::sleep};

/// Test that the action API endpoints are accessible and return expected structure
#[tokio::test]
async fn test_action_endpoints_integration() -> Result<()> {
    // Initialize logging for debugging
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    // Load the example configuration which contains action nodes
    let config_path = PathBuf::from("config.example.yaml");
    let config = Config::from_file(&config_path)?;

    println!("Loaded configuration from: {:?}", config_path);

    // Create shared configuration for the daemon
    let config_arc = Arc::new(RwLock::new(config.clone()));

    // Create and launch the daemon (this starts all services including processing)
    let mut daemon = Daemon::new();

    println!("Starting daemon with real processing graph...");
    daemon.launch(config_arc.clone()).await?;

    // Wait for the system to initialize
    println!("Waiting for processing system to initialize...");
    sleep(Duration::from_secs(3)).await;

    // Create JWT token for API authentication
    let access_token = create_admin_jwt_token(&config)?;
    println!("Created JWT token for administrator");

    // Test the API endpoints with the real running server
    let api_base_url = format!("https://localhost:{}", config.visualization.port);
    println!("Testing action API endpoints at: {}", api_base_url);

    // Create HTTP client for API calls with TLS certificate acceptance
    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;

    // Test 1: List all action nodes
    println!("\n=== Testing /api/action endpoint ===");
    let response = client
        .get(&format!("{}/api/action", api_base_url))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    assert_eq!(response.status(), 200, "Action list endpoint should return 200 OK");

    let action_nodes: Value = response.json().await?;
    println!("Action nodes response: {}", serde_json::to_string_pretty(&action_nodes)?);

    // The response should be an array
    assert!(action_nodes.is_array(), "Response should be an array of action nodes");
    
    let nodes_array = action_nodes.as_array().unwrap();
    println!("Found {} action nodes", nodes_array.len());

    // If we have action nodes, test accessing their data
    if !nodes_array.is_empty() {
        let first_node = &nodes_array[0];
        let node_id = first_node["id"].as_str()
            .expect("Action node should have an id field");
        
        println!("Testing endpoints for action node: {}", node_id);

        // Test 2: Get history for the first action node
        println!("\n=== Testing /api/action/{}/history endpoint ===", node_id);
        let history_response = client
            .get(&format!("{}/api/action/{}/history", api_base_url, node_id))
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        assert_eq!(history_response.status(), 200, "History endpoint should return 200 OK");

        let history_data: Value = history_response.json().await?;
        println!("History data: {}", serde_json::to_string_pretty(&history_data)?);
        
        // Should be an array of measurements
        assert!(history_data.is_array(), "History should be an array of measurements");

        // Test 3: Get history with limit parameter
        println!("\n=== Testing /api/action/{}/history?limit=5 endpoint ===", node_id);
        let limited_history_response = client
            .get(&format!("{}/api/action/{}/history?limit=5", api_base_url, node_id))
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        assert_eq!(limited_history_response.status(), 200, "Limited history endpoint should return 200 OK");

        let limited_history_data: Value = limited_history_response.json().await?;
        let limited_array = limited_history_data.as_array().unwrap();
        
        // Should have at most 5 entries
        assert!(limited_array.len() <= 5, "Limited history should have at most 5 entries");
        println!("Limited history returned {} entries", limited_array.len());

        // Test 4: Get statistics for the action node
        println!("\n=== Testing /api/action/{}/history/stats endpoint ===", node_id);
        let stats_response = client
            .get(&format!("{}/api/action/{}/history/stats", api_base_url, node_id))
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        assert_eq!(stats_response.status(), 200, "Stats endpoint should return 200 OK");

        let stats_data: Value = stats_response.json().await?;
        println!("Stats data: {}", serde_json::to_string_pretty(&stats_data)?);
        
        // Should be an object with statistics
        assert!(stats_data.is_object(), "Stats should be an object");
        
        // Verify it has expected fields
        assert!(stats_data.get("node_id").is_some(), "Stats should have node_id");
        assert!(stats_data.get("node_type").is_some(), "Stats should have node_type");
        
        println!("✓ All action node endpoints working correctly for node: {}", node_id);
    } else {
        println!("⚠ No action nodes found in the processing graph");
        println!("This might indicate that the example configuration doesn't contain UniversalActionNode instances");
    }

    // Clean shutdown
    println!("\n=== Shutting down daemon ===");
    daemon.shutdown();
    daemon.join().await?;

    println!("✓ Action API integration test completed successfully");
    Ok(())
}

/// Test that the action API endpoints require authentication
#[tokio::test]
async fn test_action_endpoints_authentication() -> Result<()> {
    // Initialize logging
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init();

    // Load configuration
    let config_path = PathBuf::from("config.example.yaml");
    let config = Config::from_file(&config_path)?;
    let config_arc = Arc::new(RwLock::new(config.clone()));

    // Create and launch daemon
    let mut daemon = Daemon::new();
    daemon.launch(config_arc.clone()).await?;

    // Wait for initialization
    sleep(Duration::from_secs(2)).await;

    let api_base_url = format!("https://localhost:{}", config.visualization.port);

    // Create HTTP client
    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;

    // Test without authentication - should get 401 Unauthorized
    println!("Testing endpoints without authentication...");

    let response = client
        .get(&format!("{}/api/action", api_base_url))
        .send()
        .await?;
    assert_eq!(response.status(), 401, "Should require authentication");

    let response = client
        .get(&format!("{}/api/action/test_node/history", api_base_url))
        .send()
        .await?;
    assert_eq!(response.status(), 401, "Should require authentication");

    let response = client
        .get(&format!("{}/api/action/test_node/history/stats", api_base_url))
        .send()
        .await?;
    assert_eq!(response.status(), 401, "Should require authentication");

    println!("✓ All endpoints correctly require authentication");

    // Clean shutdown
    daemon.shutdown();
    daemon.join().await?;

    Ok(())
}

/// Test that action API endpoints are properly documented in OpenAPI spec
#[tokio::test]
async fn test_action_endpoints_openapi() -> Result<()> {
    // Initialize logging
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init();

    // Load configuration
    let config_path = PathBuf::from("config.example.yaml");
    let config = Config::from_file(&config_path)?;
    let config_arc = Arc::new(RwLock::new(config.clone()));

    // Create and launch daemon
    let mut daemon = Daemon::new();
    daemon.launch(config_arc.clone()).await?;

    // Wait for initialization
    sleep(Duration::from_secs(2)).await;

    let api_base_url = format!("https://localhost:{}", config.visualization.port);

    // Create HTTP client
    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;

    // Test that OpenAPI spec is available and includes our action endpoints
    println!("Testing OpenAPI documentation...");
    let response = client
        .get(&format!("{}/openapi.json", api_base_url))
        .send()
        .await?;

    assert_eq!(response.status(), 200, "OpenAPI spec should be available");
    
    let openapi_spec = response.text().await?;
    
    // Verify that our action endpoints are documented in the OpenAPI spec
    assert!(openapi_spec.contains("/api/action"), "Should document /api/action endpoint");
    assert!(openapi_spec.contains("/api/action/{node_id}/history"), "Should document history endpoint");
    assert!(openapi_spec.contains("/api/action/{node_id}/history/stats"), "Should document stats endpoint");
    assert!(openapi_spec.contains("Action History"), "Should include our tag");
    
    println!("✓ Action endpoints are properly documented in OpenAPI spec");

    // Clean shutdown
    daemon.shutdown();
    daemon.join().await?;

    Ok(())
}

/// Create a JWT token for the administrator user from the example configuration
/// (copied from real_world_peak_endpoint_test.rs)
fn create_admin_jwt_token(config: &Config) -> Result<String> {
    let config_loader = ConfigLoader::from_config(config)?;
    let token_creator = TokenCreator::new(&config_loader)?;

    let params = TokenCreationParams {
        user_id: "administrator".to_string(), // From config.example.yaml
        client_id: "LaserSmartClient".to_string(),
        algorithm: JwtAlgorithm::RS256,
        duration_seconds: 300, // 5 minutes should be enough for the test
    };

    let result = token_creator.create_token(&params)?;
    Ok(result.token)
}
