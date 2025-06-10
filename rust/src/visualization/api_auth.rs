// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # API Authentication
//!
//! This module provides JWT-based authentication for the photoacoustic visualization API.
//! It includes request guards for Rocket that extract and validate JWT tokens from
//! incoming HTTP requests, as well as scope-based authorization.
//!
//! ## Features
//!
//! * JWT bearer token validation
//! * User information extraction from tokens
//! * Scope-based authorization for API endpoints
//! * Custom request guards for Rocket framework
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use rocket::{launch, routes};
//! use std::sync::Arc;
//! use rust_photoacoustic::visualization::api_auth::{init_jwt_validator, get_profile, get_data};
//! use rust_photoacoustic::config::AccessConfig;
//!
//! #[launch]
//! fn rocket() -> _ {
//!     // Initialize JWT validator with a secret
//!     let jwt_validator = Arc::new(init_jwt_validator("your-hmac-secret",None, AccessConfig::default()).expect("JWT init failed"));
//!
//!     rocket::build()
//!         .manage(jwt_validator)
//!         .mount("/", routes![get_profile, get_data])
//! }
//! ```

use crate::config::AccessConfig;
use crate::visualization::auth::jwt::JwtValidator;
use anyhow::Result;
use rocket::serde::json::Json;
use rocket::{
    get,
    http::Status,
    request::{self, FromRequest, Request},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::server::get_config_from_request;

/// JWT bearer token extractor for Rocket routes
///
/// This request guard extracts the JWT token from the Authorization header
/// of incoming HTTP requests. It validates that the header is present and
/// properly formatted as a Bearer token.
///
/// # Usage
///
/// ```rust,no_run
/// use rocket::get;
/// use rocket::serde::json::Json;
/// use rust_photoacoustic::visualization::api_auth::JwtToken;
///
/// #[get("/token-info")]
/// fn token_info(token: JwtToken) -> Json<String> {
///     Json(format!("Your token is: {}", token.0))
/// }
/// ```
///
/// # Authentication Process
///
/// 1. Checks for the presence of an Authorization header
/// 2. Validates that the header starts with "Bearer "
/// 3. Extracts the token string after the "Bearer " prefix
///
/// If any validation fails, the request is rejected with a 401 Unauthorized status.
pub struct JwtToken(pub String);

/// User information extracted from a JWT token
///
/// This struct represents an authenticated user after a successful JWT token validation.
/// It contains user identity information and authorization details extracted from the token.
///
/// # Usage
///
/// ```rust,no_run
/// use rocket::get;
/// use rocket::serde::json::Json;
/// use rust_photoacoustic::visualization::api_auth::AuthenticatedUser;
///
/// #[get("/user-info")]
/// fn user_info(user: AuthenticatedUser) -> Json<String> {
///     Json(format!("Hello user {}! You are using client {}",
///         user.user_id, user.client_id))
/// }
/// ```
///
/// This guard automatically validates the JWT token and only succeeds if the token
/// is valid according to the configured `JwtValidator`.
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
///
/// This enum represents the different types of authentication errors that can occur
/// when validating a JWT token in an HTTP request.
#[derive(Debug)]
pub enum AuthError {
    /// No authorization header was provided
    Missing,
    /// The authorization header was invalid
    Invalid,
    /// The token was expired or otherwise invalid
    TokenInvalid(String),
}

/// Implementation of the Rocket FromRequest trait for JwtToken
///
/// This implementation enables the JwtToken struct to be used as a request guard in
/// Rocket route handlers. It extracts and validates the JWT token from the
/// Authorization header.
#[rocket::async_trait]
impl<'r> FromRequest<'r> for JwtToken {
    type Error = AuthError;

    /// Extracts a JWT token from the request's Authorization header
    ///
    /// # Returns
    ///
    /// * `Outcome::Success(JwtToken)` - If a valid bearer token was found
    /// * `Outcome::Error((Status::Unauthorized, AuthError))` - If authentication failed
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

/// Implementation of the Rocket FromRequest trait for AuthenticatedUser
///
/// This implementation enables the AuthenticatedUser struct to be used as a request guard
/// in Rocket route handlers. It extracts and validates the JWT token, then extracts
/// user information from the validated token.
#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedUser {
    type Error = AuthError;

    /// Extracts and validates a JWT token, then extracts user information
    ///
    /// # Returns
    ///
    /// * `Outcome::Success(AuthenticatedUser)` - If authentication succeeded
    /// * `Outcome::Error((Status::Unauthorized, AuthError))` - If authentication failed
    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        // Extract token
        let token = match JwtToken::from_request(request).await {
            request::Outcome::Success(token) => token.0,
            request::Outcome::Error(failure) => return request::Outcome::Error(failure),
            request::Outcome::Forward(forward) => return request::Outcome::Forward(forward),
        };

        let access_config = get_config_from_request(request);
        // Get the validator from state
        let state = request
            .rocket()
            .state::<Arc<JwtValidator>>()
            .expect("JwtValidator not configured");

        // Validate token
        let user_info = match state.get_user_info(&token, access_config) {
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
///
/// This request guard enforces that the authenticated user has a specific scope.
/// It should be used in combination with the `AuthenticatedUser` guard to first
/// authenticate the user and then verify they have the necessary authorization.
///
/// # Usage
///
/// ```rust,no_run
/// use rocket::get;
/// use rocket::serde::json::Json;
/// use rust_photoacoustic::visualization::api_auth::{AuthenticatedUser, RequireScope};
///
/// #[get("/admin-only")]
/// fn admin_endpoint(user: AuthenticatedUser, _scope: RequireScope) -> Json<&'static str> {
///     Json("You have access to the admin endpoint!")
/// }
/// ```
pub struct RequireScope(pub &'static str);

/// Implementation of the Rocket FromRequest trait for RequireScope
///
/// This implementation enables the RequireScope struct to be used as a request guard
/// in Rocket route handlers. It validates that the authenticated user has the required scope.
#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequireScope {
    type Error = AuthError;

    /// Checks if the authenticated user has the required scope
    ///
    /// # Returns
    ///
    /// * `Outcome::Success(RequireScope)` - If the user has the required scope
    /// * `Outcome::Error((Status::Forbidden, AuthError))` - If the user lacks the required scope
    /// * `Outcome::Error((Status::Unauthorized, AuthError))` - If authentication failed
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
///
/// This struct represents a user's profile information that can be returned
/// by the API. It contains user identity and profile details.
#[derive(Serialize, Deserialize)]
pub struct UserProfile {
    /// Unique identifier for the user
    pub user_id: String,
    /// User's email address (if available)
    pub email: Option<String>,
    /// User's display name (if available)
    pub name: Option<String>,
}

/// Example protected API endpoint that requires authentication
///
/// This endpoint returns the authenticated user's profile information.
/// It demonstrates how to use the `AuthenticatedUser` request guard.
///
/// # Authentication
///
/// Requires a valid JWT token in the Authorization header.
///
/// # Returns
///
/// Returns a JSON object containing the user's profile information.
#[get("/api/profile", rank = 1)]
pub fn get_profile(user: AuthenticatedUser) -> Json<UserProfile> {
    Json(UserProfile {
        user_id: user.user_id,
        email: None,
        name: None,
    })
}

/// Example API endpoint that requires a specific scope
///
/// This endpoint returns some protected data that requires both authentication
/// and a specific scope authorization. It demonstrates how to use both the
/// `AuthenticatedUser` and `RequireScope` request guards together.
///
/// # Authentication
///
/// Requires a valid JWT token in the Authorization header with the "read:api" scope.
///
/// # Returns
///
/// Returns a JSON object containing the protected data.
#[get("/api/data", rank = 1)]
pub fn get_data(_user: AuthenticatedUser, _scope: RequireScope) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "message": "This is protected data that requires the read:api scope",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Initialize the JWT validator for the API
///
/// This function creates and configures a `JwtValidator` instance with the provided
/// HMAC secret or RS256 public key. The validator is configured with a fixed issuer
/// and audience.
///
/// # Arguments
///
/// * `hmac_secret` - The HMAC secret key used to verify JWT signatures
/// * `rs256_public_key` - Optional RS256 public key in PEM format for verifying signatures
///
/// # Returns
///
/// * `Result<JwtValidator>` - A configured JWT validator if successful
///
/// # Errors
///
/// Returns an error if the JWT validator cannot be initialized.
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
/// use rust_photoacoustic::visualization::api_auth::init_jwt_validator;
/// use rust_photoacoustic::config::AccessConfig;
///
/// let public_key_bytes = b"-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----";
/// // Using HMAC
/// let jwt_validator = init_jwt_validator("your-secret-key", None, AccessConfig::default())
///     .expect("Failed to initialize JWT validator");
///     
/// // Using RS256
/// let jwt_validator = init_jwt_validator("fallback-secret", Some(&public_key_bytes.as_ref()), AccessConfig::default())
///     .expect("Failed to initialize JWT validator");
///     
/// let validator_arc = Arc::new(jwt_validator);
/// ```
pub fn init_jwt_validator(
    hmac_secret: &str,
    rs256_public_key: Option<&[u8]>,
    access_config: AccessConfig,
) -> Result<JwtValidator> {
    // Support both keys if both are provided
    let hmac_opt = if !hmac_secret.is_empty() {
        Some(hmac_secret.as_bytes())
    } else {
        None
    };
    let validator = JwtValidator::new(hmac_opt, rs256_public_key, access_config.clone())
        .map_err(|e| anyhow::anyhow!("Failed to create JWT validator: {}", e))?;
    Ok(validator
        .with_issuer(
            access_config
                .iss
                .unwrap_or("LaserSmartServer".to_string())
                .clone(),
        )
        .with_audience("LaserSmartClient"))
}
