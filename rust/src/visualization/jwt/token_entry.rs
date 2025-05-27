// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Token entry struct for managing JWT tokens
//!
//! This module defines the TokenEntry struct which represents token sets
//! (access tokens and refresh tokens) in the JWT token store.

use chrono::{DateTime, Utc};
use oxide_auth::primitives::grant::Grant;
use serde_json::Value;
use std::collections::HashMap;

use super::claims::IdTokenClaims;

/// Token entry storing both access and refresh tokens
///
/// This structure represents a complete token set in the token store,
/// including the access token, optional refresh token, and associated metadata.
/// Each entry is associated with a specific OAuth grant and has a defined
/// expiration time.
///
/// Token entries are stored in the `JwtTokenMap` and are used to track
/// active tokens for validation, refreshing, and token introspection.
#[derive(Clone)]
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

    /// ID token claims stored for reference
    pub id_token_claims: Option<IdTokenClaims>,
}

impl TokenEntry {
    /// Create a new TokenEntry
    pub fn new(
        access_token: String,
        id_token: Option<String>,
        refresh_token: Option<String>,
        grant: Grant,
        expiry: DateTime<Utc>,
        id_token_claims: Option<IdTokenClaims>,
    ) -> Self {
        Self {
            access_token,
            id_token,
            refresh_token,
            grant,
            expiry,
            id_token_claims,
        }
    }

    /// Check if the token has expired
    pub fn is_expired(&self) -> bool {
        self.expiry < Utc::now()
    }

    /// Get remaining validity time in seconds
    pub fn valid_for_seconds(&self) -> i64 {
        let now = Utc::now();
        if self.expiry <= now {
            0
        } else {
            (self.expiry - now).num_seconds()
        }
    }

    /// Get the user ID (subject) from the grant
    pub fn user_id(&self) -> &str {
        &self.grant.owner_id
    }

    /// Get the client ID from the grant
    pub fn client_id(&self) -> &str {
        &self.grant.client_id
    }

    /// Get the scope as a space-separated string
    pub fn scope(&self) -> String {
        self.grant.scope.to_string()
    }
}
