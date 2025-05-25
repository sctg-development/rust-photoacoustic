// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! OAuth token response structures
//!
//! This module contains the response structures for OAuth token requests.

use oxide_auth::primitives::issuer::IssuedToken;
use serde::{Deserialize, Serialize};
use chrono::Utc;

/// Custom OAuth token response structure with OpenID Connect support
///
/// This structure extends the standard OAuth token response to include an ID token
/// as specified by OpenID Connect. It's used to return authentication and authorization
/// tokens to clients after successful token requests.
#[derive(Debug, Serialize, Deserialize)]
pub struct OidcTokenResponse {
    /// Access token for accessing protected resources
    pub access_token: String,

    /// Token type, usually "Bearer"
    pub token_type: String,

    /// Number of seconds until the access token expires
    pub expires_in: u64,

    /// Refresh token for obtaining new access tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// ID token containing authentication information about the user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,

    /// Space-delimited list of scopes granted to the client
    pub scope: String,
}

impl OidcTokenResponse {
    /// Create a new OIDC token response from an issued token
    pub fn from_issued_token(token: &IssuedToken, id_token: Option<String>, scope: String) -> Self {
        let now = Utc::now();
        let expires_in = if token.until > now {
            (token.until - now).num_seconds() as u64
        } else {
            0
        };

        OidcTokenResponse {
            access_token: token.token.clone(),
            token_type: "Bearer".to_string(),
            expires_in,
            refresh_token: token.refresh.clone(),
            id_token: id_token.or_else(|| token.id_token.clone()),
            scope,
        }
    }
    
    /// Create a new OIDC token response with specified parameters
    pub fn new(
        access_token: String,
        token_type: String,
        expires_in: u64,
        refresh_token: Option<String>,
        id_token: Option<String>,
        scope: String,
    ) -> Self {
        OidcTokenResponse {
            access_token,
            token_type,
            expires_in,
            refresh_token,
            id_token,
            scope,
        }
    }
}
