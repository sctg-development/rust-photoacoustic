// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use anyhow::{anyhow, Result};
use chrono::{DateTime, TimeZone, Utc};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Custom JWT claims structure matching the one in jwt.rs
#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (typically user ID)
    pub sub: String,
    /// Issued at timestamp
    pub iat: i64,
    /// Expiration timestamp
    pub exp: i64,
    /// Not before timestamp (when the token becomes valid)
    pub nbf: i64,
    /// JWT ID (unique identifier for the token)
    pub jti: String,
    /// Audience (client ID)
    pub aud: String,
    /// Issuer
    pub iss: String,
    /// Scope
    pub scope: String,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// A validator for JWT tokens
pub struct JwtValidator {
    verification_key: DecodingKey,
    algorithm: Algorithm,
    expected_issuer: Option<String>,
    expected_audience: Option<String>,
}

impl JwtValidator {
    /// Create a new JwtValidator with the given secret
    pub fn new(secret: &[u8]) -> Self {
        JwtValidator {
            verification_key: DecodingKey::from_secret(secret),
            algorithm: Algorithm::HS256,
            expected_issuer: None,
            expected_audience: None,
        }
    }

    /// Set the expected issuer name
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.expected_issuer = Some(issuer.into());
        self
    }

    /// Set the expected audience
    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.expected_audience = Some(audience.into());
        self
    }

    /// Set the JWT algorithm
    pub fn with_algorithm(mut self, algorithm: Algorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Validate a JWT token and return the decoded claims
    pub fn validate(&self, token: &str) -> Result<JwtClaims> {
        // Create validation criteria
        let mut validation = Validation::new(self.algorithm);
        validation.validate_exp = true;
        validation.validate_nbf = true;

        // Set expected issuer if provided
        if let Some(ref issuer) = self.expected_issuer {
            validation.set_issuer(&[issuer]);
        }

        // Set expected audience if provided
        if let Some(ref aud) = self.expected_audience {
            validation.set_audience(&[aud]);
        }

        // Decode the token
        let token_data = decode::<JwtClaims>(token, &self.verification_key, &validation)
            .map_err(|e| anyhow!("JWT validation failed: {}", e))?;

        // Check token expiration separately (even though the library does this)
        let now = Utc::now();
        let exp_time = Utc
            .timestamp_opt(token_data.claims.exp, 0)
            .single()
            .ok_or_else(|| anyhow!("Invalid expiry time in token"))?;

        if exp_time < now {
            return Err(anyhow!("Token has expired"));
        }

        // Return claims
        Ok(token_data.claims)
    }

    /// Extract user information from a validated token
    pub fn get_user_info(&self, token: &str) -> Result<UserInfo> {
        let claims = self.validate(token)?;

        let scopes: Vec<String> = claims.scope.split_whitespace().map(String::from).collect();

        // Extract additional information from metadata if available
        let mut email = None;
        let mut name = None;

        if let Some(metadata) = &claims.metadata {
            if let Some(email_val) = metadata.get("email") {
                email = Some(email_val.clone());
            }
            if let Some(name_val) = metadata.get("name") {
                name = Some(name_val.clone());
            }
        }

        Ok(UserInfo {
            user_id: claims.sub,
            client_id: claims.aud,
            scopes,
            email,
            name,
            token_id: claims.jti,
            issued_at: Utc
                .timestamp_opt(claims.iat, 0)
                .single()
                .ok_or_else(|| anyhow!("Invalid issued at time in token"))?,
            expiry: Utc
                .timestamp_opt(claims.exp, 0)
                .single()
                .ok_or_else(|| anyhow!("Invalid expiry time in token"))?,
        })
    }
}

/// User information extracted from a JWT token
#[derive(Debug)]
pub struct UserInfo {
    /// User ID from the subject claim
    pub user_id: String,
    /// Client ID from the audience claim
    pub client_id: String,
    /// Scopes granted to this token
    pub scopes: Vec<String>,
    /// Optional email address
    pub email: Option<String>,
    /// Optional user name
    pub name: Option<String>,
    /// Unique token identifier
    pub token_id: String,
    /// When the token was issued
    pub issued_at: DateTime<Utc>,
    /// When the token expires
    pub expiry: DateTime<Utc>,
}

impl UserInfo {
    /// Check if the token has a specific scope
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Get the remaining validity time in seconds
    pub fn validity_remaining_secs(&self) -> i64 {
        let now = Utc::now();
        if self.expiry < now {
            0
        } else {
            self.expiry.signed_duration_since(now).num_seconds()
        }
    }
}
