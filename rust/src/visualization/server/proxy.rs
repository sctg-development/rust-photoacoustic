// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Vite development server proxy functionality
//!
//! This module provides proxy functionality for routing requests to a Vite
//! development server during development mode.

use log::{debug, info};
use rocket::http::ContentType;
use std::env;
use std::path::PathBuf;

use crate::visualization::request_guard::{RawQueryString, StaticFileResponse};

/// Proxy requests to Vite development server
pub async fn proxy_to_vite_dev_server(
    path: PathBuf,
    raw_query: RawQueryString,
) -> Option<StaticFileResponse> {
    let vite_base = env::var("EXTERNAL_WEB_CLIENT").unwrap_or("http://localhost:5173".to_string());
    let path_str = path.to_str().unwrap_or("");
    let raw_query = if raw_query.0.is_empty() {
        String::new()
    } else {
        format!("?{}", raw_query.0)
    };
    let url = format!("{}/client/{}{}", vite_base, path_str, raw_query.as_str());

    info!("Proxying web client in development mode to: {}", url);

    match reqwest::get(&url).await {
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
                    //debug!("Returning content from development server: {:?}", content);
                    Some(content)
                }
                Err(e) => {
                    debug!("Failed to read response bytes: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            debug!("Failed to proxy request to development server: {}", e);
            None
        }
    }
}

/// Proxy requests to Vite development server for special paths
pub async fn proxy_to_vite_special_path(
    base_path: &str,
    path: PathBuf,
    raw_query: RawQueryString,
) -> Option<StaticFileResponse> {
    if !env::var("EXTERNAL_WEB_CLIENT").is_ok() {
        return None;
    }

    let vite_base = env::var("EXTERNAL_WEB_CLIENT").unwrap_or("http://localhost:5173".to_string());
    let path_str = path.to_str().unwrap_or("");
    let raw_query = if raw_query.0.is_empty() {
        String::new()
    } else {
        format!("?{}", raw_query.0)
    };
    let url = format!(
        "{}/client/{}/{}{}",
        vite_base,
        base_path,
        path_str,
        raw_query.as_str()
    );

    info!("Proxying {} path to: {}", base_path, url);

    match reqwest::get(&url).await {
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
                    debug!("Returning {} content: {:?}", base_path, content);
                    Some(content)
                }
                Err(e) => {
                    debug!("Failed to read response bytes: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            debug!("Failed to proxy {} request: {}", base_path, e);
            None
        }
    }
}
