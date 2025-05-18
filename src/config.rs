// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use anyhow::{Context, Result};
use base64::Engine;
use jsonschema;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

/// Configuration structure for the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Visualization settings
    #[serde(default)]
    pub visualization: VisualizationConfig,
}

/// Configuration for the visualization server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationConfig {
    /// The port to listen on
    #[serde(default = "default_port")]
    pub port: u16,
    /// The address to bind to
    #[serde(default = "default_address")]
    pub address: String,
    /// The server name
    #[serde(default = "default_name")]
    pub name: String,
    /// SSL certificate PEM data (Base64 encoded)
    #[serde(default = "default_cert")]
    pub cert: Option<String>,
    /// SSL key PEM data (Base64 encoded)
    #[serde(default = "default_key")]
    pub key: Option<String>,
    /// HMAC secret for JWT signing
    #[serde(default = "default_hmac_secret")]
    pub hmac_secret: String,
    /// RS256 private key PEM data (Base64 encoded)
    #[serde(default = "default_rs256_private_key")]
    pub rs256_private_key: String,
    /// RS256 public key PEM data (Base64 encoded)
    #[serde(default = "default_rs256_public_key")]
    pub rs256_public_key: String,
}

fn default_port() -> u16 {
    8080
}

fn default_address() -> String {
    "127.0.0.1".to_string()
}

fn default_name() -> String {
    format!("LaserSmartApiServer/{}", env!("CARGO_PKG_VERSION"))
}

// Use if exists the ../resources/cert.pem file converted to base64 at build time
fn default_cert() -> Option<String> {
    let cert_str = include_str!("../resources/cert.pem");
    if cert_str.is_empty() {
        None
    } else {
        let cert_b64 = base64::engine::general_purpose::STANDARD.encode(cert_str.as_bytes());
        Some(cert_b64.to_string())
    }
}
// Use if exists the ../resources/cert.key file converted to base64 at build time
fn default_key() -> Option<String> {
    let key_str = include_str!("../resources/cert.key");
    if key_str.is_empty() {
        None
    } else {
        let key_b64 = base64::engine::general_purpose::STANDARD.encode(key_str.as_bytes());
        Some(key_b64.to_string())
    }
}

fn default_hmac_secret() -> String {
    "my-super-secret-jwt-key-for-photoacoustic-app".to_string()
}

fn default_rs256_private_key() -> String {
    let key_str = include_str!("../resources/private.key");
    if key_str.is_empty() {
        String::new()
    } else {
        base64::engine::general_purpose::STANDARD.encode(key_str.as_bytes())
    }
}
fn default_rs256_public_key() -> String {
    let key_str = include_str!("../resources/pub.key");
    if key_str.is_empty() {
        String::new()
    } else {
        base64::engine::general_purpose::STANDARD.encode(key_str.as_bytes())
    }
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
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            visualization: VisualizationConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from a file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            debug!(
                "Configuration file not found at {:?}, creating default",
                path
            );
            let default_config = Self::default();
            default_config.save_to_file(path)?;
            return Ok(default_config);
        }

        debug!("Loading configuration from {:?}", path);
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read configuration file at {:?}", path))?;

        // First step: convert YAML to a generic Value
        let yaml_value: serde_yml::Value = serde_yml::from_str(&contents)
            .with_context(|| format!("Failed to parse YAML configuration from {:?}", path))?;

        // Convert to JSON Value for validation
        let json_value = serde_json::to_value(&yaml_value)
            .context("Failed to convert YAML to JSON for validation")?;

        debug!(
            "Raw YAML converted to JSON for validation: {:?}",
            json_value
        );

        // Load and validate with the schema
        let schema_str = include_str!("../resources/config.schema.json");
        let schema: serde_json::Value =
            serde_json::from_str(schema_str).context("Failed to parse JSON schema")?;

        // Create the validator
        let validator = jsonschema::draft202012::options()
            .should_validate_formats(true)
            .build(&schema)?;

        // Validate before deserializing to Config
        debug!("Validating {} configuration against schema", path.display());
        if let Err(error) = validator.validate(&json_value) {
            error!("Configuration validation error before deserialization");
            anyhow::bail!("Configuration validation failed: {}", error);
        }

        // Now that YAML has been validated, deserialize to Config
        debug!("Schema validation passed, deserializing into Config structure");
        let config: Config = serde_yml::from_str(&contents)
            .with_context(|| format!("Failed to deserialize validated YAML from {:?}", path))?;

        // Perform additional specific validations
        Self::validate_specific_rules(&config)?;

        Ok(config)
    }

    /// Save the configuration to a file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let yaml =
            serde_yml::to_string(self).context("Failed to serialize configuration to YAML")?;

        let mut file = File::create(path.as_ref())
            .with_context(|| format!("Failed to create config file at {:?}", path.as_ref()))?;

        file.write_all(yaml.as_bytes())
            .with_context(|| format!("Failed to write configuration to {:?}", path.as_ref()))?;

        Ok(())
    }

    /// Apply command line arguments to override configuration values
    pub fn apply_args(&mut self, web_port: u16, web_address: String, hmac_secret: Option<String>) {
        // Only override if command-line arguments are provided
        if web_port != default_port() {
            debug!("Overriding port from command line: {}", web_port);
            self.visualization.port = web_port;
        }

        if web_address != default_address() {
            debug!("Overriding address from command line: {}", web_address);
            self.visualization.address = web_address; // Use the field directly
        }

        if let Some(secret) = hmac_secret {
            debug!("Overriding HMAC secret from command line");
            self.visualization.hmac_secret = secret;
        }
    }

    /// Validate additional rules that aren't covered by the JSON schema
    fn validate_specific_rules(config: &Config) -> Result<()> {
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

        Ok(())
    }
}

/// Check if a string is a valid IP address
fn is_valid_ip_address(addr: &str) -> bool {
    if addr.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }

    // Special cases
    matches!(addr, "localhost" | "::" | "::0" | "0.0.0.0")
}
