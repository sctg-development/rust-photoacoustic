// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! JWT issuer for thread-safe token management
//!
//! This module contains the JwtIssuer, a thread-safe wrapper around JwtTokenMap.

use chrono::{Duration, TimeZone, Utc};
use jsonwebtoken::{Algorithm, Validation};
use log;
use oxide_auth::primitives::grant::Grant;
use oxide_auth::primitives::issuer::{IssuedToken, Issuer, RefreshedToken};
use std::sync::{Arc, Mutex};

use super::claims::JwtClaims;
use super::token_map::JwtTokenMap;

/// A wrapper around `Arc<Mutex<JwtTokenMap>>` that implements the Issuer trait
pub struct JwtIssuer(pub Arc<Mutex<JwtTokenMap>>);

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
    pub fn with_rs256_pem(
        private_key_pem: &[u8],
        public_key_pem: &[u8],
    ) -> Result<Self, jsonwebtoken::errors::Error> {
        let token_map = JwtTokenMap::with_rs256_pem(private_key_pem, public_key_pem)?;
        Ok(JwtIssuer(Arc::new(Mutex::new(token_map))))
    }

    /// Sets the JWT signing algorithm
    pub fn with_algorithm(&mut self, algorithm: Algorithm) -> &mut Self {
        {
            let mut map_guard = self.0.lock().unwrap();
            map_guard.algorithm = algorithm;
        }
        self
    }

    /// Sets the issuer name used in JWT claims
    pub fn with_issuer(&mut self, issuer: impl Into<String>) -> &mut Self {
        {
            let mut map = self.0.lock().unwrap();
            map.issuer = issuer.into();
        }
        self
    }

    /// Set the validity duration of all issued tokens
    pub fn valid_for(&mut self, duration: Duration) -> &mut Self {
        {
            let mut map = self.0.lock().unwrap();
            map.token_duration = Some(duration);
        }
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
        match jsonwebtoken::decode::<JwtClaims>(token, &map.verification_key, &validation) {
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

impl Issuer for &JwtIssuer {
    fn issue(&mut self, grant: Grant) -> Result<IssuedToken, ()> {
        let mut guard = self.0.lock().map_err(|_| ())?;
        guard.issue(grant)
    }

    fn refresh(&mut self, refresh: &str, grant: Grant) -> Result<RefreshedToken, ()> {
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
