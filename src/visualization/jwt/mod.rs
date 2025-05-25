// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! JWT token generation and management for OAuth authentication
//!
//! This module implements a JWT-based token issuer that integrates with the Oxide Auth
//! framework. It provides functionality for:
//!
//! - Creating and managing JWT access tokens
//! - Generating refresh tokens
//! - Token validation and verification
//! - OAuth 2.0 token issuance and refresh workflows
//!
//! The JWT tokens are signed using configurable algorithms (default: HS256) and include
//! standard claims like subject, audience, and expiration time.
//!
//! # Architecture
//!
//! The module consists of three main components:
//! - `JwtTokenMap`: Core implementation of token management
//! - `JwtIssuer`: Thread-safe wrapper around `JwtTokenMap` with Mutex
//! - `JwtClaims`: Structure representing the claims in a JWT token
//!
//! # Example Usage
//!
//! ```
//! use rust_photoacoustic::visualization::jwt::JwtIssuer;
//! use chrono::Duration;
//!
//! // Create a new JWT issuer with a secret key
//! let mut issuer = JwtIssuer::new(b"your-secret-key");
//!
//! // Configure the issuer
//! issuer
//!     .with_issuer("my-application")
//!     .valid_for(Duration::hours(2));
//!
//! // The issuer can now be used with oxide_auth to issue OAuth tokens
//! ```
//!
//! # Security Considerations
//!
//! - Use appropriate key sizes for the chosen algorithm
//! - For production, consider using RS256 with separate signing and verification keys
//! - Store secrets securely and never expose them in client-side code

// Internal modules that are not public
mod claims;
mod issuer;
mod responses;
mod token_entry;
mod token_map;

// Existing modules that remain public
/// JWT extensions for OpenID Connect support
pub mod jwt_extension;

/// RSA key management for JWT signing and verification
pub mod jwt_keys;

/// JWT token validation and user information extraction
pub mod jwt_validator;

// Re-export the public API
pub use claims::{IdTokenClaims, JwtClaims};
pub use issuer::JwtIssuer;
pub use responses::OidcTokenResponse;
pub use token_entry::TokenEntry;
pub use token_map::JwtTokenMap;
