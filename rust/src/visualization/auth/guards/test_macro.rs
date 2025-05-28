//! Test file for the protect_get macro

use auth_macros::protect_get;
use rocket::serde::json::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct TestResponse {
    message: String,
    user_id: String,
}

/// Test route using the protect_get macro without explicit OAuthBearer parameter
#[protect_get("/api/test/simple", "read:test")]
fn test_simple_route() -> Json<TestResponse> {
    // The macro should automatically inject the bearer parameter
    // and make it available in scope
    Json(TestResponse {
        message: "This is a protected route".to_string(),
        user_id: bearer.user_info.user_id.clone(),
    })
}

/// Test route using the protect_get macro with explicit OAuthBearer parameter
#[protect_get("/api/test/explicit", "admin:test")]
fn test_explicit_route(
    bearer: crate::visualization::auth::guards::OAuthBearer,
) -> Json<TestResponse> {
    // The macro should detect the existing bearer parameter
    // and just add permission checking
    Json(TestResponse {
        message: "This is a protected admin route".to_string(),
        user_id: bearer.user_info.user_id.clone(),
    })
}

/// Test route with additional parameters
#[protect_get("/api/test/with_param/<id>", "read:test")]
fn test_route_with_param(id: u32) -> Json<TestResponse> {
    Json(TestResponse {
        message: format!("Route with parameter: {}", id),
        user_id: bearer.user_info.user_id.clone(),
    })
}
