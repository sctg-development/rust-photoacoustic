// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Cross-Origin Resource Sharing (CORS) support
//!
//! This module provides CORS fairing implementation for Rocket to enable
//! cross-origin requests from web clients.

use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::{Request, Response};

/// Cross-Origin Resource Sharing (CORS) fairing for Rocket
///
/// This fairing adds CORS headers to all responses from the server,
/// enabling cross-origin requests from web clients. This is particularly
/// important for APIs that are accessed from web applications hosted
/// on different domains.
///
/// ### Security Note
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
    /// ### Returns
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
    /// ### Parameters
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
