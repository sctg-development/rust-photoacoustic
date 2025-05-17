// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use chrono::{Duration as ChronoDuration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use rust_photoacoustic::visualization::jwt::JwtIssuer;
use rust_photoacoustic::visualization::jwt_validator::JwtValidator;
use rust_photoacoustic::visualization::oxide_auth::OxideState;
use rocket::http::ContentType;
use rocket::local::asynchronous::Client;
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use url::Url;
use oxide_auth::primitives::grant::{Grant, Extensions};
use oxide_auth::primitives::issuer::Issuer;
use tokio::time::timeout;

// Import and define IntrospectionResponse structure to match the one in introspection.rs
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct IntrospectionResponse {
    active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    iat: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nbf: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jti: Option<String>,
    #[serde(rename = "token_type", skip_serializing_if = "Option::is_none")]
    token_type: Option<String>,
}

// Custom JWT claims structure to match what's used in the application
#[derive(Serialize, Deserialize)]
struct JwtClaims {
    sub: String,
    iat: i64,
    exp: i64,
    nbf: i64,
    jti: String,
    aud: String,
    iss: String,
    scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<HashMap<String, String>>,
}

async fn run_introspection_test() -> Result<(), Box<dyn std::error::Error>> {
    // Setup rocket client and test environment with shared OxideState
    // This way the tokens issued by the state can be validated by the introspection endpoint
    let oxide_state = OxideState::preconfigured();
    
    // Configure Rocket for testing with explicit shutdown
    let figment = rocket::Config::figment()
        .merge(("port", 0)) // Use a random port for testing
        .merge(("address", "127.0.0.1"))
        .merge(("shutdown.ctrlc", false)) // Don't wait for Ctrl+C
        .merge(("shutdown.grace", 0)) // No grace period
        .merge(("shutdown.mercy", 0)); // No mercy period
    
    // Build a client-only rocket instance 
    // We need to use the same state for both the token issuer and the introspection endpoint
    let rocket = rocket::custom(figment)
        .mount(
            "/",
            rocket::routes![rust_photoacoustic::visualization::introspection::introspect],
        )
        .manage(oxide_state.clone());
    
    let client = Client::tracked(rocket).await.expect("valid rocket instance");
    
    // Test 1: Create a valid JWT token and check introspection
    let now = Utc::now();
    let claims = JwtClaims {
        sub: "test_user".to_string(),
        iat: now.timestamp(),
        exp: (now + ChronoDuration::hours(1)).timestamp(),
        nbf: now.timestamp(),
        jti: "test_token_123".to_string(),
        aud: "LaserSmartClient".to_string(),
        iss: "rust-photoacoustic".to_string(),
        scope: "read:api".to_string(),
        metadata: Some(HashMap::from([
            ("email".to_string(), "user@example.com".to_string()),
            ("name".to_string(), "Test User".to_string()),
        ])),
    };
    
    let header = Header::new(Algorithm::HS256);
    let token = encode(
        &header, 
        &claims, 
        &EncodingKey::from_secret(b"my-super-secret-jwt-key-for-photoacoustic-app")
    ).expect("Token encoding failed");
    
    // Test the endpoint with a valid token
    let response = client
        .post("/introspect")
        .header(ContentType::Form)
        .body(format!("token={}", token))
        .dispatch()
        .await;
    
    let introspection_response: IntrospectionResponse = response
        .into_json()
        .await
        .expect("Failed to parse introspection response");
    
    println!("Token validé avec ces informations :");
    println!("Active: {}, Scope: {:?}", introspection_response.active, introspection_response.scope);
    println!("Client: {:?}, Sub: {:?}", introspection_response.client_id, introspection_response.sub);
    println!("Issuer: {:?}, Token Type: {:?}", introspection_response.iss, introspection_response.token_type);
    
    assert_eq!(introspection_response.active, true);
    assert_eq!(introspection_response.scope, Some("read:api".to_string()));
    assert_eq!(introspection_response.client_id, Some("LaserSmartClient".to_string()));
    assert_eq!(introspection_response.sub, Some("test_user".to_string()));
    assert_eq!(introspection_response.token_type, Some("Bearer".to_string()));
    assert_eq!(introspection_response.iss, Some("rust-photoacoustic".to_string()));
    
    // Test 2: Test with an expired token
    println!("Testing with an expired token...");
    let expired_claims = JwtClaims {
        sub: "test_user".to_string(),
        iat: (now - ChronoDuration::hours(2)).timestamp(),
        exp: (now - ChronoDuration::hours(1)).timestamp(), // Expired
        nbf: (now - ChronoDuration::hours(2)).timestamp(),
        jti: "expired_token_123".to_string(),
        aud: "LaserSmartClient".to_string(),
        iss: "rust-photoacoustic".to_string(),
        scope: "read:api".to_string(),
        metadata: None,
    };
    
    let expired_token = encode(
        &header, 
        &expired_claims, 
        &EncodingKey::from_secret(b"my-super-secret-jwt-key-for-photoacoustic-app")
    ).expect("Token encoding failed");
    
    let response = client
        .post("/introspect")
        .header(ContentType::Form)
        .body(format!("token={}", expired_token))
        .dispatch()
        .await;
    
    assert_eq!(response.status().code, 200);
    
    let introspection_response: IntrospectionResponse = response
        .into_json()
        .await
        .expect("Failed to parse introspection response");
    
    assert_eq!(introspection_response.active, false);
    
    // Test 3: Test with an invalid token
    println!("Testing with an invalid token...");
    let response = client
        .post("/introspect")
        .header(ContentType::Form)
        .body("token=invalid_token")
        .dispatch()
        .await;
    
    assert_eq!(response.status().code, 200);
    
    let introspection_response: IntrospectionResponse = response
        .into_json()
        .await
        .expect("Failed to parse introspection response");
    
    assert_eq!(introspection_response.active, false);
    
    // Test 4: Test with an oxide-auth token
    // First, create a grant in the oxide-auth state
    println!("Creating a grant in the oxide-auth state...");
    let grant = Grant {
        client_id: "test_client".to_string(),
        owner_id: "test_owner".to_string(),
        redirect_uri: Url::parse("http://localhost:8080/client/").unwrap(),
        scope: "profile email".parse().unwrap(),
        until: now + ChronoDuration::hours(1),
        extensions: Extensions::new(),
    };
    
    println!("Grant created: {:?}", grant);
    // Issue a token using the oxide-auth state - need to lock the mutex to get mutable access
    let mut issuer = oxide_state.issuer.lock().unwrap();
    let issued_token = issuer.issue(grant).expect("Failed to issue token");
    
    // Test the introspection endpoint with the oxide-auth token
    println!("Testing the introspection endpoint with the oxide-auth token...");
    let response = client
        .post("/introspect")
        .header(ContentType::Form)
        .body(format!("token={}", issued_token.token))
        .dispatch()
        .await;
    
    assert_eq!(response.status().code, 200);
    
    let introspection_response: IntrospectionResponse = response
        .into_json()
        .await
        .expect("Failed to parse introspection response");
    
    assert_eq!(introspection_response.active, true);
    assert_eq!(introspection_response.client_id, Some("test_client".to_string()));
    assert_eq!(introspection_response.sub, Some("test_owner".to_string()));
    
    // Shutdown the Rocket instance gracefully
    println!("Test completed, shutting down Rocket server...");
    // Get rocket instance from the client to properly shut it down
    let rocket_instance = client.rocket();
    rocket_instance.shutdown().await;
    println!("Rocket server shutdown complete");
    
    Ok(())
}

#[rocket::async_test]
async fn test_introspection_endpoint() {
    // Set a timeout for the test to ensure it doesn't hang
    let test_future = run_introspection_test();
    
    // Run with a timeout to ensure the test always completes
    match tokio::time::timeout(StdDuration::from_secs(30), test_future).await {
        Ok(result) => match result {
            Ok(_) => println!("Test completed successfully"),
            Err(e) => panic!("Test failed: {}", e),
        },
        Err(_) => panic!("Test timed out after 30 seconds"),
    }
}
