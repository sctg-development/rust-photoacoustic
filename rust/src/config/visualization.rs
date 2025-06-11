// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Visualization server configuration
//!
//! This module defines the structure for configuring the web-based
//! visualization server in the photoacoustic application.

use base64::Engine;
use serde::{Deserialize, Serialize};

/// Configuration for the visualization web server.
///
/// This structure contains all settings required for the visualization server component,
/// including network binding parameters, TLS certificate settings, and authentication
/// configuration with both HMAC and RSA key-based JWT options.
///
/// ### Security Options
///
/// The structure supports two JWT authentication mechanisms:
///
/// 1. **HMAC-based JWT**: A simple secret key used for both signing and verification
/// 2. **RS256-based JWT**: More secure public/private key pair where:
///    - Private key is used for signing tokens
///    - Public key is used for verifying tokens
///
/// The RS256 keys can be generated using the included `rs256keygen` binary.
///
/// ### TLS Configuration
///
/// For secure HTTPS connections, both `cert` and `key` fields must be provided as
/// Base64-encoded PEM files. If either is missing, the server will operate in non-TLS mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationConfig {
    /// The TCP port the visualization server will listen on.
    ///
    /// Valid range is 1-65534. Default value is 8080.
    #[serde(default = "default_port")]
    pub port: u16,

    /// The network address the server will bind to.
    ///
    /// Can be an IPv4/IPv6 address or a hostname. Default is "127.0.0.1".
    /// Use "0.0.0.0" to bind to all IPv4 interfaces.
    #[serde(default = "default_address")]
    pub address: String,

    /// The server name reported in HTTP headers and logs.
    ///
    /// Default is "LaserSmartApiServer/" followed by the package version.
    #[serde(default = "default_name")]
    pub name: String,

    /// SSL/TLS certificate in PEM format, Base64 encoded.
    ///
    /// If provided, `key` must also be supplied. For development,
    /// defaults to the certificate in the resources directory.
    #[serde(default = "default_cert")]
    pub cert: Option<String>,

    /// SSL/TLS private key in PEM format, Base64 encoded.
    ///
    /// If provided, `cert` must also be supplied. For development,
    /// defaults to the key in the resources directory.
    #[serde(default = "default_key")]
    pub key: Option<String>,

    /// Secret key for HMAC-based JWT token signing and verification.
    ///
    /// Used when RS256 keys are not available or for simpler deployments.
    /// Not recommended for production environments.
    #[serde(default = "default_hmac_secret")]
    pub hmac_secret: String,

    /// RS256 private key in PEM format, Base64 encoded.
    ///
    /// Used for signing JWT tokens with the RS256 algorithm.
    /// Can be generated with the `rs256keygen` binary.
    #[serde(default = "default_rs256_private_key")]
    pub rs256_private_key: String,

    /// RS256 public key in PEM format, Base64 encoded.
    ///
    /// Used for verifying JWT tokens signed with the RS256 algorithm.
    /// Can be generated with the `rs256keygen` binary.
    #[serde(default = "default_rs256_public_key")]
    pub rs256_public_key: String,

    /// Enable or disable the visualization server.
    ///
    /// This flag can be used to easily enable or disable the server
    /// without removing the configuration. Default is `true`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Session secret key for cookie-based authentication.
    #[serde(default = "default_session_secret")]
    pub session_secret: String,

    /// Optional enable compression for the server responses.
    /// This can help reduce the size of the data sent over the network,
    /// improving performance for large responses.
    /// Default is `true`, meaning compression is enabled.
    #[serde(default = "default_enabled")]
    pub enable_compression: bool,
}

/// Provides the default TCP port (8080) for the visualization server.
///
/// This port is commonly used for development web servers and is
/// generally available on most systems.
fn default_port() -> u16 {
    8080
}

/// Provides the default network binding address (127.0.0.1) for the visualization server.
///
/// This loopback address ensures the server only accepts connections from the local machine,
/// which is secure for development purposes. For production use where remote connections
/// are required, this should be changed to "0.0.0.0" or a specific network interface.
fn default_address() -> String {
    "127.0.0.1".to_string()
}

/// Generates the default server name string based on the current package version.
///
/// The server name is included in HTTP headers and used in logs to identify
/// this specific instance of the visualization server.
fn default_name() -> String {
    format!("LaserSmartApiServer/{}", env!("CARGO_PKG_VERSION"))
}

// Use if exists the ../resources/cert.pem file converted to base64 at build time
fn default_cert() -> Option<String> {
    let cert_str = include_str!("../../resources/cert.pem");
    if cert_str.is_empty() {
        None
    } else {
        let cert_b64 = base64::engine::general_purpose::STANDARD.encode(cert_str.as_bytes());
        Some(cert_b64.to_string())
    }
}

// Use if exists the ../resources/cert.key file converted to base64 at build time
fn default_key() -> Option<String> {
    let key_str = include_str!("../../resources/cert.key");
    if key_str.is_empty() {
        None
    } else {
        let key_b64 = base64::engine::general_purpose::STANDARD.encode(key_str.as_bytes());
        Some(key_b64.to_string())
    }
}

/// Provides the default HMAC secret key for JWT token signing.
///
/// This key is used for HMAC-based JWT authentication. It should be kept secret
/// and not shared publicly. The default value is a placeholder and should be
/// replaced with a strong, randomly generated key in production environments.
/// The key should be at least 256 bits (32 bytes) long for security.
fn default_hmac_secret() -> String {
    "my-super-secret-jwt-key-for-photoacoustic-app".to_string()
}

/// Provides the default RS256 private key for JWT token signing.
///
/// This key is used for RS256-based JWT authentication. It should be kept secret
/// and not shared publicly. The default value is a placeholder and should be
/// replaced with a strong, randomly generated key in production environments.
/// The key should be in PEM format and Base64 encoded.
fn default_rs256_private_key() -> String {
    let key_str = include_str!("../../resources/private.key");
    if key_str.is_empty() {
        String::new()
    } else {
        base64::engine::general_purpose::STANDARD.encode(key_str.as_bytes())
    }
}

/// Provides the default RS256 public key for JWT token verification.
///
/// This key is used for verifying JWT tokens signed with the RS256 algorithm.
/// It should be shared publicly to allow clients to verify the tokens.
/// The default value is a placeholder and should be replaced with a strong,
/// randomly generated key in production environments.
/// The key should be in PEM format and Base64 encoded.
fn default_rs256_public_key() -> String {
    let key_str = include_str!("../../resources/pub.key");
    if key_str.is_empty() {
        String::new()
    } else {
        base64::engine::general_purpose::STANDARD.encode(key_str.as_bytes())
    }
}

/// Provides the default enabled state for the visualization server.
///
/// This flag indicates whether the server should be started by default.
/// The default value is `true`, meaning the server will be enabled unless
/// explicitly disabled in the configuration file.
fn default_enabled() -> bool {
    true
}

/// Generate a random session secret key for cookie-based authentication.
fn default_session_secret() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let secret: [u8; 32] = rng.random();
    base64::engine::general_purpose::STANDARD.encode(secret)
}

impl Default for VisualizationConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            address: default_address(),
            name: default_name(),
            cert: default_cert(),
            key: default_key(),
            hmac_secret: default_hmac_secret(),
            rs256_private_key: default_rs256_private_key(),
            rs256_public_key: default_rs256_public_key(),
            enabled: default_enabled(),
            session_secret: default_session_secret(),
            enable_compression: default_enabled(),
        }
    }
}
