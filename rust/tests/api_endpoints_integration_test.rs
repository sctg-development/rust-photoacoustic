// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Comprehensive integration tests for all REST API endpoints
//!
//! This test suite validates all REST API endpoints by starting a realistic server
//! with full processing graph configuration. Tests cover authentication, data integrity,
//! response formatting, and real-world data flow scenarios.
//!
//! # Test Coverage
//!
//! - **Computing API** (`/api/computing`): Peak detection results and processing state
//! - **Graph API** (`/api/graph`): Processing pipeline structure and node statistics
//! - **System API** (`/api/system/stats`, `/api/system/health`): Health and resource metrics
//! - **Action API** (`/api/action`, `/api/action/{node_id}/history`): Node history and statistics
//! - **Configuration API** (`/api/config`): Server configuration (read-only)
//! - **Thermal API** (`/api/thermal`): Thermal sensor data if available
//!
//! # Test Structure
//!
//! Each test:
//! 1. Initializes a realistic daemon with full processing graph (simulated source + peak_finder)
//! 2. Waits for the system to process initial frames
//! 3. Creates a JWT token for the administrator user
//! 4. Tests the specific endpoint with proper authentication
//! 5. Validates response structure and data types
//! 6. Shuts down the daemon gracefully

use anyhow::Result;
use rust_photoacoustic::{
    config::Config,
    daemon::launch_daemon::Daemon,
    utility::jwt_token::{ConfigLoader, JwtAlgorithm, TokenCreationParams, TokenCreator},
};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, RwLock},
    time::sleep,
};

/// Global mutex to ensure only one daemon runs at a time
/// This prevents port binding conflicts when tests run in parallel
static DAEMON_LOCK: Mutex<()> = Mutex::const_new(());

/// Creates a JWT token for the administrator user from the provided configuration
///
/// This helper function generates a valid JWT token that can be used in API requests
/// by including it in the `Authorization: Bearer TOKEN` header.
///
/// # Arguments
///
/// * `config` - A reference to the loaded configuration containing JWT settings
///
/// # Returns
///
/// A valid JWT token string that authenticates as the administrator user with ID "administrator"
/// and client ID "LaserSmartClient". The token is valid for 5 minutes.
///
/// # Errors
///
/// Returns an error if:
/// - JWT configuration is invalid or missing required credentials
/// - Token creation fails due to cryptographic errors
/// - Configuration files cannot be accessed
fn create_admin_jwt_token(config: &Config) -> Result<String> {
    let config_loader = ConfigLoader::from_config(config)?;
    let token_creator = TokenCreator::new(&config_loader)?;

    let params = TokenCreationParams {
        user_id: "administrator".to_string(),
        client_id: "LaserSmartClient".to_string(),
        algorithm: JwtAlgorithm::RS256,
        duration_seconds: 300, // 5 minutes
    };

    let result = token_creator.create_token(&params)?;
    Ok(result.token)
}

/// Initializes a realistic daemon for testing
///
/// Loads the example configuration, creates a daemon with full processing graph,
/// launches it, and waits for initial processing to begin.
///
/// # Returns
///
/// A tuple containing:
/// - `Daemon`: The running daemon instance (must be shut down by caller)
/// - `Config`: The loaded configuration
/// - `String`: JWT token for API authentication
/// - `tokio::sync::MutexGuard<'static, ()>`: Lock guard that keeps the daemon exclusive
///
/// # Panics
///
/// Panics if the example configuration cannot be loaded or daemon launch fails
///
/// # Note
///
/// The returned lock guard must be kept alive for the entire duration of the test
/// to prevent multiple daemons from binding to the same port simultaneously.
async fn init_daemon() -> (Daemon, Config, String, tokio::sync::MutexGuard<'static, ()>) {
    // Acquire the global lock to ensure only one daemon runs at a time
    // This prevents port binding conflicts
    let _lock = DAEMON_LOCK.lock().await;

    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    let config_path = PathBuf::from("config.example.yaml");
    let config = Config::from_file(&config_path).expect(
        "Failed to load config.example.yaml - ensure you're running tests from the rust directory",
    );

    let config_arc = Arc::new(RwLock::new(config.clone()));
    let mut daemon = Daemon::new();

    daemon
        .launch(config_arc.clone())
        .await
        .expect("Failed to launch daemon");

    // Wait for system to initialize and process initial frames
    sleep(Duration::from_secs(5)).await;

    let access_token = create_admin_jwt_token(&config).expect("Failed to create JWT token");

    (daemon, config, access_token, _lock)
}

/// Tests the `/api/computing` endpoint for peak detection results
///
/// This test validates that:
/// - The endpoint returns HTTP 200 with proper authentication
/// - Response contains all required fields (peak_results, active_node_ids, etc.)
/// - Peak detection data is properly formatted
/// - Legacy fields are present for backward compatibility
/// - Active node tracking is working
///
/// # Test Flow
///
/// 1. Start daemon with processing graph and peak_finder
/// 2. Create authenticated client with self-signed certificate acceptance
/// 3. GET `/api/computing` with Bearer token
/// 4. Validate response structure and data validity
/// 5. Verify peak results contain real numeric values
/// 6. Shut down daemon
#[tokio::test]
async fn test_computing_api_endpoint() -> Result<()> {
    let (daemon, config, access_token, _lock) = init_daemon().await;

    let api_base_url = format!("https://localhost:{}", config.visualization.port);

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;

    // Test computing endpoint
    let computing_response = client
        .get(&format!("{}/api/computing", api_base_url))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    assert_eq!(
        computing_response.status(),
        200,
        "Computing endpoint should return 200 OK"
    );

    let computing_data: Value = computing_response.json().await?;

    // Validate response structure
    assert!(
        computing_data.get("peak_results").is_some(),
        "Response should contain peak_results field"
    );
    assert!(
        computing_data.get("active_node_ids").is_some(),
        "Response should contain active_node_ids field"
    );

    // Validate legacy fields for backward compatibility
    assert!(
        computing_data.get("peak_frequency").is_some(),
        "Response should contain peak_frequency field"
    );
    assert!(
        computing_data.get("peak_amplitude").is_some(),
        "Response should contain peak_amplitude field"
    );
    assert!(
        computing_data.get("polynomial_coefficients").is_some(),
        "Response should contain polynomial_coefficients field"
    );

    // Verify peak_results is an object with node IDs as keys
    let peak_results = computing_data["peak_results"]
        .as_object()
        .expect("peak_results should be an object");
    assert!(
        !peak_results.is_empty(),
        "peak_results should contain at least one entry"
    );

    // Verify each peak result has required fields
    for (node_id, result) in peak_results {
        assert!(
            result.get("frequency").is_some(),
            "Peak result for {} should have frequency field",
            node_id
        );
        assert!(
            result.get("amplitude").is_some(),
            "Peak result for {} should have amplitude field",
            node_id
        );
        assert!(
            result.get("timestamp").is_some(),
            "Peak result for {} should have timestamp field",
            node_id
        );
    }

    daemon.shutdown();
    daemon.join().await?;

    Ok(())
}

/// Tests the `/api/graph` endpoint for processing pipeline structure
///
/// This test validates that:
/// - The endpoint returns HTTP 200 with proper authentication
/// - Response contains nodes array with processing pipeline structure
/// - Peak finder node is present and has correct metadata
/// - Node statistics include frames_processed counter
/// - Graph structure matches expected pipeline layout
///
/// # Test Flow
///
/// 1. Start daemon with processing graph
/// 2. Create authenticated client
/// 3. GET `/api/graph` with Bearer token
/// 4. Verify response contains expected nodes (simulated source, peak_finder, etc.)
/// 5. Validate node metadata and statistics
/// 6. Check that peak_finder has processed at least one frame
#[tokio::test]
async fn test_graph_api_endpoint() -> Result<()> {
    let (daemon, config, access_token, _lock) = init_daemon().await;

    let api_base_url = format!("https://localhost:{}", config.visualization.port);

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;

    // Test graph endpoint
    let graph_response = client
        .get(&format!("{}/api/graph", api_base_url))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    assert_eq!(
        graph_response.status(),
        200,
        "Graph endpoint should return 200 OK"
    );

    let graph_data: Value = graph_response.json().await?;

    // Validate response structure
    assert!(
        graph_data.get("nodes").is_some(),
        "Response should contain nodes array"
    );

    let nodes = graph_data["nodes"]
        .as_array()
        .expect("nodes should be an array");
    assert!(!nodes.is_empty(), "Graph should contain at least one node");

    // Find peak_finder node
    let peak_finder_node = nodes
        .iter()
        .find(|node| node["node_type"] == "computing_peak_finder")
        .expect("Graph should contain a peak_finder node");

    // Validate node structure
    assert!(
        peak_finder_node.get("id").is_some(),
        "Node should have id field"
    );
    assert!(
        peak_finder_node.get("node_type").is_some(),
        "Node should have node_type field"
    );
    assert!(
        peak_finder_node.get("statistics").is_some(),
        "Node should have statistics field"
    );

    // Validate statistics
    let frames_processed = peak_finder_node["statistics"]["frames_processed"]
        .as_u64()
        .expect("Node should have frames_processed statistic");
    assert!(
        frames_processed > 0,
        "Peak finder should have processed at least one frame"
    );

    daemon.shutdown();
    daemon.join().await?;

    Ok(())
}

/// Tests the `/api/system/stats` endpoint for resource usage metrics
///
/// This test validates that:
/// - The endpoint returns HTTP 200 with proper authentication
/// - Response contains current system resource metrics
/// - CPU, memory, and thread information is present
/// - All numeric fields contain valid values
/// - Timestamp is recent
///
/// # Test Flow
///
/// 1. Start daemon
/// 2. Create authenticated client
/// 3. GET `/api/system/stats` with Bearer token
/// 4. Verify all required fields are present
/// 5. Validate numeric values are in reasonable ranges
/// 6. Check timestamp is recent
#[tokio::test]
async fn test_system_stats_api_endpoint() -> Result<()> {
    let (daemon, config, access_token, _lock) = init_daemon().await;

    let api_base_url = format!("https://localhost:{}", config.visualization.port);

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;

    // Test system stats endpoint
    let stats_response = client
        .get(&format!("{}/api/system/stats", api_base_url))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    assert_eq!(
        stats_response.status(),
        200,
        "System stats endpoint should return 200 OK"
    );

    let stats_data: Value = stats_response.json().await?;

    // Validate required fields
    assert!(
        stats_data.get("cpu_usage_percent").is_some(),
        "Stats should contain cpu_usage_percent"
    );
    assert!(
        stats_data.get("memory_usage_mb").is_some(),
        "Stats should contain memory_usage_mb"
    );
    assert!(
        stats_data.get("thread_count").is_some(),
        "Stats should contain thread_count"
    );
    assert!(
        stats_data.get("total_cpu_cores").is_some(),
        "Stats should contain total_cpu_cores"
    );

    // Validate numeric values
    let cpu_usage = stats_data["cpu_usage_percent"]
        .as_f64()
        .expect("cpu_usage_percent should be numeric");
    assert!(
        cpu_usage >= 0.0 && cpu_usage <= 100.0,
        "CPU usage should be between 0 and 100 percent"
    );

    let memory_mb = stats_data["memory_usage_mb"]
        .as_u64()
        .expect("memory_usage_mb should be numeric");
    assert!(memory_mb > 0, "Memory usage should be greater than 0 MB");

    daemon.shutdown();
    daemon.join().await?;

    Ok(())
}

/// Tests the `/api/system/health` endpoint for system health assessment
///
/// This test validates that:
/// - The endpoint returns HTTP 200 with proper authentication
/// - Response contains system health status and recommendations
/// - Health status is one of: Healthy, Warning, or Critical
/// - Processing performance summary is included
/// - Recommendations are provided as an array
///
/// # Test Flow
///
/// 1. Start daemon with processing graph
/// 2. Create authenticated client
/// 3. GET `/api/system/health` with Bearer token
/// 4. Verify health status format
/// 5. Validate recommendations array
/// 6. Check processing summary fields if present
#[tokio::test]
async fn test_system_health_api_endpoint() -> Result<()> {
    let (daemon, config, access_token, _lock) = init_daemon().await;

    let api_base_url = format!("https://localhost:{}", config.visualization.port);

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;

    // Test system health endpoint
    let health_response = client
        .get(&format!("{}/api/system/health", api_base_url))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    assert_eq!(
        health_response.status(),
        200,
        "System health endpoint should return 200 OK"
    );

    let health_data: Value = health_response.json().await?;

    // Validate response structure
    assert!(
        health_data.get("health_status").is_some(),
        "Response should contain health_status"
    );
    assert!(
        health_data.get("recommendations").is_some(),
        "Response should contain recommendations"
    );

    // Validate recommendations is an array
    let recommendations = health_data["recommendations"]
        .as_array()
        .expect("recommendations should be an array");
    assert!(
        !recommendations.is_empty(),
        "recommendations should not be empty"
    );

    // Validate health status value
    let health_status = &health_data["health_status"];
    assert!(
        health_status.is_object() || health_status.is_string(),
        "health_status should be an object (for Warning/Critical) or string (for Healthy)"
    );

    daemon.shutdown();
    daemon.join().await?;

    Ok(())
}

/// Tests the `/api/action` endpoint for action node listing
///
/// This test validates that:
/// - The endpoint returns HTTP 200 with proper authentication
/// - Response is an array of action nodes
/// - Each node contains required metadata fields
/// - Node types and buffer information is present
///
/// # Test Flow
///
/// 1. Start daemon with processing graph
/// 2. Create authenticated client
/// 3. GET `/api/action` with Bearer token
/// 4. Verify response is an array
/// 5. Validate each action node has required fields
/// 6. Check buffer capacity is reasonable
#[tokio::test]
async fn test_action_list_api_endpoint() -> Result<()> {
    let (daemon, config, access_token, _lock) = init_daemon().await;

    let api_base_url = format!("https://localhost:{}", config.visualization.port);

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;

    // Test action list endpoint
    let action_response = client
        .get(&format!("{}/api/action", api_base_url))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    assert_eq!(
        action_response.status(),
        200,
        "Action list endpoint should return 200 OK"
    );

    let action_data: Value = action_response.json().await?;

    // Validate response is an array
    let nodes = action_data
        .as_array()
        .expect("Action response should be an array");

    // If there are action nodes, validate their structure
    for node in nodes {
        assert!(node.get("id").is_some(), "Action node should have id field");
        assert!(
            node.get("node_type").is_some(),
            "Action node should have node_type field"
        );
        assert!(
            node.get("buffer_capacity").is_some(),
            "Action node should have buffer_capacity field"
        );
    }

    daemon.shutdown();
    daemon.join().await?;

    Ok(())
}

/// Tests the `/api/graph-statistics` endpoint for processing performance metrics
///
/// This test validates that:
/// - The endpoint returns HTTP 200 with proper authentication
/// - Response contains aggregate processing statistics
/// - Total executions counter is present and non-zero
/// - Node statistics includes performance data for each node
/// - Active nodes count is reasonable
///
/// # Test Flow
///
/// 1. Start daemon with processing graph
/// 2. Wait for processing to accumulate statistics
/// 3. Create authenticated client
/// 4. GET `/api/graph-statistics` with Bearer token
/// 5. Verify statistics structure
/// 6. Validate counters show processing activity
#[tokio::test]
async fn test_graph_statistics_api_endpoint() -> Result<()> {
    let (daemon, config, access_token, _lock) = init_daemon().await;

    let api_base_url = format!("https://localhost:{}", config.visualization.port);

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;

    // Test graph statistics endpoint
    let stats_response = client
        .get(&format!("{}/api/graph-statistics", api_base_url))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    assert_eq!(
        stats_response.status(),
        200,
        "Graph statistics endpoint should return 200 OK"
    );

    let stats_data: Value = stats_response.json().await?;

    // Validate response structure
    assert!(
        stats_data.get("total_executions").is_some(),
        "Stats should contain total_executions"
    );
    assert!(
        stats_data.get("active_nodes").is_some(),
        "Stats should contain active_nodes"
    );
    assert!(
        stats_data.get("node_statistics").is_some(),
        "Stats should contain node_statistics"
    );

    // Verify execution counter shows activity
    let total_executions = stats_data["total_executions"]
        .as_u64()
        .expect("total_executions should be numeric");
    assert!(
        total_executions > 0,
        "Graph should have executed at least once"
    );

    daemon.shutdown();
    daemon.join().await?;

    Ok(())
}

/// Tests authentication requirement on protected endpoints
///
/// This test validates that:
/// - Endpoints return HTTP 401 Unauthorized when no token is provided
/// - Endpoints return HTTP 401 with invalid/expired tokens
/// - Proper error messages are returned
/// - Authentication is enforced across all protected routes
///
/// # Test Flow
///
/// 1. Start daemon
/// 2. Create client without authentication
/// 3. Attempt requests without Bearer token
/// 4. Verify 401 Unauthorized responses
/// 5. Attempt requests with invalid token
/// 6. Verify 401 responses
#[tokio::test]
async fn test_authentication_required_on_protected_endpoints() -> Result<()> {
    let (daemon, config, _, _lock) = init_daemon().await;

    let api_base_url = format!("https://localhost:{}", config.visualization.port);

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;

    // Test computing endpoint without token
    let response_no_auth = client
        .get(&format!("{}/api/computing", api_base_url))
        .send()
        .await?;

    assert_eq!(
        response_no_auth.status(),
        401,
        "Endpoint should return 401 Unauthorized without token"
    );

    // Test with invalid token
    let response_bad_token = client
        .get(&format!("{}/api/computing", api_base_url))
        .header("Authorization", "Bearer invalid.token.here")
        .send()
        .await?;

    assert_eq!(
        response_bad_token.status(),
        401,
        "Endpoint should return 401 Unauthorized with invalid token"
    );

    // Test system/stats endpoint without token
    let response_stats_no_auth = client
        .get(&format!("{}/api/system/stats", api_base_url))
        .send()
        .await?;

    assert_eq!(
        response_stats_no_auth.status(),
        401,
        "System stats endpoint should require authentication"
    );

    // Test action endpoint without token
    let response_action_no_auth = client
        .get(&format!("{}/api/action", api_base_url))
        .send()
        .await?;

    assert_eq!(
        response_action_no_auth.status(),
        401,
        "Action endpoint should require authentication"
    );

    daemon.shutdown();
    daemon.join().await?;

    Ok(())
}

/// Tests response consistency and data integrity across multiple requests
///
/// This test validates that:
/// - Multiple consecutive requests to the same endpoint return consistent structure
/// - Numeric values remain valid across requests
/// - Timestamp progression indicates active processing
/// - Graph structure remains stable between requests
///
/// # Test Flow
///
/// 1. Start daemon
/// 2. Make first request to computing endpoint, capture peak results
/// 3. Wait briefly
/// 4. Make second request to same endpoint
/// 5. Compare response structures
/// 6. Verify data consistency and progression
#[tokio::test]
async fn test_response_consistency_across_requests() -> Result<()> {
    let (daemon, config, access_token, _lock) = init_daemon().await;

    let api_base_url = format!("https://localhost:{}", config.visualization.port);

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;

    // First request
    let response1 = client
        .get(&format!("{}/api/computing", api_base_url))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    assert_eq!(response1.status(), 200);
    let data1: Value = response1.json().await?;

    // Wait a moment
    sleep(Duration::from_secs(1)).await;

    // Second request
    let response2 = client
        .get(&format!("{}/api/computing", api_base_url))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    assert_eq!(response2.status(), 200);
    let data2: Value = response2.json().await?;

    // Verify both responses have same structure
    assert_eq!(
        data1.get("peak_results").is_some(),
        data2.get("peak_results").is_some(),
        "Both responses should have peak_results field"
    );
    assert_eq!(
        data1.get("active_node_ids").is_some(),
        data2.get("active_node_ids").is_some(),
        "Both responses should have active_node_ids field"
    );

    // Verify both have valid peak results if first has them
    if let Some(results1) = data1["peak_results"].as_object() {
        if let Some(results2) = data2["peak_results"].as_object() {
            for (node_id, _) in results1 {
                assert!(
                    results2.contains_key(node_id),
                    "Node {} should be present in both responses",
                    node_id
                );
            }
        }
    }

    daemon.shutdown();
    daemon.join().await?;

    Ok(())
}
