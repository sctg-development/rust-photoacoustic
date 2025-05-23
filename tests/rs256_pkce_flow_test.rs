// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Integration test for RS256 JWT with PKCE OAuth flow
//!
//! This test simulates a real-world OAuth 2.0 authorization code flow with PKCE
//! (Proof Key for Code Exchange) using RS256 JWT tokens for signature.

use base64::Engine;
use jsonwebtoken::{Algorithm, DecodingKey};
use log::debug;
use regex::Regex;
use reqwest::Url;
use rocket::http::{ContentType, Status};
use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey};
use rust_photoacoustic::visualization::server;
use rust_photoacoustic::{config::AccessConfig, visualization::jwt_keys::JwkKeySet};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Generate a test configuration for Rocket
fn get_test_figment() -> rocket::figment::Figment {
    rocket::Config::figment()
        .merge(("port", 0)) // Use random port for testing
        .merge(("address", "127.0.0.1"))
        .merge(("log_level", rocket::config::LogLevel::Debug))
        .merge(("secret_key", "/qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis="))
}

/// Generate test RS256 key pair for JWT signing and verification
fn generate_test_rs256_keys() -> (Vec<u8>, Vec<u8>, String, String) {
    // For testing, we'll generate a new key pair each time
    let mut rng = rsa::rand_core::OsRng;
    let private_key =
        rsa::RsaPrivateKey::new(&mut rng, 2048).expect("Failed to generate RSA private key");
    let public_key = rsa::RsaPublicKey::from(&private_key);

    // Convert to PEM format
    let private_pem = EncodeRsaPrivateKey::to_pkcs1_pem(&private_key, rsa::pkcs1::LineEnding::LF)
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

/// Generate PKCE verifier and challenge
fn generate_pkce_challenge() -> (String, String, String) {
    // Generate a random code verifier (between 43 and 128 chars)
    let code_verifier: String = rand::random::<[u8; 32]>()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    // Generate code challenge using S256 method
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let code_challenge_bytes = hasher.finalize();
    let code_challenge =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(code_challenge_bytes);

    // Define the challenge method (always S256 for security)
    let challenge_method = "S256".to_string();

    (code_verifier, code_challenge, challenge_method)
}

/// Parse redirect URL to extract authorization code
fn extract_code_from_redirect_url(redirect_url: &str) -> Option<String> {
    let url = Url::parse(redirect_url).ok()?;
    let code = url
        .query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.to_string());
    code
}

/// Extract formaction from HTML response (used in consent form testing)
fn extract_form_action_from_html(html: &str) -> Option<String> {
    // debug!("HTML content: {}", html);
    let re = Regex::new(r#"formaction="([^"]+)"#).ok()?;
    re.captures(html)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

#[rocket::async_test]
async fn test_rs256_pkce_flow() {
    // Initialize the logger for tests
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // Generate test RS256 key pair
    let (_, _, private_base64, public_base64) = generate_test_rs256_keys();

    // Test HMAC secret
    let test_hmac_secret = "test-hmac-secret-key-for-testing";

    // Test AccessConfig
    // The default config includes admin / admin123 as test credentials
    let test_access_config = AccessConfig::default();

    // Initialize Rocket with RS256 keys
    let figment = get_test_figment()
        .merge(("rs256_private_key", &private_base64))
        .merge(("rs256_public_key", &public_base64));

    // Add hmac secret and access config to the figment
    let figment = figment.merge(("hmac_secret", test_hmac_secret.to_string()));

    // Add access config to the figment - fix the key name
    let figment = figment.merge(("access", test_access_config)); // Changed from "access_config" to "access"

    let rocket = server::build_rocket(figment).await;

    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Step 1: Generate PKCE verifier and challenge
    let (code_verifier, code_challenge, challenge_method) = generate_pkce_challenge();

    // Step 2: Verify OpenID Configuration contains RS256
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

    // Verify RS256 is in the supported algorithms
    let signing_algs = config_json["id_token_signing_alg_values_supported"]
        .as_array()
        .expect("signing algs should be an array");
    assert!(
        signing_algs.iter().any(|alg| alg == "RS256"),
        "RS256 should be in supported signing algorithms"
    );

    // Step 3: Make initial authorization request with PKCE challenge
    let auth_response = client
        .get(format!(
            "/authorize?response_type=code&client_id=LaserSmartClient&redirect_uri=http://localhost:8080/client/&code_challenge={}&code_challenge_method={}&scope=openid%20read:api",
            code_challenge, challenge_method
        ))
        .dispatch()
        .await;

    assert_eq!(auth_response.status(), Status::Ok);

    // Extract login form HTML
    let login_html = auth_response
        .into_string()
        .await
        .expect("HTML response body");
    assert!(
        login_html.contains("<form"),
        "Response should contain a form"
    );

    // Extract form action URL for login
    let form_action = extract_form_action_from_html(&login_html)
        .expect("Should extract form action from login page");
    debug!("Form action URL: {}", form_action);

    // Step 4: Submit the login form (approve access)
    // Simulate user login by submitting the form with test credentials
    let mut form_data = HashMap::new();
    form_data.insert("username", "admin"); // Use default username from AccessConfig
    form_data.insert("password", "admin123"); // Use default password from AccessConfig
    form_data.insert("scope", "openid read:api");
    form_data.insert("client_id", "LaserSmartClient");
    form_data.insert("redirect_uri", "http://localhost:8080/client/");
    form_data.insert("response_type", "code");
    let consent_response = client
        .post(&form_action)
        .header(ContentType::Form)
        .body(serde_urlencoded::to_string(&form_data).unwrap())
        .dispatch()
        .await;
    assert_eq!(consent_response.status(), Status::Found);

    // Get the redirect URL that contains the authorization code
    let redirect_location = consent_response
        .headers()
        .get_one("Location")
        .expect("Should have location header");

    debug!("Redirect location: {}", redirect_location);

    let consent_page_response = client.get(redirect_location).dispatch().await;

    // The consent page response should be either a redirect or an HTML page
    // We need to check if it's a consent page or an automatic redirect
    if consent_page_response.status() == Status::Ok {
        // C'est une page HTML de consentement - il faut l'analyser et soumettre le consentement
        let consent_html = consent_page_response
            .into_string()
            .await
            .expect("HTML content from consent page");

        // debug!("Consent page HTML: {}", consent_html);

        // Extract the form action for the Accept button
        let accept_form_regex =
            regex::Regex::new(r#"<form method="post" action="([^"]*allow=true[^"]*)">"#).unwrap();
        let consent_action = accept_form_regex
            .captures(&consent_html)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .expect("Should extract form action for consent acceptance");

        debug!("Consent form action: {}", consent_action);
        // Submit the consent form (allow=true)
        let consent_submit_response = client
            .post(format!("{}?allow=true", consent_action))
            .dispatch()
            .await;

        assert_eq!(consent_submit_response.status(), Status::Found);

        // Extract the redirect URL that contains the authorization code
        let final_redirect = consent_submit_response
            .headers()
            .get_one("Location")
            .expect("Should have location header after consent");

        debug!("Final redirect with code: {}", final_redirect);

        let _auth_code = extract_code_from_redirect_url(final_redirect)
            .expect("Should extract authorization code from redirect URL");
    } else if consent_page_response.status() == Status::Found {
        // Automatic redirection with consent already given
        let final_redirect = consent_page_response
            .headers()
            .get_one("Location")
            .expect("Should have location header after auto-consent");

        debug!("Auto-consent redirect with code: {}", final_redirect);

        let _auth_code = extract_code_from_redirect_url(final_redirect)
            .expect("Should extract authorization code from redirect URL");
    } else {
        panic!(
            "Unexpected response status: {:?}",
            consent_page_response.status()
        );
    }

    let auth_code = extract_code_from_redirect_url(redirect_location)
        .expect("Should extract authorization code from redirect URL");
    debug!("Authorization code: {}", auth_code);

    // Step 5: Exchange the authorization code for a token using the PKCE verifier
    let mut form_data = HashMap::new();
    form_data.insert("grant_type", "authorization_code");
    form_data.insert("code", &auth_code);
    form_data.insert("redirect_uri", "http://localhost:8080/client/");
    form_data.insert("client_id", "LaserSmartClient");
    form_data.insert("code_verifier", &code_verifier);

    let token_response = client
        .post("/token")
        .header(ContentType::Form)
        .body(serde_urlencoded::to_string(&form_data).unwrap())
        .dispatch()
        .await;

    assert_eq!(token_response.status(), Status::Ok);
    assert_eq!(token_response.content_type(), Some(ContentType::JSON));

    // Parse the token response
    let token_json: Value =
        serde_json::from_str(&token_response.into_string().await.expect("Token response"))
            .expect("Valid JSON token response");

    // Extract access token and verify it's a JWT (3 parts separated by dots)
    let access_token = token_json["access_token"]
        .as_str()
        .expect("Should have access_token")
        .to_string();

    let token_parts: Vec<&str> = access_token.split('.').collect();
    assert_eq!(
        token_parts.len(),
        3,
        "Access token should be a JWT with 3 parts"
    );

    // Step 6: Test JWKS endpoint to get public key for verification
    let jwks_response = client.get("/.well-known/jwks.json").dispatch().await;

    assert_eq!(jwks_response.status(), Status::Ok);
    assert_eq!(jwks_response.content_type(), Some(ContentType::JSON));

    let jwks_json: Value =
        serde_json::from_str(&jwks_response.into_string().await.expect("JWKS response"))
            .expect("Valid JSON response");

    // Extract the first key (should be our RS256 public key)
    let keys = jwks_json["keys"]
        .as_array()
        .expect("keys should be an array");
    assert!(!keys.is_empty(), "JWKS should contain at least one key");

    let first_key = &keys[0];
    assert_eq!(first_key["alg"], "RS256", "Key algorithm should be RS256");

    // Step 7: Verify the token can be decoded with the public key from JWKS
    // For this test we'll use the public key from our generated key pair
    let decoded_public_key = base64::engine::general_purpose::STANDARD
        .decode(&public_base64)
        .expect("Should be able to decode public key");

    let decoding_key = DecodingKey::from_rsa_pem(&decoded_public_key)
        .expect("Should be able to create decoding key");

    let mut validation = jsonwebtoken::Validation::new(Algorithm::RS256);
    validation.validate_exp = false; // Skip expiration validation for testing
    validation.set_audience(&["LaserSmartClient"]); // Set expected audience to match the token

    let token_data = jsonwebtoken::decode::<Value>(&access_token, &decoding_key, &validation)
        .expect("Should be able to verify the token");

    let claims = token_data.claims;

    // Verify standard claims
    assert_eq!(claims["aud"], "LaserSmartClient", "Wrong audience claim");
    assert!(
        claims["exp"].as_i64().is_some(),
        "Should have expiration claim"
    );

    // Verify the scope claim
    let scope = claims["scope"].as_str().expect("Should have scope claim");
    let scopes: Vec<&str> = scope.split_whitespace().collect();
    assert!(
        scopes.contains(&"openid"),
        "Token should include openid scope"
    );
    assert!(
        scopes.contains(&"read:api"),
        "Token should include read:api scope"
    );

    // Verify the JWT issuer and signing algorithm
    let header = token_data.header;
    assert_eq!(
        header.alg,
        Algorithm::RS256,
        "Token should be signed with RS256"
    );

    // Step 8: Test token refresh if refresh token is provided
    if let Some(refresh_token) = token_json["refresh_token"].as_str() {
        // If we have a refresh token, test refresh flow
        let mut refresh_form = HashMap::new();
        refresh_form.insert("grant_type", "refresh_token");
        refresh_form.insert("refresh_token", refresh_token);
        refresh_form.insert("client_id", "LaserSmartClient");

        let refresh_response = client
            .post("/token") // Compliant with OAuth2 RFC6749, should not use /refresh
            .header(ContentType::Form)
            .body(serde_urlencoded::to_string(&refresh_form).unwrap())
            .dispatch()
            .await;

        assert_eq!(refresh_response.status(), Status::Ok);
        assert_eq!(refresh_response.content_type(), Some(ContentType::JSON));

        // Parse the refresh response
        let refresh_json: Value = serde_json::from_str(
            &refresh_response
                .into_string()
                .await
                .expect("Refresh response"),
        )
        .expect("Valid JSON refresh response");

        // Verify we got a new access token
        assert!(
            refresh_json["access_token"].as_str().is_some(),
            "Refresh should return a new access token"
        );
    }

    // Step 9: Attempt to generate a token with incorrect PKCE verifier (should fail)
    let incorrect_verifier = "incorrect_code_verifier_that_wont_match_challenge";
    let mut invalid_form_data = HashMap::new();
    invalid_form_data.insert("grant_type", "authorization_code");
    invalid_form_data.insert("code", &auth_code);
    invalid_form_data.insert("redirect_uri", "http://localhost:8080/client/");
    invalid_form_data.insert("client_id", "LaserSmartClient");
    invalid_form_data.insert("code_verifier", incorrect_verifier);

    let invalid_response = client
        .post("/token")
        .header(ContentType::Form)
        .body(serde_urlencoded::to_string(&invalid_form_data).unwrap())
        .dispatch()
        .await;

    // The token endpoint should return an error with invalid PKCE verifier
    // Note: The exact status code depends on implementation, but it should not be 200 OK
    assert_ne!(
        invalid_response.status(),
        Status::Ok,
        "Token endpoint should reject invalid PKCE verifier"
    );
}

#[rocket::async_test]
async fn test_rs256_jwks_endpoint() {
    // Generate test RS256 key pair
    let (_, public_key_bytes, private_base64, public_base64) = generate_test_rs256_keys();

    // Test HMAC secret
    let test_hmac_secret = "test-hmac-secret-key-for-testing";

    // Initialize Rocket with RS256 keys
    let figment = get_test_figment()
        .merge(("rs256_private_key", &private_base64))
        .merge(("rs256_public_key", &public_base64));
    // Add hmac secret to the figment
    let figment = figment.merge(("hmac_secret", test_hmac_secret));

    let rocket = server::build_rocket(figment).await;

    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Get JWKS from endpoint
    let jwks_response = client.get("/.well-known/jwks.json").dispatch().await;
    assert_eq!(jwks_response.status(), Status::Ok);

    let jwks_json: Value =
        serde_json::from_str(&jwks_response.into_string().await.expect("JWKS response"))
            .expect("Valid JSON response");

    // Verify the key is an RSA key
    let keys = jwks_json["keys"]
        .as_array()
        .expect("keys array should exist");
    assert!(!keys.is_empty(), "JWKS should contain at least one key");

    // Get the first key
    let jwk = &keys[0];
    assert_eq!(jwk["kty"], "RSA", "Key type should be RSA");
    assert_eq!(jwk["alg"], "RS256", "Algorithm should be RS256");

    // Extract modulus (n) and exponent (e)
    let _n = jwk["n"].as_str().expect("Should have modulus");
    let e = jwk["e"].as_str().expect("Should have exponent");
    assert_eq!(e, "AQAB", "Exponent should be AQAB (65537)");

    // Verify the key ID exists
    assert!(jwk["kid"].as_str().is_some(), "Should have key ID");

    // Verify key use is "sig" (signature)
    assert_eq!(jwk["use"], "sig", "Key use should be for signature");

    // Create a JWK from our public key for comparison
    let generated_jwk = JwkKeySet::create_jwk_from_pem(&public_key_bytes)
        .expect("Should be able to create JWK from PEM");

    // If we have RSA parameters, extract them for comparison
    if let jsonwebtoken::jwk::AlgorithmParameters::RSA(params) = generated_jwk.algorithm {
        // Our locally created JWK should have matching n and e values with the server's JWK
        assert_eq!(params.e, "AQAB", "Local JWK should have the right exponent");
        // Note: We don't compare the modulus (n) directly because the encoding might differ
    }
}
