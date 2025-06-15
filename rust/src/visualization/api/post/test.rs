use auth_macros::openapi_protect_post;
use rocket::post;
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket_okapi::JsonSchema;
use std::path::PathBuf;

#[derive(Debug, Serialize, JsonSchema)]
pub struct TestResponse {
    description: String,
    message: String,
    token: String,
    user: String,
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
        message: test_data.message.clone(),
        token: token.to_string(),
        user: bearer.user_info.user_id.clone(),
    })
}
