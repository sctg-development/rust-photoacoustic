// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use anyhow::Result;
use rocket::figment::Figment;
use rocket::{http::Status, local::asynchronous::Client};
use rust_photoacoustic::{
    config::{AccessConfig, Config},
    processing::computing_nodes::ComputingSharedData,
    thermal_regulation::shared_state::SharedThermalRegulationState,
    visualization::server::build_rocket,
    visualization::shared_state::SharedVisualizationState,
};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Ensure the openapi.json spec exposes at least 10 endpoints coming from the
/// visualization API modules that are re-exported in `visualization/api/mod.rs`.
#[tokio::test]
async fn test_openapi_json_contains_visualization_endpoints() -> Result<()> {
    // Build rocket Figment with test configuration
    let figment = rocket::Config::figment()
        .merge(("address", "127.0.0.1"))
        .merge(("port", 0))
        .merge(("log_level", rocket::config::LogLevel::Off));
    // satisfy request guards that extract access_config
    let figment = figment.merge(("access_config", AccessConfig::default()));

    let config = Arc::new(RwLock::new(Config::default()));

    // Ensure a test HMAC secret exists for JWT validation
    {
        let mut cfg = config.write().await;
        cfg.visualization.hmac_secret = "test-hmac-secret-key-for-testing".to_string();
    }

    // Provide visualization, computing and thermal states so the OpenAPI spec
    // includes all visualization endpoints
    let vis_state = Arc::new(SharedVisualizationState::default());
    let computing_state = Arc::new(RwLock::new(ComputingSharedData::default()));
    let thermal_state = Arc::new(RwLock::new(SharedThermalRegulationState::new()));

    let rocket = build_rocket(
        figment,
        config,
        None,
        Some(vis_state),
        None,
        Some(thermal_state),
        Some(computing_state),
    )
    .await;
    let client = Client::tracked(rocket).await?;

    let response = client.get("/openapi.json").dispatch().await;
    assert_eq!(
        response.status(),
        Status::Ok,
        "Expected /openapi.json to be served"
    );
    let body = response.into_string().await.unwrap_or_default();
    let json: Value = serde_json::from_str(&body)?;

    // Retrieve the paths map
    let paths = match json.get("paths") {
        Some(Value::Object(map)) => map,
        _ => panic!("openapi.json did not contain a 'paths' object"),
    };

    // Consider paths originating from visualization API modules exported in mod.rs.
    // We match by path prefixes relevant to those modules.
    let prefixes = [
        "action",
        "computing",
        "config",
        "graph",
        "system",
        "test",
        "thermal",
    ];

    let mut viz_paths = 0usize;
    let mut matched_keys: Vec<String> = Vec::new();
    for key in paths.keys() {
        if key.starts_with("/api/") {
            for p in &prefixes {
                if key.starts_with(&format!("/api/{}", p))
                    || key.starts_with(&format!("/api/{}?", p))
                {
                    viz_paths += 1;
                    matched_keys.push(key.clone());
                    break;
                }
            }
        }
    }

    println!("Found {} visualization API path entries", viz_paths);
    if !matched_keys.is_empty() {
        println!("Matched paths from OpenAPI spec:");
        for k in &matched_keys {
            println!(" - {}", k);
        }
    }

    assert!(
        viz_paths >= 10,
        "Expected at least 10 visualization API endpoints in /openapi.json, found {}",
        viz_paths
    );

    Ok(())
}
