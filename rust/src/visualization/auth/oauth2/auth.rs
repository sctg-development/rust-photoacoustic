// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Authentication utilities
//!
//! This module provides authentication helper functions for validating
//! user credentials and managing user sessions.

use base64::Engine;
use log::debug;

use crate::config::{AccessConfig, User};
use crate::visualization::pwhash;

/// Validate user credentials against the access configuration
///
/// This function validates a username and password combination against
/// the configured user database. It uses secure password hashing to
/// verify credentials and returns the authenticated user's information
/// if the credentials are valid.
///
/// # Password Verification Process
///
/// 1. **User Lookup**: Searches for the username in the access configuration
/// 2. **Hash Decoding**: Decodes the stored base64-encoded password hash
/// 3. **Format Cleanup**: Removes trailing newlines/carriage returns from the hash
/// 4. **Hash Verification**: Uses `pwhash::unix::verify` to check the password
///
/// # Supported Hash Formats
///
/// The function supports Unix-style password hashes in the format:
/// `$algorithm$salt$hash`
///
/// Common algorithms include:
/// - `$6$` - SHA-512 based crypt
/// - `$5$` - SHA-256 based crypt
/// - `$1$` - MD5 based crypt (not recommended)
///
/// # Security Features
///
/// - **Constant-time comparison**: Uses secure hash verification functions
/// - **Salt protection**: Leverages salted hashes to prevent rainbow table attacks  
/// - **Early termination**: Stops checking once the correct user is found
/// - **No timing attacks**: Hash verification timing is consistent
///
/// # Parameters
///
/// * `username` - The username to authenticate
/// * `password` - The plaintext password to verify
/// * `access_config` - The access configuration containing user credentials
///
/// # Returns
///
/// * `Some(User)` - If authentication succeeds, returns the user with permissions
/// * `None` - If authentication fails due to:
///   - Username not found
///   - Password verification failure
///   - Invalid hash format
///   - Hash decoding errors
///
/// # Examples
///
/// ```
/// use rust_photoacoustic::config::AccessConfig;
/// use rust_photoacoustic::visualization::auth::oauth2::validate_user;
///
/// // Assuming you have an AccessConfig with users
/// let access_config = AccessConfig::default();
///
/// // Validate user credentials
/// match validate_user("alice", "secret123", &access_config) {
///     Some(user) => {
///         println!("User {} authenticated with permissions: {:?}",
///                  user.user, user.permissions);
///     }
///     None => {
///         println!("Authentication failed");
///     }
/// }
/// ```
///
/// # Related Functions
///
/// - [`User::new`] - Creates new user objects
/// - [`pwhash::unix::verify`] - The underlying password verification function
pub fn validate_user(username: &str, password: &str, access_config: &AccessConfig) -> Option<User> {
    for user in &access_config.users {
        if user.user == username {
            // Decode the base64 password hash
            if let Ok(hash_bytes) = base64::engine::general_purpose::STANDARD.decode(&user.pass) {
                // If last byte is \n, remove it
                let hash_bytes = if hash_bytes.last() == Some(&b'\n') {
                    &hash_bytes[..hash_bytes.len() - 1]
                } else {
                    &hash_bytes
                };
                // if last byte is \r, remove it
                let hash_bytes = if hash_bytes.last() == Some(&b'\r') {
                    &hash_bytes[..hash_bytes.len() - 1]
                } else {
                    hash_bytes
                };
                if let Ok(stored_hash) = String::from_utf8(hash_bytes.to_vec()) {
                    // Use pwhash to verify the password
                    // The stored hash is in the format $algo$salt$hash
                    debug!(
                        "Verifying password for user: {} hash: {}",
                        username, stored_hash
                    );
                    if pwhash::verify(password, &stored_hash) {
                        return Some(user.clone());
                    }
                }
            }
            break; // Username matched but password didn't, don't check other users
        }
    }
    None
}
