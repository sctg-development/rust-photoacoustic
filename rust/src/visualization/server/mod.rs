// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Web server implementation for the photoacoustic visualization interface
//!
//! This module provides a complete web server implementation using Rocket, including:
//!
//! - Static file serving for the web client interface
//! - OAuth 2.0 authentication endpoints
//! - Cross-Origin Resource Sharing (CORS) support
//! - API endpoints for retrieving photoacoustic data
//!
//! The server is designed to serve both static content (the web client) and
//! dynamic API endpoints. It integrates with the OAuth authentication system
//! to secure API access.
//!
//! ## Architecture
//!
//! The server consists of the following main components:
//!
//! - **Static File Server**: Serves the web client interface files
//! - **OAuth Endpoints**: Handles authentication and authorization
//! - **API Endpoints**: Provides access to photoacoustic data
//! - **CORS Support**: Enables cross-origin requests for the web client
//!
//! ## Configuration
//!
//! The server can be configured through the provided `figment` configuration system,
//! allowing customization of:
//!
//! - Server address and port
//! - TLS/SSL settings
//! - Authentication settings
//!
//! ## Example
//!
//! ```
//! use rocket::figment::Figment;
//! use std::sync::Arc;
//! use tokio::sync::RwLock;
//! use rust_photoacoustic::visualization::server;
//! use rust_photoacoustic::config::Config;
//!
//! async fn start_server() {
//!     let figment = Figment::from(rocket::Config::default())
//!         .merge(("address", "127.0.0.1"))
//!         .merge(("port", 8000))
//!         .merge(("hmac_secret", "your-secret-key".to_string()));
//!     
//!     let config = Arc::new(RwLock::new(Config::default()));
//!     let rocket = server::build_rocket(figment, config, None, None, None, None, None).await;
//!     rocket.launch().await.expect("Failed to launch server");
//! }
//! ```

pub mod builder;
pub mod cors;
pub mod handlers;
pub mod proxy;

// Re-export main functions from builder
pub use self::builder::{build_rocket, build_openapi_spec, generate_openapi_json, get_generix_config};

#[cfg(test)]
pub use self::builder::build_rocket_test_instance;

use crate::config::AccessConfig;
use crate::visualization::request_guard::ConnectionInfo;
use rocket::Request;
use std::fmt::Debug;

impl Debug for ConnectionInfo<'_> {
    /// Formats the ConnectionInfo for debug output
    ///
    /// This implementation allows the ConnectionInfo struct to be used with
    /// debug formatting macros like `println!("{:?}", connection_info)`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ConnectionInfo")
            .field(&self.host_port)
            .field(&self.origin)
            .field(&self.ip)
            .field(&self.real_ip)
            .field(&self.remote)
            .field(&self.scheme)
            .field(&self.base_url)
            .field(&self.base_url_with_port)
            .finish()
    }
}

pub fn get_config_from_request<'r>(request: &'r Request<'_>) -> AccessConfig {
    // Get the system configuration from the rocket figment key
    let access_config = request
        .rocket()
        .figment()
        .extract_inner::<AccessConfig>("access_config")
        .expect("Failed to extract config from Rocket");
    // Return the configuration
    access_config
}
