// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # JWT Key Management
//!
//! This module contains functionality for managing JWT signing and verification keys,
//! with support for both symmetric and asymmetric keys.
//!
//! ## Features
//!
//! * Support for symmetric keys (HMAC)
//! * Support for asymmetric RSA key pairs
//! * Support for asymmetric Elliptic Curve (EC) key pairs
//! * Key loading from files or memory
//! * Algorithm validation for key types
//!
//! ## Examples
//!
//! ### Creating a symmetric key configuration
//!
//! ```rust
//! use rust_photoacoustic::visualization::auth::jwt::JwtKeyConfig;
//! use jsonwebtoken::Algorithm;
//!
//! // Create a new key configuration with HS256 algorithm
//! let secret = b"your-hmac-secret-key";
//! let key_config = JwtKeyConfig::new_symmetric(secret, Algorithm::HS256).unwrap();
//! ```
//!
//! ### Creating an RSA key configuration from files
//!
//! ```rust,no_run
//! use rust_photoacoustic::visualization::auth::jwt::JwtKeyConfig;
//! use jsonwebtoken::Algorithm;
//!
//! // Load RSA keys from PEM files
//! let key_config = JwtKeyConfig::new_rsa(
//!     "path/to/private_key.pem",
//!     "path/to/public_key.pem",
//!     Algorithm::RS256
//! ).unwrap();
//! ```
//!
//! ### Creating an EC key configuration from PEM data
//!
//! ```rust,no_run
//! use rust_photoacoustic::visualization::auth::jwt::JwtKeyConfig;
//! use jsonwebtoken::Algorithm;
//!
//! // EC private and public keys in PEM format
//! let private_key = b"-----BEGIN EC PRIVATE KEY-----\n...\n-----END EC PRIVATE KEY-----";
//! let public_key = b"-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----";
//!
//! let key_config = JwtKeyConfig::new_ec_from_pem(
//!     private_key,
//!     public_key,
//!     Algorithm::ES256
//! ).unwrap();
//! ```

use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::prelude::*;
use jsonwebtoken::jwk::{Jwk, PublicKeyUse};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey};
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::sha2::Digest;
use rsa::sha2::Sha256;
use rsa::traits::PublicKeyParts;
use rsa::RsaPublicKey;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Types of JWT keys supported by the application
///
/// This enum represents the different types of cryptographic keys that can be used
/// for JWT signing and verification operations in the application.
#[derive(Debug, Clone, Copy)]
pub enum KeyType {
    /// Symmetric key (HMAC)
    ///
    /// Uses the same key for both signing and verification.
    /// Suitable algorithms: HS256, HS384, HS512
    Symmetric,

    /// RSA key pair
    ///
    /// Uses an asymmetric RSA key pair where the private key is used for signing
    /// and the public key is used for verification.
    /// Suitable algorithms: RS256, RS384, RS512, PS256, PS384, PS512
    RSA,

    /// Elliptic Curve key pair
    ///
    /// Uses an asymmetric Elliptic Curve key pair where the private key is used for signing
    /// and the public key is used for verification.
    /// Suitable algorithms: ES256, ES384
    EC,
}

/// JWT key configuration for the application
///
/// This struct holds the configuration for JWT signing and verification operations,
/// including the algorithm and cryptographic keys to use. It provides a unified interface
/// for working with different types of keys (symmetric and asymmetric).
///
/// # Key Types
///
/// * **Symmetric Keys**: The same key is used for both signing and verification (HMAC)
/// * **RSA Keys**: A private key is used for signing, and a public key for verification
/// * **EC Keys**: An Elliptic Curve private key is used for signing, and a public key for verification
pub struct JwtKeyConfig {
    /// Algorithm to use for signing
    ///
    /// The JWT signing algorithm determines how tokens are signed and verified.
    /// Make sure to choose an algorithm compatible with your key type.
    pub algorithm: Algorithm,

    /// Key type
    ///
    /// Indicates whether this configuration uses symmetric or asymmetric keys,
    /// and which asymmetric key type (RSA or EC) if applicable.
    pub key_type: KeyType,

    /// Encoding key for signing tokens
    ///
    /// This key is used to sign JWT tokens. For symmetric keys, this is derived from
    /// the secret. For asymmetric keys, this is the private key.
    pub encoding_key: EncodingKey,

    /// Decoding key for verifying tokens
    ///
    /// This key is used to verify JWT token signatures. For symmetric keys, this is
    /// derived from the same secret as the encoding key. For asymmetric keys, this
    /// is the public key.
    pub decoding_key: DecodingKey,
}

/// Custom debug implementation for JwtKeyConfig that hides sensitive key material
///
/// This implementation ensures that the actual key material isn't accidentally
/// exposed in logs or debug output.
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
    ///
    /// This method creates a key configuration using a symmetric HMAC key.
    /// The same secret is used for both signing and verifying JWTs.
    ///
    /// # Arguments
    ///
    /// * `secret` - The secret bytes to use for HMAC signing/verification
    /// * `algorithm` - The HMAC algorithm to use (must be one of: HS256, HS384, HS512)
    ///
    /// # Returns
    ///
    /// * `Result<JwtKeyConfig>` - The key configuration if successful
    ///
    /// # Errors
    ///
    /// Returns an error if the algorithm is not compatible with symmetric keys.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_photoacoustic::visualization::auth::jwt::JwtKeyConfig;
    /// use jsonwebtoken::Algorithm;
    ///
    /// let secret = b"your-secure-secret-key";
    /// let key_config = JwtKeyConfig::new_symmetric(secret, Algorithm::HS256).unwrap();
    /// ```
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
    ///
    /// This method loads RSA keys from PEM files and creates a configuration
    /// for signing and verifying JWTs using RSA algorithms.
    ///
    /// # Arguments
    ///
    /// * `private_key_path` - Path to the PEM file containing the RSA private key
    /// * `public_key_path` - Path to the PEM file containing the RSA public key
    /// * `algorithm` - The RSA algorithm to use (must be one of: RS256, RS384, RS512, PS256, PS384, PS512)
    ///
    /// # Returns
    ///
    /// * `Result<JwtKeyConfig>` - The key configuration if successful
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The algorithm is not compatible with RSA keys
    /// - Either key file cannot be read
    /// - The keys are not valid RSA PEM keys
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_photoacoustic::visualization::auth::jwt::JwtKeyConfig;
    /// use jsonwebtoken::Algorithm;
    ///
    /// let key_config = JwtKeyConfig::new_rsa(
    ///     "keys/rsa_private.pem",
    ///     "keys/rsa_public.pem",
    ///     Algorithm::RS256
    /// ).unwrap();
    /// ```
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
    ///
    /// This method creates a configuration for signing and verifying JWTs using RSA
    /// keys provided directly as PEM-formatted byte arrays in memory.
    ///
    /// # Arguments
    ///
    /// * `private_key` - Byte array containing the RSA private key in PEM format
    /// * `public_key` - Byte array containing the RSA public key in PEM format
    /// * `algorithm` - The RSA algorithm to use (must be one of: RS256, RS384, RS512, PS256, PS384, PS512)
    ///
    /// # Returns
    ///
    /// * `Result<JwtKeyConfig>` - The key configuration if successful
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The algorithm is not compatible with RSA keys
    /// - Either key is not a valid RSA PEM key
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_photoacoustic::visualization::auth::jwt::JwtKeyConfig;
    /// use jsonwebtoken::Algorithm;
    ///
    /// let private_key = b"-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----";
    /// let public_key = b"-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----";
    ///
    /// let key_config = JwtKeyConfig::new_rsa_from_pem(
    ///     private_key,
    ///     public_key,
    ///     Algorithm::RS256
    /// ).unwrap();
    /// ```
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
    ///
    /// This method loads Elliptic Curve keys from PEM files and creates a configuration
    /// for signing and verifying JWTs using EC algorithms.
    ///
    /// # Arguments
    ///
    /// * `private_key_path` - Path to the PEM file containing the EC private key
    /// * `public_key_path` - Path to the PEM file containing the EC public key
    /// * `algorithm` - The EC algorithm to use (must be one of: ES256, ES384)
    ///
    /// # Returns
    ///
    /// * `Result<JwtKeyConfig>` - The key configuration if successful
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The algorithm is not compatible with EC keys
    /// - Either key file cannot be read
    /// - The keys are not valid EC PEM keys
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_photoacoustic::visualization::auth::jwt::JwtKeyConfig;
    /// use jsonwebtoken::Algorithm;
    ///
    /// let key_config = JwtKeyConfig::new_ec(
    ///     "keys/ec_private.pem",
    ///     "keys/ec_public.pem",
    ///     Algorithm::ES256
    /// ).unwrap();
    /// ```
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
    ///
    /// This method creates a configuration for signing and verifying JWTs using EC
    /// keys provided directly as PEM-formatted byte arrays in memory.
    ///
    /// # Arguments
    ///
    /// * `private_key` - Byte array containing the EC private key in PEM format
    /// * `public_key` - Byte array containing the EC public key in PEM format
    /// * `algorithm` - The EC algorithm to use (must be one of: ES256, ES384)
    ///
    /// # Returns
    ///
    /// * `Result<JwtKeyConfig>` - The key configuration if successful
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The algorithm is not compatible with EC keys
    /// - Either key is not a valid EC PEM key
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_photoacoustic::visualization::auth::jwt::JwtKeyConfig;
    /// use jsonwebtoken::Algorithm;
    ///
    /// let private_key = b"-----BEGIN EC PRIVATE KEY-----\n...\n-----END EC PRIVATE KEY-----";
    /// let public_key = b"-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----";
    ///
    /// let key_config = JwtKeyConfig::new_ec_from_pem(
    ///     private_key,
    ///     public_key,
    ///     Algorithm::ES256
    /// ).unwrap();
    /// ```
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
    ///
    /// This is a convenience method for quickly creating a symmetric key configuration
    /// using the HS256 algorithm, which is suitable for most basic JWT use cases.
    ///
    /// # Arguments
    ///
    /// * `secret` - The secret bytes to use for HMAC signing/verification
    ///
    /// # Returns
    ///
    /// A JwtKeyConfig configured with HS256 algorithm and the provided secret
    ///
    /// # Example
    ///
    /// ```rust
    /// use rust_photoacoustic::visualization::auth::jwt::JwtKeyConfig;
    ///
    /// let secret = b"your-secret-key-for-signing";
    /// let key_config = JwtKeyConfig::default_from_secret(secret);
    /// ```
    pub fn default_from_secret(secret: &[u8]) -> Self {
        Self {
            algorithm: Algorithm::HS256,
            key_type: KeyType::Symmetric,
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
        }
    }

    /// Generate a key pair for testing purposes
    ///
    /// This method creates a symmetric key configuration suitable for testing.
    /// It should not be used in production environments.
    ///
    /// # Returns
    ///
    /// * `Result<JwtKeyConfig>` - A key configuration for testing
    ///
    /// # Example
    ///
    /// ```
    /// #[cfg(test)]
    /// use rust_photoacoustic::visualization::auth::jwt::JwtKeyConfig;
    ///
    /// #[test]
    /// fn test_jwt_operations() {
    ///     let key_config = JwtKeyConfig::generate_test_key_pair().unwrap();
    ///     // Use key_config for JWT operations in tests
    /// }
    /// ```
    #[cfg(test)]
    pub fn generate_test_key_pair() -> Result<Self> {
        // For testing, just use a symmetric key
        Self::new_symmetric(
            b"test-secret-key-for-jwt-token-testing-only",
            Algorithm::HS256,
        )
    }
}

/// JSON Web Key Set
///
/// This structure represents a set of JSON Web Keys (JWKs) as defined in RFC 7517.
/// It can be used to generate and manipulate JWK representations of RSA keys for
/// use with OpenID Connect discovery endpoints.
#[derive(Debug, Serialize, Deserialize)]
pub struct JwkKeySet {
    /// The set of JWKs
    pub keys: Vec<Jwk>,
}

impl JwkKeySet {
    /// Create a new JWK from a PEM encoded RSA public key
    ///
    /// This function converts a PEM encoded RSA public key to a JWK (JSON Web Key)
    /// representation suitable for use with OpenID Connect discovery endpoints.
    ///
    /// # Parameters
    ///
    /// * `pem_data` - The PEM encoded RSA public key as bytes
    ///
    /// # Returns
    ///
    /// A JWK representing the RSA public key, or an error if parsing fails
    pub fn create_jwk_from_pem(pem_data: &[u8]) -> Result<Jwk> {
        // Parse the PEM key
        let public_key = DecodeRsaPublicKey::from_pkcs1_pem(std::str::from_utf8(pem_data)?)
            .context("Failed to parse RSA public key from PEM")?;

        // Convert to JWK
        Self::create_jwk_from_public_key(&public_key)
    }

    /// Create a JWK from an RSA public key
    ///
    /// Converts an RSA public key to a JWK representation with the necessary
    /// parameters for use with OpenID Connect.
    ///
    /// # Parameters
    ///
    /// * `public_key` - The RSA public key
    ///
    /// # Returns
    ///
    /// A JWK representing the RSA public key
    pub fn create_jwk_from_public_key(public_key: &RsaPublicKey) -> Result<Jwk> {
        // Get the modulus (n) and exponent (e) from the public key
        let n = public_key.n();
        let n = BASE64_STANDARD.encode(public_key.n().to_bytes_be());
        let e = BASE64_STANDARD.encode(public_key.e().to_bytes_be());

        // Calculate the key ID (kid) as a SHA-256 thumbprint
        let jwk_thumbprint = Self::calculate_jwk_thumbprint(&n, &e)?;

        // Build the JWK
        let jwk = Jwk {
            common: jsonwebtoken::jwk::CommonParameters {
                public_key_use: Some(PublicKeyUse::Signature),
                key_id: Some(jwk_thumbprint),
                key_algorithm: Some(jsonwebtoken::jwk::KeyAlgorithm::RS256), // Correct field name and type
                ..Default::default()
            },
            algorithm: jsonwebtoken::jwk::AlgorithmParameters::RSA(
                jsonwebtoken::jwk::RSAKeyParameters {
                    key_type: jsonwebtoken::jwk::RSAKeyType::RSA,
                    n,
                    e,
                    ..Default::default()
                },
            ),
        };

        Ok(jwk)
    }

    /// Calculate a JWK thumbprint according to RFC 7638
    ///
    /// This function calculates a thumbprint for a JWK which can be used as
    /// a key ID (kid) parameter. The thumbprint is a SHA-256 hash of the
    /// canonical JSON representation of the JWK.
    ///
    /// # Parameters
    ///
    /// * `n` - Base64URL encoded modulus
    /// * `e` - Base64URL encoded exponent
    ///
    /// # Returns
    ///
    /// Base64URL encoded SHA-256 thumbprint
    fn calculate_jwk_thumbprint(n: &str, e: &str) -> Result<String> {
        // Create canonical JWK representation
        let canonical = json!({
            "e": e,
            "kty": "RSA",
            "n": n
        });

        // Serialize to bytes in lexicographic order
        let canonical_bytes = serde_json::to_vec(&canonical)?;

        // Calculate SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(&canonical_bytes);
        let hash = hasher.finalize();

        // Encode as Base64URL
        let thumbprint = URL_SAFE_NO_PAD.encode(hash);

        Ok(thumbprint)
    }
}
