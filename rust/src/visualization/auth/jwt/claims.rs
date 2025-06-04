// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! JWT claim structures for authentication tokens
//!
//! This module defines the claim structures used in JSON Web Tokens (JWT)
//! for both access tokens and OpenID Connect ID tokens.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Custom JWT claims structure
///
/// This structure defines the claims included in JSON Web Tokens (JWT) generated
/// by this module. It follows the standard JWT claims as defined in RFC 7519,
/// plus additional custom fields for OAuth 2.0 integration.
///
/// The structure is serialized to JSON when creating tokens and deserialized
/// when validating them. The claims provide information about the token's
/// subject (user), issuer, expiration, and granted permissions (scope).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JwtClaims {
    /// Subject (typically user ID)
    ///
    /// Identifies the principal that is the subject of the JWT.
    /// In this application, it contains the authenticated user's ID.
    pub sub: String,

    /// Issued at timestamp
    ///
    /// The time at which the JWT was issued, represented as Unix time
    /// (seconds since 1970-01-01T00:00:00Z UTC).
    pub iat: i64,

    /// Expiration timestamp
    ///
    /// The expiration time after which the JWT must not be accepted for processing,
    /// represented as Unix time (seconds since 1970-01-01T00:00:00Z UTC).
    pub exp: i64,

    /// Not before timestamp (when the token becomes valid)
    ///
    /// The time before which the JWT must not be accepted for processing,
    /// represented as Unix time (seconds since 1970-01-01T00:00:00Z UTC).
    pub nbf: i64,

    /// JWT ID (unique identifier for the token)
    ///
    /// A unique identifier for the JWT, which can be used to prevent the JWT
    /// from being replayed (that is, to prevent attackers from reusing a JWT
    /// that they have intercepted).
    pub jti: String,

    /// Audience (client ID)
    ///
    /// Identifies the recipients that the JWT is intended for.
    /// In this application, it contains the OAuth client ID.
    pub aud: String,

    /// Issuer
    ///
    /// Identifies the principal that issued the JWT.
    /// Usually contains a string or URI that uniquely identifies the issuer.
    pub iss: String,

    /// Scope
    ///
    /// Space-delimited string of permissions that the token grants.
    /// This is a common extension for OAuth 2.0 access tokens.
    pub scope: String,

    /// Permissions if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,

    /// Additional metadata
    ///
    /// Custom claims containing additional information about the user
    /// or authentication context. May include fields like email, name,
    /// or other user attributes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// OIDC ID Token claims structure
///
/// This structure defines the claims included in OpenID Connect ID Tokens.
/// It follows the OpenID Connect Core 1.0 specification for ID Token claims,
/// including both standard claims and optional user profile information.
///
/// ID tokens are specifically designed to provide authentication information
/// about the user, while access tokens are designed for authorization.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdTokenClaims {
    /// Subject identifier, a unique identifier for the user
    pub sub: String,

    /// Issuer identifier, typically the URL of the identity provider
    pub iss: String,

    /// Audience, typically the client ID of the application
    pub aud: String,

    /// Issued at time (seconds since Unix epoch)
    pub iat: i64,

    /// Expiration time (seconds since Unix epoch)
    pub exp: i64,

    /// Authentication time (seconds since Unix epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_time: Option<i64>,

    /// Nonce value used to mitigate replay attacks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,

    /// Session ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,

    /// Authentication context class reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acr: Option<String>,

    /// Authentication methods references
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amr: Option<Vec<String>>,

    /// Authorized party (the party to which the ID Token was issued)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azp: Option<String>,

    /// User's full name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// User's preferred username or nickname
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,

    /// URL to the user's profile picture
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,

    /// User's email address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Whether the user's email is verified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,

    /// Additional custom claims
    #[serde(flatten)]
    pub additional_claims: HashMap<String, String>,
}

// Implement Default for IdTokenClaims for convenience
impl Default for IdTokenClaims {
    fn default() -> Self {
        Self {
            sub: String::new(),
            iss: String::new(),
            aud: String::new(),
            iat: 0,
            exp: 0,
            auth_time: None,
            nonce: None,
            sid: None,
            acr: None,
            amr: None,
            azp: None,
            name: None,
            preferred_username: None,
            picture: None,
            email: None,
            email_verified: None,
            additional_claims: HashMap::new(),
        }
    }
}
