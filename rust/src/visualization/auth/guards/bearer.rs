// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Rocket request guards for OAuth2 Bearer token validation and permission checking
//!
//! This module provides request guards that automatically validate OAuth2 Bearer tokens
//! and check user permissions for API endpoints. The guards integrate with the JWT
//! validation system and support both HMAC (HS256) and RSA (RS256) token validation.
//!
//! # Request Guards
//!
//! - [`OAuthBearer`] - Validates Bearer tokens and extracts user information
//! - [`RequiresPermission`] - Validates tokens and checks for specific permissions
//!
//! # Token Validation
//!
//! The guards support both symmetric (HMAC-SHA256) and asymmetric (RSA-SHA256) JWT validation:
//!
//! - **HS256**: Uses a shared secret key for token signing and verification
//! - **RS256**: Uses RSA public/private key pairs for enhanced security
//!
//! The validation process includes:
//! 1. Extracting the Bearer token from the Authorization header
//! 2. Verifying the JWT signature and claims
//! 3. Extracting user information and permissions from the token
//! 4. Optionally checking for specific permissions

use crate::config::Config;
use crate::visualization::auth::jwt::{JwtValidator, UserSysInfo};
use crate::visualization::auth::oauth2::OxideState;
use base64::Engine;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::State;
use rocket_okapi::okapi;
use rocket_okapi::okapi::openapi3::{SecurityRequirement, SecurityScheme, SecuritySchemeData};
use rocket_okapi::{
    gen::OpenApiGenerator,
    request::{OpenApiFromRequest, RequestHeaderInput},
};
use std::sync::Arc;

/// Request guard for extracting and validating a Bearer JWT from the Authorization header
///
/// This guard automatically validates OAuth2 Bearer tokens and extracts user information
/// from JWT claims. It supports both HMAC (HS256) and RSA (RS256) token validation
/// depending on the server configuration.
///
/// ### Authentication Process
///
/// 1. **Header Extraction**: Extracts the `Authorization: Bearer <token>` header
/// 2. **Token Validation**: Validates the JWT signature and standard claims (exp, nbf, iss)
/// 3. **User Resolution**: Extracts user information from token claims
/// 4. **Permission Loading**: Loads user permissions from the token or configuration
///
/// ### Success Conditions
///
/// The guard succeeds if:
/// - The Authorization header is present and well-formed
/// - The Bearer token is a valid JWT with correct signature
/// - The token has not expired (`exp` claim)
/// - The token is not used before its validity period (`nbf` claim)
/// - The issuer matches the expected value (`iss` claim)
///
/// ### Error Responses
///
/// | Condition | HTTP Status | Description |
/// |-----------|-------------|-------------|
/// | Missing Authorization header | 401 Unauthorized | No authentication provided |
/// | Malformed Bearer token | 401 Unauthorized | Invalid token format |
/// | Invalid JWT signature | 401 Unauthorized | Token tampered with or wrong key |
/// | Expired token | 401 Unauthorized | Token past expiration time |
/// | Server configuration error | 500 Internal Server Error | Missing state or keys |
///
/// ### Examples
///
/// ```rust,no_run
/// use rocket::get;
/// use rocket::serde::json::Json;
/// use rust_photoacoustic::visualization::auth::OAuthBearer;
///
/// #[get("/user-info")]
/// fn get_user_info(bearer: OAuthBearer) -> Json<String> {
///     Json(format!("User: {}", bearer.user_info.user_id))
/// }
///
/// #[get("/check-permission")]
/// fn check_permission(bearer: OAuthBearer) -> &'static str {
///     if bearer.has_permission("admin:api") {
///         "You have admin access"
///     } else {
///         "Regular user access"
///     }
/// }
/// ```
pub struct OAuthBearer {
    /// User information extracted from the validated JWT token
    pub user_info: UserSysInfo,
    /// The raw JWT token string
    pub token: String,
    /// User permissions extracted from the token claims
    pub permissions: Option<Vec<String>>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OAuthBearer {
    type Error = (Status, &'static str);

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Get the Authorization header
        let auth_header = request.headers().get_one("Authorization");

        // Get the Config from State instead of using get_config_from_request
        let config_state = match request.guard::<&State<Arc<Config>>>().await {
            Outcome::Success(config) => config,
            _ => {
                return Outcome::Error((
                    Status::InternalServerError,
                    (Status::InternalServerError, "Missing config state"),
                ))
            }
        };
        let access_config = &config_state.access;

        if let Some(header) = auth_header {
            if let Some(token) = header.strip_prefix("Bearer ") {
                // Get the OxideState from Rocket state
                let state = match request.guard::<&State<OxideState>>().await {
                    Outcome::Success(state) => state,
                    _ => {
                        return Outcome::Error((
                            Status::InternalServerError,
                            (Status::InternalServerError, "Missing state"),
                        ))
                    }
                };
                // Build JwtValidator from state (supporting both HS256 and RS256)
                let hmac_secret = state.hmac_secret.as_bytes();
                let rs256_public_key = if !state.rs256_public_key.is_empty() {
                    base64::engine::general_purpose::STANDARD
                        .decode(&state.rs256_public_key)
                        .ok()
                } else {
                    None
                };

                let validator = match rs256_public_key {
                    Some(ref pem) => {
                        JwtValidator::new(Some(hmac_secret), Some(&pem), access_config.clone())
                    }
                    None => JwtValidator::new(Some(hmac_secret), None, access_config.clone()),
                };
                match validator {
                    Ok(validator) => match validator.get_user_info(token, access_config.clone()) {
                        Ok(user_info) => Outcome::Success(OAuthBearer {
                            user_info: user_info.clone(),
                            token: token.to_string(),
                            permissions: user_info.permissions.clone(),
                        }),
                        Err(_) => Outcome::Error((
                            Status::Unauthorized,
                            (Status::Unauthorized, "Invalid token"),
                        )),
                    },
                    Err(_) => Outcome::Error((
                        Status::InternalServerError,
                        (Status::InternalServerError, "Validator error"),
                    )),
                }
            } else {
                Outcome::Error((
                    Status::Unauthorized,
                    (Status::Unauthorized, "Missing Bearer token"),
                ))
            }
        } else {
            Outcome::Error((
                Status::Unauthorized,
                (Status::Unauthorized, "Missing Authorization header"),
            ))
        }
    }
}

impl OAuthBearer {
    /// Check if the authenticated user has the specified permission
    ///
    /// ### Arguments
    ///
    /// * `permission` - The permission string to check for (e.g., "read:api", "admin:users")
    ///
    /// ### Returns
    ///
    /// Returns `true` if the user has the specified permission, `false` otherwise.
    /// If the user has no permissions (None), this method returns `false`.
    ///
    /// ### Examples
    ///
    /// ```rust,no_run
    /// use rocket::get;
    /// use rust_photoacoustic::visualization::auth::OAuthBearer;
    ///
    /// #[get("/sensitive-data")]
    /// fn get_sensitive_data(bearer: OAuthBearer) -> Result<&'static str, rocket::http::Status> {
    ///     if bearer.has_permission("read:sensitive") {
    ///         Ok("Sensitive information")
    ///     } else {
    ///         Err(rocket::http::Status::Forbidden)
    ///     }
    /// }
    /// ```
    pub fn has_permission(&self, permission: &str) -> bool {
        self.user_info
            .permissions
            .as_ref()
            .map(|permissions| permissions.contains(&permission.to_string()))
            .unwrap_or(false)
    }
}

impl<'r> OpenApiFromRequest<'r> for OAuthBearer {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        // Setup global requirement for Security scheme
        let security_scheme = SecurityScheme {
            description: Some(
                "Requires a valid JWT Bearer token for authentication. \
                The token must be provided in the Authorization header as 'Bearer <token>'. \
                Supports both HMAC (HS256) and RSA (RS256) signed tokens."
                    .to_owned(),
            ),
            // Setup data requirements for Bearer JWT
            data: SecuritySchemeData::Http {
                scheme: "bearer".to_owned(),
                bearer_format: Some("JWT".to_owned()),
            },
            // Add example data for RapiDoc/Swagger UI
            extensions: okapi::map! {
                "x-bearer-format".to_owned() => rocket::serde::json::json!("JWT"),
                "x-description".to_owned() => rocket::serde::json::json!(
                    "JWT tokens are issued by the /token endpoint after successful authentication"
                ),
            },
        };

        // Add the requirement for this route/endpoint
        // This specifies what scopes/permissions are needed
        let mut security_req = SecurityRequirement::new();
        // The security requirement name must match the scheme name
        security_req.insert("BearerAuth".to_owned(), vec![]);

        Ok(RequestHeaderInput::Security(
            "BearerAuth".to_owned(),
            security_scheme,
            security_req,
        ))
    }

    fn get_responses(
        _gen: &mut OpenApiGenerator,
    ) -> rocket_okapi::Result<okapi::openapi3::Responses> {
        use okapi::openapi3::*;

        let mut responses = Responses::default();

        // Add 401 Unauthorized response
        responses.responses.insert(
            "401".to_owned(),
            RefOr::Object(Response {
                description: "Unauthorized - Invalid or missing Bearer token".to_owned(),
                content: okapi::map! {
                    "application/json".to_owned() => MediaType {
                        example: Some(rocket::serde::json::json!({
                            "error": "Invalid or missing Bearer token"
                        })),
                        ..Default::default()
                    }
                },
                ..Default::default()
            }),
        );

        // Add 500 Internal Server Error response
        responses.responses.insert(
            "500".to_owned(),
            RefOr::Object(Response {
                description: "Internal Server Error - Authentication service unavailable"
                    .to_owned(),
                content: okapi::map! {
                    "application/json".to_owned() => MediaType {
                        example: Some(rocket::serde::json::json!({
                            "error": "Authentication service unavailable"
                        })),
                        ..Default::default()
                    }
                },
                ..Default::default()
            }),
        );

        Ok(responses)
    }
}
