// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Route handlers for static files and web client
//!
//! This module contains the route handlers for serving static files,
//! web client assets, and development proxy routes.

use include_dir::{include_dir, Dir};
use rocket::http::ContentType;
use rocket::response::Redirect;
use rocket::{get, options, uri};
use rocket_okapi::openapi;
use std::path::PathBuf;

use crate::visualization::request_guard::{RawQueryString, StaticFileResponse};
use crate::visualization::vite_dev_proxy;

/// Static directory containing the web client files
///
/// This constant includes the compiled web client files at compile time.
/// The files are embedded in the binary, eliminating the need for external
/// file dependencies when deploying the server.
const STATIC_DIR: Dir = include_dir!("../web/dist");

/// Handler for HTTP OPTIONS requests required for CORS preflight
///
/// This handler responds to OPTIONS requests with a 200 OK response,
/// which is necessary for CORS preflight requests. The CORS fairing
/// will add the appropriate headers to the response.
///
/// ### Parameters
///
/// * `_path` - The path requested (ignored in this implementation)
///
/// ### Returns
///
/// An empty success result to indicate that the preflight request is accepted
#[openapi(tag = "Cors")]
#[options("/<_path..>")]
pub async fn options(_path: PathBuf) -> Result<(), std::io::Error> {
    Ok(())
}

/// Serve web client static files
///
/// This route handler serves static files for the web client interface.
/// It has two modes of operation:
///
/// 1. **Development Mode**: When the `VITE_DEVELOPMENT` environment variable is set,
///    it proxies requests to a running Vite development server
/// 2. **Production Mode**: Otherwise, it serves the embedded static files from
///    the binary
///
/// If the requested file is not found, it falls back to serving index.html,
/// enabling client-side routing.
///
/// ### Parameters
///
/// * `path` - The path to the requested file relative to the web/dist directory
///
/// ### Returns
///
/// * `Some(StaticFileResponse)` - The requested file content with appropriate headers
/// * `None` - If the file cannot be found or served
///
/// ### Development Mode
///
/// When the `VITE_DEVELOPMENT` environment variable is set, requests are proxied
/// to the URL specified in that variable (defaulting to `http://localhost:5173`).
/// This allows for hot-reloading and other development features.
#[get("/client/<path..>", rank = 2)]
pub async fn webclient(path: PathBuf, raw_query: RawQueryString) -> Option<StaticFileResponse> {
    if vite_dev_proxy::is_vite_development_enabled() {
        return vite_dev_proxy::proxy_to_vite_dev_server(path, raw_query).await;
    }

    let path = path.to_str().unwrap_or("");
    let file = STATIC_DIR.get_file(path).map(|file| {
        let content_type = ContentType::from_extension(
            file.path()
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap(),
        )
        .unwrap_or(ContentType::Binary);
        StaticFileResponse(file.contents().to_vec(), content_type)
    });
    if file.is_some() {
        file
    } else {
        let file = STATIC_DIR.get_file("index.html").map(|file| {
            let content_type = ContentType::from_extension(
                file.path()
                    .extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap(),
            )
            .unwrap_or(ContentType::Binary);
            StaticFileResponse(file.contents().to_vec(), content_type)
        });
        file
    }
}

/// Redirect `/index.html` to the web client
///
/// This route handler redirects requests for `/index.html` to the web client's
/// index.html file at `/client/index.html`. This provides a convenient shorthand
/// URL for accessing the web interface.
///
/// ### Returns
///
/// A redirect response pointing to `/client/index.html`
#[openapi(tag = "webclient")]
#[get("/index.html")]
pub async fn webclient_index_html() -> Redirect {
    webclient_index_multi().await
}

/// Helper function to redirect to the web client index
///
/// This function is shared between the root and `/index.html` routes
/// to avoid duplicating the redirect logic.
///
/// ### Returns
///
/// A redirect response pointing to `/client/index.html`
async fn webclient_index_multi() -> Redirect {
    Redirect::to(uri!("/client/index.html"))
}

/// Redirect the root path to the web client
///
/// This route handler redirects requests for the root path (`/`) to
/// the web client's index.html file. This allows users to access the
/// web interface by navigating to the server's root URL.
///
/// ### Returns
///
/// A redirect response pointing to `/client/index.html`
#[openapi(tag = "webclient")]
#[get("/")]
pub async fn webclient_index() -> Redirect {
    webclient_index_multi().await
}

/// Serve the favicon.ico file
///
/// This route handler serves the website favicon from the embedded static files.
/// The favicon is used by browsers to display a small icon in the browser tab
/// and bookmarks.
///
/// ### Returns
///
/// * `Some(StaticFileResponse)` - The favicon file content with appropriate headers
/// * `None` - If the favicon file cannot be found
#[get("/favicon.ico")]
pub async fn favicon() -> Option<StaticFileResponse> {
    let file = STATIC_DIR.get_file("favicon.ico").map(|file| {
        let content_type = ContentType::from_extension(
            file.path()
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap(),
        )
        .unwrap_or(ContentType::Binary);
        StaticFileResponse(file.contents().to_vec(), content_type)
    });
    file
}

/// Serve the helper.min.js file for rapidoc
/// It is comming from SCTG Development SCTGDesk server
/// see https://github.com/sctg-development/sctgdesk-api-server/tree/main/rapidoc
#[get("/api/doc/helper.min.js")]
pub async fn helper_min_js() -> Option<StaticFileResponse> {
    let file_content = include_str!("../../../resources/rapidoc_helper/dist/helper.min.js");
    let content_type = ContentType::JavaScript;
    let response = StaticFileResponse(file_content.as_bytes().to_vec(), content_type);
    Some(response)
}
