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
    match display_data.data_type.as_str() {
        "display_update" => {
            log::info!(
                "test_api_web_dashboard_display called with Data type: {} | Concentration: {:.2} ppm | Peak amplitude: {:.2} | Peak frequency: {:.2} Hz | Source node: {} | Timestamp: {} | Retry attempt: {:?}",
                display_data.data_type,
                display_data.concentration_ppm.unwrap_or(0.0),
                display_data.peak_amplitude.unwrap_or(0.0),
                display_data.peak_frequency.unwrap_or(0.0),
                display_data.source_node_id.as_deref().unwrap_or("unknown"),
                display_data.timestamp,
                display_data.retry_attempt
            );
        }
        "alert" => {
            log::warn!(
                "test_api_web_dashboard_display received ALERT: {} | Severity: {} | Message: {} | Timestamp: {} | Retry attempt: {:?}",
                display_data.alert_type.as_deref().unwrap_or("unknown"),
                display_data.severity.as_deref().unwrap_or("unknown"),
                display_data.message.as_deref().unwrap_or("no message"),
                display_data.timestamp,
                display_data.retry_attempt
            );
        }
        "clear_action" => {
            log::info!(
                "test_api_web_dashboard_display received CLEAR_ACTION command | Timestamp: {} | Retry attempt: {:?}",
                display_data.timestamp,
                display_data.retry_attempt
            );
        }
        _ => {
            log::warn!(
                "test_api_web_dashboard_display received unknown data type: {} | Timestamp: {} | Retry attempt: {:?}",
                display_data.data_type,
                display_data.timestamp,
                display_data.retry_attempt
            );
        }
    }

    let description = match display_data.data_type.as_str() {
        "display_update" => format!(
            "Display update received - Type: {} | Concentration: {:.2} ppm | Peak amplitude: {:.2} | Peak frequency: {:.2} Hz | Source: {} | Timestamp: {}",
            display_data.data_type,
            display_data.concentration_ppm.unwrap_or(0.0),
            display_data.peak_amplitude.unwrap_or(0.0),
            display_data.peak_frequency.unwrap_or(0.0),
            display_data.source_node_id.as_deref().unwrap_or("unknown"),
            display_data.timestamp
        ),
        "alert" => format!(
            "Alert received - Type: {} | Severity: {} | Message: {} | Timestamp: {}",
            display_data.alert_type.as_deref().unwrap_or("unknown"),
            display_data.severity.as_deref().unwrap_or("unknown"),
            display_data.message.as_deref().unwrap_or("no message"),
            display_data.timestamp
        ),
        "clear_action" => format!(
            "Clear display command received - Timestamp: {}",
            display_data.timestamp
        ),
        _ => format!(
            "Unknown data type '{}' received - Timestamp: {}",
            display_data.data_type,
            display_data.timestamp
        ),
    };

    let message = match display_data.data_type.as_str() {
        "display_update" => Some(format!(
            "Successfully processed display update from node '{}'",
            display_data.source_node_id.as_deref().unwrap_or("unknown")
        )),
        "alert" => Some(format!(
            "Successfully processed alert: {}",
            display_data.message.as_deref().unwrap_or("no message")
        )),
        "clear_action" => Some("Successfully processed clear display command".to_string()),
        _ => Some(format!(
            "Processed unknown data type: {}",
            display_data.data_type
        )),
    };

    Json(TestResponse {
        description,
        token: token.to_string(),
        user: bearer.user_info.user_id.clone(),
        message,
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
    // Optional fields for display updates
    #[serde(default)]
    pub concentration_ppm: Option<f64>,
    #[serde(default)]
    pub source_node_id: Option<String>,
    #[serde(default)]
    pub peak_amplitude: Option<f32>,
    #[serde(default)]
    pub peak_frequency: Option<f32>,
    pub timestamp: u64,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub retry_attempt: Option<u32>,
    // Optional fields for alerts
    #[serde(default)]
    pub alert_type: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub data: Option<std::collections::HashMap<String, serde_json::Value>>,
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
