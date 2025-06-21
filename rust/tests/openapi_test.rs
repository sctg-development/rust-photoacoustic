// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use rocket::{config::LogLevel, http::Status};
use rust_photoacoustic::config::{AccessConfig, VisualizationConfig};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

fn get_figment() -> rocket::figment::Figment {
    rocket::Config::figment()
        .merge(("port", 8080))
        .merge(("address", "127.0.0.1"))
        .merge(("log_level", LogLevel::Debug))
        .merge((
            "hmac_secret",
            "test-hmac-secret-key-for-testing".to_string(),
        ))
        .merge(("access_config", AccessConfig::default()))
        .merge(("visualization_config", VisualizationConfig::default()))
}

fn get_test_config() -> rust_photoacoustic::config::Config {
    let mut config = rust_photoacoustic::config::Config::default();
    config.visualization.port = 8080;
    config.visualization.address = "127.0.0.1".to_string();
    config.visualization.hmac_secret = "test-hmac-secret-key-for-testing".to_string();
    config
}

#[rocket::async_test]
async fn test_openapi_endpoint() {
    // Initialize test configuration
    let test_config = get_test_config();

    // Create a SharedVisualizationState for the test
    let visualization_state = std::sync::Arc::new(
        rust_photoacoustic::visualization::shared_state::SharedVisualizationState::new(),
    );

    let rocket = rust_photoacoustic::visualization::server::build_rocket(
        get_figment(),
        Arc::new(RwLock::new(test_config)),
        None,                      // No audio stream
        Some(visualization_state), // visualization state
        None,                      // No streaming registry
        None,                      // No thermal state
    )
    .await;

    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Test the /openapi.json endpoint - should be publicly accessible (no JWT required)
    let response = client.get("/openapi.json").dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    // Get the response body and parse as JSON
    let openapi_json = response.into_string().await.expect("valid response body");
    let openapi_value: Value =
        serde_json::from_str(&openapi_json).expect("response should be valid JSON");

    // Verify it's a valid OpenAPI spec with required fields
    assert!(
        openapi_value.get("openapi").is_some(),
        "OpenAPI spec should have 'openapi' field"
    );
    assert!(
        openapi_value.get("info").is_some(),
        "OpenAPI spec should have 'info' field"
    );
    assert!(
        openapi_value.get("paths").is_some(),
        "OpenAPI spec should have 'paths' field"
    );

    // Check that paths is not empty (indicating successful merging)
    let paths = openapi_value
        .get("paths")
        .expect("paths field should exist");
    assert!(paths.as_object().is_some(), "paths should be an object");

    let paths_obj = paths.as_object().unwrap();
    assert!(
        !paths_obj.is_empty(),
        "paths should not be empty - indicates OpenAPI specs were merged"
    );

    println!("OpenAPI spec has {} paths", paths_obj.len());

    // Log some of the paths for debugging
    for (path, _) in paths_obj.iter().take(5) {
        println!("Found path: {}", path);
    }
}
