// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use thiserror::Error;

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

impl TokenCreationError {
    pub fn exit_code(&self) -> i32 {
        match self {
            TokenCreationError::ConfigError { .. } => 1,
            TokenCreationError::UserNotFound { .. } => 2,
            TokenCreationError::ClientNotFound { .. } => 3,
            TokenCreationError::KeyDecodingError { .. } => 4,
            TokenCreationError::JwtIssuerCreationError { .. } => 5,
            TokenCreationError::InvalidScope { .. } => 6,
            TokenCreationError::InvalidRedirectUri { .. } => 7,
            TokenCreationError::NoRedirectUri => 8,
            TokenCreationError::TokenIssuingError { .. } => 9,
        }
    }
}
