// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # OAuth 2.0 Token Introspection
//!
//! This module implements RFC 7662 OAuth 2.0 Token Introspection, providing
//! functionality to validate tokens and retrieve metadata about them.
//!
//! ## Features
//!
//! * Token validation against both the OAuth issuer and JWT signatures
//! * RFC 7662 compliant introspection endpoint
//! * Support for bearer tokens
//! * Extraction of token metadata (scope, subject, expiration, etc.)
//!
//! ## Usage
//!
//! The introspection endpoint can be mounted in a Rocket application:
//!
//! ```no_run
//! use rocket::{build, routes};
//! use rust_photoacoustic::visualization::introspection::introspect;
//! use rust_photoacoustic::visualization::oidc_auth::OxideState;
//!
//! fn main() {
//!     let figment = rocket::Config::figment().merge(("hmac_secret", "your-secret".to_string()));
//!
//!     let state = OxideState::preconfigured(figment);
//!     
//!     let rocket = rocket::build()
//!         .manage(state)
//!         .mount("/oauth", routes![introspect]);
//!     
//!     // Launch the server...
//! }
//! ```
//!
//! ## References
//!
//! * [RFC 7662: OAuth 2.0 Token Introspection](https://datatracker.ietf.org/doc/html/rfc7662)

use crate::visualization::oidc_auth::OxideState;
use chrono::{TimeZone, Utc};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use oxide_auth::primitives::issuer::Issuer;
use rocket::form::Form;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::FromForm;
use rocket::{post, State};
use std::collections::HashMap;
// Déjà importé via rocket::serde

/// Local representation of JWT claims for decoding and validation
///
/// This struct mirrors the structure of the JWT claims issued by the authorization
/// server, enabling validation and extraction of token information.
///
/// # Fields
///
/// All standard JWT claims are supported, along with a scope claim for OAuth 2.0
/// scope values and optional metadata for additional custom claims.
#[derive(Debug, Deserialize)]
struct JwtClaimsLocal {
    /// Subject (typically user ID)
    sub: String,
    /// Issued at timestamp
    iat: i64,
    /// Expiration timestamp
    exp: i64,
    /// Not before timestamp (when the token becomes valid)
    nbf: i64,
    /// JWT ID (unique identifier for the token)
    jti: String,
    /// Audience (client ID)
    aud: String,
    /// Issuer
    iss: String,
    /// Scope
    scope: String,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<HashMap<String, String>>,
}

/// Token introspection request parameters
///
/// This struct represents the request parameters for the token introspection endpoint
/// as defined in RFC 7662 OAuth 2.0 Token Introspection.
///
/// # Fields
///
/// * `token` - The string value of the token to be introspected
/// * `token_type_hint` - Optional hint about the type of token (e.g., "access_token")
///
/// # References
///
/// * [RFC 7662 Section 2.1](https://datatracker.ietf.org/doc/html/rfc7662#section-2.1)
#[derive(FromForm, Deserialize)]
pub struct IntrospectionRequest {
    /// The token that the client wants to introspect
    pub token: String,
    /// Token type hint for the authorization server
    pub token_type_hint: Option<String>,
}

/// Token introspection response according to RFC 7662
///
/// This struct represents the response returned by the token introspection endpoint.
/// It includes standard fields defined in RFC 7662 and supports additional custom claims.
///
/// # Required Field
///
/// * `active` - Boolean indicating whether the token is active
///
/// # Optional Fields
///
/// All other fields are only included when the token is active.
///
/// # References
///
/// * [RFC 7662 Section 2.2](https://datatracker.ietf.org/doc/html/rfc7662#section-2.2)
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct IntrospectionResponse {
    /// Is the token active?
    pub active: bool,
    /// Scope of the token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Client ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// Username/subject
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    /// Expiration timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<i64>,
    /// Issued at timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iat: Option<i64>,
    /// Not before timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nbf: Option<i64>,
    /// Audience
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aud: Option<String>,
    /// Issuer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
    /// Token ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
    /// Token type (bearer)
    #[serde(rename = "token_type", skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    /// Additional custom claims
    #[serde(flatten, skip_serializing_if = "HashMap::is_empty")]
    pub additional_claims: HashMap<String, serde_json::Value>,
}

/// RFC 7662 OAuth 2.0 Token Introspection Endpoint
///
/// This endpoint allows clients to validate an access token and obtain information about it.
/// It first tries to validate the token using the configured OAuth issuer, then falls back
/// to JWT validation if needed.
///
/// # Endpoint
///
/// `POST /introspect`
///
/// # Request Parameters
///
/// Accepts form data with the following fields:
/// * `token` - The token to introspect (required)
/// * `token_type_hint` - Optional hint about the token type
///
/// # Response
///
/// Returns a JSON object with the standard introspection response fields.
/// The `active` field is always present; other fields are only included for active tokens.
///
/// # Authentication
///
/// In a production environment, this endpoint should be protected with appropriate
/// authentication to prevent unauthorized token introspection.
///
/// # Example Request
///
/// ```text
/// POST /introspect HTTP/1.1
/// Host: server.example.com
/// Content-Type: application/x-www-form-urlencoded
/// Accept: application/json
///
/// token=2YotnFZFEjr1zCsicMWpAA
/// ```
///
/// # Example Response for an Active Token
///
/// ```json
/// {
///   "active": true,
///   "client_id": "l238j323ds-23ij4",
///   "scope": "read write",
///   "sub": "Z5O3upPC88QrAjx00dis",
///   "exp": 1419356238,
///   "iat": 1419350238,
///   "token_type": "Bearer"
/// }
/// ```
///
/// # Example Response for an Invalid Token
///
/// ```json
/// {
///   "active": false
/// }
/// ```
///
/// # References
///
/// * [RFC 7662: OAuth 2.0 Token Introspection](https://datatracker.ietf.org/doc/html/rfc7662)
#[post("/introspect", data = "<params>")]
pub fn introspect(
    params: Form<IntrospectionRequest>,
    state: &State<OxideState>,
) -> Json<IntrospectionResponse> {
    // Default response for inactive token
    let inactive_response = IntrospectionResponse {
        active: false,
        scope: None,
        client_id: None,
        sub: None,
        exp: None,
        iat: None,
        nbf: None,
        aud: None,
        iss: None,
        jti: None,
        token_type: None,
        additional_claims: HashMap::new(),
    };

    // Get the token from the request
    let token = &params.token;

    // Lock the issuer to access it safely
    let issuer = state.issuer.lock().unwrap();

    // First try to recover the token from the issuer
    match issuer.recover_token(token) {
        Ok(Some(grant)) => {
            // Token is valid and active
            let now = Utc::now();

            // If token has expired, return inactive
            if grant.until < now {
                return Json(inactive_response);
            }

            // Create active response with available information
            let mut response = IntrospectionResponse {
                active: true,
                scope: Some(grant.scope.to_string()),
                client_id: Some(grant.client_id.clone()),
                sub: Some(grant.owner_id),
                exp: Some(grant.until.timestamp()),
                iat: None, // We don't have this from the grant
                nbf: None, // We don't have this from the grant
                aud: Some(grant.client_id.clone()),
                iss: None, // We don't have this from the grant
                jti: None, // We don't have this from the grant
                token_type: Some("Bearer".to_string()),
                additional_claims: HashMap::new(),
            };

            // Extract any additional claims from extensions
            for (key, value) in grant.extensions.public() {
                if let Some(val) = value {
                    response
                        .additional_claims
                        .insert(key.to_string(), serde_json::Value::String(val.to_string()));
                } else {
                    response
                        .additional_claims
                        .insert(key.to_string(), serde_json::Value::Bool(true));
                }
            }

            return Json(response);
        }
        Ok(None) => {
            // Token exists but is invalid or expired
            // Fall through to manual JWT validation
        }
        Err(_) => {
            // Error occurred during token recovery
            // Fall through to manual JWT validation
        }
    }

    // If we get here, try to manually validate the token as a JWT
    // Get the HMAC secret from state
    let hmac_secret = state.hmac_secret.as_bytes();
    let decoding_key = DecodingKey::from_secret(hmac_secret);
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.validate_aud = false; // Disable audience validation
    validation.required_spec_claims.clear(); // Don't require any specific claims
    validation.set_audience(&["LaserSmartClient"]);

    // Try to decode the token
    match decode::<JwtClaimsLocal>(token, &decoding_key, &validation) {
        Ok(token_data) => {
            // Successfully decoded the token
            let claims = token_data.claims;
            let now = Utc::now();
            let exp = Utc.timestamp_opt(claims.exp, 0).single();

            // Make sure the token is still valid
            if let Some(exp_time) = exp {
                if exp_time < now {
                    return Json(inactive_response);
                }
            } else {
                // Invalid expiration time
                return Json(inactive_response);
            }

            // Create active response with available information
            let response = IntrospectionResponse {
                active: true,
                scope: Some(claims.scope),
                client_id: Some(claims.aud.clone()),
                sub: Some(claims.sub),
                exp: Some(claims.exp),
                iat: Some(claims.iat),
                nbf: Some(claims.nbf),
                aud: Some(claims.aud),
                iss: Some(claims.iss),
                jti: Some(claims.jti),
                token_type: Some("Bearer".to_string()),
                additional_claims: HashMap::new(),
            };

            Json(response)
        }
        Err(_) => {
            // Failed to decode as JWT
            Json(inactive_response)
        }
    }
}
