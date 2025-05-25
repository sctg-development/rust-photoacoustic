// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! JWT token map for managing OAuth tokens
//!
//! This module contains the core implementation of the JWT token store.

use chrono::{DateTime, Duration, TimeZone, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use oxide_auth::primitives::generator::{RandomGenerator, TagGrant};
use oxide_auth::primitives::grant::{Extensions, Grant, Value};
use oxide_auth::primitives::issuer::{IssuedToken, Issuer, RefreshedToken, TokenType};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use std::vec;
use url::Url;

use crate::config;

use super::claims::{IdTokenClaims, JwtClaims};
use super::token_entry::TokenEntry;

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
        let mut additional_claims = HashMap::new();

        // Add any extra claims that weren't explicitly handled
        for (key, value) in &self.claims {
            if !matches!(
                key.as_str(),
                "user_name"
                    | "name"
                    | "preferred_username"
                    | "nickname"
                    | "picture"
                    | "email"
                    | "email_verified"
                    | "sid"
                    | "user_id"
                    | "user_permissions"
            ) {
                if let Value::Public(Some(val)) = value {
                    additional_claims.insert(key.clone(), val.clone());
                }
            }
        }

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
            preferred_username: nickname,
            picture,
            email,
            email_verified,
            additional_claims,
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
            // TODO: Add user permissions from config::AccessConfig
            permissions: Some(vec![
                "read:api".to_string(),
                "admin:api".to_string(),
                "write:api".to_string(),
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ]), // Example permissions
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
        let id_token = if let Some(claims) = &id_token_claims {
            // Create ID token with specific algorithm
            let id_header = Header::new(self.algorithm);
            Some(encode(&id_header, claims, &self.signing_key).map_err(|_| ())?)
        } else {
            None // No ID token if 'openid' scope is not requested
        };

        // Store the token
        let token_entry = Arc::new(TokenEntry::new(
            access_token.clone(),
            id_token.clone(),
            refresh_token.clone(),
            grant.clone(),
            grant.until,
            id_token_claims,
        ));

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
        let new_token_entry = Arc::new(TokenEntry::new(
            new_access_token.clone(),
            None, // ID token not generated here
            new_refresh_token.clone(),
            grant.clone(),
            grant.until,
            None,
        ));

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

    fn recover_token(&self, token: &str) -> Result<Option<Grant>, ()> {
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

    fn recover_refresh(&self, token: &str) -> Result<Option<Grant>, ()> {
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
