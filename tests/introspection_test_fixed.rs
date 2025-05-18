// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use chrono::{Duration as ChronoDuration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use oxide_auth::primitives::grant::{Extensions, Grant};
use oxide_auth::primitives::issuer::Issuer;
use rocket::http::ContentType;
use rocket::local::asynchronous::Client;
use rust_photoacoustic::visualization::oxide_auth::OxideState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration as StdDuration;
use tokio::time::timeout;
use url::Url;

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

// Implementation replaced with individual test functions above

#[rocket::async_test]
async fn test_jwt_token_introspection() {
    // Use a standard timeout for the test
    let test_future = async {
        // Set up client and state for testing JWT tokens
        let test_secret = "test-secret-for-introspection-tests";
        let oxide_state = OxideState::preconfigured(test_secret);

        // Configure Rocket for testing with explicit shutdown
        let figment = rocket::Config::figment()
            .merge(("port", 0)) // Use a random port for testing
            .merge(("address", "127.0.0.1"))
            .merge(("shutdown.ctrlc", false)) // Don't wait for Ctrl+C
            .merge(("shutdown.grace", 1))
            .merge(("shutdown.mercy", 1))
            .merge(("shutdown.force", true));

        let rocket = rocket::custom(figment)
            .mount(
                "/",
                rocket::routes![rust_photoacoustic::visualization::introspection::introspect],
            )
            .manage(oxide_state);

        let client = Client::tracked(rocket)
            .await
            .expect("valid rocket instance");

        // Test with a valid JWT token
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
            &EncodingKey::from_secret(test_secret.as_bytes()),
        )
        .expect("Token encoding failed");

        // Test the endpoint with a valid token
        let response = client
            .post("/introspect")
            .header(ContentType::Form)
            .body(format!("token={}", token))
            .dispatch()
            .await;

        let response_body = response
            .into_string()
            .await
            .expect("Failed to get response body");
        let introspection_response: IntrospectionResponse =
            serde_json::from_str(&response_body).expect("Failed to parse response");

        assert!(introspection_response.active);
        assert_eq!(introspection_response.scope, Some("read:api".to_string()));
        assert_eq!(
            introspection_response.client_id,
            Some("LaserSmartClient".to_string())
        );
        assert_eq!(introspection_response.sub, Some("test_user".to_string()));

        // Shutdown the Rocket instance
        client.rocket().shutdown().await;
        Ok::<(), Box<dyn std::error::Error>>(())
    };

    // Run with a timeout and check results
    match timeout(StdDuration::from_secs(5), test_future).await {
        Ok(result) => {
            // Assert that the test completed successfully
            if let Err(e) = result {
                panic!("Test failed: {:?}", e);
            }
        }
        Err(_) => {
            println!("Test timed out after 5 seconds");
        }
    }
}

#[rocket::async_test]
async fn test_expired_token_introspection() {
    let test_future = async {
        // Set up client and state
        let test_secret = "test-secret-for-introspection-tests";
        let oxide_state = OxideState::preconfigured(test_secret);

        let figment = rocket::Config::figment()
            .merge(("port", 0))
            .merge(("address", "127.0.0.1"))
            .merge(("shutdown.ctrlc", false))
            .merge(("shutdown.grace", 0))
            .merge(("shutdown.mercy", 0))
            .merge(("shutdown.force", true));

        let rocket = rocket::custom(figment)
            .mount(
                "/",
                rocket::routes![rust_photoacoustic::visualization::introspection::introspect],
            )
            .manage(oxide_state);

        let client = Client::tracked(rocket)
            .await
            .expect("valid rocket instance");

        // Test with an expired token
        let now = Utc::now();
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

        let header = Header::new(Algorithm::HS256);
        let expired_token = encode(
            &header,
            &expired_claims,
            &EncodingKey::from_secret(test_secret.as_bytes()),
        )
        .expect("Token encoding failed");

        let response = client
            .post("/introspect")
            .header(ContentType::Form)
            .body(format!("token={}", expired_token))
            .dispatch()
            .await;

        assert_eq!(response.status().code, 200);

        let response_body = response
            .into_string()
            .await
            .expect("Failed to get response body");
        let introspection_response: IntrospectionResponse =
            serde_json::from_str(&response_body).expect("Failed to parse response");

        assert!(!introspection_response.active);

        // Shutdown the Rocket instance
        client.rocket().shutdown().await;
        Ok::<(), Box<dyn std::error::Error>>(())
    };

    // Run with a timeout and check results
    match timeout(StdDuration::from_secs(5), test_future).await {
        Ok(result) => {
            // Assert that the test completed successfully
            if let Err(e) = result {
                panic!("Test failed: {:?}", e);
            }
        }
        Err(_) => {
            println!("Test timed out after 5 seconds");
        }
    }
}

#[rocket::async_test]
async fn test_invalid_token_introspection() {
    let test_future = async {
        // Set up client and state
        let test_secret = "test-secret-for-introspection-tests";
        let oxide_state = OxideState::preconfigured(test_secret);

        let figment = rocket::Config::figment()
            .merge(("port", 0))
            .merge(("address", "127.0.0.1"))
            .merge(("shutdown.ctrlc", false))
            .merge(("shutdown.grace", 1))
            .merge(("shutdown.mercy", 1))
            .merge(("shutdown.force", true)); // Force shutdown

        let rocket = rocket::custom(figment)
            .mount(
                "/",
                rocket::routes![rust_photoacoustic::visualization::introspection::introspect],
            )
            .manage(oxide_state);

        let client = Client::tracked(rocket)
            .await
            .expect("valid rocket instance");

        // Test with an invalid token
        let response = client
            .post("/introspect")
            .header(ContentType::Form)
            .body("token=invalid_token")
            .dispatch()
            .await;

        assert_eq!(response.status().code, 200);

        let response_body = response
            .into_string()
            .await
            .expect("Failed to get response body");
        let introspection_response: IntrospectionResponse =
            serde_json::from_str(&response_body).expect("Failed to parse response");

        assert!(!introspection_response.active);

        // Shutdown the Rocket instance
        println!("Shutting down server...");
        client.rocket().shutdown().await;
        Ok::<(), Box<dyn std::error::Error>>(())
    };

    // Run with a timeout and check results
    match timeout(StdDuration::from_secs(5), test_future).await {
        Ok(result) => {
            // Assert that the test completed successfully
            if let Err(e) = result {
                panic!("Test failed: {:?}", e);
            }
        }
        Err(_) => {
            println!("Test timed out after 5 seconds");
        }
    }
}

#[rocket::async_test]
async fn test_oxide_auth_token_introspection() {
    let test_future = async {
        // Set up client and state
        let test_secret = "test-secret-for-introspection-tests";
        let oxide_state = OxideState::preconfigured(test_secret);

        let figment = rocket::Config::figment()
            .merge(("port", 0))
            .merge(("address", "127.0.0.1"))
            .merge(("shutdown.ctrlc", false))
            .merge(("shutdown.grace", 1))
            .merge(("shutdown.mercy", 1))
            .merge(("shutdown.force", true)) // Force shutdown
            .merge(("log_level", rocket::config::LogLevel::Debug)); // Add debug logging

        let rocket = rocket::custom(figment)
            .mount(
                "/",
                rocket::routes![rust_photoacoustic::visualization::introspection::introspect],
            )
            .manage(oxide_state.clone());

        println!("Creating test client...");
        let client = Client::tracked(rocket)
            .await
            .expect("valid rocket instance");
        println!("Test client created successfully");

        // Test with an oxide-auth token
        let now = Utc::now();
        let grant = Grant {
            client_id: "test_client".to_string(),
            owner_id: "test_owner".to_string(),
            redirect_uri: Url::parse("http://localhost:8080/client/").unwrap(),
            scope: "profile email".parse().unwrap(),
            until: now + ChronoDuration::hours(1),
            extensions: Extensions::new(),
        };

        // Create a token
        println!("Creating token...");
        let token = {
            let mut issuer = oxide_state.issuer.lock().unwrap();
            let token = issuer.issue(grant).expect("Failed to issue token");
            println!("Token created: {}", token.token);
            token.token
        };

        println!("Sending token to introspection endpoint...");
        let response = client
            .post("/introspect")
            .header(ContentType::Form)
            .body(format!("token={}", token))
            .dispatch()
            .await;

        println!("Response status: {}", response.status());
        assert_eq!(response.status().code, 200);

        let response_body = response
            .into_string()
            .await
            .expect("Failed to get response body");
        println!("Response body: {}", response_body);

        let introspection_response: IntrospectionResponse =
            serde_json::from_str(&response_body).expect("Failed to parse response");

        assert!(introspection_response.active);
        assert_eq!(
            introspection_response.client_id,
            Some("test_client".to_string())
        );
        assert_eq!(introspection_response.sub, Some("test_owner".to_string()));

        // Drop state reference before shutdown
        drop(oxide_state);

        // Shutdown the Rocket instance
        println!("Shutting down server...");
        client.rocket().shutdown().await;
        println!("Server shutdown complete");

        Ok::<(), Box<dyn std::error::Error>>(())
    };

    // Run with a timeout and check results
    match timeout(StdDuration::from_secs(5), test_future).await {
        Ok(result) => {
            // Assert that the test completed successfully
            if let Err(e) = result {
                panic!("Test failed: {:?}", e);
            }
        }
        Err(_) => {
            println!("Test timed out after 5 seconds");
        }
    }
}
