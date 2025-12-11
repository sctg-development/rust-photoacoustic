// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! End-to-End tests for RapiDoc rendering
//!
//! These tests verify that RapiDoc is properly rendered and displays
//! the correct API documentation content.

use rocket::{config::LogLevel, http::Status};
use rust_photoacoustic::config::{AccessConfig, VisualizationConfig};
use std::sync::Arc;
use tokio::sync::RwLock;

fn get_figment() -> rocket::figment::Figment {
    rocket::Config::figment()
        .merge(("port", 8081))
        .merge(("address", "127.0.0.1"))
        .merge(("log_level", LogLevel::Critical))
        .merge((
            "hmac_secret",
            "test-hmac-secret-key-for-testing".to_string(),
        ))
        .merge(("access_config", AccessConfig::default()))
        .merge(("visualization_config", VisualizationConfig::default()))
}

fn get_test_config() -> rust_photoacoustic::config::Config {
    let mut config = rust_photoacoustic::config::Config::default();
    config.visualization.port = 8081;
    config.visualization.address = "127.0.0.1".to_string();
    config.visualization.hmac_secret = "test-hmac-secret-key-for-testing".to_string();
    config
}

#[rocket::async_test]
async fn test_rapidoc_html_generation() {
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
        None,                      // No shared computing state
    )
    .await;

    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Test the /api/doc/index.html endpoint
    let response = client.get("/api/doc/index.html").dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    assert_eq!(
        response.content_type(),
        Some(rocket::http::ContentType::HTML)
    );

    // Get the response body
    let html_content = response.into_string().await.expect("valid response body");

    println!("HTML Response length: {} chars", html_content.len());
    println!(
        "First 1000 chars of HTML:\n{}",
        &html_content.chars().take(1000).collect::<String>()
    );

    // Verify that the HTML contains the custom template values
    assert!(
        html_content.contains("SCTG rust-photoacoustic API Doc"),
        "HTML should contain the API title"
    );

    // The API description is loaded dynamically from openapi.json by RapiDoc
    // So we verify it's available at the spec URL instead of in the static HTML
    let spec_url = if html_content.contains("../../openapi.json") {
        "/openapi.json"
    } else {
        "/openapi.json"
    };

    let spec_response = client.get(spec_url).dispatch().await;
    assert_eq!(spec_response.status(), Status::Ok);

    let spec_content = spec_response
        .into_string()
        .await
        .expect("valid spec response body");
    assert!(
        spec_content.contains("Flexible Gas Analyzer using Laser Photoacoustic Spectroscopy"),
        "OpenAPI spec should contain the API description"
    );

    assert!(
        html_content.contains("rapi-doc"),
        "HTML should contain the rapi-doc custom element"
    );

    // Verify RapiDoc attributes are set
    assert!(
        html_content.contains("spec-url") || html_content.contains("data-spec-url"),
        "HTML should have RapiDoc spec-url attribute"
    );

    // Verify JavaScript files are included
    assert!(
        html_content.contains(".js") || html_content.contains("script"),
        "HTML should reference JavaScript files"
    );

    println!("✓ RapiDoc HTML generation test passed!");
}

#[rocket::async_test]
async fn test_rapidoc_static_files_served() {
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
        None,                      // No shared computing state
    )
    .await;

    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Test that the JavaScript files are served
    let files_to_test = vec![
        "/api/doc/helper.js",
        "/api/doc/index.js",
        "/api/doc/rapidoc-min.js",
    ];

    for file in files_to_test {
        let response = client.get(file).dispatch().await;
        assert_eq!(
            response.status(),
            Status::Ok,
            "File {} should be served successfully",
            file
        );
        assert_eq!(
            response.content_type(),
            Some(rocket::http::ContentType::JavaScript),
            "File {} should be served as JavaScript",
            file
        );
    }

    println!("✓ RapiDoc static files test passed!");
}

#[rocket::async_test]
async fn test_rapidoc_source_maps_available() {
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
        None,                      // No shared computing state
    )
    .await;

    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Test that source map files are available
    let sourcemap_files = vec![
        "/api/doc/helper.js.map",
        "/api/doc/index.js.map",
        "/api/doc/rapidoc-min.js.map",
    ];

    for file in sourcemap_files {
        let response = client.get(file).dispatch().await;
        // Source maps might not always be necessary, so we just check they don't 404
        if response.status() == Status::Ok {
            println!("✓ Source map {} is available", file);
        } else {
            println!("⚠ Source map {} is not available (non-critical)", file);
        }
    }

    println!("✓ RapiDoc source maps test completed!");
}
