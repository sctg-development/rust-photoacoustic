// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Configuration utilities
//!
//! This module provides utility functions for working with configuration
//! settings, including validation and schema management.

use anyhow::{Context, Result};
use base64::Engine;
use log::debug;

use super::{Config, USER_SESSION_SEPARATOR};

/// Output the embedded JSON schema to the console.
///
/// This function is called when the `--show-config-schema` flag is provided
/// on the command line. It outputs the full JSON schema for the configuration
/// to stdout, formatted for readability.
///
/// # Example
///
/// ```bash
/// ./rust_photoacoustic --show-config-schema > config_schema.json
/// ```
pub fn output_config_schema() -> Result<()> {
    // Load the schema from the embedded string
    let schema_str = include_str!("../../resources/config.schema.json");

    // Parse the schema to a JSON Value to pretty-format it
    let schema: serde_json::Value =
        serde_json::from_str(schema_str).context("Failed to parse JSON schema")?;

    // Pretty-print the schema
    let formatted_schema =
        serde_json::to_string_pretty(&schema).context("Failed to format JSON schema")?;

    // Output to stdout
    println!("{}", formatted_schema);

    Ok(())
}

/// Check if a string is a valid IP address
///
/// Validates that a string represents a valid IPv4 or IPv6 address,
/// or is one of the special values like "localhost" or "0.0.0.0".
///
/// # Arguments
///
/// * `addr` - The address string to validate
///
/// # Returns
///
/// `true` if the address is valid, `false` otherwise
pub fn is_valid_ip_address(addr: &str) -> bool {
    if addr.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }

    // Special cases
    matches!(addr, "localhost" | "::" | "::0" | "0.0.0.0")
}

/// Validates the configuration against additional rules that aren't covered by the JSON schema.
///
/// This function performs deeper validation checks that can't be easily expressed in a JSON schema,
/// such as verifying that certificate and key pairs are both present, validating base64 encoding
/// of cryptographic material, and checking user password hashes.
///
/// # Arguments
///
/// * `config` - The configuration object to validate
///
/// # Returns
///
/// * `Ok(())` if all validations pass
/// * `Err(anyhow::Error)` with descriptive message if any validation fails
///
/// # Validation Rules
///
/// This function validates:
///
/// - **SSL Configuration**: Ensures that if a certificate is provided, a key is also provided (and vice versa)
/// - **Base64 Encoding**: Validates that certificates, keys, and RS256 keys are valid base64-encoded strings
/// - **Port Range**: Ensures the visualization port is within a valid range (1-65534)
/// - **IP Address Format**: Checks if the provided address is a valid IP address or special value
/// - **User Credentials**: Validates that user password hashes are properly base64-encoded and follow
///   the expected format from `openssl passwd`
pub fn validate_specific_rules(config: &Config) -> Result<()> {
    debug!("Performing additional validation checks");

    // Validate SSL certificates
    if let Some(cert) = &config.visualization.cert {
        if config.visualization.key.is_none() {
            anyhow::bail!("SSL certificate provided without a key");
        }

        // Validate the cert is valid base64
        let _ = base64::engine::general_purpose::STANDARD
            .decode(cert)
            .context("SSL certificate is not valid base64")?;
    }

    if let Some(key) = &config.visualization.key {
        if config.visualization.cert.is_none() {
            anyhow::bail!("SSL key provided without a certificate");
        }

        // Validate the key is valid base64
        let _ = base64::engine::general_purpose::STANDARD
            .decode(key)
            .context("SSL key is not valid base64")?;
    }

    // Check value ranges for certain fields
    if config.visualization.port < 1 || config.visualization.port > 65534 {
        anyhow::bail!("Invalid port number: {}", config.visualization.port);
    }

    // Check if the address is in a valid format
    if !is_valid_ip_address(&config.visualization.address) {
        debug!(
            "Potentially invalid address format: {}",
            config.visualization.address
        );
        // Just issue a warning but don't block
    }

    // Validate the rs256_private_key and rs256_public_key they should some valid base64 encoded strings
    let _ = base64::engine::general_purpose::STANDARD
        .decode(&config.visualization.rs256_private_key)
        .context("RS256 private key is not valid base64")?;
    let _ = base64::engine::general_purpose::STANDARD
        .decode(&config.visualization.rs256_public_key)
        .context("RS256 public key is not valid base64")?;

    // if AccessConfig contains users, validate their credentials
    // User password should be a valid base64 string
    // the decoded string should be a valid password hash conforming to the openssl passwd -1 format
    // permissions should not contain the char USER_SESSION_SEPARATOR
    for user in &config.access.users {
        if !user.pass.is_empty() {
            let decoded_pass = base64::engine::general_purpose::STANDARD
                .decode(&user.pass)
                .context("User password is not valid base64")?;
            // Check if the decoded password is a valid hash
            // Password hash should start with $1$, $5$, $6$, $apr1$
            // Next contains the salt
            // The rest is the hash
            if !decoded_pass.starts_with(b"$1$")
                && !decoded_pass.starts_with(b"$5$")
                && !decoded_pass.starts_with(b"$6$")
                && !decoded_pass.starts_with(b"$apr1$")
            {
                anyhow::bail!("User password is not a valid hash, you should use openssl passwd -5 <password> | base64 -w0");
            }
        }
        for permission in &user.permissions {
            if permission.contains(USER_SESSION_SEPARATOR) {
                anyhow::bail!(
                    "User permission contains invalid character: {}",
                    USER_SESSION_SEPARATOR
                );
            }
        }
    }

    Ok(())
}
