// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use crate::config::Config;
use rocket::get;
use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use std::sync::{Arc, RwLock};

use auth_macros::openapi_protect_get;

pub type ConfigState = State<Arc<RwLock<Config>>>;

/// Get the current application configuration
///
/// **Endpoint:** `GET /api/config`
///
/// Returns the complete application configuration as JSON, including all sections:
/// - Visualization server settings
/// - Data acquisition configuration
/// - Modbus TCP server settings
/// - Photoacoustic measurement parameters
/// - User access and permissions
/// - Processing pipeline configuration
/// - OAuth2/OIDC provider settings
///
/// This endpoint is useful for:
/// - Administrative configuration review
/// - Configuration validation and debugging
/// - Dynamic configuration discovery by client applications
/// - Configuration backup and documentation
///
/// ### Authentication
///
/// This endpoint requires a valid JWT bearer token in the Authorization header
/// with administrative privileges. The token must have the `admin:api` scope.
///
/// ### Security Considerations
///
/// **⚠️ WARNING:** This endpoint exposes sensitive configuration data including:
/// - HMAC secrets and cryptographic keys
/// - Database connection strings
/// - Internal service URLs and credentials
/// - User password hashes
///
/// Access should be restricted to trusted administrators only.
///
/// ### Returns
///
/// Returns a JSON response containing the complete `Config` structure with all
/// configuration sections populated according to the current application state.
///
/// ### Response Structure
///
/// ```json
/// {
///   "visualization": {
///     "enabled": true,
///     "port": 8080,
///     "address": "127.0.0.1",
///     "hmac_secret": "...",
///     "rs256_public_key_path": null
///   },
///   "acquisition": {
///     "enabled": true,
///     "interval_ms": 1000
///   },
///   "modbus": {
///     "enabled": false,
///     "port": 502,
///     "address": "127.0.0.1"
///   },
///   "photoacoustic": {
///     "input_device": null,
///     "input_file": null,
///     "frequency": 1000.0,
///     "bandwidth": 100.0,
///     "frame_size": 1024,
///     "averages": 10
///   },
///   "access": {
///     "users": [...],
///     "clients": [...],
///     "duration": 86400,
///     "iss": "LaserSmartServer"
///   },
///   "processing": {
///     "enabled": true,
///     "result_buffer_size": 1000,
///     "default_graph": {...},
///     "performance": {...}
///   },
///   "generix": {
///     "provider": "generix",
///     "api_base_url": "https://localhost:8080",
///     "client_id": "LaserSmartClient",
///     "scope": "openid email profile read:api write:api",
///     ...
///   }
/// }
/// ```
///
/// ### Error Responses
///
/// - `401 Unauthorized`: Missing or invalid JWT token
/// - `403 Forbidden`: Token lacks required `admin:api` scope
/// - `500 Internal Server Error`: Server error accessing configuration
#[openapi_protect_get("/api/config", "admin:api", tag = "Configuration")]
pub async fn get_config(config: &ConfigState) -> Json<Config> {
    // Return the current configuration as JSON
    Json(config.inner().read().unwrap().clone())
}

/// Get the configuration schema
///
/// **Endpoint:** `GET /api/config.schema.json`
///
/// Returns the JSON schema for the application configuration, which defines the
/// structure and validation rules for the configuration data.
///
/// This endpoint is useful for:
/// - Client applications to validate configuration data
/// - Documentation of configuration structure
#[openapi_protect_get("/api/config.schema.json", "admin:api", tag = "Configuration")]
pub async fn get_config_schema() -> Json<serde_json::Value> {
    let schema_str = include_str!("../../../../resources/config.schema.json");
    let schema: Result<serde_json::Value, serde_json::Error> = serde_json::from_str(schema_str);
    match schema {
        Ok(schema) => Json(schema),
        Err(e) => {
            eprintln!("Failed to parse config schema: {}", e);
            Json(serde_json::json!({ "error": "Invalid schema format" }))
        }
    }
}

/// Centralized function to get all config routes with OpenAPI documentation
pub fn get_config_routes() -> (Vec<rocket::Route>, OpenApi) {
    openapi_get_routes_spec![get_config, get_config_schema]
}
