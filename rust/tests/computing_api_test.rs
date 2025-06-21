// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header as JwtHeader};
use rocket::http::Header;
use rocket::{config::LogLevel, http::Status};
use rust_photoacoustic::config::{AccessConfig, VisualizationConfig};
use rust_photoacoustic::processing::computing_nodes::{ComputingSharedData, SharedComputingState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

// Import from utility module
use rust_photoacoustic::utility::jwt_token::{
    ConfigLoader, JwtAlgorithm, TokenCreationParams, TokenCreator,
};

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

/// Create a JWT token for testing purposes
fn create_test_jwt_token(
    config: &rust_photoacoustic::config::Config,
) -> Result<String, Box<dyn std::error::Error>> {
    // Simulate `create_token --user admin --client LaserSmartClient --algorithm RS256 --duration 60 --quiet`
    let config_loader = ConfigLoader::from_config(config)?;
    let token_creator = TokenCreator::new(&config_loader)?;

    let params = TokenCreationParams {
        user_id: "admin".to_string(),
        client_id: "LaserSmartClient".to_string(),
        algorithm: JwtAlgorithm::RS256,
        duration_seconds: 60,
    };

    let result = token_creator.create_token(&params)?;
    Ok(result.token)
}

#[rocket::async_test]
async fn test_computing_api_with_jwt() {
    // Create shared computing state with test data
    let mut computing_data = ComputingSharedData::default();
    computing_data.peak_frequency = Some(1234.5);
    computing_data.peak_amplitude = Some(0.8);
    computing_data.concentration_ppm = Some(456.7);
    computing_data.polynomial_coefficients = [1.0, 2.0, 3.0, 4.0, 5.0];

    let computing_state: SharedComputingState = Arc::new(RwLock::new(computing_data));

    // Create a SharedVisualizationState for the test
    let visualization_state = std::sync::Arc::new(
        rust_photoacoustic::visualization::shared_state::SharedVisualizationState::new(),
    );

    let test_config = get_test_config();

    // Create a JWT token directly for the admin user
    let access_token =
        create_test_jwt_token(&test_config).expect("Failed to create test JWT token");

    let rocket = rust_photoacoustic::visualization::server::build_rocket(
        get_figment(),
        Arc::new(RwLock::new(test_config)),
        None,
        Some(visualization_state),
        None,
        None,
        Some(computing_state),
    )
    .await;
    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Call the computing API endpoint with the token
    let computing_response = client
        .get("/api/computing")
        .header(Header::new(
            "Authorization",
            format!("Bearer {}", access_token),
        ))
        .dispatch()
        .await;

    assert_eq!(computing_response.status(), Status::Ok);

    let computing_data: Value = serde_json::from_str(
        &computing_response
            .into_string()
            .await
            .expect("Computing response"),
    )
    .expect("Valid JSON response");

    // Verify that the returned data matches what we set in the shared state
    assert_eq!(computing_data["peak_frequency"], 1234.5);
    assert_eq!(computing_data["peak_amplitude"], 0.8);
    assert_eq!(computing_data["concentration_ppm"], 456.7);

    // For the polynomial coefficients array, we need to compare as a JSON array
    let expected_coefficients = serde_json::json!([1.0, 2.0, 3.0, 4.0, 5.0]);
    assert_eq!(
        computing_data["polynomial_coefficients"],
        expected_coefficients
    );

    // Test with an invalid token (should be unauthorized)
    let invalid_response = client
        .get("/api/computing")
        .header(Header::new("Authorization", "Bearer invalid-token"))
        .dispatch()
        .await;

    assert_eq!(invalid_response.status(), Status::Unauthorized);
}
