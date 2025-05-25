// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! OpenID Connect (OIDC) implementation
//!
//! This module provides OpenID Connect (OIDC) discovery and related endpoints to make
//! the authentication system compliant with the OpenID Connect specification.
//! It includes:
//!
//! - `.well-known/openid-configuration` discovery endpoint
//! - JWKS (JSON Web Key Set) endpoint for public key exposure
//! - Helper functions for OIDC metadata generation
//!
//! These endpoints allow OAuth/OIDC clients to automatically discover the server's
//! capabilities and configuration, including supported signing algorithms and endpoints.

use base64::Engine;
use log::debug;
use rocket::serde::json::{json, Json, Value};
use rocket::{get, State};
use serde::{Deserialize, Serialize};

use super::oidc_auth::OxideState;
use super::server::ConnectionInfo;
use crate::visualization::jwt::jwt_keys::JwkKeySet;

/// OpenID Connect Discovery Configuration
///
/// This structure represents the OpenID Connect discovery document
/// returned by the `.well-known/openid-configuration` endpoint.
/// It follows the OpenID Connect Discovery 1.0 specification.
#[derive(Debug, Serialize, Deserialize)]
pub struct OpenIdConfiguration {
    /// URL using the https scheme with no query or fragment component that the OP asserts as its Issuer Identifier
    pub issuer: String,

    /// URL of the OP's OAuth 2.0 Authorization Endpoint
    pub authorization_endpoint: String,

    /// URL of the OP's OAuth 2.0 Token Endpoint
    pub token_endpoint: String,

    /// URL of the OP's JSON Web Key Set document
    pub jwks_uri: String,

    /// JSON array containing a list of the OAuth 2.0 response_type values that this server supports
    pub response_types_supported: Vec<String>,

    /// JSON array containing a list of the OAuth 2.0 Grant Type values that this server supports
    pub grant_types_supported: Vec<String>,

    /// JSON array containing a list of the Subject Identifier types that this server supports
    pub subject_types_supported: Vec<String>,

    /// JSON array containing a list of the JWS signing algorithms supported by this server for the ID Token
    pub id_token_signing_alg_values_supported: Vec<String>,

    /// JSON array containing a list of the JWS algorithms that this server supports for the UserInfo Endpoint
    pub userinfo_signing_alg_values_supported: Vec<String>,

    /// JSON array containing the scopes that this server supports
    pub scopes_supported: Vec<String>,

    /// JSON array containing a list of the claim names of the Claims that the OpenID Provider supports
    pub claims_supported: Vec<String>,
}

/// Generate OpenID Configuration based on server settings
///
/// This function creates an OpenID Configuration document based on the current
/// server configuration and state. It specifies the endpoints, supported algorithms,
/// and other capabilities of this OpenID Provider.
///
/// # Parameters
///
/// * `base_url` - The base URL of the server (e.g., "https://myserver.com")
/// * `state` - The application OAuth state
///
/// # Returns
///
/// OpenID Configuration object ready to be serialized to JSON
fn generate_openid_configuration(base_url: &str, state: &OxideState) -> OpenIdConfiguration {
    // Determine which signing algorithms are supported
    let mut signing_algs = vec!["HS256".to_string()];

    // If we have RS256 keys configured, add RS256
    log::debug!("RS256 public key length: {}", state.rs256_public_key.len());
    log::debug!(
        "RS256 private key length: {}",
        state.rs256_private_key.len()
    );

    if !state.rs256_public_key.is_empty() && !state.rs256_private_key.is_empty() {
        // Add RS256 if we have keys, regardless of whether decoding succeeds
        signing_algs.push("RS256".to_string());
        log::debug!("RS256 signing algorithm added to OpenID configuration");
    } else {
        log::warn!(
            "RS256 keys are not properly configured - public key empty: {}, private key empty: {}",
            state.rs256_public_key.is_empty(),
            state.rs256_private_key.is_empty()
        );
    }

    OpenIdConfiguration {
        issuer: base_url.to_string(),
        authorization_endpoint: format!("{}/authorize", base_url),
        token_endpoint: format!("{}/token", base_url),
        jwks_uri: format!("{}/.well-known/jwks.json", base_url),
        response_types_supported: vec!["code".to_string(), "token".to_string()],
        grant_types_supported: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        subject_types_supported: vec!["public".to_string()],
        id_token_signing_alg_values_supported: signing_algs.clone(),
        userinfo_signing_alg_values_supported: signing_algs,
        scopes_supported: vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
            "read:api".to_string(),
            "write:api".to_string(),
        ],
        claims_supported: vec![
            "sub".to_string(),
            "iss".to_string(),
            "aud".to_string(),
            "exp".to_string(),
            "iat".to_string(),
            "scope".to_string(),
        ],
    }
}

/// OpenID Connect discovery endpoint
///
/// This endpoint returns the OpenID Provider Configuration as defined by the
/// OpenID Connect Discovery specification. It provides clients with necessary
/// information to interact with this server as an OpenID Provider.
///
/// The configuration includes:
/// - URLs for authorization, token, and other endpoints
/// - Supported authentication flows and algorithms
/// - Supported claims and scopes
///
/// # URL
///
/// `GET /.well-known/openid-configuration`
///
/// # Returns
///
/// JSON object containing OpenID Connect discovery configuration
#[get("/.well-known/openid-configuration")]
pub async fn openid_configuration(
    state: &State<OxideState>,
    connection: ConnectionInfo<'_>,
) -> Json<OpenIdConfiguration> {
    let base_url = &connection.base_url;
    debug!("Base URL for OpenID configuration: {}", base_url);
    // Generate the configuration document
    let config = generate_openid_configuration(base_url, state);

    Json(config)
}

/// JSON Web Key Set (JWKS) endpoint
///
/// This endpoint exposes the public keys used for token verification
/// in JSON Web Key Set (JWKS) format as defined in RFC 7517.
/// Clients can use these keys to verify the signatures of tokens
/// issued by this server.
///
/// # URL
///
///
///
///
/// `GET /.well-known/jwks.json`
///
/// # Returns
///
/// JSON object containing the JWKS with public keys
#[get("/.well-known/jwks.json")]
pub async fn jwks(state: &State<OxideState>) -> Json<Value> {
    // Create a key set for our public keys
    let mut keys = vec![];

    // If we have an RS256 public key, add it to the key set
    if let Ok(rs256_pub_key) =
        base64::engine::general_purpose::STANDARD.decode(&state.rs256_public_key)
    {
        if !rs256_pub_key.is_empty() {
            // Parse the PEM encoded public key
            if let Ok(jwk) = JwkKeySet::create_jwk_from_pem(&rs256_pub_key) {
                keys.push(jwk);
            }
        }
    }

    // Return the key set
    Json(json!({
        "keys": keys
    }))
}
