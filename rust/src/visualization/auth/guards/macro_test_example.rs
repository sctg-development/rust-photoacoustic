//! Example demonstrating the working protect_get macro

use auth_macros::protect_get;
use rocket::serde::json::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct TestResponse {
    message: String,
    user_id: String,
}

/// Example of a working protected route using the protect_get macro
/// This function will automatically:
/// 1. Add OAuthBearer parameter to function signature
/// 2. Validate Bearer token from Authorization header
/// 3. Check if user has "read:test" permission
/// 4. Return 403 Forbidden if permission check fails
/// 5. Execute original function if permission check passes
#[protect_get("/api/example/protected", "read:test")]
fn protected_example() -> Json<TestResponse> {
    // The macro automatically injects 'bearer: OAuthBearer' parameter
    // and makes it available in the function scope
    Json(TestResponse {
        message: "Successfully accessed protected route!".to_string(),
        user_id: bearer.user_info.user_id.clone(),
    })
}

/// Example with explicit OAuthBearer parameter
#[protect_get("/api/example/explicit", "admin:users")]
fn explicit_bearer_example(
    bearer: crate::visualization::auth::guards::OAuthBearer,
) -> Json<TestResponse> {
    // When OAuthBearer is explicitly in the signature,
    // the macro just adds permission checking
    Json(TestResponse {
        message: "Admin access granted!".to_string(),
        user_id: bearer.user_info.user_id.clone(),
    })
}

/// Example with additional route parameters
#[protect_get("/api/example/user/<user_id>", "read:users")]
fn user_specific_example(user_id: String) -> Json<TestResponse> {
    // Additional route parameters work normally
    // The bearer token is automatically injected by the macro
    Json(TestResponse {
        message: format!("Accessing user data for: {}", user_id),
        user_id: bearer.user_info.user_id.clone(),
    })
}

// Note: To use these routes in your Rocket application, mount them like this:
//
// rocket::build()
//     .mount("/", routes![
//         protected_example,
//         explicit_bearer_example,
//         user_specific_example
//     ])
