// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use base64::Engine;
use jsonwebtoken::jwk::AlgorithmParameters;
use jsonwebtoken::jwk::PublicKeyUse;
use jsonwebtoken::{Algorithm, DecodingKey};
use log::debug;
use oxide_auth::endpoint::Issuer;
use rocket::config::LogLevel;
use rocket::http::{ContentType, Status};
use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey};
use rust_photoacoustic::config::AccessConfig;
use rust_photoacoustic::visualization::auth::jwt::JwkKeySet;
use rust_photoacoustic::visualization::auth::jwt::JwtIssuer;
use serde_json::Value;
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

static INIT: Once = Once::new();

/// Setup logger for tests
fn setup() {
    INIT.call_once(|| {
        env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .init();
    });
}

/// Generate a test configuration for Rocket
fn get_test_figment() -> rocket::figment::Figment {
    rocket::Config::figment()
        .merge(("port", 0)) // Use random port for testing
        .merge(("address", "127.0.0.1"))
        .merge(("log_level", LogLevel::Debug))
}

/// Generate test RS256 key pair for JWT signing and verification
fn generate_test_rs256_keys() -> (Vec<u8>, Vec<u8>, String, String) {
    // For testing, we'll generate a new key pair each time
    let mut rng = rsa::rand_core::OsRng;
    let private_key =
        rsa::RsaPrivateKey::new(&mut rng, 2048).expect("Failed to generate RSA private key");
    let public_key = rsa::RsaPublicKey::from(&private_key);

    // Convert to PEM format
    let private_pem = EncodeRsaPrivateKey::to_pkcs1_pem(&private_key, rsa::pkcs1::LineEnding::LF) //private_key.to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
        .expect("Failed to convert private key to PEM");
    let public_pem = EncodeRsaPublicKey::to_pkcs1_pem(&public_key, rsa::pkcs1::LineEnding::LF)
        .expect("Failed to convert public key to PEM");

    // Convert to byte arrays for direct use
    let private_bytes = private_pem.as_bytes().to_vec();
    let public_bytes = public_pem.as_bytes().to_vec();

    // Convert to base64 for config
    let private_base64 = base64::engine::general_purpose::STANDARD.encode(&private_bytes);
    let public_base64 = base64::engine::general_purpose::STANDARD.encode(&public_bytes);

    (private_bytes, public_bytes, private_base64, public_base64)
}

#[test]
fn test_rs256_jwt_token_generation_and_validation() {
    // Generate test RS256 key pair
    let (private_key_bytes, public_key_bytes, _, _) = generate_test_rs256_keys();

    // Create JWT issuer with RS256 keys
    let jwt_issuer = JwtIssuer::with_rs256_pem(&private_key_bytes, &public_key_bytes)
        .expect("Failed to create JWT issuer with RS256 keys");

    // Get the inner token map to issue a token directly for testing
    let mut token_map = jwt_issuer.0.lock().unwrap();

    // Create test claims
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64;
    let expiry = now + 3600; // 1 hour from now

    // Create a JWT with test data using our RS256 issuer
    let grant = oxide_auth::primitives::grant::Grant {
        owner_id: "test_user".to_string(),
        client_id: "test_client".to_string(),
        redirect_uri: "http://localhost/callback".parse().unwrap(),
        scope: "read:api write:api".parse().unwrap(),
        until: chrono::Utc::now() + chrono::Duration::hours(1),
        extensions: oxide_auth::primitives::grant::Extensions::new(),
    };

    // Issue a token
    let token_result = token_map.issue(grant.clone());
    assert!(token_result.is_ok(), "Should be able to issue a token");

    let issued_token = token_result.unwrap();
    let token = issued_token.token;

    // The token should be in the format of a JWT (three segments separated by dots)
    let segments: Vec<&str> = token.split('.').collect();
    assert_eq!(segments.len(), 3, "JWT should have 3 segments");

    // Verify the JWT can be decoded with the public key
    let decoding_key =
        DecodingKey::from_rsa_pem(&public_key_bytes).expect("Failed to create decoding key");

    let mut validation = jsonwebtoken::Validation::new(Algorithm::RS256);
    validation.validate_exp = false; // Skip expiration validation for testing
    validation.set_audience(&["test_client"]); // Set expected audience to match the token

    let token_data = jsonwebtoken::decode::<serde_json::Value>(&token, &decoding_key, &validation);
    if let Err(err) = &token_data {
        println!("JWT Verification Error: {:?}", err);
    }
    assert!(token_data.is_ok(), "Should be able to verify the token");
    let claims = token_data.unwrap().claims;

    // Verify the claims
    assert_eq!(claims["sub"], "test_user", "Wrong subject claim");
    assert_eq!(claims["aud"], "test_client", "Wrong audience claim");

    // Compare scopes regardless of order by splitting and checking each scope exists
    let expected_scopes: Vec<&str> = "read:api write:api".split_whitespace().collect();
    let actual_scopes_str = claims["scope"].as_str().expect("Scope should be a string");
    let actual_scopes: Vec<&str> = actual_scopes_str.split_whitespace().collect();

    // Verify both scope lists have the same length
    assert_eq!(
        expected_scopes.len(),
        actual_scopes.len(),
        "Scope count mismatch"
    );

    // Verify each expected scope is in the actual scopes
    for scope in expected_scopes {
        assert!(
            actual_scopes.contains(&scope),
            "Missing scope: {} in token scopes: {}",
            scope,
            actual_scopes_str
        );
    }

    // Attempt to verify with wrong key should fail
    let (_, wrong_public_key_bytes, _, _) = generate_test_rs256_keys();
    let wrong_decoding_key = DecodingKey::from_rsa_pem(&wrong_public_key_bytes)
        .expect("Failed to create wrong decoding key");

    // The validation settings remain the same as above, with audience already set
    let wrong_verify_result =
        jsonwebtoken::decode::<serde_json::Value>(&token, &wrong_decoding_key, &validation);

    assert!(
        wrong_verify_result.is_err(),
        "Verification with wrong key should fail"
    );
}

#[rocket::async_test]
async fn test_oidc_endpoints_with_rs256() {
    // Initialize the logger
    setup();
    // Generate test RS256 key pair
    let (_, _, private_base64, public_base64) = generate_test_rs256_keys();

    // Test HMAC secret
    let test_hmac_secret = "test-hmac-secret-key-for-testing";

    // Initialize Rocket with RS256 keys
    let figment = get_test_figment()
        .merge(("rs256_private_key", &private_base64))
        .merge(("access_config", AccessConfig::default()))
        .merge((
            "visualization_config",
            rust_photoacoustic::config::VisualizationConfig::default(),
        ))
        .merge(("rs256_public_key", &public_base64));

    // Add hmac secret to the figment
    let figment = figment.merge(("hmac_secret", test_hmac_secret));

    let rocket =
        rust_photoacoustic::visualization::server::build_rocket(figment, None, None, None).await;

    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Test the OpenID Configuration endpoint
    let openid_config_response = client
        .get("/.well-known/openid-configuration")
        .dispatch()
        .await;

    assert_eq!(openid_config_response.status(), Status::Ok);
    assert_eq!(
        openid_config_response.content_type(),
        Some(ContentType::JSON)
    );

    let config_json: Value = serde_json::from_str(
        &openid_config_response
            .into_string()
            .await
            .expect("OpenID Config response"),
    )
    .expect("Valid JSON response");

    debug!("OpenID Config JSON: {:?}", config_json);
    // Check that RS256 is in the supported algorithms
    let signing_algs = config_json["id_token_signing_alg_values_supported"]
        .as_array()
        .expect("signing algs should be an array");

    assert!(
        signing_algs.iter().any(|alg| alg == "RS256"),
        "RS256 should be in supported signing algorithms"
    );

    // Test the JWKS endpoint
    let jwks_response = client.get("/.well-known/jwks.json").dispatch().await;

    assert_eq!(jwks_response.status(), Status::Ok);
    assert_eq!(jwks_response.content_type(), Some(ContentType::JSON));

    let jwks_json: Value =
        serde_json::from_str(&jwks_response.into_string().await.expect("JWKS response"))
            .expect("Valid JSON response");

    // The JWKS should contain at least one key
    let keys = jwks_json["keys"]
        .as_array()
        .expect("keys should be an array");
    assert!(!keys.is_empty(), "JWKS should contain at least one key");

    // The first key should have kty=RSA and alg=RS256
    let first_key = &keys[0];
    assert_eq!(first_key["kty"], "RSA", "Key type should be RSA");
    assert_eq!(first_key["alg"], "RS256", "Algorithm should be RS256");
    assert!(first_key["n"].is_string(), "Key should have modulus (n)");
    assert!(first_key["e"].is_string(), "Key should have exponent (e)");
    assert!(first_key["kid"].is_string(), "Key should have key ID (kid)");
}

#[rocket::async_test]
async fn test_token_endpoint_with_rs256() {
    // Generate test RS256 key pair
    let (_, _, private_base64, public_base64) = generate_test_rs256_keys();

    // Test HMAC secret
    let test_hmac_secret = "test-hmac-secret-key-for-testing";

    // Initialize Rocket with RS256 keys
    let figment = get_test_figment()
        .merge(("rs256_private_key", &private_base64))
        .merge(("rs256_public_key", &public_base64))
        .merge(("access_config", AccessConfig::default()))
        .merge((
            "visualization_config",
            rust_photoacoustic::config::VisualizationConfig::default(),
        ));
    // Add hmac secret to the figment
    let figment = figment.merge(("hmac_secret", test_hmac_secret));

    let rocket =
        rust_photoacoustic::visualization::server::build_rocket(figment, None, None, None).await;

    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // TODO: The full OAuth flow test would be more complex and requires multiple steps:
    // 1. Call the authorize endpoint
    // 2. Submit the consent form
    // 3. Exchange the authorization code for a token
    // For now, we'll just verify the endpoints are working

    // Test the authorization endpoint
    let auth_response = client
        .get("/authorize?response_type=code&client_id=LaserSmartClient&redirect_uri=http://localhost:8080/client/")
        .dispatch()
        .await;

    assert_eq!(auth_response.status(), Status::Ok);

    // The authorization endpoint should return HTML containing a consent form
    let body = auth_response
        .into_string()
        .await
        .expect("HTML response body");
    assert!(body.contains("<form"), "Response should contain a form");
    assert!(
        body.contains("LaserSmartClient"),
        "Response should mention the client"
    );
}

#[rocket::async_test]
async fn test_jwk_key_generation_from_public_key() {
    // Generate test RS256 key pair
    let (_, public_key_bytes, _, _) = generate_test_rs256_keys();

    // Create a JWK from the public key
    let jwk = JwkKeySet::create_jwk_from_pem(&public_key_bytes)
        .expect("Should be able to create JWK from PEM");

    // Verify JWK properties
    assert!(
        matches!(jwk.algorithm, AlgorithmParameters::RSA(_)),
        "JWK key type should be RSA"
    );

    assert!(
        matches!(jwk.common.public_key_use, Some(PublicKeyUse::Signature)),
        "JWK key use should be 'sig' (signature)"
    );

    // Verify the key ID was generated
    assert!(jwk.common.key_id.is_some(), "JWK should have a key ID");

    // Extract RSA parameters
    if let jsonwebtoken::jwk::AlgorithmParameters::RSA(params) = jwk.algorithm {
        assert!(!params.n.is_empty(), "RSA modulus should not be empty");
        assert!(!params.e.is_empty(), "RSA exponent should not be empty");
        assert_eq!(
            params.e, "AQAB",
            "RSA exponent should be AQAB (65537 in base64url)"
        );
    } else {
        panic!("JWK should have RSA parameters");
    }
}
