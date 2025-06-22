// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Real-world integration test for peak detection endpoints
//!
//! This test creates a full server instance using the example configuration
//! which includes a real processing graph with peak_finder and simulated source.
//! It tests the actual data flow from simulation → processing → shared state → API endpoints.

use anyhow::Result;
use rand::rand_core::le;
use rust_photoacoustic::{
    config::Config,
    daemon::launch_daemon::Daemon,
    utility::jwt_token::{ConfigLoader, JwtAlgorithm, TokenCreationParams, TokenCreator},
};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{sync::RwLock, time::sleep};

/// Integration test that starts a real server with example configuration
/// and tests the peak detection endpoints with live data processing
#[tokio::test]
async fn test_real_world_peak_detection_endpoints() -> Result<()> {
    // Initialize logging for debugging
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    // Load the example configuration which contains a working peak_finder setup
    let config_path = PathBuf::from("config.example.yaml");
    let config = Config::from_file(&config_path)?;

    println!("Loaded configuration from: {:?}", config_path);
    println!("Processing enabled: {}", config.processing.enabled);
    println!(
        "Simulated source type: {:?}",
        config
            .photoacoustic
            .simulated_source
            .as_ref()
            .map(|s| &s.source_type)
    );

    // Create shared configuration for the daemon
    let config_arc = Arc::new(RwLock::new(config.clone()));

    // Create and launch the daemon (this starts all services including processing)
    let mut daemon = Daemon::new();

    println!("Starting daemon with real processing graph...");
    daemon.launch(config_arc.clone()).await?;

    // Wait for the system to initialize and start processing some frames
    // The processing graph needs time to start and accumulate enough data for peak detection
    println!("Waiting for processing system to initialize and generate data...");
    sleep(Duration::from_secs(5)).await;

    // Create JWT token for API authentication using the administrator user from config
    let access_token = create_admin_jwt_token(&config)?;
    println!("Created JWT token for administrator");

    // Test the API endpoints with the real running server
    // Use HTTPS since the example config has TLS certificates configured
    // The server binds to ::0 (all interfaces) but we connect to localhost
    let api_base_url = format!("https://localhost:{}", config.visualization.port);

    println!("Testing API endpoints at: {}", api_base_url);

    // Create HTTP client for API calls with TLS certificate acceptance
    // Since we're using self-signed certificates, we need to accept invalid certificates
    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true) // Accept self-signed certificates
        .danger_accept_invalid_hostnames(true) // Accept hostname mismatches
        .build()?;

    // Test 1: Get processing graph structure
    println!("\n=== Testing /api/graph endpoint ===");
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
    println!("Graph response received, checking structure...");

    // Verify the graph contains the expected peak_finder node
    let nodes = graph_data["nodes"]
        .as_array()
        .expect("Graph should contain nodes array");

    let peak_finder_node = nodes
        .iter()
        .find(|node| node["node_type"] == "computing_peak_finder")
        .expect("Graph should contain a peak_finder node");

    println!("✓ Found peak_finder node: {}", peak_finder_node["id"]);

    // Check that the node has processed at least one frame
    let frames_processed = peak_finder_node["statistics"]["frames_processed"]
        .as_u64()
        .expect("Peak finder should have statistics");

    println!("Peak finder frames processed: {}", frames_processed);
    assert!(
        frames_processed > 0,
        "Peak finder should have processed at least one frame"
    );

    // Test 2: Get computing state (peak detection results)
    println!("\n=== Testing /api/computing endpoint ===");
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
    println!(
        "Computing response: {}",
        serde_json::to_string_pretty(&computing_data)?
    );

    // Verify the structure exists (legacy fields for compatibility)
    assert!(
        computing_data.get("peak_frequency").is_some(),
        "Should have peak_frequency field"
    );
    assert!(
        computing_data.get("peak_amplitude").is_some(),
        "Should have peak_amplitude field"
    );
    assert!(
        computing_data.get("concentration_ppm").is_some(),
        "Should have concentration_ppm field"
    );

    // Verify new structure exists
    assert!(
        computing_data.get("active_node_ids").is_some(),
        "Should have active_node_ids field"
    );
    assert!(
        computing_data.get("peak_results").is_some(),
        "Should have peak_results field"
    );

    // Get active node IDs
    let active_node_ids = computing_data["active_node_ids"]
        .as_array()
        .expect("active_node_ids should be an array");

    println!("Active node IDs: {:?}", active_node_ids);

    // Check if we have peak_detector in active nodes
    let peak_detector_id = active_node_ids
        .iter()
        .find(|id| id.as_str() == Some("peak_detector"))
        .expect("Should have peak_detector in active_node_ids");

    println!("✓ Found active peak detector: {}", peak_detector_id);

    // Get peak results for the active peak detector
    let peak_results = computing_data["peak_results"]
        .as_object()
        .expect("peak_results should be an object");

    let peak_detector_result = peak_results
        .get("peak_detector")
        .expect("Should have peak_detector in peak_results");

    println!(
        "Peak detector result: {}",
        serde_json::to_string_pretty(peak_detector_result)?
    );

    // Extract data from the peak detector result
    let peak_frequency = peak_detector_result["frequency"].as_f64();
    let peak_amplitude = peak_detector_result["amplitude"].as_f64();
    let peak_concentration = peak_detector_result["concentration_ppm"].as_f64();

    println!("Peak frequency from peak_detector: {:?}", peak_frequency);
    println!("Peak amplitude from peak_detector: {:?}", peak_amplitude);
    println!(
        "Peak concentration from peak_detector: {:?} ppm",
        peak_concentration
    );
    assert!(
        peak_frequency.is_some(),
        "Peak frequency should not be null"
    );
    // peak frequency should be between 1900 and 2200 Hz
    assert!(
        peak_frequency.unwrap() >= 1900.0 && peak_frequency.unwrap() <= 2200.0,
        "Peak frequency should be in the range [1900, 2200] Hz"
    );
    assert!(
        peak_amplitude.is_some(),
        "Peak amplitude should not be null"
    );
    // peak amplitude should be 45 and 55
    assert!(
        peak_amplitude.unwrap() >= 45.0 && peak_amplitude.unwrap() <= 55.0,
        "Peak amplitude should be in the range [45, 55]"
    );
    // peak concentration should be between 0 and 100 ppm
    assert!(
        peak_concentration.is_some(),
        "Peak concentration should not be null"
    );
    assert!(
        peak_concentration.unwrap() >= 0.0 && peak_concentration.unwrap() <= 100.0,
        "Peak concentration should be in the range [0, 100] ppm"
    );

    // Check concentration calculation result
    let concentration_ppm = computing_data["concentration_ppm"].as_f64();
    println!("Calculated concentration: {:?} ppm", concentration_ppm);

    // Check polynomial coefficients
    let polynomial_coefficients = computing_data["polynomial_coefficients"]
        .as_array()
        .expect("Should have polynomial_coefficients array");
    println!("Polynomial coefficients: {:?}", polynomial_coefficients);

    // The main test: verify we get real data, not null values
    if peak_frequency.is_some() && peak_amplitude.is_some() {
        println!("✓ SUCCESS: Peak detection is working - got real values!");
        println!("  Peak frequency: {} Hz", peak_frequency.unwrap());
        println!("  Peak amplitude: {}", peak_amplitude.unwrap());

        if concentration_ppm.is_some() {
            println!(
                "  Calculated concentration: {} ppm",
                concentration_ppm.unwrap()
            );
            println!("✓ SUCCESS: Concentration calculation is also working!");
        } else {
            println!("⚠ WARNING: Concentration calculation returning null");
        }
    } else {
        println!("⚠ WARNING: Peak detection returning null values");
        println!(
            "  This indicates the SharedComputingState is not being updated by the peak_finder"
        );

        // Let's wait a bit more and try again - maybe the system needs more time
        println!("Waiting additional time for peak detection to stabilize...");
        sleep(Duration::from_secs(3)).await;

        let retry_response = client
            .get(&format!("{}/api/computing", api_base_url))
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        let retry_data: Value = retry_response.json().await?;
        println!(
            "Retry computing response: {}",
            serde_json::to_string_pretty(&retry_data)?
        );

        let retry_peak_results = retry_data["peak_results"]
            .as_object()
            .expect("peak_results should be an object");

        let retry_peak_detector_result = retry_peak_results
            .get("peak_detector")
            .expect("Should have peak_detector in peak_results");

        let retry_peak_frequency = retry_peak_detector_result["frequency"].as_f64();
        let retry_peak_amplitude = retry_peak_detector_result["amplitude"].as_f64();

        if retry_peak_frequency.is_some() && retry_peak_amplitude.is_some() {
            println!("✓ SUCCESS on retry: Peak detection working!");
        } else {
            // This is the bug we're trying to debug
            println!("❌ CONFIRMED BUG: Peak detection not updating SharedComputingState");
        }
    }

    // Test 3: Get processing graph statistics
    println!("\n=== Testing /api/graph-statistics endpoint ===");
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
    println!("Graph statistics summary:");
    println!("  Total executions: {}", stats_data["total_executions"]);
    println!("  Active nodes: {}", stats_data["active_nodes"]);

    // Verify the peak_finder appears in node statistics
    let node_stats = stats_data["node_statistics"]
        .as_object()
        .expect("Should have node_statistics");

    let peak_finder_stats = node_stats
        .values()
        .find(|stat| stat["node_type"] == "computing_peak_finder")
        .expect("Should have peak_finder in statistics");

    println!(
        "  Peak finder processed {} frames",
        peak_finder_stats["frames_processed"]
    );

    // Clean shutdown
    println!("\n=== Shutting down daemon ===");
    daemon.shutdown();
    daemon.join().await?;

    println!("✓ Test completed successfully");

    Ok(())
}

/// Create a JWT token for the administrator user from the example configuration
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
