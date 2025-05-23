// Test file to isolate server tests

use base64::{engine::general_purpose, Engine as _};
use rocket::{
    config::LogLevel,
    http::{ContentType, Status},
    local::asynchronous::Client,
};
use rust_photoacoustic::config::AccessConfig;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use url::Url;

fn get_figment() -> rocket::figment::Figment {
    rocket::Config::figment()
        .merge(("port", 8080))
        .merge(("address", "127.0.0.1"))
        .merge(("log_level", LogLevel::Debug))
}

// Function to generate a PKCE code verifier and its code challenge
fn generate_pkce_pair() -> (String, String) {
    // Generate a random code verifier of 128 bytes
    let code_verifier: String = (0..128)
        .map(|_| {
            let idx = rand::random::<u8>() % 62;
            match idx {
                0..=9 => (b'0' + idx) as char,
                10..=35 => (b'a' + (idx - 10)) as char,
                _ => (b'A' + (idx - 36)) as char,
            }
        })
        .collect();

    // Generate the code challenge by applying SHA-256 then Base64URL encoding
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hashed = hasher.finalize();
    let code_challenge = general_purpose::URL_SAFE_NO_PAD.encode(hashed);

    (code_verifier, code_challenge)
}

// Function to extract parameters from a URL
fn extract_params_from_url(url: &str) -> HashMap<String, String> {
    let parsed_url = Url::parse(url).expect("Valid URL");
    let mut params = HashMap::new();

    for (key, value) in parsed_url.query_pairs() {
        params.insert(key.to_string(), value.to_string());
    }

    params
}

#[rocket::async_test]
async fn test_oauth2_pkce_flow() {
    // Initialize the logger for tests
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // Test HMAC secret
    let test_hmac_secret = "test-hmac-secret-key-for-testing";

    // Test AccessConfig with default admin user
    let test_access_config = AccessConfig::default();

    // Configure the Rocket test client with additional configuration
    let figment = get_figment()
        .merge(("shutdown.ctrlc", false))
        .merge(("shutdown.grace", 1))
        .merge(("shutdown.mercy", 1))
        .merge(("shutdown.force", true))
        .merge(("hmac_secret", test_hmac_secret))
        .merge(("access", test_access_config)); // Add access config

    let rocket = rust_photoacoustic::visualization::server::build_rocket(figment).await;
    let client = Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Generate PKCE pair
    let (code_verifier, code_challenge) = generate_pkce_pair();

    println!("Code Verifier: {}", code_verifier);
    println!("Code Challenge: {}", code_challenge);

    // Step 1: Authorization request with PKCE
    let query_params = format!(
        "response_type=code&response_mode=query&client_id=LaserSmartClient&redirect_uri=http://localhost:8080/client/&scope=openid+profile+email+read:api+write:api&state=test-state&audience=https://myname.local&code_challenge={}&code_challenge_method=S256",
        code_challenge
    );

    println!("Sending authorization request...");
    let response = client
        .get(format!("/authorize?{}", query_params))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    assert!(response.content_type().unwrap().is_html());

    let body = response.into_string().await.expect("Response body");

    // Should now receive a login form instead of consent form
    assert!(
        body.contains("Username"),
        "The login page should contain a Username field"
    );
    assert!(
        body.contains("Password"),
        "The login page should contain a Password field"
    );
    assert!(
        body.contains("Login"),
        "The login page should contain a Login button"
    );

    // Step 2: Extract form action and submit login credentials
    println!("Submitting login credentials...");

    // Submit login form with default admin credentials
    let mut form_data = HashMap::new();
    form_data.insert("username", "admin");
    form_data.insert("password", "admin123");
    form_data.insert("response_type", "code");
    form_data.insert("client_id", "LaserSmartClient");
    form_data.insert("redirect_uri", "http://localhost:8080/client/");
    form_data.insert("scope", "openid profile email read:api write:api");
    form_data.insert("state", "test-state");

    let login_response = client
        .post("/login")
        .header(ContentType::Form)
        .body(serde_urlencoded::to_string(&form_data).unwrap())
        .dispatch()
        .await;

    // Should redirect back to /authorize with session established
    assert_eq!(login_response.status(), Status::Found);

    let redirect_location = login_response
        .headers()
        .get_one("Location")
        .expect("Should have location header after login");

    println!("Login redirect location: {}", redirect_location);

    // Step 3: Follow redirect to get consent page
    let consent_response = client.get(redirect_location).dispatch().await;

    assert_eq!(consent_response.status(), Status::Ok);
    assert!(consent_response.content_type().unwrap().is_html());

    let consent_body = consent_response
        .into_string()
        .await
        .expect("Consent page body");
    assert!(
        consent_body.contains("Accept"),
        "The consent page should contain an Accept button"
    );
    assert!(
        consent_body.contains("Deny"),
        "The consent page should contain a Deny button"
    );

    // Step 4: Extract consent form action and simulate user consent (accept)
    println!("Simulating user consent (Accept)...");

    // Extract the form action for the Accept button
    let accept_form_regex =
        regex::Regex::new(r#"<form method="post" action="([^"]*allow=true[^"]*)">"#).unwrap();
    let consent_action = accept_form_regex
        .captures(&consent_body)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .expect("Should extract form action for consent acceptance");

    println!("Consent form action: {}", consent_action);

    // Submit the consent form
    let consent_submit_response = client.post(&consent_action).dispatch().await;

    // The response should be a redirect to the callback URI with an authorization code
    assert_eq!(consent_submit_response.status(), Status::Found);

    // Extract the redirect URL and authorization code
    let location_header = consent_submit_response
        .headers()
        .get_one("Location")
        .expect("Location header missing in response");
    println!("Final redirect URL: {}", location_header);

    let params = extract_params_from_url(location_header);
    let authorization_code = params.get("code").expect("Authorization code missing");
    println!("Authorization code: {}", authorization_code);

    // Step 5: Exchange authorization code for tokens
    println!("Exchanging authorization code for tokens...");
    let token_body = format!(
        "grant_type=authorization_code&code={}&redirect_uri=http://localhost:8080/client/&client_id=LaserSmartClient&code_verifier={}&code_challenge_method=S256",
        authorization_code, code_verifier
    );

    let token_response = client
        .post("/token")
        .header(ContentType::Form)
        .body(token_body)
        .dispatch()
        .await;

    assert_eq!(token_response.status(), Status::Ok);

    // Verify the token response
    let token_response_body = token_response
        .into_string()
        .await
        .expect("Token response body");
    println!("Token response: {}", token_response_body);

    let token_json: Value = serde_json::from_str(&token_response_body).expect("Valid JSON");

    // Verify that we received an access_token
    assert!(
        token_json.get("access_token").is_some(),
        "Response should contain an access_token"
    );

    // Note: The token_type case is not standardized, but our implementation uses lowercase
    assert_eq!(
        token_json
            .get("token_type")
            .and_then(Value::as_str)
            .map(|s| s.to_lowercase()),
        Some("bearer".to_lowercase()),
        "Token type should be Bearer (case insensitive)"
    );

    // Check that the token is a valid JWT (should have 3 parts separated by dots)
    if let Some(access_token) = token_json.get("access_token").and_then(Value::as_str) {
        println!("Access token: {}", access_token);
        println!("HS256 signing key: {}", test_hmac_secret);
        let token_parts: Vec<&str> = access_token.split('.').collect();
        assert_eq!(
            token_parts.len(),
            3,
            "Access token should be a valid JWT with three parts"
        );

        // Verify the token can be decoded as base64
        let header_bytes = general_purpose::URL_SAFE_NO_PAD
            .decode(token_parts[0])
            .expect("Header should be valid base64");
        let header_json = String::from_utf8_lossy(&header_bytes);
        println!("JWT Header: {}", header_json);
        assert!(
            header_json.contains("alg"),
            "JWT header should contain algorithm"
        );

        // We could decode the payload too, but this is enough to verify it's a JWT
    }

    // Step 6: Test with invalid credentials (should fail)
    println!("Testing with invalid credentials...");

    // Make a new authorization request
    let invalid_auth_response = client
        .get(format!("/authorize?{}", query_params))
        .dispatch()
        .await;

    assert_eq!(invalid_auth_response.status(), Status::Ok);

    // Try to login with invalid credentials
    let mut invalid_form_data = HashMap::new();
    invalid_form_data.insert("username", "admin");
    invalid_form_data.insert("password", "wrongpassword");
    invalid_form_data.insert("response_type", "code");
    invalid_form_data.insert("client_id", "LaserSmartClient");
    invalid_form_data.insert("redirect_uri", "http://localhost:8080/client/");

    let invalid_login_response = client
        .post("/login")
        .header(ContentType::Form)
        .body(serde_urlencoded::to_string(&invalid_form_data).unwrap())
        .dispatch()
        .await;

    // Should return unauthorized status or redirect back to login
    assert!(
        invalid_login_response.status() == Status::Unauthorized
            || invalid_login_response.status() == Status::Ok, // Might return OK with error message
        "Invalid credentials should be rejected"
    );

    // The complete OAuth2 PKCE flow has been successfully tested
    println!("OAuth2 PKCE flow test with authentication completed successfully!");
}
