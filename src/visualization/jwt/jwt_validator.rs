// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! JWT token validation and user information extraction
//!
//! This module provides functionality to validate JWT (JSON Web Tokens) and extract
//! user information from them. It includes:
//!
//! - A configurable JWT validator that can validate tokens with different algorithms and requirements
//! - User information extraction from validated tokens
//! - Scope validation and token expiration utilities
//!
//! # Example
//!
//! ```
//! use rust_photoacoustic::visualization::jwt::jwt_validator::{JwtValidator, UserInfo};
//!
//! // Create a validator with a secret key
//! let secret = b"your-secret-key";
//! let validator = JwtValidator::new(Some(secret),None).unwrap()
//!     .with_issuer("https://api.example.com")
//!     .with_audience("web-client");
//!     
//! // Validate a token and get user info
//! match validator.get_user_info("your.jwt.token") {
//!     Ok(user_info) => {
//!         println!("User ID: {}", user_info.user_id);
//!         
//!         // Check if user has a specific scope
//!         if user_info.has_scope("read:data") {
//!             println!("User can read data");
//!         }
//!         
//!         // Check token validity
//!         let remaining = user_info.validity_remaining_secs();
//!         println!("Token valid for {} more seconds", remaining);
//!     },
//!     Err(e) => println!("Token validation failed: {}", e),
//! }
//! ```
//!
//! # Security Considerations
//!
//! - Always validate tokens before trusting their contents
//! - Use appropriate algorithm and key length for your security requirements
//! - Consider token expiration and refresh strategies

use anyhow::{anyhow, Result};
use chrono::{DateTime, TimeZone, Utc};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Custom JWT claims structure matching the one in jwt.rs
///
/// This structure represents the standard JWT claims as defined in RFC 7519,
/// with additional fields for scope and metadata. It is compatible with
/// the JWT generation in the `jwt.rs` module.
#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (typically user ID)
    ///
    /// The "sub" (subject) claim identifies the principal that is the
    /// subject of the JWT. In this application, it typically contains the user ID.
    pub sub: String,

    /// Issued at timestamp
    ///
    /// The "iat" (issued at) claim identifies the time at which the JWT was
    /// issued, represented as the number of seconds from 1970-01-01T00:00:00Z UTC.
    pub iat: i64,

    /// Expiration timestamp
    ///
    /// The "exp" (expiration time) claim identifies the expiration time on
    /// or after which the JWT MUST NOT be accepted for processing.
    /// Represented as the number of seconds from 1970-01-01T00:00:00Z UTC.
    pub exp: i64,

    /// Not before timestamp (when the token becomes valid)
    ///
    /// The "nbf" (not before) claim identifies the time before which the JWT
    /// MUST NOT be accepted for processing.
    /// Represented as the number of seconds from 1970-01-01T00:00:00Z UTC.
    pub nbf: i64,

    /// JWT ID (unique identifier for the token)
    ///
    /// The "jti" (JWT ID) claim provides a unique identifier for the JWT.
    /// This can be used to prevent the JWT from being replayed.
    pub jti: String,

    /// Audience (client ID)
    ///
    /// The "aud" (audience) claim identifies the recipients that the JWT is
    /// intended for. In this application, it typically contains the client ID.
    pub aud: String,

    /// Issuer
    ///
    /// The "iss" (issuer) claim identifies the principal that issued the
    /// JWT. This is typically a URL or domain name.
    pub iss: String,

    /// Scope
    ///
    /// Space-delimited list of permissions that the token grants.
    /// This is not part of the standard JWT claims but is commonly used
    /// for OAuth 2.0 access tokens.
    pub scope: String,

    /// Additional metadata
    ///
    /// Custom claims that can contain additional information about the user
    /// or the context. Commonly used for storing email, name, and other user
    /// attributes that don't fit into the standard claims.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// A validator for JWT tokens
///
/// The `JwtValidator` is responsible for validating JWT tokens using configurable
/// criteria such as the signing algorithm, issuer, and audience. It can be configured
/// with different validation parameters to match your security requirements.
///
/// # Features
///
/// - Support for different JWT algorithms (HS256, RS256, etc.)
/// - Validation of token expiration and activation time
/// - Optional verification of issuer and audience claims
/// - Extraction of user information from validated tokens
///
/// # Examples
///
/// Basic setup with HS256 algorithm:
///
/// ```
/// use rust_photoacoustic::visualization::jwt::jwt_validator::JwtValidator;
///
/// // Create a validator with a secret key (HS256)
/// let validator = JwtValidator::new(Some(b"your-secret-key"), None).unwrap();
///
/// // Validate a token (token must be signed with HS256)
/// // let result = validator.validate("your.jwt.token");
/// // assert!(result.is_ok() || result.is_err());
/// ```
///
/// Basic setup with RS256 algorithm:
///
/// ```no_run
/// use rust_photoacoustic::visualization::jwt::jwt_validator::JwtValidator;
/// // Example public key in PEM format (for demonstration only)
/// let public_pem = b"-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----\n";
/// let validator = JwtValidator::new(None, Some(public_pem)).unwrap();
/// // Validate a token (token must be signed with RS256)
/// // let result = validator.validate("your.jwt.token");
/// // assert!(result.is_ok() || result.is_err());
/// ```
///
/// Dual algorithm (HS256 and RS256):
///
/// ```no_run
/// use rust_photoacoustic::visualization::jwt::jwt_validator::JwtValidator;
/// let hmac = b"your-secret-key";
/// let public_pem = b"-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----\n";
/// let validator = JwtValidator::new(Some(hmac), Some(public_pem)).unwrap();
/// // Now validator can validate both HS256 and RS256 tokens.
/// ```
///
pub struct JwtValidator {
    /// Optional HMAC secret for HS256
    hmac_key: Option<DecodingKey>,
    /// Optional RS256 public key
    rs256_key: Option<DecodingKey>,
    /// The expected issuer of the token, if any
    expected_issuer: Option<String>,

    /// The expected audience of the token, if any
    expected_audience: Option<String>,
}

impl JwtValidator {
    /// Create a new JwtValidator with optional HS256 and RS256 keys
    pub fn new(
        hmac_secret: Option<&[u8]>,
        rs256_public_key_pem: Option<&[u8]>,
    ) -> Result<Self, jsonwebtoken::errors::Error> {
        let hmac_key = hmac_secret.map(DecodingKey::from_secret);
        let rs256_key = if let Some(pem) = rs256_public_key_pem {
            Some(DecodingKey::from_rsa_pem(pem)?)
        } else {
            None
        };
        Ok(JwtValidator {
            hmac_key,
            rs256_key,
            expected_issuer: None,
            expected_audience: None,
        })
    }

    /// Set the expected issuer name
    ///
    /// Configures the validator to verify that the token's "iss" claim
    /// matches the specified issuer. This is useful for ensuring that tokens
    /// come from the expected authentication server.
    ///
    /// # Parameters
    ///
    /// * `issuer` - The expected issuer value to match against the token's "iss" claim
    ///
    /// # Returns
    ///
    /// Self with the updated configuration, allowing for method chaining
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::visualization::jwt::jwt_validator::JwtValidator;
    /// let validator = JwtValidator::new(Some(b"secret-key"), None).unwrap()
    ///     .with_issuer("https://auth.example.com");
    /// ```
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.expected_issuer = Some(issuer.into());
        self
    }

    /// Set the expected audience
    ///
    /// Configures the validator to verify that the token's "aud" claim
    /// matches the specified audience. This ensures that the token was
    /// intended for your application.
    ///
    /// # Parameters
    ///
    /// * `audience` - The expected audience value to match against the token's "aud" claim
    ///
    /// # Returns
    ///
    /// Self with the updated configuration, allowing for method chaining
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::visualization::jwt::jwt_validator::JwtValidator;
    /// let validator = JwtValidator::new(Some(b"secret-key"), None).unwrap()
    ///     .with_audience("web-client");
    /// ```
    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.expected_audience = Some(audience.into());
        self
    }

    /// Validate a JWT token and return the decoded claims, supporting both HS256 and RS256
    ///
    /// Validates the JWT token by:
    /// - Verifying the signature using the configured key and algorithm
    /// - Checking that the token is not expired (exp claim)
    /// - Verifying that the token is active (nbf claim)
    /// - Comparing issuer and audience if configured
    ///
    /// # Parameters
    ///
    /// * `token` - The JWT token string to validate
    ///
    /// # Returns
    ///
    /// * `Ok(JwtClaims)` - The validated and decoded claims from the token
    /// * `Err(Error)` - If the token is invalid, expired, or fails any validation check
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The token's signature is invalid
    /// - The token has expired (current time > exp claim)
    /// - The token is not yet valid (current time < nbf claim)
    /// - The token's issuer doesn't match the expected issuer (if configured)
    /// - The token's audience doesn't match the expected audience (if configured)
    /// - The token contains invalid claim values (like malformed timestamps)
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::visualization::jwt::jwt_validator::JwtValidator;
    /// let validator = JwtValidator::new(Some(b"secret-key"), None).unwrap();
    /// // let result = validator.validate("your.jwt.token");
    /// // assert!(result.is_ok() || result.is_err());
    /// ```
    pub fn validate(&self, token: &str) -> Result<JwtClaims> {
        // Parse the header to determine the algorithm
        let header = jsonwebtoken::decode_header(token)
            .map_err(|e| anyhow!("Failed to decode JWT header: {}", e))?;
        let alg = header.alg;
        let (key, algorithm) = match alg {
            Algorithm::HS256 => {
                let key = self
                    .hmac_key
                    .as_ref()
                    .ok_or_else(|| anyhow!("HS256 key not configured"))?;
                (key, Algorithm::HS256)
            }
            Algorithm::RS256 => {
                let key = self
                    .rs256_key
                    .as_ref()
                    .ok_or_else(|| anyhow!("RS256 key not configured"))?;
                (key, Algorithm::RS256)
            }
            _ => return Err(anyhow!("Unsupported JWT algorithm: {:?}", alg)),
        };
        let mut validation = Validation::new(algorithm);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        if let Some(ref issuer) = self.expected_issuer {
            validation.set_issuer(&[issuer]);
        }
        if let Some(ref aud) = self.expected_audience {
            validation.set_audience(&[aud]);
        }
        let token_data = decode::<JwtClaims>(token, key, &validation)
            .map_err(|e| anyhow!("JWT validation failed: {}", e))?;
        let now = Utc::now();
        let exp_time = Utc
            .timestamp_opt(token_data.claims.exp, 0)
            .single()
            .ok_or_else(|| anyhow!("Invalid expiry time in token"))?;
        if exp_time < now {
            return Err(anyhow!("Token has expired"));
        }
        Ok(token_data.claims)
    }

    /// Extract user information from a validated token
    ///
    /// This method validates the token and converts the JWT claims into a more
    /// user-friendly `UserInfo` structure. It extracts standard claims like user ID
    /// and expiration, as well as additional information from the metadata field.
    ///
    /// # Parameters
    ///
    /// * `token` - The JWT token string to extract information from
    ///
    /// # Returns
    ///
    /// * `Ok(UserInfo)` - User information extracted from the token
    /// * `Err(Error)` - If the token validation fails for any reason
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The token fails validation (see `validate` method for details)
    /// - Required timestamps (iat, exp) can't be converted to DateTime objects
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::visualization::jwt::jwt_validator::JwtValidator;
    /// let validator = JwtValidator::new(Some(b"secret-key"), None).unwrap();
    /// // let result = validator.get_user_info("your.jwt.token");
    /// // assert!(result.is_ok() || result.is_err());
    /// ```
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
///
/// This structure provides a more user-friendly representation of the claims
/// contained in a JWT token. It includes the core user identity information,
/// permission scopes, and token metadata.
///
/// The `UserInfo` structure is designed to be:
/// - Easy to work with in application code
/// - Provide utility methods for common token operations
/// - Include only the relevant information for authentication and authorization
///
/// # Example
///
/// ```
/// use rust_photoacoustic::visualization::jwt::jwt_validator::{JwtValidator, UserInfo};
///
/// // After getting the UserInfo from a token
/// fn process_user_info(user: &UserInfo) {
///     println!("User ID: {}", user.user_id);
///     
///     if user.has_scope("admin") {
///         println!("User has admin privileges");
///     }
///     
///     if user.validity_remaining_secs() < 300 {
///         println!("Token will expire soon (less than 5 minutes)");
///     }
///     
///     if let Some(email) = &user.email {
///         println!("User email: {}", email);
///     }
/// }
/// ```
#[derive(Debug)]
pub struct UserInfo {
    /// User ID from the subject claim
    ///
    /// This is typically a unique identifier for the user in your system,
    /// extracted from the "sub" claim of the JWT.
    pub user_id: String,

    /// Client ID from the audience claim
    ///
    /// Identifies the intended recipient of the token (usually your application),
    /// extracted from the "aud" claim of the JWT.
    pub client_id: String,

    /// Scopes granted to this token
    ///
    /// A list of permission scopes that were granted to this token.
    /// These determine what actions the token allows the user to perform.
    pub scopes: Vec<String>,

    /// Optional email address
    ///
    /// The user's email address, if provided in the token metadata.
    pub email: Option<String>,

    /// Optional user name
    ///
    /// The user's display name, if provided in the token metadata.
    pub name: Option<String>,

    /// Unique token identifier
    ///
    /// A unique ID for this specific token, extracted from the "jti" claim.
    /// This can be used for token revocation or tracking.
    pub token_id: String,

    /// When the token was issued
    ///
    /// The timestamp when the token was created, extracted from the "iat" claim.
    pub issued_at: DateTime<Utc>,

    /// When the token expires
    ///
    /// The timestamp when the token will expire, extracted from the "exp" claim.
    pub expiry: DateTime<Utc>,
}

impl UserInfo {
    /// Create a new UserInfo instance from a JSON claims object (for documentation examples)
    ///
    /// This method is used primarily in documentation examples to create a UserInfo instance
    /// from JSON data representing JWT claims.
    ///
    /// # Parameters
    ///
    /// * `claims` - A JSON value representing the JWT claims
    ///
    /// # Returns
    ///
    /// A new UserInfo instance with data from the claims
    #[doc(hidden)]
    pub fn from_claims(claims: &serde_json::Value) -> Self {
        use chrono::TimeZone;

        let scopes = claims["scope"]
            .as_str()
            .unwrap_or("")
            .split_whitespace()
            .map(String::from)
            .collect();

        let exp = claims["exp"]
            .as_i64()
            .unwrap_or_else(|| Utc::now().timestamp() + 3600);

        Self {
            user_id: claims["sub"].as_str().unwrap_or("unknown").to_string(),
            client_id: claims["aud"].as_str().unwrap_or("unknown").to_string(),
            scopes,
            email: claims["email"].as_str().map(String::from),
            name: claims["name"].as_str().map(String::from),
            token_id: claims
                .get("jti")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            issued_at: Utc::now() - chrono::Duration::hours(1),
            expiry: Utc
                .timestamp_opt(exp, 0)
                .single()
                .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1)),
        }
    }

    /// Check if the token has a specific scope
    ///
    /// This method checks if the user has been granted a specific permission
    /// scope in their token. Scopes are typically space-separated strings in the
    /// original token, and this method checks if the given scope exists in the
    /// parsed collection.
    ///
    /// # Parameters
    ///
    /// * `scope` - The scope string to check for (case-sensitive)
    ///
    /// # Returns
    ///
    /// `true` if the user has the specified scope, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rust_photoacoustic::visualization::jwt::jwt_validator::UserInfo;
    /// # use serde_json::json;
    /// # let claims = serde_json::from_value(json!({
    /// #     "sub": "user123",
    /// #     "name": "Test User",
    /// #     "email": "test@example.com",
    /// #     "scope": "read:data write:data",
    /// #     "exp": 1719619200
    /// # })).unwrap();
    /// # let user_info = UserInfo::from_claims(&claims);
    ///
    /// if user_info.has_scope("read:data") {
    ///     // Allow reading data
    /// }
    ///
    /// if user_info.has_scope("write:data") {
    ///     // Allow writing data
    /// }
    /// ```
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Get the remaining validity time in seconds
    ///
    /// Calculates how many seconds remain until the token expires.
    /// Returns 0 if the token has already expired.
    ///
    /// # Returns
    ///
    /// The number of seconds until the token expires, or 0 if already expired
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rust_photoacoustic::visualization::jwt::jwt_validator::UserInfo;
    /// # use serde_json::json;
    /// # let claims = serde_json::from_value(json!({
    /// #     "sub": "user123",
    /// #     "name": "Test User",
    /// #     "email": "test@example.com",
    /// #     "exp": 1719619200
    /// # })).unwrap();
    /// # let user_info = UserInfo::from_claims(&claims);
    ///
    /// let remaining = user_info.validity_remaining_secs();
    ///
    /// if remaining < 60 {
    ///     println!("Warning: Token will expire in less than a minute");
    /// } else {
    ///     println!("Token valid for {} more seconds", remaining);
    /// }
    ///
    /// # fn get_user_info() -> UserInfo {
    /// #     // This is a mock function for the example
    /// #     unimplemented!()
    /// # }
    /// ```
    pub fn validity_remaining_secs(&self) -> i64 {
        let now = Utc::now();
        if self.expiry < now {
            0
        } else {
            self.expiry.signed_duration_since(now).num_seconds()
        }
    }
}
