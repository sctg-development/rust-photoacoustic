use auth_macros::openapi_protect_get;
use rocket_okapi::JsonSchema;
use std::path::PathBuf;

use rocket::get;
use rocket::serde::json::Json;
use rocket::serde::Serialize;

#[derive(Debug, Serialize, JsonSchema)]
pub struct TestResponse {
    description: String,
    token: String,
    user: String,
}

#[openapi_protect_get("/api/test/<path..>", "read:api", tag = "Test")]
pub async fn test_api(path: PathBuf) -> Json<TestResponse> {
    let token = bearer.token.clone();
    Json(TestResponse {
        description: format!("Test API called with path: {:?}", path),
        token: token.to_string(),
        user: bearer.user_info.user_id.clone(),
    })
}
