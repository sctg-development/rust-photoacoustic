// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use crate::error::TokenCreationError;
use rust_photoacoustic::config::{access::Client, access::User, Config};
use std::path::Path;

/// Configuration loader with validation
pub struct ConfigLoader {
    config: Config,
}

impl ConfigLoader {
    /// Load and validate configuration
    pub fn load<P: AsRef<Path>>(config_path: P) -> Result<Self, TokenCreationError> {
        let config = Config::from_file(config_path.as_ref())
            .map_err(|e| TokenCreationError::ConfigError { source: e.into() })?;

        Ok(Self { config })
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

    /// Return the token duration (with possible override)
    pub fn get_token_duration(&self, override_duration: Option<u64>) -> u64 {
        override_duration.unwrap_or(self.config.access.duration.unwrap_or(86400) as u64)
    }

    /// List all available users
    pub fn list_users(&self) -> Vec<&str> {
        self.config
            .access
            .users
            .iter()
            .map(|u| u.user.as_str())
            .collect()
    }

    /// List all available clients
    pub fn list_clients(&self) -> Vec<&str> {
        self.config
            .access
            .clients
            .iter()
            .map(|c| c.client_id.as_str())
            .collect()
    }
}
