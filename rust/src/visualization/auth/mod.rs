//! Authentication and authorization module
//!
//! This module provides a unified authentication system supporting multiple
//! authentication mechanisms including OAuth2, JWT tokens, and request guards.

pub mod guards;
pub mod jwt;
pub mod oauth2;

// Re-export commonly used items for convenience
pub use guards::OAuthBearer;
pub use jwt::JwtValidator;
pub use oauth2::{authorize, refresh, token, OxideState};

use crate::config::AccessConfig;
use anyhow::Result;

/// Initialize the authentication system with the provided configuration
pub fn init_auth_system(
    hmac_secret: &str,
    rs256_public_key: Option<&[u8]>,
    access_config: AccessConfig,
) -> Result<JwtValidator> {
    jwt::init_jwt_validator(hmac_secret, rs256_public_key, access_config)
}
