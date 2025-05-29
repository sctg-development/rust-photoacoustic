// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use oxide_auth::endpoint::{Issuer, Scope};
use oxide_auth::primitives::grant::{Extensions, Grant};
use std::str::FromStr;
use url::Url;

use crate::config_loader::ConfigLoader;
use crate::error::TokenCreationError;
use rust_photoacoustic::config::access::{Client, User};
use rust_photoacoustic::config::Config;
use rust_photoacoustic::visualization::auth::jwt::JwtIssuer;

/// Supported JWT algorithms
#[derive(Debug, Clone)]
pub enum JwtAlgorithm {
    HS256,
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

/// Token creation parameters
#[derive(Debug)]
pub struct TokenCreationParams {
    pub username: String,
    pub client_id: String,
    pub algorithm: JwtAlgorithm,
    pub duration: u64,
}

/// Token creation result
#[derive(Debug)]
pub struct TokenCreationResult {
    pub token: String,
    pub username: String,
    pub algorithm: String,
    pub duration: u64,
    pub permissions: Vec<String>,
}

/// JWT token creator with improved error handling
pub struct TokenCreator {
    config_loader: ConfigLoader,
}

impl TokenCreator {
    /// Creates a new token creator
    pub fn new(config_loader: ConfigLoader) -> Self {
        Self { config_loader }
    }

    /// Creates a JWT token with the specified parameters
    pub async fn create_token(
        &self,
        params: TokenCreationParams,
    ) -> Result<TokenCreationResult, TokenCreationError> {
        // Input data validation
        let user = self.config_loader.find_user(&params.username)?;
        let client = self.config_loader.find_client(&params.client_id)?;

        // Create the JWT issuer according to the algorithm
        let mut jwt_issuer = self.create_jwt_issuer(&params.algorithm)?;

        // Create the token
        let token = self
            .issue_token(
                &mut jwt_issuer,
                &params,
                user,
                client,
                self.config_loader.config(),
            )
            .await?;

        Ok(TokenCreationResult {
            token,
            username: params.username,
            algorithm: params.algorithm.as_str().to_string(),
            duration: params.duration,
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
    async fn issue_token(
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
            owner_id: params.username.clone(),
            client_id: client.client_id.clone(),
            scope,
            redirect_uri,
            until: chrono::Utc::now() + chrono::Duration::seconds(params.duration as i64),
            extensions: Extensions::default(),
        };

        let issuer_name = config
            .access
            .iss
            .clone()
            .unwrap_or_else(|| "LaserSmartServer".to_string());

        let token = jwt_issuer
            .with_issuer(issuer_name)
            .valid_for(chrono::TimeDelta::seconds(params.duration as i64))
            .with_algorithm(params.algorithm.to_jsonwebtoken_algorithm())
            .add_user_claims(&params.username, &user.permissions)
            .issue(grant)
            .map_err(|e| TokenCreationError::TokenIssuingError {
                reason: format!("Failed to issue JWT token: {:?}", e),
            })?;

        Ok(token.token)
    }
}
