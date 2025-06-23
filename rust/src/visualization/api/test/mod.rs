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

#[openapi_protect_get("/api/test/web_dashboard_display", "read:api", tag = "Test")]
pub async fn test_api_web_dashboard_display() -> Json<TestResponse> {
    let token = bearer.token.clone();
    log::info!(
        "test_api_web_dashboard_display called with token: {}",
        token
    );
    Json(TestResponse {
        description: format!("Test API called with path: {:?}", "web_dashboard_displa"),
        token: token.to_string(),
        user: bearer.user_info.user_id.clone(),
        message: None,
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
    openapi_get_routes_spec![test_api_web_dashboard_display, test_api, test_post_api]
}
