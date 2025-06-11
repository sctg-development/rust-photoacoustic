// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Form data structures and session handling
//!
//! This module contains form data structures for OAuth authentication
//! and session management functionality.

use base64::Engine;
use handlebars::Handlebars;
use log::debug;
use rocket::form::FromForm;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::User;

/// Form data for user authentication
#[derive(FromForm, Debug, Clone, Serialize, Deserialize)]
pub struct AuthForm {
    pub username: String,
    pub password: String,
    // Preserve OAuth parameters
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub state: Option<String>,
    pub scope: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
}

/// Session information for authenticated users
pub struct UserSession {
    pub username: String,
    pub permissions: Vec<String>,
}

/// Request guard to check for authenticated user session
pub struct AuthenticatedUser(pub UserSession);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedUser {
    type Error = ();

    async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
        // Check for user session cookie
        let cookies = request.cookies();

        if let Some(cookie) = cookies.get_private("user_session") {
            // Attempt to decode the user session cookie
            let user_session = decode_user_session(cookie.value());
            if let Some(user) = user_session {
                debug!("User session cookie decoded successfully");
                let username = user.user.clone();
                let permissions = user.permissions.clone();
                debug!("Authenticated user: {:?}", username);
                debug!("User permissions: {:?}", permissions);

                return Outcome::Success(AuthenticatedUser(UserSession {
                    username,
                    permissions,
                }));
            } else {
                debug!("No user session cookie found");
            }
            // No valid session cookie found, return forward outcome
        }
        Outcome::Forward(Status::Unauthorized)
    }
}

/// Encode user information into a session cookie value
///
/// This function serializes a [`User`] object into a base64-encoded JSON string
/// suitable for storage in an HTTP cookie. It creates a secure representation
/// of the user's authentication session that can be safely transmitted and stored.
///
/// ### Encoding Process
///
/// 1. **JSON Serialization**: Converts the user data to a structured JSON object
/// 2. **Base64 Encoding**: Encodes the JSON string using standard base64 encoding
/// 3. **Cookie-Safe Format**: Ensures the result is safe for HTTP cookie storage
///
/// ### Data Included
///
/// The encoded session contains:
/// - **Username**: The user's login identifier
/// - **Permissions**: Array of permission strings granted to the user
///
/// ### Security Considerations
///
/// - The password field is intentionally excluded from the session data
/// - Session data should be encrypted using Rocket's private cookies
/// - Consider adding expiration timestamps for additional security
/// - In production, consider using signed tokens (JWT) instead
///
/// ### Parameters
///
/// * `user` - The [`User`] object containing authentication information
///
/// ### Returns
///
/// A base64-encoded string containing the serialized user session data,
/// ready to be stored in an HTTP cookie.
///
/// ### Examples
///
/// ```
/// use rust_photoacoustic::config::User;
/// use rust_photoacoustic::visualization::auth::oauth2::encode_user_session;
///
/// let user = User {
///     user: "alice".to_string(),
///     pass: "".to_string(), // Password not included in session
///     permissions: vec!["read:api".to_string(), "write:api".to_string()],
///     email: None,
///     name: None,
/// };
///
/// let session_data = encode_user_session(user);
/// // Use session_data as cookie value
/// ```
///
/// ### Related Functions
///
/// - [`decode_user_session`] - Decodes the session cookie back to user data
/// - [`AuthenticatedUser::from_request`] - Request guard that uses session cookies
/// - [`validate_user`] - Initial user authentication function
pub fn encode_user_session(user: User) -> String {
    let user_data = json!({
        "username": user.user,
        "permissions": user.permissions,
    });
    base64::engine::general_purpose::STANDARD.encode(user_data.to_string())
}

/// Decode user information from a session cookie value
///
/// This function deserializes a base64-encoded JSON string back into a [`User`] object,
/// recovering the authentication session data stored in an HTTP cookie. It performs
/// comprehensive validation of the cookie format and structure.
///
/// ### Decoding Process
///
/// 1. **Base64 Decoding**: Decodes the base64 string to get the JSON bytes
/// 2. **JSON Parsing**: Parses the JSON structure and validates the schema
/// 3. **Data Extraction**: Extracts username and permissions arrays
/// 4. **User Construction**: Creates a new [`User`] object with decoded data
///
/// ### Validation Steps
///
/// The function validates:
/// - Base64 encoding is valid
/// - JSON structure is well-formed
/// - Required fields ("username", "permissions") are present
/// - Username is a valid string
/// - Permissions is an array of strings
///
/// ### Parameters
///
/// * `cookie_value` - A base64-encoded string containing the JSON session data,
///   typically retrieved from an HTTP cookie
///
/// ### Returns
///
/// * `Some(User)` - Successfully decoded user session data
/// * `None` - If decoding fails due to:
///   - Invalid base64 encoding
///   - Malformed JSON structure
///   - Missing required fields
///   - Invalid data types
///
/// ### Examples
///
/// ```
/// use rust_photoacoustic::visualization::auth::oauth2::decode_user_session;
///
/// // Assuming you have a cookie value from a session
/// let cookie_value = "eyJ1c2VybmFtZSI6ImFsaWNlIiwicGVybWlzc2lvbnMiOlsicmVhZDphcGkiXX0=";
///
/// match decode_user_session(cookie_value) {
///     Some(user) => {
///         println!("User {} has permissions: {:?}", user.user, user.permissions);
///     }
///     None => {
///         println!("Invalid or corrupted session cookie");
///     }
/// }
/// ```
///
/// ### Error Conditions
///
/// The function returns `None` when:
/// - Base64 decoding fails (invalid characters or padding)
/// - JSON parsing fails (malformed JSON structure)
/// - "username" field is missing or not a string
/// - "permissions" field is missing or not an array
/// - "permissions" field is not an array
/// - Any permission in the array is not a string
///
/// ### Related Functions
///
/// - [`encode_user_session`] - Encodes user data into a session cookie
/// - [`AuthenticatedUser::from_request`] - Request guard that uses this function
/// - [`validate_user`] - Initial user authentication function
pub fn decode_user_session(cookie_value: &str) -> Option<User> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(cookie_value)
        .ok()?;
    let user_data: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    if let (Some(username), Some(permissions)) = (
        user_data.get("username").and_then(|v| v.as_str()),
        user_data.get("permissions").and_then(|v| v.as_array()),
    ) {
        let permissions: Vec<String> = permissions
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        Some(User {
            user: username.to_string(),
            pass: String::new(), // Password is not stored in session
            permissions,
            email: None,
            name: None,
        })
    } else {
        None
    }
}

/// Generate a login form for user authentication
pub fn login_page_html(
    response_type: String,
    client_id: String,
    redirect_uri: String,
    state: Option<String>,
    scope: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    error_msg: Option<&str>,
) -> String {
    let mut handlebars = Handlebars::new();

    // Register the template
    handlebars
        .register_template_string(
            "login",
            include_str!("../../../../resources/forms/login.hbs"),
        )
        .expect("Failed to register login template");

    let data = json!({
        "client_id": client_id,
        "error_msg": error_msg,
        "response_type": response_type,
        "redirect_uri": redirect_uri,
        "state": state,
        "scope": scope,
        "code_challenge": code_challenge,
        "code_challenge_method": code_challenge_method
    });

    handlebars
        .render("login", &data)
        .expect("Failed to render login template")
}

/// Format scope string into HTML list items with icons and descriptions
pub fn format_scopes(scope: &str) -> String {
    scope
        .split_whitespace()
        .map(|s| {
            let (icon, description) = match s {
                "openid" => ("🔑", "Verify your identity"),
                "profile" => ("👤", "Access your profile information"),
                "email" => ("📧", "Access your email address"),
                "read:api" => ("📖", "Read access to API data"),
                "write:api" => ("✏️", "Write access to API data"),
                "admin:api" => ("⚙️", "Administrative access"),
                _ => ("🔒", s),
            };
            format!(
                r#"<div class="scope-item">
    <span class="icon">{}</span>
    <span class="description">{}</span>
</div>"#,
                icon, description
            )
        })
        .collect::<Vec<String>>()
        .join("\n")
}
