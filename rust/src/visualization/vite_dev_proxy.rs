// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Vite Development Proxy Module
//!
//! This module provides proxy functionality for Vite development server integration.
//! When the `VITE_DEVELOPMENT` environment variable is set, all web client requests
//! are proxied to the running Vite development server for hot reloading and
//! development features.
//!
//! ## Architecture
//!
//! The module provides specialized route handlers for different types of Vite requests:
//!
//! - **Standard Routes**: General web client routes (`/client/<path>`)
//! - **Vite Special Paths**: Development-specific paths (`/client/.vite/<path>`)
//! - **Vite Client Assets**: Vite client scripts (`/client/@vite/<path>`)
//! - **Node Modules**: Vite-processed node modules (`/client/node_modules/.vite/<path>`)
//! - **File System Access**: Direct file system access (`/client/@fs/<path>`)
//! - **WebSocket Proxy**: Hot Module Replacement (HMR) WebSocket (`/client/@vite/client`)
//!
//! ## Environment Configuration
//!
//! - `VITE_DEVELOPMENT`: Base URL of the Vite development server (default: `http://localhost:5173`)
//! - Note: that vite must use https if the main server is running with TLS (needed for WebSocket connections)
//!
//! ## Example
//!
//! ```bash
//! # Start Vite development server
//! cd web && npm run dev
//!
//! # Start Rust server with Vite proxy enabled
//! VITE_DEVELOPMENT=http://localhost:5173 cargo run
//! ```
#![doc = include_str!("../../../docs/vite_dev_with_rocket.md")]

use super::request_guard::{RawQueryString, StaticFileResponse};
use log::{debug, info};
use rocket::get;
use rocket::http::ContentType;
use std::env;
use std::path::PathBuf;

/// Get the Vite development server base URL from environment
///
/// Returns the URL from the `VITE_DEVELOPMENT` environment variable,
/// or defaults to `http://localhost:5173` if not set.
fn get_vite_base_url() -> String {
    env::var("VITE_DEVELOPMENT").unwrap_or_else(|_| "http://localhost:5173".to_string())
}

/// Check if Vite development mode is enabled
///
/// Returns `true` if the `VITE_DEVELOPMENT` environment variable is set.
pub fn is_vite_development_enabled() -> bool {
    env::var("VITE_DEVELOPMENT").is_ok()
}

/// Build URL for proxying to Vite development server
///
/// # Parameters
///
/// * `path` - The request path
/// * `raw_query` - Raw query string from the request
/// * `prefix` - URL prefix to prepend to the path
///
/// # Returns
///
/// Complete URL for the Vite development server request
fn build_vite_url(path: &PathBuf, raw_query: &RawQueryString, prefix: &str) -> String {
    let vite_base = get_vite_base_url();
    let path_str = path.to_str().unwrap_or("");
    let query_str = if raw_query.0.is_empty() {
        String::new()
    } else {
        format!("?{}", raw_query.0)
    };

    format!("{}/{}{}{}", vite_base, prefix, path_str, query_str)
}

/// Generic proxy function for Vite development server requests
///
/// This function handles the common proxy logic for all Vite request types.
///
/// # Parameters
///
/// * `url` - Complete URL to proxy to
/// * `request_type` - Description of the request type for logging
///
/// # Returns
///
/// * `Some(StaticFileResponse)` - Proxied content from Vite server
/// * `None` - If the request fails or Vite development is not enabled
async fn proxy_to_vite(url: &str, request_type: &str) -> Option<StaticFileResponse> {
    if !is_vite_development_enabled() {
        return None;
    }

    info!("Proxying {} to: {}", request_type, url);

    match reqwest::get(url).await {
        Ok(response) => {
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<ContentType>().ok())
                .unwrap_or(ContentType::Binary);

            match response.bytes().await {
                Ok(bytes) => {
                    let response_content: Vec<u8> = bytes.iter().copied().collect();
                    let content = StaticFileResponse(response_content, content_type);
                    debug!("Returning {} content: {:?}", request_type, content);
                    Some(content)
                }
                Err(e) => {
                    debug!("Failed to read {} response bytes: {}", request_type, e);
                    None
                }
            }
        }
        Err(e) => {
            debug!("Failed to proxy {} request: {}", request_type, e);
            None
        }
    }
}

/// Proxy requests to Vite development server for standard web client routes
///
/// This function proxies general web client requests to the Vite development server.
/// It's used by the main webclient route handler when development mode is enabled.
///
/// # Parameters
///
/// * `path` - The requested file path
/// * `raw_query` - Raw query string from the request
///
/// # Returns
///
/// * `Some(StaticFileResponse)` - The proxied content from Vite server
/// * `None` - If the request fails or Vite development is not enabled
pub async fn proxy_to_vite_dev_server(
    path: PathBuf,
    raw_query: RawQueryString,
) -> Option<StaticFileResponse> {
    if !is_vite_development_enabled() {
        return None;
    }

    let url = build_vite_url(&path, &raw_query, "client/");
    proxy_to_vite(&url, "web client in development mode").await
}

/// WebSocket proxy handler for Vite development server
///
/// This route handles WebSocket connections for Vite's hot module replacement (HMR).
/// Since Rocket doesn't have built-in WebSocket proxying, this attempts to proxy
/// the initial request to the Vite development server.
///
/// # Note
///
/// This is a specialized route for Vite's HMR WebSocket client script.
/// The actual WebSocket upgrade is handled by the browser and Vite server directly.
#[get("/client/@vite/client")]
pub async fn websocket_proxy() -> Option<StaticFileResponse> {
    if !is_vite_development_enabled() {
        return None;
    }

    let vite_base = get_vite_base_url();
    let url = format!("{}/client/@vite/client", vite_base);

    // Override content type for Vite client script
    match reqwest::get(&url).await {
        Ok(response) => {
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<ContentType>().ok())
                .unwrap_or(ContentType::JavaScript);

            match response.bytes().await {
                Ok(bytes) => {
                    let response_content: Vec<u8> = bytes.iter().copied().collect();
                    let content = StaticFileResponse(response_content, content_type);
                    debug!("Returning @vite/client content: {:?}", content);
                    Some(content)
                }
                Err(e) => {
                    debug!("Failed to read @vite/client response bytes: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            debug!("Failed to proxy @vite/client request: {}", e);
            None
        }
    }
}

/// Web client route handler for Vite development paths with special characters
///
/// This route handles specific Vite development paths that contain special
/// characters like dots that would be rejected by PathBuf. It has rank 1 to
/// be tried before the regular webclient route.
///
/// # Parameters
///
/// * `path` - The requested path under `.vite/`
/// * `raw_query` - Raw query string from the request
///
/// # Returns
///
/// * `Some(StaticFileResponse)` - The proxied content from Vite server
/// * `None` - If the request fails or Vite development is not enabled
#[get("/client/.vite/<path..>", rank = 1)]
pub async fn webclient_vite_special(
    path: PathBuf,
    raw_query: RawQueryString,
) -> Option<StaticFileResponse> {
    let url = build_vite_url(&path, &raw_query, "client/.vite/");
    proxy_to_vite(&url, "Vite special path").await
}

/// Web client route handler for @vite paths (development mode)
///
/// This route handles @vite paths that contain special characters
/// that would be rejected by standard path handling.
///
/// # Parameters
///
/// * `path` - The requested path under `@vite/`
/// * `raw_query` - Raw query string from the request
///
/// # Returns
///
/// * `Some(StaticFileResponse)` - The proxied content from Vite server
/// * `None` - If the request fails or Vite development is not enabled
#[get("/client/@vite/<path..>", rank = 1)]
pub async fn webclient_at_vite(
    path: PathBuf,
    raw_query: RawQueryString,
) -> Option<StaticFileResponse> {
    let url = build_vite_url(&path, &raw_query, "client/@vite/");
    proxy_to_vite(&url, "@vite path").await
}

/// Web client route handler for node_modules/.vite paths (development mode)
///
/// This route handles node_modules/.vite paths that contain special characters
/// that would be rejected by standard path handling.
///
/// # Parameters
///
/// * `path` - The requested path under `node_modules/.vite/`
/// * `raw_query` - Raw query string from the request
///
/// # Returns
///
/// * `Some(StaticFileResponse)` - The proxied content from Vite server
/// * `None` - If the request fails or Vite development is not enabled
#[get("/client/node_modules/.vite/<path..>", rank = 1)]
pub async fn webclient_node_modules_vite(
    path: PathBuf,
    raw_query: RawQueryString,
) -> Option<StaticFileResponse> {
    let url = build_vite_url(&path, &raw_query, "client/node_modules/.vite/");
    proxy_to_vite(&url, "node_modules/.vite path").await
}

/// Web client route handler for @fs paths (development mode)
///
/// This route handles @fs paths used by Vite for file system access.
/// These paths allow Vite to serve files from anywhere in the file system
/// during development.
///
/// # Parameters
///
/// * `path` - The requested path under `@fs/`
/// * `raw_query` - Raw query string from the request
///
/// # Returns
///
/// * `Some(StaticFileResponse)` - The proxied content from Vite server
/// * `None` - If the request fails or Vite development is not enabled
#[get("/client/@fs/<path..>", rank = 1)]
pub async fn webclient_at_fs(
    path: PathBuf,
    raw_query: RawQueryString,
) -> Option<StaticFileResponse> {
    let url = build_vite_url(&path, &raw_query, "client/@fs/");
    proxy_to_vite(&url, "@fs path").await
}

/// Get all Vite development proxy routes
///
/// Returns a vector of all route handlers for Vite development proxy functionality.
/// This function is used by the main server module to mount all Vite-related routes.
///
/// # Returns
///
/// Vector of Rocket routes for Vite development proxy
pub fn get_vite_dev_routes() -> Vec<rocket::Route> {
    rocket::routes![
        websocket_proxy,
        webclient_at_vite,
        webclient_at_fs,
        webclient_vite_special,
        webclient_node_modules_vite,
    ]
}
