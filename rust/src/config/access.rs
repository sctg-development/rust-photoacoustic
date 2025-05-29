// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! User access and permissions configuration
//!
//! This module defines the structures for managing users, OAuth clients,
//! and their respective permissions within the application.

use crate::visualization::auth::OxideState;

use rocket::{
    request::{FromRequest, Outcome},
    Request, State,
};
use serde::{Deserialize, Serialize};

/// OAuth2 client configuration for authorization code flow
///
/// This structure represents an OAuth2 client that is allowed to use
/// the authorization code flow with this server.
///
/// # Fields
///
/// * `client_id` - The unique identifier for the OAuth2 client
/// * `allowed_callbacks` - List of URLs that this client is allowed to redirect to
///
/// # Example
///
/// ```
/// use rust_photoacoustic::config::access::Client;
///
/// let client = Client {
///     client_id: "LaserSmartClient".to_string(),
///     default_scope: "openid profile email read:api write:api".to_string(),
///     allowed_callbacks: vec![
///         "http://localhost:8080/client/".to_string(),
///         "https://localhost:8080/client/".to_string(),
///     ],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    /// The unique identifier for the OAuth2 client
    pub client_id: String,

    /// List of URLs that this client is allowed to redirect to after authorization
    ///
    /// These URLs must match exactly during the OAuth2 flow for security.
    /// Both HTTP and HTTPS URLs are supported, but HTTPS is recommended for production.
    pub allowed_callbacks: Vec<String>,

    /// Default scope for the client
    ///
    /// This is a space-separated list of scopes that the client can request.
    /// The default scope is used if the client does not specify a scope during the authorization request.
    pub default_scope: String,
}

fn default_duration() -> Option<i64> {
    Some(86400)
}

/// User definition for authentication and authorization
///
/// This structure represents a user with authentication credentials and
/// associated permissions for controlling access to API endpoints.
///
/// # Fields
///
/// * `user` - The username used for authentication
/// * `pass` - Base64-encoded password hash (created with openssl passwd -5 | base64 -w0)
/// * `permissions` - List of permission strings that define what actions the user can perform
///
/// # Example
///
/// ```
/// use rust_photoacoustic::config::User;
///
/// let user = User {
///     user: "admin".to_string(),
///     pass: "JDEkYTRuMy5jZmUkRU93djlOYXBKYjFNTXRTMHA1UzN1MQo=".to_string(),
///     email: None,
///     name: None,
///     permissions: vec!["read:api".to_string(), "write:api".to_string(), "admin:api".to_string()],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// The username used for authentication
    pub user: String,

    /// Base64-encoded password hash
    ///
    /// This should be created using: `openssl passwd -5 <password> | base64 -w0`
    pub pass: String,

    /// List of permission strings that define what actions the user can perform
    ///
    /// Common permissions include:
    /// * "read:api" - Allows read-only access to API endpoints
    /// * "write:api" - Allows modification operations on API endpoints
    /// * "admin:api" - Allows administrative operations
    pub permissions: Vec<String>,

    pub email: Option<String>,
    pub name: Option<String>,
}

/// Configuration for user access and permissions
///
/// This structure defines both users who can access the application directly
/// and OAuth2 clients that can use the authorization flow. Users have usernames,
/// password hashes, and permissions, while clients have identifiers and allowed
/// callback URLs.
///
/// # Example
///
/// ```rust
/// use rust_photoacoustic::config::access::{AccessConfig, User, Client};
///
/// let access_config = AccessConfig {
///     duration: Some(86400), // Token duration in seconds
///     users: vec![
///          User {
///              user: "admin".to_string(),
///              pass: "JDEkYTRuMy5jZmUkRU93djlOYXBKYjFNTXRTMHA1UzN1MQo=".to_string(),
///              permissions: vec!["read:api".to_string(), "write:api".to_string(), "admin:api".to_string()],
///              email: None,
///              name: None,
///          },
///          User {
///              user: "reader".to_string(),
///              pass: "JDEkUTJoSGZWU3ckT3NIVTUzamhCY3pYVmRHTGlTazg4Lwo=".to_string(),
///              permissions: vec!["read:api".to_string()],
///              email: None,
///              name: None,
///          }],
///      clients: vec![
///          Client {
///              client_id: "LaserSmartClient".to_string(),
///              default_scope: "openid profile email read:api write:api".to_string(),
///              allowed_callbacks: vec![
///                  "http://localhost:8080/client/".to_string(),
///                  "https://localhost:8080/client/".to_string(),
///              ],
///          }],
///     };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessConfig {
    /// List of users with their credentials and permissions
    pub users: Vec<User>,

    /// List of OAuth2 clients with their identifiers and allowed callback URLs
    pub clients: Vec<Client>,

    /// Duration of the issued token
    #[serde(default = "default_duration")]
    pub duration: Option<i64>,
}

impl Default for User {
    fn default() -> Self {
        Self {
            user: "admin".to_string(),
            // Default password hash for "admin123" (should be changed in production)
            pass: "JDUkM2E2OUZwQW0xejZBbWV2QSRvMlhhN0lxcVdVU1VPTUh6UVJiM3JjRlRhZy9WYjdpSWJtZUJFaXA3Y1ZECg==".to_string(),
            permissions: vec![
                "read:api".to_string(), 
                "write:api".to_string(), 
                "admin:api".to_string(),
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            email: Some("email@example.org".to_string()),
            name: Some("Admin User".to_string()),
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self {
            client_id: "LaserSmartClient".to_string(),
            default_scope: "openid profile email read:api write:api".to_string(),
            allowed_callbacks: vec![
                "http://localhost:8080/client/".to_string(),
                "https://localhost:8080/client/".to_string(),
            ],
        }
    }
}

impl Default for AccessConfig {
    fn default() -> Self {
        Self {
            users: vec![User::default()],
            clients: vec![Client::default()],
            duration: default_duration(),
        }
    }
}

/// Rocket request guard for extracting [`AccessConfig`] from the application state.
///
/// This guard retrieves the [`AccessConfig`] from the [`OxideState`] managed by Rocket.
/// It allows routes to access the configuration as a request guard parameter.
///
/// # Errors
/// Returns a 500 error if the [`OxideState`] is missing from Rocket state.
#[rocket::async_trait]
impl<'r> FromRequest<'r> for AccessConfig {
    type Error = &'static str;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match request.guard::<&State<OxideState>>().await {
            Outcome::Success(oxide_state) => Outcome::Success(oxide_state.access_config.clone()),
            Outcome::Error((status, _)) => Outcome::Error((status, "Missing oxide state")),
            Outcome::Forward(status) => Outcome::Forward(status),
        }
    }
}
