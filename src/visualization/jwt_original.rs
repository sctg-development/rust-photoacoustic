// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! JWT token generation and management for OAuth authentication
//!
//! This module implements a JWT-based token issuer that integrates with the Oxide Auth
//! framework. It provides functionality for:
//!
//! - Creating and managing JWT access tokens
//! - Generating refresh tokens
//! - Token validation and verification
//! - OAuth 2.0 token issuance and refresh workflows
//!
//! The JWT tokens are signed using configurable algorithms (default: HS256) and include
//! standard claims like subject, audience, and expiration time.
//!
//! # Architecture
//!
//! The module consists of three main components:
//! - `JwtTokenMap`: Core implementation of token management
//! - `JwtIssuer`: Thread-safe wrapper around `JwtTokenMap` with Mutex
//! - `JwtClaims`: Structure representing the claims in a JWT token
//!
//! # Example Usage
//!
//! ```
//! use rust_photoacoustic::visualization::jwt_original::JwtIssuer;
//! use chrono::Duration;
//!
//! // Create a new JWT issuer with a secret key
//! let mut issuer = JwtIssuer::new(b"your-secret-key");
//!
//! // Configure the issuer
//! issuer
//!     .with_issuer("my-application")
//!     .valid_for(Duration::hours(2));
//!
//! // The issuer can now be used with oxide_auth to issue OAuth tokens
//! ```
//!
//! # Security Considerations
//!
//! - Use appropriate key sizes for the chosen algorithm
//! - For production, consider using RS256 with separate signing and verification keys
//! - Store secrets securely and never expose them in client-side code

use chrono::{DateTime, Duration, TimeZone, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use oxide_auth::primitives::generator::{RandomGenerator, TagGrant};
use oxide_auth::primitives::grant::{Extensions, Grant, Value};
use oxide_auth::primitives::issuer::{IssuedToken, Issuer, RefreshedToken, TokenType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use url::Url;

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
struct JwtClaims {
    /// Subject (typically user ID)
    ///
    /// Identifies the principal that is the subject of the JWT.
    /// In this application, it contains the authenticated user's ID.
    sub: String,

    /// Issued at timestamp
    ///
    /// The time at which the JWT was issued, represented as Unix time
    /// (seconds since 1970-01-01T00:00:00Z UTC).
    iat: i64,

    /// Expiration timestamp
    ///
    /// The expiration time after which the JWT must not be accepted for processing,
    /// represented as Unix time (seconds since 1970-01-01T00:00:00Z UTC).
    exp: i64,

    /// Not before timestamp (when the token becomes valid)
    ///
    /// The time before which the JWT must not be accepted for processing,
    /// represented as Unix time (seconds since 1970-01-01T00:00:00Z UTC).
    nbf: i64,

    /// JWT ID (unique identifier for the token)
    ///
    /// A unique identifier for the JWT, which can be used to prevent the JWT
    /// from being replayed (that is, to prevent attackers from reusing a JWT
    /// that they have intercepted).
    jti: String,

    /// Audience (client ID)
    ///
    /// Identifies the recipients that the JWT is intended for.
    /// In this application, it contains the OAuth client ID.
    aud: String,

    /// Issuer
    ///
    /// Identifies the principal that issued the JWT.
    /// Usually contains a string or URI that uniquely identifies the issuer.
    iss: String,

    /// Scope
    ///
    /// Space-delimited string of permissions that the token grants.
    /// This is a common extension for OAuth 2.0 access tokens.
    scope: String,

    /// Additional metadata
    ///
    /// Custom claims containing additional information about the user
    /// or authentication context. May include fields like email, name,
    /// or other user attributes.
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<HashMap<String, String>>,
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
struct IdTokenClaims {
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
    pub nickname: Option<String>,

    /// URL to the user's profile picture
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,

    /// User's email address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Whether the user's email is verified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,

    /// User's last update timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Token entry storing both access and refresh tokens
///
/// This structure represents a complete token set in the token store,
/// including the access token, optional refresh token, and associated metadata.
/// Each entry is associated with a specific OAuth grant and has a defined
/// expiration time.
///
/// Token entries are stored in the `JwtTokenMap` and are used to track
/// active tokens for validation, refreshing, and token introspection.
pub struct TokenEntry {
    /// Access token data
    ///
    /// The JWT string that the client uses to access protected resources.
    pub access_token: String,

    /// ID token for OpenID Connect
    ///
    /// Contains claims about the authentication of an End-User by an Authorization Server.
    /// May be None if not using OpenID Connect or if the scope doesn't include 'openid'.
    pub id_token: Option<String>,

    /// Optional refresh token
    ///
    /// A token that clients can use to obtain a new access token without
    /// requiring the user to be redirected. May be None if refresh tokens
    /// are not enabled or not issued for this particular grant.
    pub refresh_token: Option<String>,

    /// The grant used to create this token
    ///
    /// Contains information about the authorization grant that led to this token,
    /// including the client ID, user ID (owner), scope, and redirect URI.
    pub grant: Grant,

    /// Expiration time for the token
    ///
    /// The time at which this token will expire and no longer be valid for use.
    /// Both access and refresh tokens share the same expiration time in this implementation.
    pub expiry: DateTime<Utc>,
}

/// A custom JWT token issuer implementation
///
/// `JwtTokenMap` is the core implementation of the JWT token issuing and
/// management functionality. It maintains in-memory maps of active tokens
/// and implements the Oxide Auth `Issuer` trait to integrate with the OAuth 2.0
/// workflow.
///
/// This struct is responsible for:
/// - Creating and signing JWT tokens with appropriate claims
/// - Generating secure refresh tokens
/// - Storing and retrieving active tokens
/// - Verifying token validity
/// - Refreshing expired tokens
///
/// The implementation is not thread-safe on its own, which is why it's
/// typically wrapped in the `JwtIssuer` struct that provides thread safety
/// through a mutex.
///
/// # Token Storage
///
/// Tokens are stored in two hash maps:
/// - `access_tokens`: Maps access token strings to token entries
/// - `refresh_tokens`: Maps refresh token strings to the same token entries
///
/// This allows efficient lookup of tokens by either the access token or
/// refresh token value.
pub struct JwtTokenMap {
    /// Map of access tokens
    ///
    /// Maps access token strings to their corresponding token entries.
    /// Used for token validation and introspection.
    pub access_tokens: HashMap<String, Arc<TokenEntry>>,

    /// Map of refresh tokens
    ///
    /// Maps refresh token strings to their corresponding token entries.
    /// Used during token refresh operations.
    pub refresh_tokens: HashMap<String, Arc<TokenEntry>>,

    /// JWT signing key
    ///
    /// The key used to sign JWT tokens. For symmetric algorithms like HS256,
    /// this is derived from the secret. For asymmetric algorithms like RS256,
    /// this contains the private key.
    pub signing_key: EncodingKey,

    /// JWT verification key
    ///
    /// The key used to verify JWT signatures. For symmetric algorithms like HS256,
    /// this is derived from the same secret as the signing key. For asymmetric
    /// algorithms like RS256, this contains the public key.
    pub verification_key: DecodingKey,

    /// Random generator for refresh tokens
    ///
    /// Generates cryptographically secure random tokens for use as refresh tokens.
    pub refresh_generator: RandomGenerator,

    /// Token validity duration
    ///
    /// Specifies how long issued tokens remain valid before expiring.
    /// If None, tokens do not expire by default.
    pub token_duration: Option<Duration>,

    /// Issuer name for JWT
    ///
    /// A string identifier for the token issuer, included in the "iss" claim
    /// of generated JWT tokens.
    pub issuer: String,

    /// Counter for token generation
    ///
    /// Used to ensure unique token identifiers (JTI claim) across token generation.
    pub usage_counter: u64,

    /// JWT signing algorithm
    ///
    /// The algorithm used to sign and verify JWT tokens.
    /// Default is HS256 (HMAC with SHA-256).
    pub algorithm: Algorithm,

    /// Additional claims to include in tokens
    ///
    /// This allows adding extra claims to the JWT tokens being generated.
    /// Used for including user-specific information in tokens.
    pub claims: HashMap<String, Value>,
}

impl JwtTokenMap {
    /// Create a new JWT token issuer with the given secret key
    pub fn new(secret: &[u8]) -> Self {
        JwtTokenMap {
            access_tokens: HashMap::new(),
            refresh_tokens: HashMap::new(),
            signing_key: EncodingKey::from_secret(secret),
            verification_key: DecodingKey::from_secret(secret),
            refresh_generator: RandomGenerator::new(16),
            token_duration: Some(Duration::hours(1)), // Default 1 hour
            issuer: "rust-photoacoustic".to_string(),
            usage_counter: 0,
            algorithm: Algorithm::HS256, // Default to HMAC-SHA256
            claims: HashMap::new(),
        }
    }

    /// Create a new JWT token issuer with the given algorithm and keys
    pub fn with_keys(
        algorithm: Algorithm,
        encoding_key: EncodingKey,
        decoding_key: DecodingKey,
    ) -> Self {
        JwtTokenMap {
            access_tokens: HashMap::new(),
            refresh_tokens: HashMap::new(),
            signing_key: encoding_key,
            verification_key: decoding_key,
            refresh_generator: RandomGenerator::new(16),
            token_duration: Some(Duration::hours(1)), // Default 1 hour
            issuer: "rust-photoacoustic".to_string(),
            usage_counter: 0,
            algorithm,
            claims: HashMap::new(),
        }
    }

    /// Create a new JWT token issuer with RS256 algorithm using PEM encoded keys
    ///
    /// # Parameters
    ///
    /// * `private_key_pem` - PEM encoded private key
    /// * `public_key_pem` - PEM encoded public key
    ///
    /// # Returns
    ///
    /// A new JwtTokenMap configured to use RS256 algorithm with the provided keys
    pub fn with_rs256_pem(
        private_key_pem: &[u8],
        public_key_pem: &[u8],
    ) -> Result<Self, jsonwebtoken::errors::Error> {
        let encoding_key = EncodingKey::from_rsa_pem(private_key_pem)?;
        let decoding_key = DecodingKey::from_rsa_pem(public_key_pem)?;

        Ok(JwtTokenMap {
            access_tokens: HashMap::new(),
            refresh_tokens: HashMap::new(),
            signing_key: encoding_key,
            verification_key: decoding_key,
            refresh_generator: RandomGenerator::new(16),
            token_duration: Some(Duration::hours(1)), // Default 1 hour
            issuer: "rust-photoacoustic".to_string(),
            usage_counter: 0,
            algorithm: Algorithm::RS256,
            claims: HashMap::new(),
        })
    }

    /// Sets the JWT signing algorithm
    pub fn with_algorithm(mut self, algorithm: Algorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Sets the issuer name used in JWT claims
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = issuer.into();
        self
    }

    /// Set the validity of all issued tokens to the specified duration
    pub fn valid_for(mut self, duration: Duration) -> Self {
        self.token_duration = Some(duration);
        self
    }

    /// Add user information to token claims that will be included in the next issued token
    pub fn add_user_claims(&mut self, username: &str, permissions: &[String]) -> &mut Self {
        // Clear previous user claims to avoid accumulation
        self.claims.retain(|key, _| !key.starts_with("user_"));

        // Add user information to claims that will be included in JWT
        self.claims.insert(
            "user_id".to_string(),
            Value::public(Some(username.to_string())),
        );

        // Add permissions as a space-separated string
        let perms_str = permissions.join(" ");
        self.claims.insert(
            "user_permissions".to_string(),
            Value::public(Some(perms_str)),
        );

        // Common identity claims
        self.claims.insert(
            "preferred_username".to_string(),
            Value::public(Some(username.to_string())),
        );
        self.claims.insert(
            "user_name".to_string(),
            Value::public(Some(username.to_string())),
        );

        self
    }

    /// Create ID token claims for OpenID Connect
    ///
    /// This method generates the claims for an ID token according to the OpenID Connect specification.
    /// Unlike access tokens, ID tokens contain user profile information and authentication details.
    fn create_id_token_claims(
        &self,
        grant: &Grant,
        now: DateTime<Utc>,
        expiry: DateTime<Utc>,
    ) -> Option<IdTokenClaims> {
        // Only create ID token if 'openid' is in the scope
        if !grant
            .scope
            .to_string()
            .split_whitespace()
            .any(|s| s == "openid")
        {
            return None;
        }

        let mut name = None;
        let mut nickname = None;
        let mut picture = None;
        let mut email = None;
        let mut email_verified = None;
        let mut nonce = None;
        let mut sid = None;

        // Extract user information from grant extensions and additional claims
        for (key, value) in grant.extensions.public() {
            match key {
                "nonce" => nonce = value.clone(),
                _ => {}
            }
        }

        // Add user info from claims
        for (key, value) in &self.claims {
            if let Value::Public(Some(val)) = value {
                match key.as_str() {
                    "user_name" | "name" => name = Some(val.clone()),
                    "preferred_username" | "nickname" => nickname = Some(val.clone()),
                    "picture" => picture = Some(val.clone()),
                    "email" => email = Some(val.clone()),
                    "email_verified" => email_verified = val.parse().ok(),
                    "sid" => sid = Some(val.clone()),
                    _ => {}
                }
            }
        }

        // Generate a unique session ID if not provided
        let sid = sid.or_else(|| Some(format!("session-{}", self.usage_counter)));
        // Convert nonce to a string if it exists
        let nonce = nonce.map(|n| n.to_string());
        // Create the ID token claims
        Some(IdTokenClaims {
            sub: grant.owner_id.clone(),
            iss: self.issuer.clone(),
            aud: grant.client_id.clone(),
            iat: now.timestamp(),
            exp: expiry.timestamp(),
            auth_time: Some(now.timestamp()),
            nonce,
            sid,
            acr: Some("0".to_string()), // Basic level of authentication
            amr: Some(vec!["pwd".to_string()]), // Password authentication
            azp: Some(grant.client_id.clone()),
            name,
            nickname,
            picture,
            email,
            email_verified,
            updated_at: Some(now.to_rfc3339()),
        })
    }

    /// Create JWT claims from a grant, including any additional user claims
    fn create_claims(&self, grant: &Grant, now: DateTime<Utc>, expiry: DateTime<Utc>) -> JwtClaims {
        // Create a map for any public extensions and additional claims
        let mut metadata = HashMap::new();

        // Add grant extensions to metadata
        for (key, value) in grant.extensions.public() {
            if let Some(val) = value {
                metadata.insert(key.to_string(), val.to_string());
            } else {
                metadata.insert(key.to_string(), "true".to_string());
            }
        }

        // Add any additional claims (including user claims)
        for (key, value) in &self.claims {
            match value {
                Value::Public(Some(val)) => {
                    metadata.insert(key.to_string(), val.to_string());
                }
                Value::Public(None) => {
                    metadata.insert(key.to_string(), "true".to_string());
                }
                Value::Private(_) => {
                    // Skip private values
                }
            }
        }

        // Store the redirect URI in the metadata
        metadata.insert("redirect_uri".to_string(), grant.redirect_uri.to_string());

        // Generate a unique token ID (jti)
        let jti = format!("{}-{}", grant.client_id, self.usage_counter);

        JwtClaims {
            sub: grant.owner_id.clone(),
            iat: now.timestamp(),
            exp: expiry.timestamp(),
            nbf: now.timestamp(), // Token is valid immediately
            jti,
            aud: grant.client_id.clone(),
            iss: self.issuer.clone(),
            scope: grant.scope.to_string(),
            metadata: if metadata.is_empty() {
                None
            } else {
                Some(metadata)
            },
        }
    }
}

impl Issuer for JwtTokenMap {
    fn issue(&mut self, mut grant: Grant) -> Result<IssuedToken, ()> {
        // Set expiration if duration is specified
        let now = Utc::now();
        if let Some(duration) = self.token_duration {
            grant.until = now + duration;
        }

        // Generate claims (this now includes user claims automatically)
        let claims = self.create_claims(&grant, now, grant.until);

        // Create JWT token with specific algorithm
        let header = Header::new(self.algorithm);
        let access_token = encode(&header, &claims, &self.signing_key).map_err(|_| ())?;

        // Generate a refresh token using random generator
        self.usage_counter += 1;
        let refresh_token = self.refresh_generator.tag(self.usage_counter, &grant).ok();

        // generate ID token claims if 'openid' scope is requested
        let id_token_claims = self.create_id_token_claims(&grant, now, grant.until);
        let id_token = if let Some(claims) = id_token_claims {
            // Create ID token with specific algorithm
            let id_header = Header::new(self.algorithm);
            Some(encode(&id_header, &claims, &self.signing_key).map_err(|_| ())?)
        } else {
            None // No ID token if 'openid' scope is not requested
        };
        // Store the token
        let token_entry = Arc::new(TokenEntry {
            access_token: access_token.clone(),
            id_token: id_token.clone(),
            refresh_token: refresh_token.clone(),
            grant: grant.clone(),
            expiry: grant.until,
        });

        // Add to maps
        self.access_tokens
            .insert(access_token.clone(), Arc::clone(&token_entry));
        if let Some(ref refresh) = refresh_token {
            self.refresh_tokens
                .insert(refresh.clone(), Arc::clone(&token_entry));
        }

        // Create the token response
        let token = IssuedToken {
            token: access_token,
            refresh: refresh_token,
            until: grant.until,
            token_type: TokenType::Bearer,
            id_token,
        };

        // Clear user claims after use to prevent them from being included in subsequent tokens
        self.claims.retain(|key, _| !key.starts_with("user_"));

        Ok(token)
    }

    fn refresh(&mut self, refresh: &str, mut grant: Grant) -> Result<RefreshedToken, ()> {
        // Get the data we need from refresh_tokens without keeping a borrow
        let (old_access_token, old_refresh_token) = {
            let token_entry = self.refresh_tokens.get(refresh).ok_or(())?;

            // Verify that the grant matches
            if token_entry.grant.client_id != grant.client_id
                || token_entry.grant.owner_id != grant.owner_id
            {
                return Err(());
            }

            // Get what we need before releasing the borrow
            (
                token_entry.access_token.clone(),
                token_entry.refresh_token.clone(),
            )
        };

        // Set up a new expiration time
        let now = Utc::now();
        if let Some(duration) = self.token_duration {
            grant.until = now + duration;
        }

        // Generate new claims
        let claims = self.create_claims(&grant, now, grant.until);

        // Create new JWT token with specific algorithm
        let header = Header::new(self.algorithm);
        let new_access_token = encode(&header, &claims, &self.signing_key).map_err(|_| ())?;

        // Generate a new refresh token
        self.usage_counter += 1;
        let new_refresh_token = self.refresh_generator.tag(self.usage_counter, &grant).ok();

        // Remove the old tokens
        self.access_tokens.remove(&old_access_token);
        if let Some(ref old_refresh) = old_refresh_token {
            self.refresh_tokens.remove(old_refresh);
        }

        // Create and store the new token
        let new_token_entry = Arc::new(TokenEntry {
            access_token: new_access_token.clone(),
            id_token: None, // ID token not generated here
            refresh_token: new_refresh_token.clone(),
            grant: grant.clone(),
            expiry: grant.until,
        });

        // Add to maps
        self.access_tokens
            .insert(new_access_token.clone(), Arc::clone(&new_token_entry));
        if let Some(ref refresh) = new_refresh_token {
            self.refresh_tokens
                .insert(refresh.clone(), Arc::clone(&new_token_entry));
        }

        // Create the refreshed token
        let token = RefreshedToken {
            token: new_access_token,
            refresh: new_refresh_token,
            until: grant.until,
            token_type: TokenType::Bearer,
        };

        // Clear user claims after use
        self.claims.retain(|key, _| !key.starts_with("user_"));

        Ok(token)
    }

    fn recover_token<'a>(&'a self, token: &'a str) -> Result<Option<Grant>, ()> {
        // First try to find the token in our map
        if let Some(entry) = self.access_tokens.get(token) {
            // Check if the token has expired
            if entry.expiry < Utc::now() {
                return Ok(None);
            }
            return Ok(Some(entry.grant.clone()));
        }

        // Create custom validation
        let mut validation = Validation::new(self.algorithm);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.set_issuer(&[&self.issuer]);
        // We should extract the audience from the token first and then validate it
        // This approach is needed because we may not know the audience in advance

        let token_data = match decode::<JwtClaims>(token, &self.verification_key, &validation) {
            Ok(data) => data,
            Err(err) => {
                log::debug!("JWT validation failed: {:?}", err);
                return Ok(None);
            }
        };

        // Reconstruct grant from claims
        let exp_time = Utc
            .timestamp_opt(token_data.claims.exp, 0)
            .single()
            .ok_or(())?;

        let mut extensions = Extensions::new();
        let mut redirect_uri_str = "http://localhost:8080/client/".to_string();

        if let Some(metadata) = &token_data.claims.metadata {
            for (key, value) in metadata {
                if key == "redirect_uri" {
                    redirect_uri_str = value.clone();
                } else {
                    extensions.set_raw(key.clone(), Value::public(Some(value.clone())));
                }
            }
        }

        // Parse redirect URI
        let redirect_uri = match redirect_uri_str.parse::<Url>() {
            Ok(url) => url,
            Err(e) => {
                log::error!("Failed to parse redirect URI '{}': {}", redirect_uri_str, e);
                "http://localhost:8080/client/".parse().unwrap()
            }
        };

        let grant = Grant {
            owner_id: token_data.claims.sub,
            client_id: token_data.claims.aud,
            scope: token_data.claims.scope.parse().map_err(|e| {
                log::error!("Failed to parse scope from token: {}", e);
            })?,
            redirect_uri,
            until: exp_time,
            extensions,
        };

        Ok(Some(grant))
    }

    fn recover_refresh<'a>(&'a self, token: &'a str) -> Result<Option<Grant>, ()> {
        // Find the refresh token
        match self.refresh_tokens.get(token) {
            Some(entry) => {
                // Check if the token has expired
                if entry.expiry < Utc::now() {
                    Ok(None)
                } else {
                    Ok(Some(entry.grant.clone()))
                }
            }
            None => Ok(None),
        }
    }
}

/// Custom OAuth token response structure with OpenID Connect support
///
/// This structure extends the standard OAuth token response to include an ID token
/// as specified by OpenID Connect. It's used to return authentication and authorization
/// tokens to clients after successful token requests.
#[derive(Debug, Serialize, Deserialize)]
pub struct OidcTokenResponse {
    /// Access token for accessing protected resources
    access_token: String,

    /// Token type, usually "Bearer"
    token_type: String,

    /// Number of seconds until the access token expires
    expires_in: u64,

    /// Refresh token for obtaining new access tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,

    /// ID token containing authentication information about the user
    #[serde(skip_serializing_if = "Option::is_none")]
    id_token: Option<String>,

    /// Space-delimited list of scopes granted to the client
    scope: String,
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
            id_token,
            scope,
        }
    }
}

/// A wrapper around `Arc<Mutex<JwtTokenMap>>` that implements the Issuer trait
pub struct JwtIssuer(pub Arc<Mutex<JwtTokenMap>>);

// Implement Clone for JwtIssuer
impl Clone for JwtIssuer {
    fn clone(&self) -> Self {
        JwtIssuer(Arc::clone(&self.0))
    }
}

impl JwtIssuer {
    /// Create a new JwtIssuer with the given secret
    pub fn new(secret: &[u8]) -> Self {
        JwtIssuer(Arc::new(Mutex::new(JwtTokenMap::new(secret))))
    }

    /// Create a new JwtIssuer with RS256 algorithm using PEM encoded keys
    ///
    /// # Parameters
    ///
    /// * `private_key_pem` - PEM encoded private key
    /// * `public_key_pem` - PEM encoded public key
    ///
    /// # Returns
    ///
    /// A new JwtIssuer configured to use RS256 algorithm with the provided keys
    pub fn with_rs256_pem(
        private_key_pem: &[u8],
        public_key_pem: &[u8],
    ) -> Result<Self, jsonwebtoken::errors::Error> {
        let token_map = JwtTokenMap::with_rs256_pem(private_key_pem, public_key_pem)?;
        Ok(JwtIssuer(Arc::new(Mutex::new(token_map))))
    }

    /// Sets the JWT signing algorithm
    pub fn with_algorithm(&mut self, algorithm: Algorithm) -> &mut Self {
        // Create a closure to modify the map
        {
            let mut map_guard = self.0.lock().unwrap();
            map_guard.algorithm = algorithm;
        } // map_guard is dropped here, releasing the lock

        // Return self reference
        self
    }

    /// Sets the issuer name used in JWT claims
    pub fn with_issuer(&mut self, issuer: impl Into<String>) -> &mut Self {
        // Create a closure to modify the map
        {
            let mut map = self.0.lock().unwrap();
            map.issuer = issuer.into();
        } // map is dropped here, releasing the lock

        // Return self reference
        self
    }

    /// Set the validity duration of all issued tokens
    pub fn valid_for(&mut self, duration: Duration) -> &mut Self {
        // Create a closure to modify the map
        {
            let mut map = self.0.lock().unwrap();
            map.token_duration = Some(duration);
        } // map is dropped here, releasing the lock

        // Return self reference
        self
    }

    /// Add user information to token claims
    pub fn add_user_claims(&mut self, username: &str, permissions: &[String]) -> &mut Self {
        {
            let mut map = self.0.lock().unwrap();
            map.add_user_claims(username, permissions);
        }
        self
    }

    /// Print the decoded contents of a JWT token for debugging purposes
    /// Returns Ok if the token could be decoded, Err otherwise
    pub fn debug_token(&self, token: &str) -> Result<JwtClaims, String> {
        let map = self.map();

        let token_parts: Vec<&str> = token.split('.').collect();
        if token_parts.len() != 3 {
            return Err("Invalid JWT token format".to_string());
        }

        // Try to decode from map first
        if let Some(entry) = map.access_tokens.get(token) {
            log::debug!("Found token in map: {:?}", entry.grant);
        }

        // Try to decode as JWT
        let validation = Validation::new(map.algorithm);
        match decode::<JwtClaims>(token, &map.verification_key, &validation) {
            Ok(token_data) => {
                log::debug!("JWT Claims: {:?}", token_data.claims);

                // Format expiry time
                let exp_time = Utc
                    .timestamp_opt(token_data.claims.exp, 0)
                    .single()
                    .ok_or("Invalid expiry time")?;
                log::debug!("Token expires at: {}", exp_time);

                // Check if token is expired
                let now = Utc::now();
                if exp_time < now {
                    log::debug!("Token is EXPIRED");
                } else {
                    let remaining = exp_time.signed_duration_since(now);
                    log::debug!("Token is valid for: {} seconds", remaining.num_seconds());
                }

                Ok(token_data.claims)
            }
            Err(e) => Err(format!("Failed to decode JWT: {}", e)),
        }
    }

    /// Internal helper to get mutex guard
    fn map_mut(&mut self) -> std::sync::MutexGuard<'_, JwtTokenMap> {
        self.0.lock().unwrap()
    }

    /// Internal helper to get mutex guard for reading
    fn map(&self) -> std::sync::MutexGuard<'_, JwtTokenMap> {
        self.0.lock().unwrap()
    }
}

impl Issuer for JwtIssuer {
    fn issue(&mut self, grant: Grant) -> Result<IssuedToken, ()> {
        self.map_mut().issue(grant)
    }

    fn refresh(&mut self, refresh: &str, grant: Grant) -> Result<RefreshedToken, ()> {
        self.map_mut().refresh(refresh, grant)
    }

    fn recover_token<'a>(&'a self, token: &'a str) -> Result<Option<Grant>, ()> {
        self.map().recover_token(token)
    }

    fn recover_refresh<'a>(&'a self, token: &'a str) -> Result<Option<Grant>, ()> {
        self.map().recover_refresh(token)
    }
}

// Implement Issuer for &JwtIssuer so it can be used in OxideState::endpoint()
impl Issuer for &JwtIssuer {
    fn issue(&mut self, grant: Grant) -> Result<IssuedToken, ()> {
        // For an immutable reference, we need to use internal mutability
        let mut guard = self.0.lock().map_err(|_| ())?;
        guard.issue(grant)
    }

    fn refresh(&mut self, refresh: &str, grant: Grant) -> Result<RefreshedToken, ()> {
        // For an immutable reference, we need to use internal mutability
        let mut guard = self.0.lock().map_err(|_| ())?;
        guard.refresh(refresh, grant)
    }

    fn recover_token<'b>(&'b self, token: &'b str) -> Result<Option<Grant>, ()> {
        let guard = self.0.lock().map_err(|_| ())?;
        guard.recover_token(token)
    }

    fn recover_refresh<'b>(&'b self, token: &'b str) -> Result<Option<Grant>, ()> {
        let guard = self.0.lock().map_err(|_| ())?;
        guard.recover_refresh(token)
    }
}
