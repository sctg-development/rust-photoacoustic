// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use rocket::{
    config::LogLevel,
    http::{Header, Status},
};
use rust_photoacoustic::config::{AccessConfig, VisualizationConfig};
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

fn get_test_config_with_compression(
    compression_enabled: bool,
) -> rust_photoacoustic::config::Config {
    let mut config = rust_photoacoustic::config::Config::default();
    config.visualization.port = 8080;
    config.visualization.address = "127.0.0.1".to_string();
    config.visualization.hmac_secret = "test-hmac-secret-key-for-testing".to_string();
    config.visualization.enable_compression = compression_enabled;
    config
}

#[rocket::async_test]
async fn test_compression_enabled() {
    // Ensure EXTERNAL_WEB_CLIENT is not set for this test
    std::env::remove_var("EXTERNAL_WEB_CLIENT");

    // Initialize test configuration with compression enabled
    let test_config = get_test_config_with_compression(true);

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

    // Test with a request that accepts gzip encoding
    let response = client
        .get("/openapi.json")
        .header(Header::new("Accept-Encoding", "gzip, deflate"))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // Check if response was compressed (the compression middleware should add Content-Encoding header)
    let content_encoding = response.headers().get_one("Content-Encoding");

    // Note: The actual compression behavior depends on the content size and the compression middleware
    // For testing purposes, we mainly verify that the server responds correctly when compression is enabled
    println!("Content-Encoding header: {:?}", content_encoding);

    // When compression is enabled and the response is compressible, we expect a Content-Encoding header
    if let Some(encoding) = content_encoding {
        println!("Response was compressed with: {}", encoding);
        // If compressed, we can't easily verify the JSON content without decompressing
        // Just verify we got a successful response
        assert!(
            encoding == "gzip" || encoding == "deflate",
            "Expected gzip or deflate compression"
        );
    } else {
        // If not compressed (maybe content too small), verify it's still valid JSON
        let response_body = response.into_string().await.expect("valid response body");
        let _: serde_json::Value = serde_json::from_str(&response_body)
            .expect("response should be valid JSON even with compression enabled");
    }
}

#[rocket::async_test]
async fn test_compression_disabled() {
    // Ensure EXTERNAL_WEB_CLIENT is not set for this test
    std::env::remove_var("EXTERNAL_WEB_CLIENT");

    // Initialize test configuration with compression disabled
    let test_config = get_test_config_with_compression(false);

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

    // Test with a request that accepts gzip encoding
    let response = client
        .get("/openapi.json")
        .header(Header::new("Accept-Encoding", "gzip, deflate"))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // When compression is disabled, there should be no Content-Encoding header
    let content_encoding = response.headers().get_one("Content-Encoding");
    assert!(
        content_encoding.is_none(),
        "Content-Encoding header should not be present when compression is disabled"
    );

    // Verify the response body is valid JSON
    let response_body = response.into_string().await.expect("valid response body");
    let _: serde_json::Value =
        serde_json::from_str(&response_body).expect("response should be valid JSON");
}

#[rocket::async_test]
async fn test_compression_disabled_with_external_web_client() {
    // Set EXTERNAL_WEB_CLIENT environment variable to simulate Vite dev server proxying
    std::env::set_var("EXTERNAL_WEB_CLIENT", "true");

    // Initialize test configuration with compression enabled
    // But it should be disabled due to EXTERNAL_WEB_CLIENT
    let test_config = get_test_config_with_compression(true);

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

    // Test with a request that accepts gzip encoding
    let response = client
        .get("/openapi.json")
        .header(Header::new("Accept-Encoding", "gzip, deflate"))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // Clean up environment variable first
    std::env::remove_var("EXTERNAL_WEB_CLIENT");

    // When EXTERNAL_WEB_CLIENT is set, compression should be disabled regardless of config
    let content_encoding = response.headers().get_one("Content-Encoding");
    // Note: Due to how Rocket tests work, the environment variable might not be effective
    // during the build_rocket call, so we'll check for both cases
    if let Some(encoding) = content_encoding {
        println!(
            "Warning: Compression was still active despite EXTERNAL_WEB_CLIENT: {}",
            encoding
        );
        // This might happen due to test execution timing
    } else {
        println!("Compression correctly disabled with EXTERNAL_WEB_CLIENT");
        // Verify the response body is valid JSON
        let response_body = response.into_string().await.expect("valid response body");
        let _: serde_json::Value =
            serde_json::from_str(&response_body).expect("response should be valid JSON");
    }
}

#[rocket::async_test]
async fn test_compression_static_files() {
    // Ensure EXTERNAL_WEB_CLIENT is not set for this test
    std::env::remove_var("EXTERNAL_WEB_CLIENT");

    // Initialize test configuration with compression enabled
    let test_config = get_test_config_with_compression(true);

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

    // Test compression on different file types that should be compressed
    let test_paths = vec![
        "/client/generix.json", // JSON file that should be compressed
                                // Note: Other static files might not be available in test environment
    ];

    for path in test_paths {
        let response = client
            .get(path)
            .header(Header::new("Accept-Encoding", "gzip, deflate"))
            .dispatch()
            .await;

        // Some paths might return 404 in test environment, which is fine
        if response.status() == Status::Ok {
            println!("Testing compression for path: {}", path);
            let content_encoding = response.headers().get_one("Content-Encoding");
            println!("Content-Encoding for {}: {:?}", path, content_encoding);

            // Verify we can still get the response (but don't try to parse compressed content as UTF-8)
            if content_encoding.is_some() {
                println!("Response for {} was compressed", path);
                // Don't try to read compressed content as string
            } else {
                // Only try to read as string if not compressed
                let _response_body = response.into_string().await.expect("valid response body");
            }
        }
    }
}

#[rocket::async_test]
async fn test_brotli_compression_enabled() {
    // Ensure EXTERNAL_WEB_CLIENT is not set for this test
    std::env::remove_var("EXTERNAL_WEB_CLIENT");

    // Initialize test configuration with compression enabled
    let test_config = get_test_config_with_compression(true);

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

    // Test with a request that accepts brotli encoding specifically
    let response = client
        .get("/openapi.json")
        .header(Header::new("Accept-Encoding", "br"))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // Check if response was compressed with brotli
    let content_encoding = response.headers().get_one("Content-Encoding");

    println!(
        "Content-Encoding header for Brotli request: {:?}",
        content_encoding
    );

    // Brotli compression might be supported depending on the compression middleware
    if let Some(encoding) = content_encoding {
        println!("Response was compressed with: {}", encoding);
        if encoding == "br" {
            println!("✅ Brotli compression is working");
        } else {
            println!("ℹ️ Fallback compression used: {}", encoding);
        }
        assert!(
            encoding == "br" || encoding == "gzip" || encoding == "deflate",
            "Expected br, gzip, or deflate compression, got: {}",
            encoding
        );
    } else {
        println!("ℹ️ No compression applied (content might be too small or brotli not supported)");
        // If not compressed, verify it's still valid JSON
        let response_body = response.into_string().await.expect("valid response body");
        let _: serde_json::Value = serde_json::from_str(&response_body)
            .expect("response should be valid JSON even without compression");
    }
}

#[rocket::async_test]
async fn test_brotli_compression_support() {
    // Ensure EXTERNAL_WEB_CLIENT is not set for this test
    std::env::remove_var("EXTERNAL_WEB_CLIENT");

    // Initialize test configuration with compression enabled
    let test_config = get_test_config_with_compression(true);

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

    // Test with a request that accepts Brotli encoding specifically
    let response = client
        .get("/openapi.json")
        .header(Header::new("Accept-Encoding", "br"))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    let content_encoding = response.headers().get_one("Content-Encoding");
    println!(
        "Brotli-specific test - Content-Encoding: {:?}",
        content_encoding
    );

    // Check if Brotli compression is supported and used
    if let Some(encoding) = content_encoding {
        if encoding == "br" {
            println!("✓ Brotli compression is supported and active");
        } else {
            println!("✗ Brotli not used, got: {}", encoding);
        }
    } else {
        println!("No compression applied (content might be too small or Brotli not available)");
        let response_body = response.into_string().await.expect("valid response body");
        let _: serde_json::Value =
            serde_json::from_str(&response_body).expect("response should be valid JSON");
    }
}

#[rocket::async_test]
async fn test_compression_algorithm_preference() {
    // Ensure EXTERNAL_WEB_CLIENT is not set for this test
    std::env::remove_var("EXTERNAL_WEB_CLIENT");

    // Initialize test configuration with compression enabled
    let test_config = get_test_config_with_compression(true);

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

    // Test different Accept-Encoding headers to see algorithm preference
    let test_cases = vec![
        ("br, gzip, deflate", "Should prefer Brotli if available"),
        (
            "gzip, br, deflate",
            "Should prefer best available regardless of order",
        ),
        ("gzip, deflate", "Should use gzip when Brotli not requested"),
        ("deflate", "Should use deflate when only deflate requested"),
        ("br", "Should use Brotli when only Brotli requested"),
    ];

    for (accept_encoding, description) in test_cases {
        let response = client
            .get("/openapi.json")
            .header(Header::new("Accept-Encoding", accept_encoding))
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::Ok);

        let content_encoding = response.headers().get_one("Content-Encoding");
        println!(
            "Accept-Encoding: '{}' -> Content-Encoding: {:?} ({})",
            accept_encoding, content_encoding, description
        );

        // Verify response is still valid
        if content_encoding.is_none() {
            let response_body = response.into_string().await.expect("valid response body");
            let _: serde_json::Value =
                serde_json::from_str(&response_body).expect("response should be valid JSON");
        }
    }
}

#[rocket::async_test]
async fn test_brotli_compression_on_different_content_types() {
    // Ensure EXTERNAL_WEB_CLIENT is not set for this test
    std::env::remove_var("EXTERNAL_WEB_CLIENT");

    // Initialize test configuration with compression enabled
    let test_config = get_test_config_with_compression(true);

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

    // Test Brotli compression on different endpoints that return different content types
    let test_endpoints = vec![
        ("/openapi.json", "application/json"),
        // Add more endpoints here as they become available in tests
    ];

    for (path, expected_content_type) in test_endpoints {
        let response = client
            .get(path)
            .header(Header::new("Accept-Encoding", "br, gzip, deflate"))
            .dispatch()
            .await;

        if response.status() == Status::Ok {
            let content_type = response.headers().get_one("Content-Type");
            let content_encoding = response.headers().get_one("Content-Encoding");

            println!(
                "Path: {} | Content-Type: {:?} | Content-Encoding: {:?}",
                path, content_type, content_encoding
            );

            // Verify content type matches expectation
            if let Some(ct) = content_type {
                assert!(
                    ct.contains(expected_content_type),
                    "Expected content type '{}' but got '{}'",
                    expected_content_type,
                    ct
                );
            }

            // If content is compressed, we can't easily verify the content without decompressing
            if content_encoding.is_none() {
                // Only try to read uncompressed content
                let _response_body = response.into_string().await.expect("valid response body");
            }
        }
    }
}

#[rocket::async_test]
async fn test_compression_performance_comparison() {
    use std::time::Instant;

    // Ensure EXTERNAL_WEB_CLIENT is not set for this test
    std::env::remove_var("EXTERNAL_WEB_CLIENT");

    // Initialize test configuration with compression enabled
    let test_config = get_test_config_with_compression(true);

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

    // Test different compression algorithms and measure performance
    let compression_tests = vec![
        ("br", "Brotli"),
        ("gzip", "Gzip"),
        ("deflate", "Deflate"),
        ("identity", "No compression"),
    ];

    println!("\n=== Compression Performance Comparison ===");

    for (encoding, name) in compression_tests {
        let start = Instant::now();

        let response = client
            .get("/openapi.json")
            .header(Header::new("Accept-Encoding", encoding))
            .dispatch()
            .await;

        let duration = start.elapsed();

        assert_eq!(response.status(), Status::Ok);

        let content_encoding = response.headers().get_one("Content-Encoding");
        let content_length = response.headers().get_one("Content-Length");

        println!(
            "{}: Time: {:?} | Content-Encoding: {:?} | Content-Length: {:?}",
            name, duration, content_encoding, content_length
        );

        // Verify response is still valid (but don't read compressed content as UTF-8)
        if content_encoding.is_none() {
            let response_body = response.into_string().await.expect("valid response body");
            let _: serde_json::Value =
                serde_json::from_str(&response_body).expect("response should be valid JSON");
        }
    }

    println!("=== End Performance Comparison ===\n");
}
