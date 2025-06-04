//! JWT token management and validation
//!
//! This submodule handles JWT token creation, validation, and user information extraction.

mod claims;
mod issuer;
mod keys;
mod token_entry;
mod token_map;
mod validator;

// Re-export public API
pub use issuer::JwtIssuer;
pub use keys::JwkKeySet;
pub use validator::{JwtValidator, UserSysInfo};

use crate::config::AccessConfig;
use anyhow::Result;

/// Initialize JWT validator with the provided configuration
pub fn init_jwt_validator(
    hmac_secret: &str,
    rs256_public_key: Option<&[u8]>,
    access_config: AccessConfig,
) -> Result<JwtValidator> {
    // Support both keys if both are provided
    let hmac_opt = if !hmac_secret.is_empty() {
        Some(hmac_secret.as_bytes())
    } else {
        None
    };
    let validator = JwtValidator::new(hmac_opt, rs256_public_key, access_config)
        .map_err(|e| anyhow::anyhow!("Failed to create JWT validator: {}", e))?;
    Ok(validator
        .with_issuer("LaserSmartServer")
        .with_audience("LaserSmartClient"))
}
