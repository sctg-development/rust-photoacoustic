// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
//!
//! Visualization module
//!
//! This module handles visualization and data presentation,
//! including a web server for interactive visualization.

pub mod api;
pub mod api_auth;
pub mod introspection;
pub mod jwt;
pub mod jwt_keys;
pub mod jwt_validator;
pub mod oxide_auth;
pub mod server;

use crate::{config::Config, AnalysisResult};
use anyhow::Result;
use rocket::{config::LogLevel, data::{Limits, ToByteUnit}};
use base64::{self, Engine};

/// Start the visualization web server
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
        figment = figment.merge(("tls.certs", cert_path))
                        .merge(("tls.key", key_path));
        
        log::info!("TLS enabled for web server");
    }

    // In a real implementation, this would:
    // 1. Initialize Rocket server
    // 2. Set up API routes
    // 3. Start the server in a background thread or tokio runtime

    Ok(())
}
