// Test file to isolate server tests

use rocket::{
    config::LogLevel, http::{ContentType, Status}, local::asynchronous::Client
};
use std::collections::HashMap;
use url::Url;
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose};
use serde_json::Value;

fn get_figment() -> rocket::figment::Figment {
    rocket::Config::figment().merge(("port", 8080))
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
    // Configure the Rocket test client
    let rocket = rust_photoacoustic::visualization::server::build_rocket(get_figment()).await;
    let client = Client::tracked(rocket).await.expect("valid rocket instance");

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
        .dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    assert!(response.content_type().unwrap().is_html());

    let body = response.into_string().await.expect("Response body");
    assert!(body.contains("Accept"), "The consent page should contain an Accept button");
    assert!(body.contains("Deny"), "The consent page should contain a Deny button");
    
    // Step 2: Simulate user consent (accept)
    println!("Simulating user consent (Accept)...");
    let consent_response = client
        .post(format!("/authorize?{}&allow=true", query_params))
        .dispatch().await;
    
    // The response should be a redirect to the callback URI with an authorization code
    assert_eq!(consent_response.status(), Status::Found);
    
    // Extract the redirect URL and authorization code
    let location_header = consent_response.headers().get_one("Location")
        .expect("Location header missing in response");
    println!("Redirect URL: {}", location_header);
    
    let params = extract_params_from_url(location_header);
    let authorization_code = params.get("code").expect("Authorization code missing");
    println!("Authorization code: {}", authorization_code);
    
    // Step 3: Exchange authorization code for tokens
    println!("Exchanging authorization code for tokens...");
    let token_body = format!(
        "grant_type=authorization_code&code={}&redirect_uri=http://localhost:8080/client/&client_id=LaserSmartClient&code_verifier={}",
        authorization_code, code_verifier
    );
    
    let token_response = client
        .post("/token")
        .header(ContentType::Form)
        .body(token_body)
        .dispatch().await;
    
    assert_eq!(token_response.status(), Status::Ok);
    
    // Verify the token response
    let token_response_body = token_response.into_string().await.expect("Token response body");
    println!("Token response: {}", token_response_body);
    
    let token_json: Value = serde_json::from_str(&token_response_body).expect("Valid JSON");
    
    // Verify that we received an access_token
    assert!(token_json.get("access_token").is_some(), "Response should contain an access_token");
    
    // Optionally check other fields like refresh_token, token_type, etc.
    if let Some(access_token) = token_json.get("access_token").and_then(Value::as_str) {
        println!("Access token: {}", access_token);
    }
    
    // The complete OAuth2 PKCE flow has been successfully tested
    println!("OAuth2 PKCE flow test completed successfully!");
}
