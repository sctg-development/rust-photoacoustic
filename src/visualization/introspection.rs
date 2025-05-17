// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use rocket::FromForm;
use rocket::{post, State, http::Status};
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::form::Form;
use chrono::{TimeZone, Utc};
use std::collections::HashMap;
use crate::visualization::oxide_auth::OxideState;
use crate::visualization::jwt_validator::JwtValidator;
use oxide_auth::primitives::issuer::Issuer;
use jsonwebtoken::{decode, Validation, DecodingKey, Algorithm};
// Déjà importé via rocket::serde

/// Copie de la structure JwtClaims pour le décodage local
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

/// Token introspection request
#[derive(FromForm, Deserialize)]
pub struct IntrospectionRequest {
    /// The token that the client wants to introspect
    pub token: String,
    /// Token type hint for the authorization server
    pub token_type_hint: Option<String>,
}

/// Token introspection response as per RFC 7662
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
/// This allows clients to validate an access token and get information about it.
/// 
/// This is particularly useful for resource servers that need to validate tokens.
#[post("/introspect", data = "<params>")]
pub fn introspect(
    params: Form<IntrospectionRequest>,
    state: &State<OxideState>
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
                    response.additional_claims.insert(key.to_string(), serde_json::Value::String(val.to_string()));
                } else {
                    response.additional_claims.insert(key.to_string(), serde_json::Value::Bool(true));
                }
            }
            
            return Json(response);
        },
        Ok(None) => {
            // Token exists but is invalid or expired
            // Fall through to manual JWT validation
        },
        Err(_) => {
            // Error occurred during token recovery
            // Fall through to manual JWT validation
        }
    }
    
    // If we get here, try to manually validate the token as a JWT
    let secret = b"my-super-secret-jwt-key-for-photoacoustic-app"; // Use the same secret as in OxideState::preconfigured
    let decoding_key = DecodingKey::from_secret(secret);
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
                aud: Some(claims.aud), // Also set aud directly
                iss: Some(claims.iss),
                jti: Some(claims.jti),
                token_type: Some("Bearer".to_string()),
                additional_claims: HashMap::new(),
            };
            
            return Json(response);
        },
        Err(_) => {
            // Failed to decode as JWT
            return Json(inactive_response);
        }
    }
}

