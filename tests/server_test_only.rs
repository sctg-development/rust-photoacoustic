// Test file to isolate server tests

use rocket::{
    config::LogLevel, http::{ContentType, Status}, local::asynchronous::Client
};
use std::collections::HashMap;
use url::Url;

fn get_figment() -> rocket::figment::Figment {
    rocket::Config::figment().merge(("port", 8080))
    .merge(("address", "127.0.0.1"))
    .merge(("log_level", LogLevel::Debug))
}

#[rocket::async_test]
async fn test_oauth_server_authorization_endpoint() {
    // Configuration du client de test Rocket
    let rocket = rust_photoacoustic::visualization::server::build_rocket(get_figment()).await;
    let client = Client::tracked(rocket).await.expect("valid rocket instance");

    // Test de l'endpoint d'autorisation
    let query_params = format!(
        "response_type=code&client_id=LocalClient&redirect_uri=http://localhost:8080/clientside/endpoint&scope=default-scope&state=test-state"
    );

    let response = client
        .get(format!("/authorize?{}", query_params))
        .dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    assert!(response.content_type().unwrap().is_html());

    let body = response.into_string().await.expect("Response body");
    assert!(
        body.contains("Accept"),
        "La page de consentement devrait contenir un bouton Accept"
    );
    assert!(
        body.contains("Deny"),
        "La page de consentement devrait contenir un bouton Deny"
    );
}
