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

use crate::config::AccessConfig;
use crate::include_png_as_base64;
use crate::visualization::oidc::{jwks, openid_configuration}; // Add this import
use crate::visualization::oxide_auth::{authorize, authorize_consent, refresh, token};
use anyhow::Context;
use base64::Engine;
use include_dir::{include_dir, Dir};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::figment::Figment;
use rocket::http::uri::{Host, Origin};
use rocket::http::{ContentType, Header, HeaderMap};
use rocket::request::FromRequest;
use rocket::response::{Redirect, Responder};
use rocket::{async_trait, get, options, routes, uri, Build, Rocket};
use rocket::{Request, Response};
use rocket_okapi::{openapi, openapi_get_routes, rapidoc::*, settings::UrlObject};
use std::env;
use std::fmt::Debug;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::Deref;
use std::path::PathBuf;

use super::oxide_auth::{login, OxideState};

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

/// Request guard for accessing HTTP headers in a route
///
/// This struct acts as a wrapper around Rocket's `HeaderMap`, providing a
/// type-safe way to access the HTTP headers of an incoming request. It can be
/// used directly as a parameter in route handlers to access all request headers.
///
/// # Usage in Routes
///
/// ```
/// use rocket::get;
/// use rust_photoacoustic::visualization::server::Headers;
/// #[get("/example")]
/// fn example_route(headers: Headers<'_>) -> String {
///     // Check if a specific header exists
///     let has_auth = headers.contains("Authorization");
///     
///     // Get a specific header value
///     let user_agent = headers.get_one("User-Agent").unwrap_or("Unknown");
///     
///     format!("Has Auth: {}, User-Agent: {}", has_auth, user_agent)
/// }
/// ```
///
/// # Implementation Details
///
/// This struct implements Rocket's `FromRequest` trait, allowing it to be used
/// as a request guard in route handlers. When a route with this parameter is invoked,
/// Rocket will automatically extract the request headers and make them available
/// through this struct.
pub struct Headers<'r>(pub &'r HeaderMap<'r>);

impl<'r> Deref for Headers<'r> {
    type Target = HeaderMap<'r>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Headers<'r> {
    type Error = ();

    /// Extracts the HTTP headers from the request
    ///
    /// This implementation always succeeds and provides access to the request's
    /// headers through the `Headers` struct.
    ///
    /// # Parameters
    ///
    /// * `req` - The incoming HTTP request
    ///
    /// # Returns
    ///
    /// A successful outcome containing the headers from the request
    async fn from_request(req: &'r Request<'_>) -> rocket::request::Outcome<Self, Self::Error> {
        rocket::request::Outcome::Success(Headers(req.headers()))
    }
}

impl<'r> Debug for Headers<'r> {
    /// Formats the Headers for debug output
    ///
    /// This implementation allows the Headers struct to be used with
    /// debug formatting macros like `println!("{:?}", headers)`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Headers").field(self.0).finish()
    }
}

/// Request guard for accessing detailed connection information from the client
///
/// This struct provides comprehensive information about the incoming HTTP connection,
/// including host, IP addresses, URL structure, and connection scheme (HTTP/HTTPS).
/// It can be used in route handlers to obtain details about how a client is connecting
/// to the server, which is useful for logging, analytics, and generating absolute URLs.
///
/// # Fields
///
/// * `host_port` - The host and port as a string (e.g., "example.com:8080")
/// * `origin` - The normalized URI origin from the request
/// * `ip` - The client's IP address, or 127.0.0.1 if unavailable
/// * `real_ip` - The client's real IP address from X-Forwarded-For header if available
/// * `remote` - The client's socket address if available
/// * `scheme` - The URL scheme ("http" or "https")
/// * `base_url_with_port` - The base URL including the port (e.g., "https://example.com:8080")
/// * `base_url` - The base URL without the port if standard (e.g., "https://example.com")
///
/// # Usage in Routes
///
/// ```
/// use rocket::get;
/// use rust_photoacoustic::visualization::server::ConnectionInfo;
///
/// #[get("/connection-info")]
/// fn show_connection_info(conn_info: ConnectionInfo<'_>) -> String {
///     format!(
///         "Connected via: {}\nYour IP: {}\nBase URL: {}",
///         conn_info.scheme, conn_info.ip, conn_info.base_url
///     )
/// }
/// ```
///
/// # Security Considerations
///
/// This struct provides information that could be useful for logging and debugging,
/// but care should be taken when exposing client IP addresses or other connection
/// details in responses, as this could have privacy implications. Additionally, in
/// production environments with reverse proxies, ensure proper configuration of
/// the X-Forwarded-For and related headers for accurate client IP detection.
pub struct ConnectionInfo<'r> {
    pub host_port: String,
    pub origin: Origin<'r>,
    pub ip: IpAddr,
    pub real_ip: Option<IpAddr>,
    pub remote: Option<SocketAddr>,
    pub scheme: String,
    pub base_url_with_port: String,
    pub base_url: String,
}
/// Request guard for accessing connection information
#[rocket::async_trait]
impl<'r> FromRequest<'r> for ConnectionInfo<'r> {
    type Error = ();

    /// Extracts connection information from the request
    ///
    /// This implementation provides access to the host, port, scheme,
    /// and path of the incoming request.
    /// NOTE: if the host is not set in the request, it will use localhost:8080 hardcoded
    ///
    /// # Parameters
    ///
    /// * `req` - The incoming HTTP request
    ///
    /// # Returns
    ///
    /// A successful outcome containing the connection information
    async fn from_request(req: &'r Request<'_>) -> rocket::request::Outcome<Self, Self::Error> {
        let default_host_string = env::var("HOST").unwrap_or_else(|_| "localhost:8080".to_string());
        let default_host = Host::parse(default_host_string.as_str()).expect("valid host");
        let host_port = req.host().unwrap_or(&default_host);
        let port = host_port.port().unwrap_or(80);
        let host: &str = host_port.domain().as_str();
        let origin = req.uri().to_owned().into_normalized();
        let ip = req
            .client_ip()
            .unwrap_or(Ipv4Addr::new(127, 0, 0, 1).into());
        let real_ip = req.real_ip();
        let remote = req.remote();
        let scheme = if req.rocket().config().tls_enabled() {
            "https".to_string()
        } else {
            "http".to_string()
        };
        let base_url_with_port = format!("{}://{}", scheme, host_port);
        let base_url = if port == 80 || port == 443 {
            format!("{}://{}", scheme, host)
        } else {
            format!("{}://{}:{}", scheme, host, port)
        };
        rocket::request::Outcome::Success(ConnectionInfo {
            host_port: host_port.to_string(),
            origin,
            ip,
            real_ip,
            remote,
            scheme,
            base_url_with_port,
            base_url,
        })
    }
}

impl<'r> Debug for ConnectionInfo<'r> {
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
///     let rocket = server::build_rocket(config).await;
///     // Launch the server
///     // rocket.launch().await.expect("Failed to launch");
/// }
/// ```
pub async fn build_rocket(figment: Figment) -> Rocket<Build> {

    let hmac_secret = figment
        .extract_inner::<String>("hmac_secret").context("Missing HMAC secret in config").unwrap();
    // Create OAuth2 state with the HMAC secret from config
    let mut oxide_state = OxideState::preconfigured(figment.clone());

    // Extract RS256 keys from figment if present
    if let Some(private_key) = figment.extract_inner::<String>("rs256_private_key").ok() {
        oxide_state.rs256_private_key = private_key;
    }

    if let Some(public_key) = figment.extract_inner::<String>("rs256_public_key").ok() {
        oxide_state.rs256_public_key = public_key;

        // If we have RS256 keys, update the JWT issuer
        if !oxide_state.rs256_public_key.is_empty() && !oxide_state.rs256_private_key.is_empty() {
            if let Ok(decoded_private) =
                base64::engine::general_purpose::STANDARD.decode(&oxide_state.rs256_private_key)
            {
                if let Ok(decoded_public) =
                    base64::engine::general_purpose::STANDARD.decode(&oxide_state.rs256_public_key)
                {
                    // Create a new JWT issuer with RS256 keys
                    if let Ok(jwt_issuer) =
                        super::jwt::JwtIssuer::with_rs256_pem(&decoded_private, &decoded_public)
                    {
                        oxide_state.issuer = std::sync::Arc::new(std::sync::Mutex::new(jwt_issuer));
                    }
                }
            }
        }
    }

    // Extract user access configuration from figment
    if let Some(access_config) = figment.extract_inner::<AccessConfig>("access").ok() {
        oxide_state.access_config = access_config;
    }

    // Initialize JWT validator for API authentication with the HMAC secret
    let jwt_validator = match super::api_auth::init_jwt_validator(hmac_secret.clone().as_str()) {
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
            openapi_get_routes![webclient_index, webclient_index_html,options],
        )
        .mount(
            "/",
            routes![
                favicon,
                webclient,
                authorize,
                authorize_consent,
                login,
                token,
                refresh,
                super::introspection::introspect,
                openid_configuration,
                jwks,
            ],
        )
        .mount(
            "/api",
            routes![super::api_auth::get_profile, super::api_auth::get_data,],
        )
        .mount(
            "/api/doc/",
            make_rapidoc(&RapiDocConfig {
                title: Some("SCTG rust-photoacoustic API Doc".to_owned()),
                custom_html: Some(include_str!("../../resources/rapidoc/index.html").to_owned()),
                slots: SlotsConfig{
                    logo: Some(include_png_as_base64!("../../resources/rapidoc/logo.png")),
                    footer: Some(r#"© 2025 <a style="color: #ffffff; text-decoration: none;" href='https://sctg.eu.org/'>SCTG</a>. All rights reserved. <a style="color: #ffffff; text-decoration: none;" href="https://github.com/sctg-development/sctgdesk-server">sctgdesk-server <svg style="height:1.25em" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 496 512"><path d="M165.9 397.4c0 2-2.3 3.6-5.2 3.6-3.3 .3-5.6-1.3-5.6-3.6 0-2 2.3-3.6 5.2-3.6 3-.3 5.6 1.3 5.6 3.6zm-31.1-4.5c-.7 2 1.3 4.3 4.3 4.9 2.6 1 5.6 0 6.2-2s-1.3-4.3-4.3-5.2c-2.6-.7-5.5 .3-6.2 2.3zm44.2-1.7c-2.9 .7-4.9 2.6-4.6 4.9 .3 2 2.9 3.3 5.9 2.6 2.9-.7 4.9-2.6 4.6-4.6-.3-1.9-3-3.2-5.9-2.9zM244.8 8C106.1 8 0 113.3 0 252c0 110.9 69.8 205.8 169.5 239.2 12.8 2.3 17.3-5.6 17.3-12.1 0-6.2-.3-40.4-.3-61.4 0 0-70 15-84.7-29.8 0 0-11.4-29.1-27.8-36.6 0 0-22.9-15.7 1.6-15.4 0 0 24.9 2 38.6 25.8 21.9 38.6 58.6 27.5 72.9 20.9 2.3-16 8.8-27.1 16-33.7-55.9-6.2-112.3-14.3-112.3-110.5 0-27.5 7.6-41.3 23.6-58.9-2.6-6.5-11.1-33.3 2.6-67.9 20.9-6.5 69 27 69 27 20-5.6 41.5-8.5 62.8-8.5s42.8 2.9 62.8 8.5c0 0 48.1-33.6 69-27 13.7 34.7 5.2 61.4 2.6 67.9 16 17.7 25.8 31.5 25.8 58.9 0 96.5-58.9 104.2-114.8 110.5 9.2 7.9 17 22.9 17 46.4 0 33.7-.3 75.4-.3 83.6 0 6.5 4.6 14.4 17.3 12.1C428.2 457.8 496 362.9 496 252 496 113.3 383.5 8 244.8 8zM97.2 352.9c-1.3 1-1 3.3 .7 5.2 1.6 1.6 3.9 2.3 5.2 1 1.3-1 1-3.3-.7-5.2-1.6-1.6-3.9-2.3-5.2-1zm-10.8-8.1c-.7 1.3 .3 2.9 2.3 3.9 1.6 1 3.6 .7 4.3-.7 .7-1.3-.3-2.9-2.3-3.9-2-.6-3.6-.3-4.3 .7zm32.4 35.6c-1.6 1.3-1 4.3 1.3 6.2 2.3 2.3 5.2 2.6 6.5 1 1.3-1.3 .7-4.3-1.3-6.2-2.2-2.3-5.2-2.6-6.5-1zm-11.4-14.7c-1.6 1-1.6 3.6 0 5.9 1.6 2.3 4.3 3.3 5.6 2.3 1.6-1.3 1.6-3.9 0-6.2-1.4-2.3-4-3.3-5.6-2z"/></svg></a>"#.to_owned()),
                    ..Default::default()
                },
                general: GeneralConfig {
                    spec_urls: vec![UrlObject::new("General", "../../openapi.json")],
                    ..Default::default()
                },
                hide_show: HideShowConfig {
                    allow_spec_url_load: false,
                    allow_spec_file_load: false,
                    allow_spec_file_download: true,
                    show_curl_before_try: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
        )
        .mount("/api/doc/", routes![helper_min_js])
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
    // Add the test HMAC secret to the configuration
    let config = config.merge(("hmac_secret", test_hmac_secret.to_string()));

    // Create OAuth2 state with the test secret
    let oxide_state = super::oxide_auth::OxideState::preconfigured(config.clone());

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
                openid_configuration, // Add OIDC configuration endpoint
                jwks,                 // Add JWKS endpoint
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

/// Serve the helper.min.js file for rapidoc
/// It is comming from SCTG Development SCTGDesk server
/// see https://github.com/sctg-development/sctgdesk-api-server/tree/main/rapidoc
#[get("/helper.min.js")]
async fn helper_min_js() -> Option<StaticFileResponse> {
    let file_content = include_str!("../../resources/rapidoc/helper.min.js");
    let content_type = ContentType::JavaScript;
    let response = StaticFileResponse(file_content.as_bytes().to_vec(), content_type);
    Some(response)
}
