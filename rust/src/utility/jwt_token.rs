// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! JWT Token Creation Utility
//!
//! This module provides utilities for creating JWT tokens programmatically,
//! extracted from the create_token binary for reuse across the codebase.

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use oxide_auth::endpoint::{Issuer, Scope};
use oxide_auth::primitives::grant::{Extensions, Grant};
use std::str::FromStr;
use thiserror::Error;
use url::Url;

use crate::config::access::{Client, User};
use crate::config::Config;
use crate::visualization::auth::jwt::JwtIssuer;

/// Specific errors for token creation
#[derive(Error, Debug)]
pub enum TokenCreationError {
    #[error("Configuration loading failed: {source}")]
    ConfigError {
        #[from]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("User '{user}' not found in configuration. Available users: {available_users}")]
    UserNotFound {
        user: String,
        available_users: String,
    },

    #[error("Client '{client}' not found in configuration")]
    ClientNotFound { client: String },

    #[error("Failed to decode RS256 key from base64: {reason}")]
    KeyDecodingError { reason: String },

    #[error("Failed to create JwtIssuer with RS256 keys: {reason}")]
    JwtIssuerCreationError { reason: String },

    #[error("Invalid scope: {scope}")]
    InvalidScope { scope: String },

    #[error("Invalid redirect URI: {uri}")]
    InvalidRedirectUri { uri: String },

    #[error("No redirect URI configured for client")]
    NoRedirectUri,

    #[error("JWT token creation failed: {reason}")]
    TokenIssuingError { reason: String },
}

/// Supported JWT algorithms for token signing
///
/// This enum represents the cryptographic algorithms that can be used
/// to sign JWT tokens. Each algorithm has different security characteristics
/// and key requirements.
///
/// # Examples
///
/// ```
/// use rust_photoacoustic::utility::jwt_token::JwtAlgorithm;
/// use std::str::FromStr;
///
/// // Parse algorithm from string
/// let algo = JwtAlgorithm::from_str("RS256").unwrap();
/// assert_eq!(algo.as_str(), "RS256");
///
/// // Convert to jsonwebtoken algorithm
/// let jwt_algo = algo.to_jsonwebtoken_algorithm();
/// assert_eq!(jwt_algo, jsonwebtoken::Algorithm::RS256);
/// ```
#[derive(Debug, Clone)]
pub enum JwtAlgorithm {
    /// HMAC using SHA-256 hash algorithm (symmetric key)
    HS256,
    /// RSA signature with SHA-256 hash algorithm (asymmetric key)
    RS256,
}

impl FromStr for JwtAlgorithm {
    type Err = TokenCreationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "HS256" => Ok(JwtAlgorithm::HS256),
            "RS256" => Ok(JwtAlgorithm::RS256),
            _ => Err(TokenCreationError::TokenIssuingError {
                reason: format!("Unsupported algorithm: {}", s),
            }),
        }
    }
}

impl JwtAlgorithm {
    pub fn to_jsonwebtoken_algorithm(&self) -> jsonwebtoken::Algorithm {
        match self {
            JwtAlgorithm::HS256 => jsonwebtoken::Algorithm::HS256,
            JwtAlgorithm::RS256 => jsonwebtoken::Algorithm::RS256,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            JwtAlgorithm::HS256 => "HS256",
            JwtAlgorithm::RS256 => "RS256",
        }
    }
}

/// Parameters required for creating a JWT token
///
/// This structure contains all the necessary information to generate
/// a JWT token for a specific user and client combination.
///
/// # Examples
///
/// ```
/// use rust_photoacoustic::utility::jwt_token::{TokenCreationParams, JwtAlgorithm};
///
/// let params = TokenCreationParams {
///     user_id: "admin".to_string(),
///     client_id: "LaserSmartClient".to_string(),
///     algorithm: JwtAlgorithm::RS256,
///     duration_seconds: 3600, // 1 hour
/// };
///
/// assert_eq!(params.user_id, "admin");
/// assert_eq!(params.duration_seconds, 3600);
/// ```
#[derive(Debug)]
pub struct TokenCreationParams {
    /// The user identifier for whom the token is being created
    pub user_id: String,
    /// The client application identifier that will use the token
    pub client_id: String,
    /// The cryptographic algorithm to use for signing the token
    pub algorithm: JwtAlgorithm,
    /// Token validity duration in seconds
    pub duration_seconds: u64,
}

/// Result of a successful JWT token creation operation
///
/// This structure contains the generated token along with metadata
/// about the token creation process.
///
/// # Examples
///
/// ```
/// # use rust_photoacoustic::utility::jwt_token::TokenCreationResult;
/// #
/// # // This would typically be created by TokenCreator::create_token()
/// let result = TokenCreationResult {
///     token: "eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiJ9...".to_string(),
///     user_id: "admin".to_string(),
///     algorithm: "RS256".to_string(),
///     duration_seconds: 3600,
///     permissions: vec!["read:api".to_string(), "write:config".to_string()],
/// };
///
/// assert_eq!(result.user_id, "admin");
/// assert_eq!(result.algorithm, "RS256");
/// assert!(result.token.starts_with("eyJ"));
/// ```
#[derive(Debug)]
pub struct TokenCreationResult {
    /// The generated JWT token string
    pub token: String,
    /// The user ID for whom the token was created
    pub user_id: String,
    /// The algorithm used to sign the token
    pub algorithm: String,
    /// The token validity duration in seconds
    pub duration_seconds: u64,
    /// The permissions granted to the user
    pub permissions: Vec<String>,
}

/// Configuration loader with validation for JWT token creation
///
/// This structure wraps the application configuration and provides
/// helper methods for validating users and clients during token creation.
///
/// # Examples
///
/// ```
/// use rust_photoacoustic::config::Config;
/// use rust_photoacoustic::utility::jwt_token::ConfigLoader;
///
/// let config = Config::default();
/// let config_loader = ConfigLoader::from_config(&config).unwrap();
///
/// // The default config includes an admin user
/// let admin_user = config_loader.find_user("admin");
/// assert!(admin_user.is_ok());
/// ```
pub struct ConfigLoader {
    config: Config,
}

impl ConfigLoader {
    /// Create a ConfigLoader from an existing Config instance
    ///
    /// This method clones the provided configuration and wraps it
    /// in a ConfigLoader for token creation operations.
    ///
    /// # Arguments
    ///
    /// * `config` - The application configuration to use
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::config::Config;
    /// use rust_photoacoustic::utility::jwt_token::ConfigLoader;
    ///
    /// let config = Config::default();
    /// let config_loader = ConfigLoader::from_config(&config).unwrap();
    ///
    /// // ConfigLoader is now ready to validate users and clients
    /// assert!(config_loader.find_user("admin").is_ok());
    /// ```
    pub fn from_config(config: &Config) -> Result<Self, TokenCreationError> {
        Ok(Self {
            config: config.clone(),
        })
    }

    /// Return the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Find a user by name
    pub fn find_user(&self, username: &str) -> Result<&User, TokenCreationError> {
        self.config
            .access
            .users
            .iter()
            .find(|u| u.user == username)
            .ok_or_else(|| {
                let available_users = self
                    .config
                    .access
                    .users
                    .iter()
                    .map(|u| u.user.clone())
                    .collect::<Vec<_>>()
                    .join(", ");

                TokenCreationError::UserNotFound {
                    user: username.to_string(),
                    available_users,
                }
            })
    }

    /// Find a client by ID
    pub fn find_client(&self, client_id: &str) -> Result<&Client, TokenCreationError> {
        self.config
            .access
            .clients
            .iter()
            .find(|c| c.client_id == client_id)
            .ok_or_else(|| TokenCreationError::ClientNotFound {
                client: client_id.to_string(),
            })
    }
}

/// JWT token creator for generating authenticated tokens
///
/// This is the main interface for creating JWT tokens. It handles
/// validation of users and clients, algorithm selection, and token generation.
///
/// # Examples
///
/// Creating a JWT token for testing:
///
/// ```
/// use rust_photoacoustic::config::Config;
/// use rust_photoacoustic::utility::jwt_token::{
///     ConfigLoader, TokenCreator, TokenCreationParams, JwtAlgorithm
/// };
///
/// // Setup configuration with test values
/// let mut config = Config::default();
/// config.visualization.hmac_secret = "test-secret-key".to_string();
///
/// // Create token creator
/// let config_loader = ConfigLoader::from_config(&config).unwrap();
/// let token_creator = TokenCreator::new(&config_loader).unwrap();
///
/// // Define token parameters
/// let params = TokenCreationParams {
///     user_id: "admin".to_string(),
///     client_id: "LaserSmartClient".to_string(),
///     algorithm: JwtAlgorithm::HS256, // Use HS256 for simplicity in tests
///     duration_seconds: 60,
/// };
///
/// // Create the token
/// let result = token_creator.create_token(&params).unwrap();
/// assert_eq!(result.user_id, "admin");
/// assert!(result.token.len() > 0);
/// ```
pub struct TokenCreator {
    config_loader: ConfigLoader,
}

impl TokenCreator {
    /// Creates a new token creator from a configuration loader
    ///
    /// # Arguments
    ///
    /// * `config_loader` - The configuration loader containing user and client definitions
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::config::Config;
    /// use rust_photoacoustic::utility::jwt_token::{ConfigLoader, TokenCreator};
    ///
    /// let config = Config::default();
    /// let config_loader = ConfigLoader::from_config(&config).unwrap();
    /// let token_creator = TokenCreator::new(&config_loader).unwrap();
    ///
    /// // TokenCreator is now ready to generate tokens
    /// ```
    pub fn new(config_loader: &ConfigLoader) -> Result<Self, TokenCreationError> {
        Ok(Self {
            config_loader: ConfigLoader::from_config(config_loader.config())?,
        })
    }

    /// Creates a JWT token with the specified parameters
    ///
    /// This is the main method for generating JWT tokens. It validates the user
    /// and client, creates the appropriate JWT issuer based on the algorithm,
    /// and generates a signed token.
    ///
    /// # Arguments
    ///
    /// * `params` - Token creation parameters including user, client, algorithm, and duration
    ///
    /// # Returns
    ///
    /// * `Ok(TokenCreationResult)` - Contains the generated token and metadata
    /// * `Err(TokenCreationError)` - If validation fails or token generation encounters an error
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::config::Config;
    /// use rust_photoacoustic::utility::jwt_token::{
    ///     ConfigLoader, TokenCreator, TokenCreationParams, JwtAlgorithm
    /// };
    ///
    /// let mut config = Config::default();
    /// config.visualization.hmac_secret = "test-secret-for-hmac".to_string();
    ///
    /// let config_loader = ConfigLoader::from_config(&config).unwrap();
    /// let token_creator = TokenCreator::new(&config_loader).unwrap();
    ///
    /// let params = TokenCreationParams {
    ///     user_id: "admin".to_string(),
    ///     client_id: "LaserSmartClient".to_string(),
    ///     algorithm: JwtAlgorithm::HS256,
    ///     duration_seconds: 3600,
    /// };
    ///
    /// let result = token_creator.create_token(&params).unwrap();
    /// assert_eq!(result.user_id, "admin");
    /// assert_eq!(result.algorithm, "HS256");
    /// assert!(result.token.len() > 50); // JWT tokens are typically quite long
    /// ```
    pub fn create_token(
        &self,
        params: &TokenCreationParams,
    ) -> Result<TokenCreationResult, TokenCreationError> {
        // Input data validation
        let user = self.config_loader.find_user(&params.user_id)?;
        let client = self.config_loader.find_client(&params.client_id)?;

        // Create the JWT issuer according to the algorithm
        let mut jwt_issuer = self.create_jwt_issuer(&params.algorithm)?;

        // Create the token
        let token = self.issue_token(
            &mut jwt_issuer,
            params,
            user,
            client,
            self.config_loader.config(),
        )?;

        Ok(TokenCreationResult {
            token,
            user_id: params.user_id.clone(),
            algorithm: params.algorithm.as_str().to_string(),
            duration_seconds: params.duration_seconds,
            permissions: user.permissions.clone(),
        })
    }

    /// Creates a JWT issuer according to the specified algorithm
    fn create_jwt_issuer(&self, algorithm: &JwtAlgorithm) -> Result<JwtIssuer, TokenCreationError> {
        match algorithm {
            JwtAlgorithm::HS256 => {
                let hmac_secret = self
                    .config_loader
                    .config()
                    .visualization
                    .hmac_secret
                    .as_bytes();
                Ok(JwtIssuer::new(hmac_secret))
            }
            JwtAlgorithm::RS256 => {
                let config = self.config_loader.config();
                let rsa_private_key = BASE64_STANDARD
                    .decode(&config.visualization.rs256_private_key)
                    .map_err(|e| TokenCreationError::KeyDecodingError {
                        reason: format!("RS256 private key: {}", e),
                    })?;

                let rsa_public_key = BASE64_STANDARD
                    .decode(&config.visualization.rs256_public_key)
                    .map_err(|e| TokenCreationError::KeyDecodingError {
                        reason: format!("RS256 public key: {}", e),
                    })?;

                JwtIssuer::with_rs256_pem(&rsa_private_key, &rsa_public_key).map_err(|e| {
                    TokenCreationError::JwtIssuerCreationError {
                        reason: format!("Failed to create RS256 issuer: {:?}", e),
                    }
                })
            }
        }
    }

    /// Issues the JWT token
    fn issue_token(
        &self,
        jwt_issuer: &mut JwtIssuer,
        params: &TokenCreationParams,
        user: &User,
        client: &Client,
        config: &Config,
    ) -> Result<String, TokenCreationError> {
        let scope = Scope::from_str(&client.default_scope).map_err(|_| {
            TokenCreationError::InvalidScope {
                scope: client.default_scope.clone(),
            }
        })?;

        let redirect_uri = client
            .allowed_callbacks
            .first()
            .ok_or(TokenCreationError::NoRedirectUri)?;

        let redirect_uri =
            Url::parse(redirect_uri).map_err(|e| TokenCreationError::InvalidRedirectUri {
                uri: format!("{}: {}", redirect_uri, e),
            })?;

        let grant = Grant {
            owner_id: params.user_id.clone(),
            client_id: client.client_id.clone(),
            scope,
            redirect_uri,
            until: chrono::Utc::now() + chrono::Duration::seconds(params.duration_seconds as i64),
            extensions: Extensions::default(),
        };

        let issuer_name = config
            .access
            .iss
            .clone()
            .unwrap_or_else(|| "LaserSmartServer".to_string());

        let token = jwt_issuer
            .with_issuer(issuer_name)
            .valid_for(chrono::TimeDelta::seconds(params.duration_seconds as i64))
            .with_algorithm(params.algorithm.to_jsonwebtoken_algorithm())
            .add_user_claims(&params.user_id, &user.permissions)
            .issue(grant)
            .map_err(|e| TokenCreationError::TokenIssuingError {
                reason: format!("Failed to issue JWT token: {:?}", e),
            })?;

        Ok(token.token)
    }
}
