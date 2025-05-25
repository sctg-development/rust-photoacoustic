// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! JWT token generation and management extension for ID Tokens
//!
//! This implementation adds support for retrieving ID tokens for OpenID Connect
//! functionality from the JwtTokenMap object.

use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};

/// Extension trait for JwtIssuer to support ID tokens
pub trait JwtIssuerExtension {
    /// Get the ID token for a given access token
    fn get_id_token_for_access_token(&self, access_token: &str) -> Result<Option<String>>;
}

/// Implementation of JwtIssuerExtension for the JwtIssuer struct
impl JwtIssuerExtension for crate::visualization::jwt_original::JwtIssuer {
    /// Get the ID token associated with an access token
    ///
    /// This method looks up the access token in the token map and returns
    /// the associated ID token if one exists.
    ///
    /// # Parameters
    ///
    /// * `access_token` - The access token to look up
    ///
    /// # Returns
    ///
    /// * `Result<Option<String>>` - The ID token if found, None if not found
    fn get_id_token_for_access_token(&self, access_token: &str) -> Result<Option<String>> {
        let map = self
            .0
            .lock()
            .map_err(|e| anyhow!("Failed to acquire lock: {}", e))?;

        // Look up the token entry in the access tokens map
        if let Some(entry) = map.access_tokens.get(access_token) {
            Ok(entry.id_token.clone())
        } else {
            // Access token not found
            Ok(None)
        }
    }
}
