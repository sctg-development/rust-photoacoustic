// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Visualization Module
//!
//! The visualization module provides a comprehensive web-based interface for
//! presenting and interacting with photoacoustic data analysis results. It enables
//! researchers and clinicians to view, explore, and interpret complex photoacoustic
//! measurements through an interactive dashboard.
//!
//! ## Key Features
//!
//! - **Interactive Web Interface**: A modern web server for visualizing data
//! - **Secure Authentication**: OAuth 2.0 compatible JWT-based authentication
//! - **API Access**: RESTful API for programmatic data access
//! - **TLS Support**: Optional encrypted connections for enhanced security
//! - **Customizable**: Configurable server settings through the application config
//!
//! ## Architecture
//!
//! The visualization system is built on the Rocket web framework and consists of:
//!
//! - **Server**: Core HTTP/HTTPS server implementation
//! - **API**: RESTful endpoints for data access
//! - **Authentication**: JWT token generation, validation and introspection
//! - **OAuth Integration**: Support for standard OAuth 2.0 workflows
//!
//! ## Usage
//!
//! ```text
//! use rust_photoacoustic::{config::Config, AnalysisResult};
//! use rust_photoacoustic::visualization;
//! use rocket::routes;  // Import routes macro for rocket
//!
//! async fn example() -> anyhow::Result<()> {
//!     // Load configuration
//!     let config = Config::from_file("config.yaml")?;
//!     
//!     // Prepare analysis results (simplified example)
//!     let analysis_result = AnalysisResult::default();
//!     
//!     // Start the visualization server
//!     visualization::start_server(analysis_result, &config).await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Security
//!
//! The module implements several security features:
//!
//! - JWT token-based authentication
//! - Configurable TLS/HTTPS
//! - OAuth 2.0 token introspection
//! - Scope-based authorization

/// API implementation modules
pub mod api;

/// Audio streaming endpoints for real-time data
pub mod streaming;

/// API authentication mechanisms
///
/// This module provides JWT-based authentication for API endpoints, including:
///
/// - JWT token validation and extraction
/// - Route guards for securing endpoints
/// - Scope-based authorization
///
/// # Example
///
/// ```no_run
/// use rocket::{get, build, routes};
/// use rocket::serde::json::Json;
/// use rust_photoacoustic::visualization::api_auth::AuthenticatedUser;
///
/// #[get("/profile")]
/// fn get_profile(user: AuthenticatedUser) -> Json<String> {
///     Json(format!("Hello, {}!", user.user_id))
/// }
///
/// #[get("/data")]
/// fn get_data() -> &'static str {
///     "Public data"
/// }
///
/// fn setup() -> rocket::Rocket<rocket::Build> {
///     build()
///         .mount("/", routes![get_profile, get_data])
/// }
/// ```

/// Authentication and authorization system
///
/// This module provides a unified authentication system supporting:
/// - OAuth 2.0 authorization code flow
/// - JWT token validation and management
/// - Request guards for API protection
/// - Permission-based access control
pub mod auth;

pub mod api_auth;
pub mod introspection;
pub mod oidc;
pub mod pwhash;
pub mod request_guard;
pub mod server;
pub mod user_info_reponse;
pub mod vite_dev_proxy;

// Re-export commonly used auth items
pub use auth::{JwtValidator, OAuthBearer};

/// Token introspection functionality for validating OAuth tokens
///
/// This module provides OAuth 2.0 token introspection endpoint implementation
/// according to RFC 7662, allowing clients to verify token validity and scope.
///
/// # Example
///
/// ```no_run
/// use rocket::{build, post, routes};
/// use rust_photoacoustic::visualization::auth::OxideState;
///
/// #[post("/introspect")]
/// fn introspect() -> &'static str {
///     "Token information"
/// }
///
/// fn setup() {
///     let figment = rocket::Config::figment().merge(("hmac_secret", "your-secret".to_string()));
///     let state = OxideState::preconfigured(figment);
///     let rocket = build()
///         .manage(state)
///         .mount("/oauth", routes![introspect]);
///     // Start the server
/// }
/// ```

/// JWT token generation and management
pub mod jwt;

use crate::{config::Config, AnalysisResult};
use anyhow::Result;
use base64::{self, Engine};
use rocket::{
    config::LogLevel,
    data::{Limits, ToByteUnit},
};

/// Start the visualization web server with the provided analysis data and configuration
///
/// This function initializes and launches a web server for visualizing photoacoustic
/// analysis results. It configures the server based on the provided configuration
/// and sets up all necessary components for the web interface.
///
/// # Parameters
///
/// * `data` - The analysis results to be visualized
/// * `config` - Application configuration containing server settings
///
/// # Returns
///
/// * `Result<()>` - Success or error status from the server initialization
///
/// # Errors
///
/// This function can fail if:
/// * TLS certificate decoding fails
/// * Writing temporary certificate files fails
/// * Server initialization fails
///
/// # Example
///
/// ```no_run
/// use rust_photoacoustic::{config::Config, AnalysisResult};
///
/// async fn run_server() -> anyhow::Result<()> {
///     let config = Config::from_file("config.yaml")?;
///     let analysis_data = AnalysisResult::default(); // Replace with actual analysis
///     
///     rust_photoacoustic::visualization::start_server(analysis_data, &config).await?;
///     Ok(())
/// }
/// ```
pub async fn start_server(data: AnalysisResult, config: &Config) -> Result<()> {
    log::info!("Starting visualization server with data: {:?}", data);

    // Configure Rocket
    let mut figment = rocket::Config::figment()
        .merge(("ident", config.visualization.name.clone()))
        .merge(("limits", Limits::new().limit("json", 2.mebibytes())))
        .merge(("address", config.visualization.address.clone()))
        .merge(("port", config.visualization.port))
        .merge(("log_level", LogLevel::Normal));

    // Configure TLS if certificates are provided
    if let (Some(cert), Some(key)) = (&config.visualization.cert, &config.visualization.key) {
        log::debug!("SSL certificates found in configuration, enabling TLS");

        // Decode base64 certificates
        let cert_data = base64::engine::general_purpose::STANDARD.decode(cert)?;
        let key_data = base64::engine::general_purpose::STANDARD.decode(key)?;

        // Create temporary files for the certificates
        let temp_dir = std::env::temp_dir();
        let cert_path = temp_dir.join("server.crt");
        let key_path = temp_dir.join("server.key");

        // Write the certificates to temporary files
        std::fs::write(&cert_path, cert_data)?;
        std::fs::write(&key_path, key_data)?;

        // Configure TLS
        figment = figment
            .merge(("tls.certs", cert_path))
            .merge(("tls.key", key_path));

        log::info!("TLS enabled for web server");
    }

    // In a real implementation, this would:
    // 1. Initialize Rocket server
    // 2. Set up API routes
    // 3. Start the server in a background thread or tokio runtime

    Ok(())
}
