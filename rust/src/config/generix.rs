// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Generix OAuth2 Provider Configuration
//!
//! This module defines the [`GenerixConfig`] struct, which holds all configuration parameters required for generating the OAuth2/OIDC provider configuration for the web console.
//!
//! The configuration is used to dynamically generate the `/client/generix.json` endpoint and to drive OAuth/OIDC authentication flows in the application.
//!
//! ## Example
//!
//! ```rust
//! use rust_photoacoustic::config::generix::GenerixConfig;
//! let config = GenerixConfig::default();
//! println!("OAuth provider: {}", config.provider);
//! ```
//!
//! ## Fields
//! - `provider`: Name of the OAuth2 provider (e.g., "generix").
//! - `api_base_url`: Base URL for the provider's API.
//! - `client_id`: OAuth2 client ID registered with the provider.
//! - `scope`: Space-separated list of OAuth2 scopes to request.
//! - `redirect_uri`: Redirect URI registered with the provider for OAuth2 callbacks.
//! - `audience`: Expected audience claim in JWTs.
//! - `token_issuer`: Expected issuer claim in JWTs.
//! - `jwks_endpoint`: URL to the provider's JWKS (JSON Web Key Set) endpoint.
//! - `domain`: Domain name for the provider or application.
//!
//! ## Usage
//! This struct is typically loaded from a YAML configuration file and made available via Rocket state. Use the provided request guard to access it in Rocket routes.

use rocket::{
    request::{FromRequest, Outcome},
    Request, State,
};
use rocket_okapi::{
    gen::OpenApiGenerator,
    request::{OpenApiFromRequest, RequestHeaderInput},
};
use serde::{Deserialize, Serialize};

use crate::visualization::auth::OxideState;

/// Configuration for a Generix-compatible OAuth2/OIDC provider.
///
/// This struct contains all parameters required to interact with the provider for authentication and token validation.
/// It is typically loaded from a YAML file and injected into Rocket state for use throughout the application.
#[derive(Debug, Deserialize, Serialize, Clone, rocket_okapi::JsonSchema)]
pub struct GenerixConfig {
    /// Name of the OAuth2 provider (e.g., "generix").
    pub provider: String,
    /// Base URL for the provider's API.
    pub api_base_url: String,
    /// Authority URL for the OAuth2 provider, used for token validation.
    pub authority: String,
    /// OAuth2 client ID registered with the provider.
    pub client_id: String,
    /// Space-separated list of OAuth2 scopes to request.
    pub scope: String,
    /// Redirect URI registered for OAuth2 callbacks.
    pub redirect_uri: String,
    /// Expected audience claim in JWTs.
    pub audience: String,
    /// Expected issuer claim in JWTs.
    pub token_issuer: String,
    /// URL to the provider's JWKS (JSON Web Key Set) endpoint.
    pub jwks_endpoint: String,
    /// Domain name for the provider or application.
    pub domain: String,
    /// Issuer for the OpenID Connect discovery document.
    pub issuer: String,
}

impl Default for GenerixConfig {
    fn default() -> Self {
        Self {
            provider: "generix".to_string(),
            api_base_url: "https://localhost:8080".to_string(),
            authority: "https://localhost:8080".to_string(),
            client_id: "LaserSmartClient".to_string(),
            scope: "openid email profile read:api write:api".to_string(),
            redirect_uri: "https://localhostAD:8080/client/".to_string(),
            audience: "LaserSmartClient".to_string(),
            token_issuer: "https://localhost:8080".to_string(),
            jwks_endpoint: "https://localhost:8080/.well-known/jwks.json".to_string(),
            domain: "localhost".to_string(),
            issuer: "LaserSmartServer".to_string(),
        }
    }
}

/// Rocket request guard for extracting [`GenerixConfig`] from the application state.
///
/// This guard retrieves the [`GenerixConfig`] from the [`OxideState`] managed by Rocket.
/// It allows routes to access the configuration as a request guard parameter.
///
/// # Errors
/// Returns a 500 error if the [`OxideState`] is missing from Rocket state.
#[rocket::async_trait]
impl<'r> FromRequest<'r> for GenerixConfig {
    type Error = &'static str;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match request.guard::<&State<OxideState>>().await {
            Outcome::Success(oxide_state) => Outcome::Success(oxide_state.generix_config.clone()),
            Outcome::Error((status, _)) => Outcome::Error((status, "Missing oxide state")),
            Outcome::Forward(status) => Outcome::Forward(status),
        }
    }
}

/// OpenAPI implementation for [`GenerixConfig`] request guard.
///
/// Since [`GenerixConfig`] is extracted from Rocket's managed state and doesn't require
/// any special headers or parameters, this implementation returns `RequestHeaderInput::None`.
impl<'r> OpenApiFromRequest<'r> for GenerixConfig {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        Ok(RequestHeaderInput::None)
    }
}
