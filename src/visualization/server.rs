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
//! use rust_photoacoustic::visualization::server;
//!
//! async fn start_server() {
//!     let config = Figment::from(rocket::Config::default())
//!         .merge(("address", "127.0.0.1"))
//!         .merge(("port", 8000));
//!     
//!     let secret = "your-secret-key";
//!     let rocket = server::build_rocket(config, secret).await;
//!     rocket.launch().await.expect("Failed to launch server");
//! }
//! ```

use crate::visualization::oxide_auth::{authorize, authorize_consent, refresh, token};
use include_dir::{include_dir, Dir};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::figment::Figment;
use rocket::http::{ContentType, Header};
use rocket::response::{Redirect, Responder};
use rocket::{async_trait, get, options, routes, uri, Build, Rocket};
use rocket::{Request, Response};
use rocket_okapi::{openapi, openapi_get_routes};
use std::env;
use std::io::Cursor;
use std::path::PathBuf;

use super::oxide_auth::OxideState;

/// Static directory containing the web client files
///
/// This constant includes the compiled web client files at compile time.
/// The files are embedded in the binary, eliminating the need for external
/// file dependencies when deploying the server.
const STATIC_DIR: Dir = include_dir!("web/dist");

/// Response type for serving static files
///
/// This struct encapsulates the binary content of a static file along
/// with its content type. It implements Rocket's `Responder` trait to
/// allow direct return from route handlers.
///
/// # Fields
///
/// * `0` - The binary content of the file
/// * `1` - The content type of the file
#[derive(Debug)]
struct StaticFileResponse(Vec<u8>, ContentType);

/// Implementation of Rocket's Responder trait for StaticFileResponse
///
/// This implementation allows StaticFileResponse to be returned directly
/// from route handlers. It sets appropriate headers including caching
/// directives to optimize performance.
#[async_trait]
impl<'r> Responder<'r, 'r> for StaticFileResponse {
    fn respond_to(self, _: &'r Request<'_>) -> rocket::response::Result<'static> {
        Response::build()
            .header(self.1) // Content-Type header
            .header(Header {
                name: "Cache-Control".into(),
                value: "max-age=604800".into(), // 1 week cache
            })
            .sized_body(self.0.len(), Cursor::new(self.0))
            .ok()
    }
}

/// Cross-Origin Resource Sharing (CORS) fairing for Rocket
///
/// This fairing adds CORS headers to all responses from the server,
/// enabling cross-origin requests from web clients. This is particularly
/// important for APIs that are accessed from web applications hosted
/// on different domains.
///
/// # Security Note
///
/// The current implementation uses very permissive settings (`*` for origins
/// and headers). For production environments, consider restricting these to
/// specific origins and headers needed by your application.
pub struct CORS;

/// Implementation of the Rocket Fairing trait for CORS
///
/// This implementation modifies HTTP responses by adding the necessary
/// CORS headers to enable cross-origin requests.
#[rocket::async_trait]
impl Fairing for CORS {
    /// Provides information about this fairing to Rocket
    ///
    /// # Returns
    ///
    /// Information about the fairing, including its name and when it should run
    fn info(&self) -> Info {
        Info {
            name: "Add CORS headers to responses",
            kind: Kind::Response, // Run after a response has been generated
        }
    }

    /// Modifies responses to include CORS headers
    ///
    /// This method is called for every response and adds the appropriate
    /// CORS headers to enable cross-origin requests.
    ///
    /// # Parameters
    ///
    /// * `_request` - The request that generated this response (unused)
    /// * `response` - The response to modify with CORS headers
    async fn on_response<'r>(&self, _request: &'r Request<'_>, response: &mut Response<'r>) {
        // Allow requests from any origin
        response.set_header(Header::new("Access-Control-Allow-Origin", "*"));

        // Allow common HTTP methods
        response.set_header(Header::new(
            "Access-Control-Allow-Methods",
            "POST, GET, PUT, DELETE, OPTIONS",
        ));

        // Allow all headers
        response.set_header(Header::new("Access-Control-Allow-Headers", "*"));

        // Allow credentials (cookies, etc.)
        response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
    }
}

/// Handler for HTTP OPTIONS requests required for CORS preflight
///
/// This handler responds to OPTIONS requests with a 200 OK response,
/// which is necessary for CORS preflight requests. The CORS fairing
/// will add the appropriate headers to the response.
///
/// # Parameters
///
/// * `_path` - The path requested (ignored in this implementation)
///
/// # Returns
///
/// An empty success result to indicate that the preflight request is accepted
#[openapi(tag = "Cors")]
#[options("/<_path..>")]
async fn options(_path: PathBuf) -> Result<(), std::io::Error> {
    Ok(())
}

/// Build a configured Rocket server instance
///
/// This function creates and configures a Rocket server instance with all
/// necessary routes, fairings, and state management for the photoacoustic
/// visualization application.
///
/// # Parameters
///
/// * `figment` - The Rocket configuration figment containing server settings
/// * `hmac_secret` - The HMAC secret key used for JWT signing and validation
///
/// # Returns
///
/// A configured Rocket instance ready to be launched
///
/// # Panics
///
/// This function will exit the process if:
/// * The JWT validator cannot be initialized with the provided secret
///
/// # Example
///
/// ```
/// use rocket::figment::Figment;
/// use rust_photoacoustic::visualization::server;
///
/// async fn example() {
///     let config = Figment::from(rocket::Config::default());
///     let rocket = server::build_rocket(config, "your-secret-key").await;
///     // Launch the server
///     // rocket.launch().await.expect("Failed to launch");
/// }
/// ```
pub async fn build_rocket(figment: Figment, hmac_secret: &str) -> Rocket<Build> {
    // Create OAuth2 state with the HMAC secret from config
    let oxide_state = OxideState::preconfigured(hmac_secret);

    // Initialize JWT validator for API authentication with the HMAC secret
    let jwt_validator = match super::api_auth::init_jwt_validator(hmac_secret) {
        Ok(validator) => std::sync::Arc::new(validator),
        Err(e) => {
            eprintln!("Failed to initialize JWT validator: {}", e);
            std::process::exit(1);
        }
    };

    rocket::custom(figment)
        .attach(CORS)
        .mount(
            "/",
            openapi_get_routes![webclient_index, webclient_index_html,],
        )
        .mount(
            "/",
            routes![
                options,
                favicon,
                webclient,
                authorize,
                authorize_consent,
                token,
                refresh,
                super::introspection::introspect,
            ],
        )
        .mount(
            "/api",
            routes![super::api_auth::get_profile, super::api_auth::get_data,],
        )
        .manage(oxide_state)
        .manage(jwt_validator)
}

#[cfg(test)]
/// Build a Rocket instance configured specifically for testing
///
/// This function creates a Rocket instance with settings optimized for
/// automated testing. It uses a random port to avoid conflicts with
/// other running services and disables logging for cleaner test output.
///
/// # Returns
///
/// A configured Rocket instance ready for testing
///
/// # Panics
///
/// This function will exit the process if:
/// * The JWT validator cannot be initialized with the test secret
///
/// # Note
///
/// This function is only available when compiled with the `test` configuration
/// and is primarily intended for internal unit and integration tests.
pub fn build_rocket_test_instance() -> Rocket<Build> {
    use rocket::Config;

    // Create a test configuration
    let config = Config::figment()
        .merge(("address", "localhost"))
        .merge(("port", 0)) // Random port for tests
        .merge(("log_level", rocket::config::LogLevel::Off));

    // Use a test HMAC secret
    let test_hmac_secret = "test-hmac-secret-key-for-testing";

    // Create OAuth2 state with the test secret
    let oxide_state = super::oxide_auth::OxideState::preconfigured(test_hmac_secret);

    // Initialize JWT validator with the test secret
    let jwt_validator = match super::api_auth::init_jwt_validator(test_hmac_secret) {
        Ok(validator) => std::sync::Arc::new(validator),
        Err(e) => {
            eprintln!("Failed to initialize JWT validator: {}", e);
            std::process::exit(1);
        }
    };

    // Build Rocket instance for tests
    rocket::custom(config)
        .attach(CORS)
        .mount(
            "/",
            routes![
                // Routes for OAuth tests
                authorize,
                authorize_consent,
                token,
                refresh,
                // TODO: Add introspection endpoint once fixed
                // super::introspection::introspect,
            ],
        )
        .mount(
            "/api",
            routes![super::api_auth::get_profile, super::api_auth::get_data,],
        )
        .manage(oxide_state)
        .manage(jwt_validator)
}

/// Retrieves a static file from the web/dist directory
///
/// # Arguments
///
/// * `path` - the path to the file relative to the web/dist directory
///
/// # Returns
///
/// * `Some(StaticFileResponse)` if the file exists, containing the file data and content type
/// * `None` if the file does not exist
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
/// # Parameters
///
/// * `path` - The path to the requested file relative to the web/dist directory
///
/// # Returns
///
/// * `Some(StaticFileResponse)` - The requested file content with appropriate headers
/// * `None` - If the file cannot be found or served
///
/// # Development Mode
///
/// When the `VITE_DEVELOPMENT` environment variable is set, requests are proxied
/// to the URL specified in that variable (defaulting to `http://localhost:5173`).
/// This allows for hot-reloading and other development features.
#[get("/client/<path..>")]
async fn webclient(path: PathBuf) -> Option<StaticFileResponse> {
    if env::var("VITE_DEVELOPMENT").is_ok() {
        let vite_base = env::var("VITE_DEVELOPMENT").unwrap_or("http://localhost:5173".to_string());
        let url = format!("{}/{}", vite_base, path.to_str().unwrap_or(""));
        let response = reqwest::get(&url).await.unwrap();
        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .parse::<ContentType>()
            .unwrap();
        let bytes = response.bytes().await.unwrap();
        let response_content: Vec<u8> = bytes.iter().copied().collect();
        let content = StaticFileResponse(response_content, content_type);
        return Some(content);
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
/// # Returns
///
/// A redirect response pointing to `/client/index.html`
#[openapi(tag = "webclient")]
#[get("/index.html")]
async fn webclient_index_html() -> Redirect {
    webclient_index_multi().await
}

/// Helper function to redirect to the web client index
///
/// This function is shared between the root and `/index.html` routes
/// to avoid duplicating the redirect logic.
///
/// # Returns
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
/// # Returns
///
/// A redirect response pointing to `/client/index.html`
#[openapi(tag = "webclient")]
#[get("/")]
async fn webclient_index() -> Redirect {
    webclient_index_multi().await
}

/// Serve the favicon.ico file
///
/// This route handler serves the website favicon from the embedded static files.
/// The favicon is used by browsers to display a small icon in the browser tab
/// and bookmarks.
///
/// # Returns
///
/// * `Some(StaticFileResponse)` - The favicon file content with appropriate headers
/// * `None` - If the favicon file cannot be found
#[get("/favicon.ico")]
async fn favicon() -> Option<StaticFileResponse> {
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
