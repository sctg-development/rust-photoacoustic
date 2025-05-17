// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

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
#[derive(Debug, Serialize, Deserialize, Clone)]
struct JwtClaims {
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

/// Token entry storing both access and refresh tokens
struct TokenEntry {
    /// Access token data
    access_token: String,
    /// Optional refresh token
    refresh_token: Option<String>,
    /// The grant used to create this token
    grant: Grant,
    /// Expiration time for the token
    expiry: DateTime<Utc>,
}

/// A custom JWT token issuer implementation
pub struct JwtTokenMap {
    /// Map of access tokens
    access_tokens: HashMap<String, Arc<TokenEntry>>,
    /// Map of refresh tokens
    refresh_tokens: HashMap<String, Arc<TokenEntry>>,
    /// JWT signing key
    signing_key: EncodingKey,
    /// JWT verification key
    verification_key: DecodingKey,
    /// Random generator for refresh tokens
    refresh_generator: RandomGenerator,
    /// Token validity duration
    token_duration: Option<Duration>,
    /// Issuer name for JWT
    issuer: String,
    /// Counter for token generation
    usage_counter: u64,
    /// JWT signing algorithm
    algorithm: Algorithm,
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
        }
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

    /// Create JWT claims from a grant
    fn create_claims(&self, grant: &Grant, now: DateTime<Utc>, expiry: DateTime<Utc>) -> JwtClaims {
        // Create a map for any public extensions
        let mut metadata = HashMap::new();

        // Add grant extensions to metadata
        for (key, value) in grant.extensions.public() {
            if let Some(val) = value {
                metadata.insert(key.to_string(), val.to_string());
            } else {
                metadata.insert(key.to_string(), "true".to_string());
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

        // Generate claims
        let claims = self.create_claims(&grant, now, grant.until);

        // Create JWT token with specific algorithm
        let header = Header::new(self.algorithm);
        let access_token = encode(&header, &claims, &self.signing_key).map_err(|_| ())?;

        // Generate a refresh token using random generator
        self.usage_counter += 1;
        let refresh_token = self.refresh_generator.tag(self.usage_counter, &grant).ok();

        // Store the token
        let token_entry = Arc::new(TokenEntry {
            access_token: access_token.clone(),
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

        // Return the issued token
        Ok(IssuedToken {
            token: access_token,
            refresh: refresh_token,
            until: grant.until,
            token_type: TokenType::Bearer,
        })
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

        // Return the refreshed token
        Ok(RefreshedToken {
            token: new_access_token,
            refresh: new_refresh_token,
            until: grant.until,
            token_type: TokenType::Bearer,
        })
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
        // Note: we don't validate audience here since it depends on the client

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
                ()
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

/// A wrapper around Arc<Mutex<JwtTokenMap>> that implements the Issuer trait
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
impl<'a> Issuer for &'a JwtIssuer {
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
