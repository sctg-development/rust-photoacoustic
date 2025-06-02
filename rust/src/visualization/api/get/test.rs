use std::path::PathBuf;

use auth_macros::protect_get;

use rocket::serde::json::Json;
use rocket::serde::Serialize;
#[derive(Debug, Serialize)]
struct TestResponse {
    description: String,
    token: String,
    user: String,
}
#[protect_get("/test/<path..>", "read:api")]
pub async fn test_api(path: PathBuf) -> Json<TestResponse> {
    let token = bearer.token;
    Json(TestResponse {
        description: format!("Test API called with path: {:?}", path),
        token: token.to_string(),
        user: bearer.user_info.user_id.clone(),
    })
}
