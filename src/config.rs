// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Configuration Management
//!
//! This module implements configuration handling for the photoacoustic application.
//! It supports loading, validating, and saving configuration from YAML files using
//! JSON Schema validation for robust error checking.
//!
//! ## Configuration Structure
//!
//! The application's configuration is organized as a nested structure with sections:
//! - `visualization`: Settings for the visualization web server
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

use anyhow::{Context, Result};
use base64::Engine;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

/// Separator character used in user session identifiers
pub const USER_SESSION_SEPARATOR: char = '⛷';

/// Configuration for the data acquisition process.
///
/// This structure contains settings that control how data is acquired
/// from the photoacoustic sensor, including timing parameters and
/// whether the acquisition system is enabled.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AcquisitionConfig {
    /// Enable or disable data acquisition.
    ///
    /// When set to `false`, the system will not perform any data acquisition
    /// operations. Default is `true`.
    pub enabled: bool,

    /// Time interval between consecutive data acquisitions in milliseconds.
    ///
    /// This controls how frequently the system samples data from the sensor.
    /// Lower values provide more frequent updates but may increase system load.
    /// Default value is 1000ms (1 second).
    pub interval_ms: u64,
    // Other acquisition settings
}
// implement Default for AcquisitionConfig
impl Default for AcquisitionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_ms: 1000,
        }
    }
}

/// Configuration for the Modbus TCP server component.
///
/// This structure contains settings that control the Modbus TCP server functionality,
/// including network binding parameters and whether the server is enabled.
///
/// # Fields
///
/// * `enabled` - Flag to enable or disable the Modbus server
/// * `port` - TCP port number for the Modbus server (default: 502)
/// * `address` - Network address for the Modbus server to bind to (default: 127.0.0.1)
///
/// # Example
///
/// ```
/// use rust_photoacoustic::config::ModbusConfig;
///
/// let modbus_config = ModbusConfig {
///     enabled: true,
///     port: 503,
///     address: "0.0.0.0".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusConfig {
    /// Enable or disable the Modbus TCP server.
    ///
    /// When set to `false`, the Modbus server will not be started.
    /// Default is `false`.
    pub enabled: bool,

    /// The TCP port the Modbus server will listen on.
    ///
    /// Valid range is 1-65534. Default value is 502, which is the standard Modbus TCP port.
    pub port: u16,

    /// The network address the Modbus server will bind to.
    ///
    /// Can be an IPv4/IPv6 address or a hostname. Default is "127.0.0.1".
    /// Use "0.0.0.0" to bind to all IPv4 interfaces.
    pub address: String,
}
// implement Default for ModbusConfig
impl Default for ModbusConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 502,
            address: "127.0.0.1".to_string(),
        }
    }
}

/// Configuration for the photoacoustic measurement system.
///
/// This structure contains settings that control the photoacoustic measurement process,
/// including input sources, signal processing parameters, and analysis settings.
///
/// # Input Sources
///
/// The configuration supports two mutually exclusive input sources:
/// * `input_device` - A hardware audio device (e.g., "hw:0,0" for ALSA)
/// * `input_file` - A path to a WAV file for offline analysis
///
/// One of these must be specified, but not both simultaneously.
///
/// # Signal Processing Parameters
///
/// * `frequency` - The primary excitation frequency in Hz
/// * `bandwidth` - Filter bandwidth in Hz around the excitation frequency
/// * `window_size` - FFT window size (power of 2 recommended)
/// * `averages` - Number of spectra to average for noise reduction
///
/// # Example
///
/// ```
/// use rust_photoacoustic::config::PhotoacousticConfig;
///
/// let pa_config = PhotoacousticConfig {
///     input_device: Some("hw:0,0".to_string()),
///     input_file: None,
///     frequency: 1000.0,
///     bandwidth: 50.0,
///     window_size: 4096,
///     averages: 10,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoacousticConfig {
    /// The input device to use for data acquisition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_device: Option<String>,
    /// The input file to use for data acquisition mutually exclusive with input_device
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file: Option<String>,
    /// The excitation frequency in Hz
    pub frequency: f32,
    /// Filter bandwidth in Hz
    pub bandwidth: f32,
    /// Window size for FFT analysis
    pub window_size: u16,
    /// Number of spectra to average
    pub averages: u16,
}
// implement Default for PhotoacousticConfig
impl Default for PhotoacousticConfig {
    fn default() -> Self {
        Self {
            input_device: None,
            input_file: Some("input.wav".to_string()),
            frequency: 2000.0,
            bandwidth: 100.0,
            window_size: 4096,
            averages: 10,
        }
    }
}
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
}

/// Configuration for the visualization web server.
///
/// This structure contains all settings required for the visualization server component,
/// including network binding parameters, TLS certificate settings, and authentication
/// configuration with both HMAC and RSA key-based JWT options.
///
/// # Security Options
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
/// # TLS Configuration
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
    let key_str = include_str!("../resources/private.key");
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
    let key_str = include_str!("../resources/pub.key");
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
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            visualization: VisualizationConfig::default(),
            acquisition: AcquisitionConfig {
                enabled: true,
                interval_ms: 1000,
            },
            modbus: ModbusConfig::default(),
            photoacoustic: PhotoacousticConfig::default(),
            access: AccessConfig::default(),
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
                        "Failed to create directory for sample config at {:?}",
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
        if let Err(err) = Self::validate_specific_rules(&config) {
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
            self.visualization.address = web_address; // Use the field directly
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

    /// Validates the configuration against additional rules that aren't covered by the JSON schema.
    ///
    /// This method performs deeper validation checks that can't be easily expressed in a JSON schema,
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
}

/// Check if a string is a valid IP address
fn is_valid_ip_address(addr: &str) -> bool {
    if addr.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }

    // Special cases
    matches!(addr, "localhost" | "::" | "::0" | "0.0.0.0")
}

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
    let schema_str = include_str!("../resources/config.schema.json");

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

/// User definition for authentication and authorization
///
/// This structure represents a user with authentication credentials and
/// associated permissions for controlling access to API endpoints.
///
/// # Fields
///
/// * `user` - The username used for authentication
/// * `pass` - Base64-encoded password hash (created with openssl passwd -1 | base64 -w0)
/// * `permissions` - List of permission strings that define what actions the user can perform
///
/// # Example
///
/// ```
/// use rust_photoacoustic::config::User;
///
/// let user = User {
///     user: "admin".to_string(),
///     pass: "JDEkYTRuMy5jZmUkRU93djlOYXBKYjFNTXRTMHA1UzN1MQo=".to_string(),
///     permissions: vec!["read:api".to_string(), "write:api".to_string(), "admin:api".to_string()],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// The username used for authentication
    pub user: String,

    /// Base64-encoded password hash
    ///
    /// This should be created using: `openssl passwd -1 <password> | base64 -w0`
    pub pass: String,

    /// List of permission strings that define what actions the user can perform
    ///
    /// Common permissions include:
    /// * "read:api" - Allows read-only access to API endpoints
    /// * "write:api" - Allows modification operations on API endpoints
    /// * "admin:api" - Allows administrative operations
    pub permissions: Vec<String>,
}

/// Configuration for user access and permissions
///
/// This structure defines the users who can access the application
/// and their associated permissions. Each user has a username, password hash,
/// and a list of permissions that control what actions they can perform.
/// A valid password hash is generated using the `openssl` command:
/// ```bash
/// openssl passwd -5 admin123 | base64 -w0
/// ```
///
/// # Example
///
/// ```rust
/// use rust_photoacoustic::config::{AccessConfig, User};
///
/// let access_config = AccessConfig {
///     users: vec![
///          User {
///              user: "admin".to_string(),
///              pass: "JDEkYTRuMy5jZmUkRU93djlOYXBKYjFNTXRTMHA1UzN1MQo=".to_string(),
///              permissions: vec!["read:api".to_string(), "write:api".to_string(), "admin:api".to_string()],
///          },
///          User {
///              user: "reader".to_string(),
///              pass: "JDEkUTJoSGZWU3ckT3NIVTUzamhCY3pYVmRHTGlTazg4Lwo=".to_string(),
///              permissions: vec!["read:api".to_string()],
///          }],
///      allowed_callbacks: vec![
///          "http://localhost:8080/client/".to_string(),
///          "https://localhost:8080/client/".to_string(),
///      ],
///     };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessConfig {
    pub users: Vec<User>,
    pub allowed_callbacks: Vec<String>,
}

impl Default for User {
    fn default() -> Self {
        Self {
            user: "admin".to_string(),
            // Default password hash for "admin123" (should be changed in production)
            pass: "JDUkM2E2OUZwQW0xejZBbWV2QSRvMlhhN0lxcVdVU1VPTUh6UVJiM3JjRlRhZy9WYjdpSWJtZUJFaXA3Y1ZECg==".to_string(),
            permissions: vec![
                "read:api".to_string(), 
                "write:api".to_string(), 
                "admin:api".to_string()
            ],
        }
    }
}

impl Default for AccessConfig {
    fn default() -> Self {
        Self {
            users: vec![User::default()],
            allowed_callbacks: vec![
                "http://localhost:8080/client/".to_string(),
                "https://localhost:8080/client/".to_string(),
            ],
        }
    }
}
