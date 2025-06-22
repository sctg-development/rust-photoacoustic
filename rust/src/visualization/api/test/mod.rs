// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use auth_macros::openapi_protect_get;
use auth_macros::openapi_protect_post;
use rocket::post;
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
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
