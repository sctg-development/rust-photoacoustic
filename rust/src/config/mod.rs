// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Configuration management for the photoacoustic application
//!
//! This module provides functionality for loading, validating, and applying
//! configuration settings for the photoacoustic application. The configuration
//! is backed by a YAML file and validated against a JSON schema for robustness.
//!
//! ## Configuration Structure
//!
//! The application's configuration is organized as a nested structure with sections:
//! - `visualization`: Settings for the visualization web server
//! - `acquisition`: Settings for data acquisition
//! - `modbus`: Settings for Modbus TCP server functionality
//! - `photoacoustic`: Settings for photoacoustic measurements
//! - `access`: Settings for user access and permissions
//!
//! ## Security Features
//!
//! The configuration supports both HMAC and RSA-based JWT token authentication:
//! - HMAC-based JWT: Simple secret key-based token signing and verification
//! - RS256 JWT: Public/private key pair for more secure token handling
//!
//! ## Usage
//!
//! ```no_run
//! use rust_photoacoustic::config::Config;
//! use std::path::Path;
//!
//! // Load config from file, creates a default if not found
//! let mut config = Config::from_file(Path::new("config.yaml")).unwrap();
//!
//! // Apply command line overrides if needed
//! config.apply_args(
//!     Some(8081),                     // Web port
//!     Some("0.0.0.0".to_string()),    // Web address
//!     Some("new_secret".to_string()), // HMAC secret
//!     true,                           // Daemon mode
//!     Some("hw:0,0".to_string()),     // Input device
//!     None,                           // Input file
//!     Some(1000.0),                   // Frequency
//!     Some(50.0),                     // Bandwidth
//!     Some(2048),                     // Window size
//!     Some(5),                        // Averages
//!     Some(true),                     // Enable Modbus
//!     Some("0.0.0.0".to_string()),    // Modbus address
//!     Some(502),                      // Modbus port  
//! );
//!
//! // Access configuration values
//! println!("Server port: {}", config.visualization.port);
//! ```

pub mod access;
pub mod acquisition;
pub mod generix;
pub mod modbus;
pub mod photoacoustic;
pub mod utils;
pub mod visualization;

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use log::{debug, error};
use serde::{Deserialize, Serialize};

// Re-export all types for public API
pub use access::{AccessConfig, Client, User};
pub use acquisition::AcquisitionConfig;
pub use generix::GenerixConfig;
pub use modbus::ModbusConfig;
pub use photoacoustic::PhotoacousticConfig;
pub use utils::{is_valid_ip_address, output_config_schema};
pub use visualization::VisualizationConfig;

/// Separator character used in user session identifiers
pub const USER_SESSION_SEPARATOR: char = '⛷';

/// Root configuration structure for the photoacoustic application.
///
/// This structure serves as the main container for all configuration sections
/// of the application. Currently, it only contains visualization settings, but
/// it can be expanded to include other sections as the application grows.
///
/// # Structure
///
/// The configuration is designed to be deserialized from and serialized to YAML
/// using the serde framework. The structure is validated against a JSON schema
/// to ensure all required fields are present and have valid values.
///
/// # Default Values
///
/// Each section uses default values when not explicitly specified in the configuration
/// file, allowing for minimal configuration when custom settings are not required.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Settings for the visualization web server component.
    ///
    /// These settings control how the visualization server behaves, including
    /// network binding, security, and authentication mechanisms.
    /// If not specified in the configuration file, default values are used.
    #[serde(default)]
    pub visualization: VisualizationConfig,

    /// Acquisition settings for the photoacoustic application.
    ///
    /// This section controls parameters related to the data acquisition process,
    /// such as enabling/disabling acquisition, and the interval between acquisitions.
    /// If not specified, default values will be used.
    #[serde(default)]
    pub acquisition: AcquisitionConfig,

    /// Modbus settings for the photoacoustic application.
    ///
    /// This section controls parameters related to the Modbus communication,
    /// such as enabling/disabling Modbus, the port to use, and the address.
    /// If not specified, default values will be used.
    #[serde(default)]
    pub modbus: ModbusConfig,

    /// Photoacoustic settings for the photoacoustic application.
    ///
    /// This section controls parameters related to the photoacoustic
    /// acquisition, such as the input device, input file, frequency,
    /// bandwidth, window size, and number of spectra to average.
    /// If not specified, default values will be used.
    #[serde(default)]
    pub photoacoustic: PhotoacousticConfig,

    /// User access and permissions settings for the photoacoustic application.
    ///
    /// This section controls parameters related to user authentication and
    /// authorization, such as the list of users, their credentials, and
    /// associated permissions. If not specified, default values will be used.
    #[serde(default)]
    pub access: AccessConfig,

    #[serde(default)]
    pub generix: GenerixConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            visualization: VisualizationConfig::default(),
            acquisition: AcquisitionConfig::default(),
            modbus: ModbusConfig::default(),
            photoacoustic: PhotoacousticConfig::default(),
            access: AccessConfig::default(),
            generix: GenerixConfig::default(),
        }
    }
}

impl Config {
    /// Helper method to create a sample config file when validation fails
    fn create_sample_config<P: AsRef<Path>>(path: P) -> Result<()> {
        let path = path.as_ref();
        debug!("Creating sample configuration file at {:?}", path);
        let sample_path = path.with_extension("sample.yaml");

        // Print more debug information
        debug!("Original path: {:?}, Sample path: {:?}", path, sample_path);

        // Create parent directories if they don't exist
        if let Some(parent) = sample_path.parent() {
            if !parent.exists() {
                debug!("Creating parent directory: {:?}", parent);
                std::fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "Failed to create parent directory for sample config at {:?}",
                        parent
                    )
                })?;
            }
        }

        let sample_config = Self::default();
        debug!("About to save sample config to {:?}", sample_path);
        sample_config
            .save_to_file(&sample_path)
            .with_context(|| format!("Failed to save sample config to {:?}", sample_path))?;

        debug!("Sample config file created successfully, checking if it exists");
        if sample_path.exists() {
            debug!("Confirmed sample file exists at {:?}", sample_path);
        } else {
            error!(
                "Sample file was supposedly created but doesn't exist at {:?}",
                sample_path
            );
        }

        error!(
            "Sample configuration file created at {:?}\nPlease edit and rename it",
            sample_path
        );
        Ok(())
    }

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
        let json_value = serde_json::to_value(&yaml_value).with_context(|| {
            format!("Failed to convert YAML to JSON for validation: {:?}", path)
        })?;

        // Load and validate with the schema
        let schema_str = include_str!("../../resources/config.schema.json");
        let schema: serde_json::Value = serde_json::from_str(schema_str).with_context(|| {
            debug!("JSON schema string: {}", schema_str);
            "Failed to parse JSON schema"
        })?;

        // Create the validator
        let validator = jsonschema::draft202012::options()
            .should_validate_formats(true)
            .build(&schema)?;

        // Validate before deserializing to Config
        debug!("Validating {} configuration against schema", path.display());
        if let Err(error) = validator.validate(&json_value) {
            error!("Configuration validation error before deserialization");
            // We generate a config.sample.yaml file with the default values
            // for the user to edit
            Self::create_sample_config(path)?;
            anyhow::bail!("Configuration validation failed: {}", error);
        }

        // Now that YAML has been validated, deserialize to Config
        debug!("Schema validation passed, deserializing into Config structure");
        let config: Config = match serde_yml::from_str(&contents) {
            Ok(config) => config,
            Err(err) => {
                error!("Configuration deserialization error: {}", err);
                // Generate a sample config file just like we do for schema validation failures

                // Log the path for debugging
                debug!("About to create sample config for path: {:?}", path);

                // Create the sample file
                match Self::create_sample_config(path) {
                    Ok(_) => debug!("Successfully created sample config"),
                    Err(e) => error!("Failed to create sample config: {}", e),
                }

                // Return the original error enhanced with context
                return Err(anyhow::anyhow!(
                    "Failed to deserialize configuration from {}: {}",
                    path.display(),
                    err
                ));
            }
        };

        // Perform additional specific validations
        if let Err(err) = utils::validate_specific_rules(&config) {
            error!("Configuration specific validation error: {}", err);
            // Generate a sample config file
            Self::create_sample_config(path)?;
            return Err(err);
        }

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

    /// Apply command line arguments to override configuration values.
    ///
    /// This method allows configuration values to be overridden with command line arguments.
    /// Only values that differ from defaults or are explicitly provided will override
    /// the existing configuration.
    ///
    /// # Parameters
    ///
    /// * `web_port` - TCP port for the visualization server
    /// * `web_address` - Network address for the visualization server to bind to
    /// * `hmac_secret` - Optional HMAC secret for JWT token signing
    /// * `daemon_mode` - If true, ensures visualization server is enabled
    /// * `input_device` - Optional audio input device for photoacoustic analysis
    /// * `input_file` - Optional input file path for photoacoustic analysis
    /// * `frequency` - Optional excitation frequency in Hz
    /// * `bandwidth` - Optional filter bandwidth in Hz
    /// * `window_size` - Optional FFT window size
    /// * `averages` - Optional number of spectra to average
    /// * `modbus_enabled` - Optional flag to enable/disable Modbus server
    /// * `modbus_port` - Optional TCP port for Modbus server
    /// * `modbus_address` - Optional network address for Modbus server
    ///
    /// # Example
    ///
    /// ```rust
    /// use rust_photoacoustic::config::Config;
    /// let mut config = Config::from_file("config.yaml").unwrap();
    /// config.apply_args(
    ///     Some(8081),                     // Web port
    ///     Some("0.0.0.0".to_string()),    // Web address
    ///     Some("new_secret".to_string()), // HMAC secret
    ///     true,                           // Daemon mode
    ///     Some("hw:0,0".to_string()),     // Input device
    ///     None,                           // Input file
    ///     Some(1000.0),                   // Frequency
    ///     Some(50.0),                     // Bandwidth
    ///     Some(2048),                     // Window size
    ///     Some(5),                        // Averages
    ///     Some(true),                     // Enable Modbus
    ///     Some("0.0.0.0".to_string()),    // Modbus address
    ///     Some(502),                      // Modbus port  
    /// );
    /// ```
    pub fn apply_args(
        &mut self,
        web_port: Option<u16>,
        web_address: Option<String>,
        hmac_secret: Option<String>,
        daemon_mode: bool,
        input_device: Option<String>,
        input_file: Option<PathBuf>,
        frequency: Option<f32>,
        bandwidth: Option<f32>,
        window_size: Option<u16>,
        averages: Option<u16>,
        modbus_enabled: Option<bool>,
        modbus_address: Option<String>,
        modbus_port: Option<u16>,
    ) {
        // Only override if command-line arguments are provided
        if let Some(web_port) = web_port {
            debug!("Overriding port from command line: {}", web_port);
            self.visualization.port = web_port;
        }

        if let Some(web_address) = web_address {
            debug!("Overriding address from command line: {}", web_address);
            self.visualization.address = web_address;
        }

        if let Some(secret) = hmac_secret {
            debug!("Overriding HMAC secret from command line");
            self.visualization.hmac_secret = secret;
        }

        // Enable visualization in daemon mode
        if daemon_mode {
            self.visualization.enabled = true;
        }

        // Apply photoacoustic settings
        if let Some(device) = input_device {
            debug!("Overriding input device from command line: {}", device);
            self.photoacoustic.input_device = Some(device);
        }
        if let Some(file) = input_file {
            debug!("Overriding input file from command line: {:?}", file);
            self.photoacoustic.input_file = Some(file.to_string_lossy().to_string());
        }
        if let Some(freq) = frequency {
            debug!("Overriding frequency from command line: {}", freq);
            self.photoacoustic.frequency = freq;
        }
        if let Some(band) = bandwidth {
            debug!("Overriding bandwidth from command line: {}", band);
            self.photoacoustic.bandwidth = band;
        }
        if let Some(size) = window_size {
            debug!("Overriding window size from command line: {}", size);
            self.photoacoustic.window_size = size;
        }
        if let Some(avg) = averages {
            debug!("Overriding averages from command line: {}", avg);
            self.photoacoustic.averages = avg;
        }

        // Apply Modbus settings
        if let Some(enabled) = modbus_enabled {
            debug!("Overriding Modbus enabled from command line: {}", enabled);
            self.modbus.enabled = enabled;
        }
        if let Some(port) = modbus_port {
            debug!("Overriding Modbus port from command line: {}", port);
            self.modbus.port = port;
        }
        if let Some(address) = modbus_address {
            debug!("Overriding Modbus address from command line: {}", address);
            self.modbus.address = address;
        }
    }
}
