// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use auth_macros::openapi_protect_get;
use auth_macros::openapi_protect_post;
use rocket::post;
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use rocket_okapi::JsonSchema;
use std::path::PathBuf;

use rocket::get;

#[derive(Debug, Serialize, JsonSchema)]
pub struct TestResponse {
    description: String,
    token: String,
    user: String,
    message: Option<String>,
}

/// Test API endpoint for web dashboard display
///
/// This endpoint is used to test the API functionality for the web dashboard display.
/// It returns a JSON response containing the request path, user information, and an optional message.
/// It logs the concentration, amplitude, frequency, and timestamp from the display update data.
#[openapi_protect_post(
    "/api/test/web_dashboard_action",
    "read:api",
    tag = "Test",
    data = "<display_data>"
)]
pub async fn test_api_post_web_dashboard_display(
    display_data: Json<DisplayUpdateData>,
) -> Json<TestResponse> {
    let token = bearer.token.clone();

    // Log the display update data
    log::info!(
        "test_api_web_dashboard_display called with Data type: {} | Concentration: {:.2} ppm | Peak amplitude: {:.2} | Peak frequency: {:.2} Hz | Source node: {} | Timestamp: {} | Retry attempt: {:?}",
        display_data.data_type,
        display_data.concentration_ppm,
        display_data.peak_amplitude,
        display_data.peak_frequency,
        display_data.source_node_id,
        display_data.timestamp,
        display_data.retry_attempt
    );

    Json(TestResponse {
        description: format!(
            "Display update received - Type: {} | Concentration: {:.2} ppm | Peak amplitude: {:.2} | Peak frequency: {:.2} Hz | Source: {} | Timestamp: {}",
            display_data.data_type,
            display_data.concentration_ppm,
            display_data.peak_amplitude,
            display_data.peak_frequency,
            display_data.source_node_id,
            display_data.timestamp
        ),
        token: token.to_string(),
        user: bearer.user_info.user_id.clone(),
        message: Some(format!("Successfully processed display update from node '{}'", display_data.source_node_id)),
    })
}

#[openapi_protect_get("/api/test/<path..>", "read:api", tag = "Test")]
pub async fn test_api(path: PathBuf) -> Json<TestResponse> {
    let token = bearer.token.clone();
    Json(TestResponse {
        description: format!("Test API called with path: {:?}", path),
        token: token.to_string(),
        user: bearer.user_info.user_id.clone(),
        message: None,
    })
}

#[derive(Debug, Serialize, JsonSchema, Deserialize)]
pub struct TestData {
    message: String,
}

#[derive(Debug, Serialize, JsonSchema, Deserialize)]
pub struct DisplayUpdateData {
    #[serde(rename = "type")]
    pub data_type: String,
    pub concentration_ppm: f64,
    pub source_node_id: String,
    pub peak_amplitude: f32,
    pub peak_frequency: f32,
    pub timestamp: u64,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub retry_attempt: Option<u32>,
}

#[openapi_protect_post("/api/test/<path..>", "read:api", tag = "Test", data = "<test_data>")]
pub async fn test_post_api(path: PathBuf, test_data: Json<TestData>) -> Json<TestResponse> {
    let token = bearer.token.clone();
    Json(TestResponse {
        description: format!("Test API called with path: {:?}", path),
        message: Some(test_data.message.clone()),
        token: token.to_string(),
        user: bearer.user_info.user_id.clone(),
    })
}

pub fn get_test_routes() -> (Vec<rocket::Route>, OpenApi) {
    openapi_get_routes_spec![test_api, test_post_api, test_api_post_web_dashboard_display]
}
