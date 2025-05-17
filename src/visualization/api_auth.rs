// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use crate::visualization::jwt_validator::JwtValidator;
use anyhow::Result;
use rocket::serde::json::Json;
use rocket::{
    get,
    http::Status,
    request::{self, FromRequest, Request},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// JWT bearer token extractor for Rocket routes
pub struct JwtToken(pub String);

/// User information extracted from a JWT token
pub struct AuthenticatedUser {
    /// The user ID from the token
    pub user_id: String,
    /// The client ID from the token
    pub client_id: String,
    /// The scopes granted to this token
    pub scopes: Vec<String>,
    /// The raw token for passing to other services
    pub token: String,
}

/// Error type for authentication failures
#[derive(Debug)]
pub enum AuthError {
    /// No authorization header was provided
    Missing,
    /// The authorization header was invalid
    Invalid,
    /// The token was expired or otherwise invalid
    TokenInvalid(String),
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for JwtToken {
    type Error = AuthError;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        // Get authorization header
        let auth_header = match request.headers().get_one("Authorization") {
            Some(header) => header,
            None => return request::Outcome::Error((Status::Unauthorized, AuthError::Missing)),
        };

        // Check it's a bearer token
        if !auth_header.starts_with("Bearer ") {
            return request::Outcome::Error((Status::Unauthorized, AuthError::Invalid));
        }

        // Extract the token
        let token = auth_header[7..].to_string();
        if token.is_empty() {
            return request::Outcome::Error((Status::Unauthorized, AuthError::Invalid));
        }

        request::Outcome::Success(JwtToken(token))
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedUser {
    type Error = AuthError;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        // Extract token
        let token = match JwtToken::from_request(request).await {
            request::Outcome::Success(token) => token.0,
            request::Outcome::Error(failure) => return request::Outcome::Error(failure),
            request::Outcome::Forward(forward) => return request::Outcome::Forward(forward),
        };

        // Get the validator from state
        let state = request
            .rocket()
            .state::<Arc<JwtValidator>>()
            .expect("JwtValidator not configured");

        // Validate token
        let user_info = match state.get_user_info(&token) {
            Ok(info) => info,
            Err(e) => {
                return request::Outcome::Error((
                    Status::Unauthorized,
                    AuthError::TokenInvalid(e.to_string()),
                ))
            }
        };

        request::Outcome::Success(AuthenticatedUser {
            user_id: user_info.user_id,
            client_id: user_info.client_id,
            scopes: user_info.scopes.iter().map(|s| s.to_string()).collect(),
            token,
        })
    }
}

/// Required scope guard for API endpoints
pub struct RequireScope(pub &'static str);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequireScope {
    type Error = AuthError;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        // Define a fixed scope for the endpoint - in a real app, this could be stored as state
        let required_scope = "read:api"; // Default scope for API endpoints

        // Extract user
        let user = match AuthenticatedUser::from_request(request).await {
            request::Outcome::Success(user) => user,
            request::Outcome::Error(failure) => return request::Outcome::Error(failure),
            request::Outcome::Forward(forward) => return request::Outcome::Forward(forward),
        };

        // Check if the user has the required scope
        if user.scopes.iter().any(|s| s == required_scope) {
            request::Outcome::Success(RequireScope(required_scope))
        } else {
            request::Outcome::Error((
                Status::Forbidden,
                AuthError::TokenInvalid(format!("Missing required scope: {}", required_scope)),
            ))
        }
    }
}

// Data structures for API responses

/// User profile information
#[derive(Serialize, Deserialize)]
pub struct UserProfile {
    pub user_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

/// Example protected API endpoint that requires authentication
#[get("/profile", rank = 1)]
pub fn get_profile(user: AuthenticatedUser) -> Json<UserProfile> {
    Json(UserProfile {
        user_id: user.user_id,
        email: None,
        name: None,
    })
}

/// Example API endpoint that requires a specific scope
#[get("/api/data", rank = 1)]
pub fn get_data(_user: AuthenticatedUser, _scope: RequireScope) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "message": "This is protected data that requires the read:api scope",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Initialize the JWT validator for the API
pub fn init_jwt_validator() -> Result<JwtValidator> {
    // In a real application, these values should come from environment variables or config
    let secret = b"my-super-secret-jwt-key-for-photoacoustic-app";

    Ok(JwtValidator::new(secret)
        .with_issuer("rust-photoacoustic")
        .with_audience("LaserSmartClient"))
}
