// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use anyhow::{Context, Result};
use base64::Engine;
use jsonschema;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs::{self, File},
    io::Write,
    path::{Path},
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
    pub port: i16,
    /// The address to bind to
    #[serde(default = "default_address")]
    pub address: String,
    /// The server name
    #[serde(default = "default_name")]
    pub name: String,
    /// SSL certificate PEM data (Base64 encoded)
    #[serde(default)]
    pub cert: Option<String>,
    /// SSL key PEM data (Base64 encoded)
    #[serde(default)]
    pub key: Option<String>,
}

fn default_port() -> i16 {
    8080
}

fn default_address() -> String {
    "127.0.0.1".to_string()
}

fn default_name() -> String {
    format!("LaserSmartApiServer/{}", env!("CARGO_PKG_VERSION"))
}

impl Default for VisualizationConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            address: default_address(),
            name: default_name(),
            cert: None,
            key: None,
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
            debug!("Configuration file not found at {:?}, creating default", path);
            let default_config = Self::default();
            default_config.save_to_file(path)?;
            return Ok(default_config);
        }

        debug!("Loading configuration from {:?}", path);
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read configuration file at {:?}", path))?;

        let config: Config = serde_yml::from_str(&contents)
            .with_context(|| format!("Failed to parse YAML configuration from {:?}", path))?;

        // Validate the configuration
        config.validate()?;

        Ok(config)
    }

    /// Save the configuration to a file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let yaml = serde_yml::to_string(self)
            .context("Failed to serialize configuration to YAML")?;

        let mut file = File::create(path.as_ref())
            .with_context(|| format!("Failed to create config file at {:?}", path.as_ref()))?;

        file.write_all(yaml.as_bytes())
            .with_context(|| format!("Failed to write configuration to {:?}", path.as_ref()))?;

        Ok(())
    }

    /// Validate the configuration against schema
    pub fn validate(&self) -> Result<()> {
        // Convert the config to a JSON value for validation
        let instance = serde_json::to_value(self)?;

        // Define the JSON schema
        let schema = json!({
            "type": "object",
            "properties": {
                "visualization": {
                    "type": "object",
                    "properties": {
                        "port": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 65535
                        },
                        "address": {
                            "type": "string",
                            "format": "ipv4"
                        },
                        "name": {
                            "type": "string"
                        },
                        "cert": {
                            "type": ["string", "null"]
                        },
                        "key": {
                            "type": ["string", "null"]
                        }
                    },
                    "required": ["port", "address", "name"]
                }
            },
            "required": ["visualization"]
        });

        // Validate the configuration
        if let Err(error) = jsonschema::validate(&schema,&instance) {
             error!("Configuration validation error: {}", error);
            anyhow::bail!("Configuration validation failed");
        }

        // Additional validation: if cert is provided, key must also be provided
        if let Some(cert) = &self.visualization.cert {
            if self.visualization.key.is_none() {
                anyhow::bail!("SSL certificate provided without a key");
            }
            
            // Validate the cert is valid base64
            let _ = base64::engine::general_purpose::STANDARD
                .decode(cert)
                .context("SSL certificate is not valid base64")?;
        }

        if let Some(key) = &self.visualization.key {
            if self.visualization.cert.is_none() {
                anyhow::bail!("SSL key provided without a certificate");
            }
            
            // Validate the key is valid base64
            let _ = base64::engine::general_purpose::STANDARD
                .decode(key)
                .context("SSL key is not valid base64")?;
        }

        Ok(())
    }

    /// Apply command line arguments to override configuration values
    pub fn apply_args(&mut self, web_port: i16, web_address: String) {
        // Only override if command-line arguments are provided
        if web_port != default_port() {
            debug!("Overriding port from command line: {}", web_port);
            self.visualization.port = web_port;
        }

        if web_address != default_address() {
            debug!("Overriding address from command line: {}", web_address);
            self.visualization.address = web_address;  // Fixed: Use the field directly instead of to_string()
        }
    }
}
