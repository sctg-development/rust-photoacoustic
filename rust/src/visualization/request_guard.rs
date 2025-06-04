// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use rocket::http::uri::{Host, Origin};
use rocket::http::{ContentType, Header, HeaderMap};
use rocket::request::FromRequest;
use rocket::response::Responder;

use rocket::async_trait;
use rocket::{Request, Response};

use std::env;
use std::fmt::Debug;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::Deref;

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
pub struct StaticFileResponse(pub Vec<u8>, pub ContentType);

/// Implementation of Debug trait for StaticFileResponse
/// If content/type is a text type, it will print the first 1000 characters
/// otherwise it will print the first of the binary content and the content type
impl Debug for StaticFileResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.1.is_text() || self.1.is_html() || self.1.is_json() {
            let text_content = String::from_utf8_lossy(&self.0);
            f.debug_struct("StaticFileResponse")
                .field(
                    "content",
                    &text_content.chars().take(1000).collect::<String>(),
                )
                .field("content_type", &self.1)
                .finish()
        } else {
            f.debug_struct("StaticFileResponse")
                .field("binary_content_start", &self.0.get(0..100))
                .field("content_type", &self.1)
                .finish()
        }
    }
}

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
/// use rust_photoacoustic::visualization::request_guard::Headers;
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

impl Debug for Headers<'_> {
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
/// use rust_photoacoustic::visualization::request_guard::ConnectionInfo;
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

/// Request guard for accessing to the raw query string of the request
/// It is useful for proxying request to the development server
pub struct RawQueryString(pub String);

impl RawQueryString {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Implentaion of into string for RawQueryString
impl Into<String> for RawQueryString {
    /// Converts the RawQueryString into a String
    ///
    /// This allows easy conversion to a string representation of the query string,
    /// which can be useful for logging or passing to other functions.
    ///
    /// # Returns
    ///
    /// The raw query string as a String
    fn into(self) -> String {
        self.0
    }
}

/// Implementation of AsRef<str> for RawQueryString
impl AsRef<str> for RawQueryString {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

/// Implementation of Debug trait for RawQueryString
impl Debug for RawQueryString {
    /// Formats the RawQueryString for debug output
    ///
    /// This implementation allows the RawQueryString struct to be used with
    /// debug formatting macros like `println!("{:?}", raw_query)`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RawQueryString").field(&self.0).finish()
    }
}

/// Implementation of Rocket's FromRequest trait for RawQueryString
#[rocket::async_trait]
impl<'r> FromRequest<'r> for RawQueryString {
    type Error = ();

    /// Extracts the raw query string from the request
    ///
    /// This implementation provides access to the raw query string of the incoming request.
    ///
    /// # Parameters
    ///
    /// * `req` - The incoming HTTP request
    ///
    /// # Returns
    ///
    /// A successful outcome containing the raw query string
    async fn from_request(req: &'r Request<'_>) -> rocket::request::Outcome<Self, Self::Error> {
        if let Some(query) = req.uri().query() {
            // If the query string is present, return it
            return rocket::request::Outcome::Success(RawQueryString(query.to_string()));
        } else {
            let default_query = RawQueryString("".to_string());
            // If the query string is not present, return an empty one
            return rocket::request::Outcome::Success(default_query);
        }
    }
}
