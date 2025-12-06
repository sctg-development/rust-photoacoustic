// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use anyhow::Result;
use rocket::figment::Figment;
use rocket::{http::Status, local::asynchronous::Client};
use rust_photoacoustic::{
    config::{AccessConfig, Config},
    visualization::server::build_rocket,
};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

// Validate the openapi.json endpoint is available at the root path `/openapi.json`
#[tokio::test]
async fn test_openapi_json_root_route() -> Result<()> {
    // Build a test figment and config
    let figment = rocket::Config::figment()
        .merge(("address", "127.0.0.1"))
        .merge(("port", 0))
        .merge(("log_level", rocket::config::LogLevel::Off));
    // Add an access_config to figment to satisfy request guards that extract configuration
    let figment = figment.merge(("access_config", AccessConfig::default()));

    let config = Arc::new(RwLock::new(Config::default()));

    // Ensure a test HMAC secret is present for the JWT validator
    {
        let mut cfg = config.write().await;
        cfg.visualization.hmac_secret = "test-hmac-secret-key-for-testing".to_string();
    }

    // Build the Rocket instance which mounts openapi documentation routes
    let rocket = build_rocket(figment, config, None, None, None, None, None).await;

    let client = match Client::tracked(rocket).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Client::tracked() failed: {:?}", e);
            return Err(e.into());
        }
    };

    // Call the openapi.json route directly at root
    let response = client.get("/openapi.json").dispatch().await;

    // The route should exist and return 200
    let status = response.status();
    let body = response.into_string().await.unwrap_or_default();
    println!(
        "openapi response status: {:?}\nopenapi response body:\n{}",
        status, body
    );
    assert_eq!(status, Status::Ok, "Expected /openapi.json to be served");
    let json: Value = serde_json::from_str(&body)?;
    assert_eq!(json.get("openapi").and_then(|v| v.as_str()), Some("3.0.0"));

    Ok(())
}
