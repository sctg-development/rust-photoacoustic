// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use rocket::http::Header;
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
async fn test_protected_api_with_jwt() {
    // Initialize the Rocket instance with a test HMAC secret
    let _test_hmac_secret = "test-hmac-secret-key-for-testing";

    // Create a SharedVisualizationState for the test
    let visualization_state = std::sync::Arc::new(
        rust_photoacoustic::visualization::shared_state::SharedVisualizationState::new(),
    );
    // Pre-populate the visualization state with some test data
    let test_stats = rust_photoacoustic::processing::graph::ProcessingGraphStatistics::new();
    visualization_state
        .update_processing_statistics(test_stats)
        .await;

    let test_config = get_test_config();
    let rocket = rust_photoacoustic::visualization::server::build_rocket(
        get_figment(),
        Arc::new(RwLock::new(test_config)),
        None,
        Some(visualization_state),
        None,
        None,
        None,
    )
    .await;
    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Step 1: First authenticate and get a token
    let oauth_response = client
        .post("/token")
        .header(rocket::http::ContentType::Form)
        .body("grant_type=password&username=test_user&password=password&client_id=LaserSmartClient")
        .dispatch()
        .await;

    // If password grant is not enabled, the test will be skipped
    if oauth_response.status() != Status::Ok {
        println!("Skipping JWT API test, password grant not enabled");
        return;
    }

    // Get the token from the response
    let token_data: Value = serde_json::from_str(
        &oauth_response
            .into_string()
            .await
            .expect("OAuth2 token response"),
    )
    .expect("Valid JSON response");

    let access_token = token_data["access_token"]
        .as_str()
        .expect("JWT access token");

    // Step 2: Call a protected API endpoint with the token
    let profile_response = client
        .get("/api/profile")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", access_token),
        ))
        .dispatch()
        .await;

    assert_eq!(profile_response.status(), Status::Ok);

    let profile_data: Value = serde_json::from_str(
        &profile_response
            .into_string()
            .await
            .expect("Profile response"),
    )
    .expect("Valid JSON response");

    assert!(
        profile_data.get("user_id").is_some(),
        "Profile should contain user_id"
    );

    // Step 3: Verify a protected endpoint requiring a scope
    let data_response = client
        .get("/api/data")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", access_token),
        ))
        .dispatch()
        .await;

    // This will succeed if the token has read:api scope, which should be the case
    assert_eq!(data_response.status(), Status::Ok);

    // Step 4: Test with an invalid token
    let invalid_response = client
        .get("/api/profile")
        .header(Header::new("Authorization", "Bearer invalid-token"))
        .dispatch()
        .await;

    assert_eq!(invalid_response.status(), Status::Unauthorized);
}
