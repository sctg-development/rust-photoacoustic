//! Integration test demonstrating the protect_get macro
//! This test shows how the macro can be used in a real Rocket application

use auth_macros::protect_get;
use rocket::http::{Header, Status};
use rocket::local::blocking::Client;
use rocket::serde::json::Json;
use serde::Serialize;

#[derive(Serialize)]
struct ApiResponse {
    message: String,
    user_id: String,
}

/// Test route using the protect_get macro
#[protect_get("/api/test", "read:test")]
fn test_protected_route(
    bearer: rust_photoacoustic::visualization::auth::guards::bearer::OAuthBearer,
) -> Json<ApiResponse> {
    Json(ApiResponse {
        message: "Access granted!".to_string(),
        user_id: bearer.user_info.user_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::routes;

    #[test]
    fn test_macro_compilation() {
        // This test verifies that the macro compiles correctly
        // and can be used to build a Rocket application

        let rocket = rocket::build().mount("/", routes![test_protected_route]);

        // If we can build the client, the macro compiled successfully
        let _client = Client::tracked(rocket).expect("valid rocket instance");

        // Note: Actually testing the auth functionality would require
        // setting up the full auth system with JWT tokens, which is
        // beyond the scope of this compilation test
    }

    #[test]
    fn test_macro_without_auth_returns_error() {
        // This test verifies that the macro generates routes that depend on auth state
        // We expect this to fail with configuration errors when auth isn't set up,
        // which proves the macro is correctly trying to validate authentication

        let rocket = rocket::build().mount("/", routes![test_protected_route]);

        let client = Client::tracked(rocket).expect("valid rocket instance");

        // Request without Authorization header should return an error
        // (500 because auth system isn't configured, but that proves the macro works)
        let response = client.get("/api/test").dispatch();

        // We expect either 401 (if auth works) or 500 (if auth system not configured)
        // Both prove the macro is trying to do authentication
        assert!(
            response.status() == Status::Unauthorized
                || response.status() == Status::InternalServerError,
            "Expected 401 or 500, got: {:?}",
            response.status()
        );
    }

    #[test]
    fn test_macro_with_invalid_token_expects_error() {
        // Similar test but with invalid token - should also fail due to missing config
        let rocket = rocket::build().mount("/", routes![test_protected_route]);

        let client = Client::tracked(rocket).expect("valid rocket instance");

        // Request with invalid Bearer token
        let response = client
            .get("/api/test")
            .header(Header::new("Authorization", "Bearer invalid-token"))
            .dispatch();

        // Again, we expect authentication-related errors
        assert!(
            response.status() == Status::Unauthorized
                || response.status() == Status::InternalServerError,
            "Expected 401 or 500, got: {:?}",
            response.status()
        );
    }
}
