// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! JWT Key Management
//!
//! This module contains functionality for managing JWT signing and verification keys,
//! with support for both symmetric and asymmetric keys.

use anyhow::{anyhow, Result};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Types of JWT keys supported by the application
#[derive(Debug, Clone, Copy)]
pub enum KeyType {
    /// Symmetric key (HMAC)
    Symmetric,
    /// RSA key pair
    RSA,
    /// Elliptic Curve key pair
    EC,
}

/// JWT key configuration for the application
pub struct JwtKeyConfig {
    /// Algorithm to use for signing
    pub algorithm: Algorithm,
    /// Key type
    pub key_type: KeyType,
    /// Encoding key for signing tokens
    pub encoding_key: EncodingKey,
    /// Decoding key for verifying tokens
    pub decoding_key: DecodingKey,
}

impl std::fmt::Debug for JwtKeyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtKeyConfig")
            .field("algorithm", &self.algorithm)
            .field("key_type", &self.key_type)
            .field("encoding_key", &"<EncodingKey>")
            .field("decoding_key", &"<DecodingKey>")
            .finish()
    }
}

impl JwtKeyConfig {
    /// Create a new JWT key configuration with a symmetric key (HMAC)
    pub fn new_symmetric(secret: &[u8], algorithm: Algorithm) -> Result<Self> {
        match algorithm {
            Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => (),
            _ => {
                return Err(anyhow!(
                    "Algorithm {:?} is not valid for symmetric keys",
                    algorithm
                ))
            }
        }

        Ok(Self {
            algorithm,
            key_type: KeyType::Symmetric,
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
        })
    }

    /// Create a new JWT key configuration with RSA keys
    pub fn new_rsa(
        private_key_path: impl AsRef<Path>,
        public_key_path: impl AsRef<Path>,
        algorithm: Algorithm,
    ) -> Result<Self> {
        match algorithm {
            Algorithm::RS256
            | Algorithm::RS384
            | Algorithm::RS512
            | Algorithm::PS256
            | Algorithm::PS384
            | Algorithm::PS512 => (),
            _ => {
                return Err(anyhow!(
                    "Algorithm {:?} is not valid for RSA keys",
                    algorithm
                ))
            }
        }

        let mut private_key = Vec::new();
        File::open(private_key_path)?.read_to_end(&mut private_key)?;

        let mut public_key = Vec::new();
        File::open(public_key_path)?.read_to_end(&mut public_key)?;

        Ok(Self {
            algorithm,
            key_type: KeyType::RSA,
            encoding_key: EncodingKey::from_rsa_pem(&private_key)?,
            decoding_key: DecodingKey::from_rsa_pem(&public_key)?,
        })
    }

    /// Create a new JWT key configuration with RSA keys from PEM strings
    pub fn new_rsa_from_pem(
        private_key: &[u8],
        public_key: &[u8],
        algorithm: Algorithm,
    ) -> Result<Self> {
        match algorithm {
            Algorithm::RS256
            | Algorithm::RS384
            | Algorithm::RS512
            | Algorithm::PS256
            | Algorithm::PS384
            | Algorithm::PS512 => (),
            _ => {
                return Err(anyhow!(
                    "Algorithm {:?} is not valid for RSA keys",
                    algorithm
                ))
            }
        }

        Ok(Self {
            algorithm,
            key_type: KeyType::RSA,
            encoding_key: EncodingKey::from_rsa_pem(private_key)?,
            decoding_key: DecodingKey::from_rsa_pem(public_key)?,
        })
    }

    /// Create a new JWT key configuration with EC keys
    pub fn new_ec(
        private_key_path: impl AsRef<Path>,
        public_key_path: impl AsRef<Path>,
        algorithm: Algorithm,
    ) -> Result<Self> {
        match algorithm {
            Algorithm::ES256 | Algorithm::ES384 => (),
            _ => {
                return Err(anyhow!(
                    "Algorithm {:?} is not valid for EC keys",
                    algorithm
                ))
            }
        }

        let mut private_key = Vec::new();
        File::open(private_key_path)?.read_to_end(&mut private_key)?;

        let mut public_key = Vec::new();
        File::open(public_key_path)?.read_to_end(&mut public_key)?;

        Ok(Self {
            algorithm,
            key_type: KeyType::EC,
            encoding_key: EncodingKey::from_ec_pem(&private_key)?,
            decoding_key: DecodingKey::from_ec_pem(&public_key)?,
        })
    }

    /// Create a new JWT key configuration with EC keys from PEM strings
    pub fn new_ec_from_pem(
        private_key: &[u8],
        public_key: &[u8],
        algorithm: Algorithm,
    ) -> Result<Self> {
        match algorithm {
            Algorithm::ES256 | Algorithm::ES384 => (),
            _ => {
                return Err(anyhow!(
                    "Algorithm {:?} is not valid for EC keys",
                    algorithm
                ))
            }
        }

        Ok(Self {
            algorithm,
            key_type: KeyType::EC,
            encoding_key: EncodingKey::from_ec_pem(private_key)?,
            decoding_key: DecodingKey::from_ec_pem(public_key)?,
        })
    }

    /// Create a default HS256 key configuration from a secret
    pub fn default_from_secret(secret: &[u8]) -> Self {
        Self {
            algorithm: Algorithm::HS256,
            key_type: KeyType::Symmetric,
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
        }
    }

    /// Generate a key pair for testing purposes
    #[cfg(test)]
    pub fn generate_test_key_pair() -> Result<Self> {
        // For testing, just use a symmetric key
        Self::new_symmetric(
            b"test-secret-key-for-jwt-token-testing-only",
            Algorithm::HS256,
        )
    }
}
