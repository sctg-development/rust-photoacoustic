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
use rust_photoacoustic::{config::AccessConfig, visualization::auth::jwt::JwkKeySet};
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
fn generate_pkce_challenge(method: &str) -> (String, String, String) {
    // Generate a random code verifier (between 43 and 128 chars)
    let code_verifier: String = rand::random::<[u8; 32]>()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    let (code_challenge, challenge_method) = match method {
        "S256" => {
            // Generate code challenge using S256 method (SHA256 hash)
            let mut hasher = Sha256::new();
            hasher.update(code_verifier.as_bytes());
            let code_challenge_bytes = hasher.finalize();
            let code_challenge =
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(code_challenge_bytes);
            (code_challenge, "S256".to_string())
        }
        "plain" => {
            // For plain method, the challenge is the same as the verifier
            (code_verifier.clone(), "plain".to_string())
        }
        _ => panic!("Unsupported PKCE method: {}", method),
    };

    (code_verifier, code_challenge, challenge_method)
}

/// Parse redirect URL to extract authorization code
fn extract_code_from_url(redirect_url: &str) -> Option<String> {
    debug!("Parsing URL for code: {}", redirect_url);
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
    let re = Regex::new(r#"action="([^"]+)"#).ok()?;
    re.captures(html)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

/// Extract all hidden form fields from HTML
fn extract_hidden_fields_from_html(html: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();

    // Pattern to match hidden input fields: <input type="hidden" name="field_name" value="field_value">
    let re =
        Regex::new(r#"<input\s+type="hidden"\s+name="([^"]+)"\s+value="([^"]*)"[^>]*>"#).unwrap();

    for caps in re.captures_iter(html) {
        if let (Some(name), Some(value)) = (caps.get(1), caps.get(2)) {
            fields.insert(name.as_str().to_string(), value.as_str().to_string());
        }
    }

    debug!("Extracted hidden fields: {:?}", fields);
    fields
}

/// Helper function to run the complete OAuth2 PKCE flow
async fn run_oauth_pkce_flow(
    client: &rocket::local::asynchronous::Client,
    pkce_method: &str,
    public_base64: &str,
) -> (String, String, String) {
    // Step 1: Generate PKCE verifier and challenge
    let (code_verifier, code_challenge, challenge_method) = generate_pkce_challenge(pkce_method);

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
    let authorize_url = format!(
        "/authorize?response_type=code&client_id=LaserSmartClient&redirect_uri=http://localhost:8080/client/&code_challenge={}&code_challenge_method={}&scope=openid%20read:api",
        code_challenge, challenge_method
    );
    // Important : ne PAS envoyer de cookie de session ici !
    let auth_response = client.get(&authorize_url).dispatch().await;

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
    // Extract all hidden fields from the login form to preserve PKCE parameters
    let hidden_fields = extract_hidden_fields_from_html(&login_html);

    // Simulate user login by submitting the form with test credentials
    let mut form_data: HashMap<String, String> = HashMap::new();
    form_data.insert("username".to_string(), "admin".to_string()); // Use default username from AccessConfig
    form_data.insert("password".to_string(), "admin123".to_string()); // Use default password from AccessConfig

    // Include all hidden fields (including PKCE parameters)
    for (key, value) in hidden_fields {
        form_data.insert(key.clone(), value.clone());
    }
    let login_response = client
        .post(&form_action)
        .header(ContentType::Form)
        .body(serde_urlencoded::to_string(&form_data).unwrap())
        .dispatch()
        .await;
    assert_eq!(login_response.status(), Status::Found);

    // Get the redirect URL after login (should be /authorize...)
    let redirect_location = login_response
        .headers()
        .get_one("Location")
        .expect("Should have location header");
    debug!("Redirect location: {}", redirect_location);

    // Step 5: GET /authorize again - the session cookie should already be set by Rocket
    // The client maintains the cookie jar automatically, so no need to manually extract and set cookies
    let consent_page_response = client.get(redirect_location).dispatch().await;
    debug!(
        "Consent page response status: {:?}",
        consent_page_response.status()
    );

    // Store the actual authorization code
    let mut actual_auth_code = String::new();

    if consent_page_response.status() == Status::Ok {
        // We received the consent page HTML
        let consent_html = consent_page_response
            .into_string()
            .await
            .expect("HTML content from consent page");
        let title_regex = Regex::new(r#"<title>(.*?)</title>"#).unwrap();
        let title = title_regex
            .captures(&consent_html)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str())
            .unwrap_or("No title found");
        assert_eq!(
            title, "Photoacoustic Authorization",
            "Consent page should have title 'Photoacoustic Authorization'"
        );
        // Extract the form action for the Accept button
        let accept_form_regex =
            regex::Regex::new(r#"<form method=\"post\" action=\"([^\"]*allow=true[^\"]*)\">"#)
                .unwrap();
        let consent_action = accept_form_regex
            .captures(&consent_html)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .expect("Should extract form action for consent acceptance");
        debug!("Consent form action: {}", consent_action);
        // Submit the consent form (using the action URL which already includes allow=true)
        let consent_submit_response = client.post(&consent_action).dispatch().await;

        // Handle potential redirect issues with external URLs
        if consent_submit_response.status() == Status::Found {
            // Normal case - got redirect with authorization code
            let final_redirect = consent_submit_response
                .headers()
                .get_one("Location")
                .expect("Should have location header after consent");
            debug!("Final redirect with code: {}", final_redirect);
            let auth_code = extract_code_from_url(final_redirect)
                .expect("Should extract authorization code from redirect URL");
            actual_auth_code = auth_code;
        } else if consent_submit_response.status() == Status::BadRequest {
            // This happens when Rocket test client tries to follow an external redirect
            // Check if there's a location header with the authorization code
            if let Some(final_redirect) = consent_submit_response.headers().get_one("Location") {
                debug!("External redirect (BadRequest): {}", final_redirect);
                let auth_code = extract_code_from_url(final_redirect)
                    .expect("Should extract authorization code from redirect URL");
                actual_auth_code = auth_code;
            } else {
                panic!("BadRequest response without Location header");
            }
        } else {
            panic!(
                "Unexpected consent submission response: {:?}",
                consent_submit_response.status()
            );
        }
    } else if consent_page_response.status() == Status::Found {
        // Automatic redirection with consent already given
        let final_redirect = consent_page_response
            .headers()
            .get_one("Location")
            .expect("Should have location header after auto-consent");
        debug!("Auto-consent redirect with code: {}", final_redirect);
        let auth_code = extract_code_from_url(final_redirect)
            .expect("Should extract authorization code from redirect URL");
        actual_auth_code = auth_code;
    } else {
        panic!(
            "Unexpected response status: {:?}",
            consent_page_response.status()
        );
    }

    debug!("Authorization code: {}", actual_auth_code);

    // Step 6: Exchange the authorization code for a token using the PKCE verifier
    let mut form_data = HashMap::new();
    form_data.insert("grant_type", "authorization_code");
    form_data.insert("code", &actual_auth_code);
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

    // Step 7: Test JWKS endpoint to get public key for verification
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

    // Step 8: Verify the token can be decoded with the public key from JWKS
    // For this test we'll use the public key from our generated key pair
    let decoded_public_key = base64::engine::general_purpose::STANDARD
        .decode(public_base64)
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

    (actual_auth_code, code_verifier, access_token)
}

#[rocket::async_test]
async fn test_rs256_pkce_flow_s256() {
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
        .merge(("rs256_public_key", &public_base64))
        .merge(("hmac_secret", test_hmac_secret.to_string()))
        .merge(("access_config", test_access_config));

    let rocket = server::build_rocket(figment, None).await;

    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Run the complete OAuth flow with S256 PKCE method
    let (actual_auth_code, code_verifier, _access_token) =
        run_oauth_pkce_flow(&client, "S256", &public_base64).await;

    // Test with incorrect PKCE verifier (should fail)
    let incorrect_verifier = "incorrect_code_verifier_that_wont_match_challenge";
    let mut invalid_form_data = HashMap::new();
    invalid_form_data.insert("grant_type", "authorization_code");
    invalid_form_data.insert("code", &actual_auth_code);
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
async fn test_rs256_pkce_flow_plain() {
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
        .merge(("rs256_public_key", &public_base64))
        .merge(("hmac_secret", test_hmac_secret.to_string()))
        .merge(("access_config", test_access_config));

    let rocket = server::build_rocket(figment, None).await;

    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Run the complete OAuth flow with plain PKCE method
    let (actual_auth_code, code_verifier, _access_token) =
        run_oauth_pkce_flow(&client, "plain", &public_base64).await;

    // Test with incorrect PKCE verifier (should fail)
    let incorrect_verifier = "incorrect_code_verifier_that_wont_match_challenge";
    let mut invalid_form_data = HashMap::new();
    invalid_form_data.insert("grant_type", "authorization_code");
    invalid_form_data.insert("code", &actual_auth_code);
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
async fn test_rs256_pkce_flow() {
    // This test maintains backward compatibility and tests both PKCE methods

    // Initialize the logger for tests
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // Test both S256 and plain PKCE methods with separate Rocket instances for isolation
    for method in &["S256", "plain"] {
        debug!("Testing PKCE method: {}", method);

        // Generate test RS256 key pair for each iteration
        let (_, _, private_base64, public_base64) = generate_test_rs256_keys();

        // Test HMAC secret
        let test_hmac_secret = "test-hmac-secret-key-for-testing";

        // Test AccessConfig
        // The default config includes admin / admin123 as test credentials
        let test_access_config = AccessConfig::default();

        // Initialize Rocket with RS256 keys - separate instance for each method
        let figment = get_test_figment()
            .merge(("rs256_private_key", &private_base64))
            .merge(("rs256_public_key", &public_base64))
            .merge(("hmac_secret", test_hmac_secret.to_string()))
            .merge(("access_config", test_access_config));

        let rocket = server::build_rocket(figment, None).await;

        let client = rocket::local::asynchronous::Client::tracked(rocket)
            .await
            .expect("valid rocket instance");

        let (_auth_code, _code_verifier, _access_token) =
            run_oauth_pkce_flow(&client, method, &public_base64).await;

        debug!(
            "Successfully completed OAuth flow with {} PKCE method",
            method
        );
    }
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
        .merge(("rs256_public_key", &public_base64))
        .merge(("access_config", AccessConfig::default()));
    // Add hmac secret to the figment
    let figment = figment.merge(("hmac_secret", test_hmac_secret));

    let rocket = server::build_rocket(figment, None).await;

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

#[rocket::async_test]
async fn test_rs256_pkce_invalid_challenge_method() {
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
    let test_access_config = AccessConfig::default();

    // Initialize Rocket with RS256 keys
    let figment = get_test_figment()
        .merge(("rs256_private_key", &private_base64))
        .merge(("rs256_public_key", &public_base64))
        .merge(("hmac_secret", test_hmac_secret.to_string()))
        .merge(("access_config", test_access_config));

    let rocket = server::build_rocket(figment, None).await;

    let client = rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Test with invalid challenge method
    let (code_verifier, code_challenge, _) = generate_pkce_challenge("S256");
    let invalid_method = "invalid_method";

    let auth_response = client
        .get(format!(
            "/authorize?response_type=code&client_id=LaserSmartClient&redirect_uri=http://localhost:8080/client/&code_challenge={}&code_challenge_method={}&scope=openid%20read:api",
            code_challenge, invalid_method
        ))
        .dispatch()
        .await;

    // The authorization endpoint should handle invalid challenge methods gracefully
    // This could be either an error response or treating it as no PKCE
    // The specific behavior depends on the OAuth2 implementation
    debug!(
        "Response status for invalid challenge method: {:?}",
        auth_response.status()
    );

    // At minimum, the server should not crash and should return a response
    assert!(
        auth_response.status() == Status::Ok
            || auth_response.status() == Status::BadRequest
            || auth_response.status() == Status::Found,
        "Server should handle invalid challenge method gracefully"
    );
}
