use std::path::PathBuf;
use rocket_okapi::openapi;
use rocket_okapi::JsonSchema;
use auth_macros::protect_get;

use rocket::serde::json::Json;
use rocket::serde::Serialize;
#[derive(Debug, Serialize, JsonSchema)]
struct TestResponse {
    description: String,
    token: String,
    user: String,
}
#[openapi(tag = "Test API")]
#[protect_get("/api/test/<path..>", "read:api")]
pub async fn test_api(path: PathBuf) -> Json<TestResponse> {
    let token = bearer.token;
    Json(TestResponse {
        description: format!("Test API called with path: {:?}", path),
        token: token.to_string(),
        user: bearer.user_info.user_id.clone(),
    })
}
