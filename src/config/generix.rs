// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Configuration for the Generix OAuth2 provider
//! This configuration is used to dynamically generate the /generix.json endpoint

use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
    Request, State,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GenerixConfig {
    pub provider: String,
    pub api_base_url: String,
    pub client_id: String,
    pub scope: String,
    pub redirect_uri: String,
    pub audience: String,
    pub token_issuer: String,
    pub jwks_endpoint: String,
    pub domain: String,
}

impl Default for GenerixConfig {
    fn default() -> Self {
        Self {
            provider: "generix".to_string(),
            api_base_url: "https://localhost:8080".to_string(),
            client_id: "LaserSmartClient".to_string(),
            scope: "openid email profile read:api write:api".to_string(),
            redirect_uri: "https://localhost:8080/client/".to_string(),
            audience: "LaserSmart".to_string(),
            token_issuer: "https://localhost:8080".to_string(), // Fix the typo here
            jwks_endpoint: "https://localhost:8080/.well-known/jwks.json".to_string(),
            domain: "localhost".to_string(),
        }
    }
}

/// Request guard for accessing the GenerixConfig from the request state
#[rocket::async_trait]
impl<'r> FromRequest<'r> for GenerixConfig {
    type Error = &'static str;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match request.guard::<&State<GenerixConfig>>().await {
            Outcome::Success(config) => Outcome::Success(config.inner().clone()),
            Outcome::Error((status, _)) => Outcome::Error((status, "Missing generix config")),
            Outcome::Forward(status) => Outcome::Forward(status),
        }
    }
}
