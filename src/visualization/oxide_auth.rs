// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! OAuth 2.0 server implementation using Oxide Auth
//!
//! This module implements an OAuth 2.0 authorization server for the photoacoustic
//! visualization web interface, using the Oxide Auth framework integrated with Rocket.
//! It provides endpoints for authorization, token issuance, and token refreshing.
//!
//! ## Architecture
//!
//! The OAuth implementation consists of:
//!
//! - `OxideState`: The central state container for the OAuth services
//! - Rocket handlers for various OAuth endpoints
//! - Integration with the JWT issuer for token generation
//!
//! ## Features
//!
//! - Authorization Code flow
//! - Token issuance and validation
//! - Token refreshing
//! - JWT-based access tokens with embedded claims
//!
//! ## Example Usage
//!
//! ```
//! use rocket::{build, routes};
//! use rust_photoacoustic::visualization::oxide_auth::{OxideState, authorize, token, refresh};
//!
//! // Create preconfigured OAuth state
//! let figment = rocket::Config::figment().merge(("hmac_secret", "your-secret".to_string()));
//! let state = OxideState::preconfigured(figment);
//!
//! // Configure Rocket with OAuth routes
//! let rocket = build()
//!     .mount("/oauth", routes![authorize, token, refresh])
//!     .manage(state);
//! ```
//!
//! ## Security Considerations
//!
//! - The HMAC secret should be kept secure and have sufficient entropy
//! - For production use, consider using asymmetric keys (RS256) instead of symmetric keys
//! - Client credentials should be properly validated and secured

use std::sync::{Arc, Mutex};

use handlebars::Handlebars;
use log::debug;
use oxide_auth::endpoint::{OwnerConsent, Solicitation, WebRequest};
use oxide_auth::frontends::simple::endpoint::{FnSolicitor, Generic, Vacant};
use oxide_auth::primitives::prelude::*;
use oxide_auth::primitives::registrar::RegisteredUrl;
use oxide_auth_rocket;
use oxide_auth_rocket::{OAuthFailure, OAuthRequest, OAuthResponse};
use rocket::figment::Figment;
use rocket::State;
use rocket::{get, post};
use serde_json::json;

use super::jwt::JwtIssuer;

use crate::config::{AccessConfig, User, USER_SESSION_SEPARATOR};
use base64::Engine;
use rocket::form::{Form, FromForm};
use rocket::http::{Cookie, CookieJar, Status};
use rocket::request::{FromRequest, Outcome};
use rocket::time::Duration;
use std::collections::HashMap;

/// Form data for user authentication
#[derive(FromForm)]
pub struct AuthForm {
    username: String,
    password: String,
    // Preserve OAuth parameters
    response_type: String,
    client_id: String,
    redirect_uri: String,
    state: Option<String>,
    scope: Option<String>,
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

    /// Extracts an `AuthenticatedUser` from the request if a valid session cookie is present.
    ///
    /// This function checks for a private cookie named "user_session". If the cookie
    /// exists and can be successfully parsed into a `UserSession` (containing username
    /// and permissions), it returns `Outcome::Success` with the `AuthenticatedUser`.
    /// If the cookie is missing or invalid, it returns `Outcome::Forward(())` to
    /// allow the request to continue without authentication.
    async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
        // Check for user session cookie
        let cookies = request.cookies();

        if let Some(cookie) = cookies.get_private("user_session") {
            debug!("User session cookie found: {:?}", cookie.value());
            // Parse the cookie value (format: "username:permission1,permission2")
            let parts: Vec<&str> = cookie.value().split(USER_SESSION_SEPARATOR).collect();
            debug!("Parsed cookie parts: {:?}", parts);
            if parts.len() == 2 {
                let username = parts[0].to_string();
                let permissions: Vec<String> = parts[1].split(',').map(|s| s.to_string()).collect();
                debug!("Authenticated user: {:?}", username);
                debug!("User permissions: {:?}", permissions);

                return Outcome::Success(AuthenticatedUser(UserSession {
                    username,
                    permissions,
                }));
            } else {
                debug!("Invalid user session cookie format should be 'username:permission1,permission2'");
            }
        } else {
            debug!("No user session cookie found");
        }
        // No valid session cookie found, return forward outcome

        Outcome::Forward(Status::Unauthorized)
    }
}

/// Validates user credentials against the configured user database
///
/// This function performs authentication by checking the provided username and password
/// against the users defined in the [`AccessConfig`]. It uses secure password hashing
/// verification to ensure credentials are properly validated.
///
/// # Authentication Process
///
/// 1. **Username Lookup**: Searches for a matching username in the access configuration
/// 2. **Password Hash Decoding**: Decodes the base64-encoded password hash from storage
/// 3. **Hash Cleaning**: Removes trailing newline and carriage return characters from the hash
/// 4. **Password Verification**: Uses `pwhash::unix::verify` to securely compare the provided
///    password against the stored hash using the appropriate algorithm (bcrypt, scrypt, etc.)
///
/// # Security Features
///
/// - **Constant-time comparison**: Uses `pwhash` for secure password verification
/// - **Multiple hash algorithms**: Supports various Unix crypt formats ($algo$salt$hash)
/// - **Early termination**: Stops checking after finding the username to prevent timing attacks
/// - **Base64 decoding**: Safely handles base64-encoded password hashes with error checking
///
/// # Parameters
///
/// * `username` - The username to authenticate
/// * `password` - The plaintext password provided by the user
/// * `access_config` - Reference to the [`AccessConfig`] containing user definitions
///
/// # Returns
///
/// * `Some(User)` - If authentication succeeds, returns a clone of the [`User`] object
///   containing the username, permissions, and other user metadata
/// * `None` - If authentication fails (username not found or password incorrect)
///
/// # Examples
///
/// ```rust
/// use rust_photoacoustic::config::{AccessConfig, User};
/// use rust_photoacoustic::visualization::oxide_auth::validate_user;
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
    for user in &access_config.0 {
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
                    &hash_bytes
                };
                if let Ok(stored_hash) = String::from_utf8(hash_bytes.to_vec()) {
                    // Use pwhash to verify the password
                    // The stored hash is in the format $algo$salt$hash
                    debug!(
                        "Verifying password for user: {} hash: {}",
                        username, stored_hash
                    );
                    if pwhash::unix::verify(password, &stored_hash) {
                        return Some(user.clone());
                    }
                }
            }
            break; // Username matched but password didn't, don't check other users
        }
    }
    None
}

/// Main state container for the OAuth 2.0 server implementation
///
/// `OxideState` encapsulates all the components needed for the OAuth 2.0 server:
/// client registrations, authorization storage, and token issuance. It's designed
/// to be shared across multiple routes and threads using Rocket's state management.
///
/// This structure uses thread-safe wrappers (`Arc<Mutex<>>`) around the core
/// components to ensure safe concurrent access from multiple Rocket workers.
///
/// # Components
///
/// * `registrar` - Stores registered OAuth clients
/// * `authorizer` - Manages authorization grants and codes
/// * `issuer` - JWT token issuer for generating access tokens
/// * `hmac_secret` - Shared secret for JWT token validation
///
/// # Thread Safety
///
/// All mutable components are protected by mutexes and shared via Arc to ensure
/// thread safety when used across multiple Rocket worker threads.
pub struct OxideState {
    /// Registry of OAuth clients
    ///
    /// Stores information about registered clients including:
    /// - Client IDs
    /// - Redirect URIs
    /// - Allowed scopes
    /// - Client type (public/confidential)
    registrar: Arc<Mutex<ClientMap>>,

    /// Authorization state storage
    ///
    /// Manages authorization grants and authorization codes during
    /// the OAuth flow. Uses a random generator for creating secure codes.
    authorizer: Arc<Mutex<AuthMap<RandomGenerator>>>,

    /// JWT token issuer
    ///
    /// Responsible for generating JWT access tokens with embedded claims.
    /// This is wrapped in Arc<Mutex<>> to allow shared mutable access.
    pub issuer: Arc<Mutex<JwtIssuer>>,

    /// HMAC secret for JWT validation
    ///
    /// The secret key used for signing and validating JWT tokens.
    /// This is stored here for reference by other components.
    pub hmac_secret: String,

    /// RS256 private key in base64 encoded PEM format
    ///
    /// Used for signing JWT tokens with the RS256 algorithm.
    pub rs256_private_key: String,

    /// RS256 public key in base64 encoded PEM format
    ///
    /// Used for verifying JWT tokens signed with the RS256 algorithm.
    pub rs256_public_key: String,

    /// User access configuration
    ///
    /// Contains the list of users and their permissions used for authentication
    /// and authorization in the OAuth flow.
    pub access_config: AccessConfig,
}

/// Implementation of Clone for OxideState
///
/// This implementation properly clones the Arc references without
/// duplicating the underlying data, ensuring that all clones
/// point to the same shared state.
impl Clone for OxideState {
    fn clone(&self) -> Self {
        OxideState {
            registrar: Arc::clone(&self.registrar),
            authorizer: Arc::clone(&self.authorizer),
            issuer: Arc::clone(&self.issuer),
            hmac_secret: self.hmac_secret.clone(),
            rs256_private_key: self.rs256_private_key.clone(),
            rs256_public_key: self.rs256_public_key.clone(),
            access_config: self.access_config.clone(),
        }
    }
}

/// OAuth 2.0 authorization endpoint
///
/// This Rocket handler implements the OAuth 2.0 authorization endpoint,
/// which is the entry point for the authorization code flow. It presents
/// a consent form to the user, allowing them to authorize or deny the
/// client's request for access.
///
/// # URL
///
/// `GET /authorize`
///
/// # Query Parameters
///
/// Standard OAuth 2.0 parameters:
/// - `response_type`: Must be "code"
/// - `client_id`: The client identifier
/// - `redirect_uri`: Where to send the authorization code
/// - `scope`: Requested permission scopes
/// - `state`: Optional state for CSRF protection
///
/// # Returns
///
/// - On initial access: A consent form HTML page
/// - After consent: A redirect to the client with an authorization code
/// - On error: An OAuth error response
#[get("/authorize")]
pub fn authorize(
    mut oauth: OAuthRequest<'_>,
    authenticated_user: Option<AuthenticatedUser>,
    state: &State<OxideState>,
    cookies: &CookieJar<'_>,
) -> Result<OAuthResponse, OAuthFailure> {
    debug!(
        "Cookies in /authorize: {:?}",
        cookies.iter().collect::<Vec<_>>()
    );
    debug!("User authenticated: {:?}", authenticated_user.is_some());
    // If user is already authenticated, proceed to consent
    if authenticated_user.is_some() {
        return state
            .endpoint()
            .with_solicitor(FnSolicitor(consent_form))
            .authorization_flow()
            .execute(oauth)
            .map_err(|err| err.pack::<OAuthFailure>());
    }

    // Otherwise show login form
    let query = oauth.query().unwrap_or_default();

    // Extract OAuth parameters for the login form
    let response_type = query
        .unique_value("response_type")
        .unwrap_or(std::borrow::Cow::Borrowed("code"));
    let client_id = query
        .unique_value("client_id")
        .unwrap_or(std::borrow::Cow::Borrowed(""));
    let redirect_uri = query
        .unique_value("redirect_uri")
        .unwrap_or(std::borrow::Cow::Borrowed(""));
    let state_param = query.unique_value("state").map(|s| s.to_string());
    let scope = query.unique_value("scope").map(|s| s.to_string());

    let output = login_page_html(
        response_type.to_string(),
        client_id.to_string(),
        redirect_uri.to_string(),
        state_param,
        scope,
        Some("Error: You must be logged in to authorize this client."),
    );

    Ok(OAuthResponse::new()
        .body_html(&output)
        .set_status(Status::Ok)
        .clone())
}

/// Handles user login credentials and sets session if valid
#[post("/login", data = "<form>")]
pub fn login(
    form: Form<AuthForm>,
    state: &State<OxideState>,
    cookies: &CookieJar<'_>,
) -> Result<OAuthResponse, OAuthFailure> {
    // Get access config from state
    let access_config = &state.access_config;

    // Validate user credentials
    if let Some(user) = validate_user(&form.username, &form.password, access_config) {
        // Set authenticated session cookie
        let permissions_str = user.permissions.join(",");
        let cookie_value = format!("{}{}{}", user.user, USER_SESSION_SEPARATOR, permissions_str);

        let mut cookie = Cookie::new("user_session", cookie_value);
        cookie.set_http_only(true);
        cookie.set_path("/");
        cookie.set_max_age(Duration::hours(1));
        cookies.add_private(cookie);

        // Redirect back to authorize endpoint with original parameters
        let mut query_params = HashMap::new();
        query_params.insert("response_type", form.response_type.clone());
        query_params.insert("client_id", form.client_id.clone());
        query_params.insert("redirect_uri", form.redirect_uri.clone());

        if let Some(state) = &form.state {
            query_params.insert("state", state.clone());
        }

        if let Some(scope) = &form.scope {
            query_params.insert("scope", scope.clone());
        }

        let query_string =
            serde_urlencoded::to_string(&query_params).unwrap_or_else(|_| String::new());

        return Ok(OAuthResponse::new()
            .set_location(Some(format!("/authorize?{}", query_string).as_str()))
            .set_status(Status::Found)
            .clone());
    }

    // Authentication failed, show login form again with error
    let output = login_page_html(
        form.response_type.clone(),
        form.client_id.clone(),
        form.redirect_uri.clone(),
        form.state.clone(),
        form.scope.clone(),
        Some("Invalid username or password"),
    );
    Ok(OAuthResponse::new()
        .body_html(&output)
        .set_status(Status::Unauthorized)
        .clone())
}

/// OAuth 2.0 authorization consent handling endpoint
///
/// This Rocket handler processes the user's consent decision from the
/// authorization form. It completes the authorization flow by either
/// granting or denying the client's request based on user input.
///
/// # URL
///
/// `POST /authorize?allow=[true|false]`
///
/// # Query Parameters
///
/// - `allow`: Boolean flag indicating user consent (true) or denial (false)
/// - Standard OAuth parameters carried over from the authorize request
///
/// # Returns
///
/// - On consent: A redirect to the client with an authorization code
/// - On denial: A redirect to the client with an error
/// - On error: An OAuth error response
#[post("/authorize?<allow>")]
pub fn authorize_consent(
    oauth: OAuthRequest<'_>,
    allow: Option<bool>,
    authenticated_user: Option<AuthenticatedUser>,
    state: &State<OxideState>,
) -> Result<OAuthResponse, OAuthFailure> {
    let allowed = allow.unwrap_or(false);

    // Ensure user is authenticated
    if authenticated_user.is_none() {
        return Err(OAuthFailure::from(
            oxide_auth::endpoint::OAuthError::BadRequest,
        ));
    }

    let user = authenticated_user.unwrap();

    state
        .endpoint()
        .with_solicitor(FnSolicitor(move |_: &mut _, grant: Solicitation<'_>| {
            consent_decision(allowed, grant, user.0.username.clone())
        }))
        .authorization_flow()
        .execute(oauth)
        .map_err(|err| err.pack::<OAuthFailure>())
}

/// OAuth 2.0 token endpoint
///
/// This Rocket handler implements the OAuth 2.0 token endpoint, which
/// exchanges an authorization code for an access token and optional
/// refresh token. It validates the authorization code and client
/// credentials before issuing tokens.
///
/// # URL
///
/// `POST /token`
///
/// # Request Body
///
/// Form-encoded with standard OAuth 2.0 parameters:
/// - `grant_type`: Must be "authorization_code"
/// - `code`: The authorization code from the authorize endpoint
/// - `redirect_uri`: Must match the original authorization request
/// - `client_id`: The client identifier
///
/// # Returns
///
/// - On success: A JSON response with access_token, token_type, expires_in, and refresh_token
/// - On error: An OAuth error response
#[post("/token", data = "<oauth>")]
pub async fn token<'r>(
    mut oauth: OAuthRequest<'r>,
    state: &State<OxideState>,
) -> Result<OAuthResponse, OAuthFailure> {
    let body = oauth.urlbody()?;
    let grant_type = body.unique_value("grant_type");
    debug!("grant_type: {:?}", grant_type);

    // Extract username from the OAuth request if available
    let username = body.unique_value("username").or_else(|| {
        // Try to extract from other sources if needed
        // This might need adjustment based on your OAuth flow
        None
    });

    // If we have a username, add user claims before token issuance
    if let Some(username_cow) = username {
        let username_str = username_cow.as_ref();

        // Find the user in our access config and add claims
        for user in &state.access_config.0 {
            if user.user == username_str {
                if let Ok(mut issuer) = state.issuer.lock() {
                    issuer.add_user_claims(&username_str, &user.permissions);
                }
                break;
            }
        }
    }

    if grant_type == Some(std::borrow::Cow::Borrowed("refresh_token")) {
        // Handle refresh token flow
        let mut endpoint = state.endpoint().refresh_flow();
        endpoint
            .execute(oauth)
            .map_err(|err| err.pack::<OAuthFailure>())
    } else {
        // Handle authorization code flow
        let mut endpoint = state.endpoint().access_token_flow();
        endpoint
            .execute(oauth)
            .map_err(|err| err.pack::<OAuthFailure>())
    }
}

/// OAuth 2.0 token refresh endpoint
///
/// This Rocket handler implements the OAuth 2.0 token refresh flow,
/// which issues a new access token using a previously issued refresh token.
/// It validates the refresh token before issuing a new access token.
///
/// # URL
///
/// `POST /refresh`
///
/// # Request Body
///
/// Form-encoded with standard OAuth 2.0 parameters:
/// - `grant_type`: Must be "refresh_token"
/// - `refresh_token`: The refresh token from a previous token response
/// - `client_id`: The client identifier
///
/// # Returns
///
/// - On success: A JSON response with a new access_token, token_type, expires_in, and refresh_token
/// - On error: An OAuth error response
#[post("/refresh", data = "<oauth>")]
pub async fn refresh<'r>(
    oauth: OAuthRequest<'r>,
    state: &State<OxideState>,
) -> Result<OAuthResponse, OAuthFailure> {
    state
        .endpoint()
        .refresh_flow()
        .execute(oauth)
        .map_err(|err| err.pack::<OAuthFailure>())
}

impl OxideState {
    /// Create a preconfigured OxideState with default settings
    ///
    /// This factory method creates an OxideState with sensible defaults:
    /// - A public client for the LaserSmartClient
    /// - HS256 JWT tokens valid for 1 hour
    /// - Standard scopes for API access
    /// - Multiple allowed redirect URIs for development and production
    ///
    /// # Parameters
    ///
    /// * `hmac_secret` - The secret key used for signing and validating JWT tokens
    ///
    /// # Returns
    ///
    /// A preconfigured `OxideState` instance ready to use with Rocket
    ///
    /// # Example
    ///
    /// ```
    /// use rust_photoacoustic::visualization::oxide_auth::OxideState;
    ///
    /// // Create the OAuth state with a secret key
    /// let figment = rocket::Config::figment().merge(("hmac_secret", "your-secret".to_string()));
    /// let state = OxideState::preconfigured(figment);
    /// ```
    pub fn preconfigured(figment: Figment) -> Self {
        // Extract the HMAC secret from the configuration
        let hmac_secret = figment
            .extract_inner::<String>("hmac_secret")
            .unwrap_or_else(|_| {
                panic!("Missing hmac_secret in configuration");
            });
        // Use the HMAC secret from configuration
        let jwt_secret = hmac_secret.as_bytes();

        // Create and configure the JWT issuer
        let mut jwt_issuer = JwtIssuer::new(jwt_secret);
        jwt_issuer
            .with_issuer("rust-photoacoustic") // Set the issuer name
            .valid_for(chrono::Duration::hours(1)); // Tokens valid for 1 hour

        OxideState {
            registrar: Arc::new(Mutex::new(
                vec![Client::public(
                    "LaserSmartClient",
                    RegisteredUrl::Semantic("http://localhost:8080/client/".parse().unwrap()),
                    "openid profile email read:api write:api".parse().unwrap(),
                )
                .with_additional_redirect_uris(vec![
                    RegisteredUrl::Semantic("http://localhost:5173/client/".parse().unwrap()),
                    RegisteredUrl::Semantic("https://myname.local/client/".parse().unwrap()),
                ])]
                .into_iter()
                .collect(),
            )),
            // Authorization tokens are 16 byte random keys to a memory hash map.
            authorizer: Arc::new(Mutex::new(AuthMap::new(RandomGenerator::new(16)))),
            // Use JWT issuer for access tokens
            // These tokens can be verified independently by the resource server
            // and contain user information embedded within them
            issuer: Arc::new(Mutex::new(jwt_issuer)),
            // Store the HMAC secret for validation elsewhere
            hmac_secret: hmac_secret.to_string(),
            // Add RS256 keys (to be set later)
            rs256_private_key: String::new(),
            rs256_public_key: String::new(),
            // Initialize access config with default values
            access_config: AccessConfig::default(),
        }
    }

    /// Create an OAuth endpoint with this state
    ///
    /// This method creates a new OAuth endpoint configured with this state's
    /// registrar, authorizer, and issuer. The endpoint can then be further
    /// customized with solicitors and scope validators before executing
    /// an OAuth flow.
    ///
    /// # Returns
    ///
    /// A Generic OAuth endpoint ready to be configured for a specific flow
    ///
    /// # Panics
    ///
    /// This method will panic if any of the internal mutexes are poisoned
    /// (which would indicate a thread panic while holding the lock).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_photoacoustic::visualization::oxide_auth::OxideState;
    /// let figment = rocket::Config::figment().merge(("hmac_secret", "your-secret"));
    /// let state = OxideState::preconfigured(figment);
    /// # // We don't need an oauth_request for this example
    ///
    /// // Configure and execute an authorization flow
    /// let endpoint = state.endpoint();
    /// // From this point we would use the endpoint for OAuth authorization
    /// ```
    pub fn endpoint(&self) -> Generic<impl Registrar + '_, impl Authorizer + '_, impl Issuer + '_> {
        Generic {
            registrar: self.registrar.lock().unwrap(),
            authorizer: self.authorizer.lock().unwrap(),
            issuer: self.issuer.lock().unwrap(),
            // Solicitor configured later.
            solicitor: Vacant,
            // Scope configured later.
            scopes: Vacant,
            // `rocket::Response` is `Default`, so we don't need more configuration.
            response: Vacant,
        }
    }
}

/// Generate a consent form for the user to authorize a client
///
/// This function is used as a solicitor in the OAuth authorization flow.
/// It generates an HTML form asking the user to either grant or deny
/// permission for the client to access resources on their behalf.
///
/// # Parameters
///
/// * `_` - The OAuth request (unused in this implementation)
/// * `solicitation` - Contains information about the authorization request,
///   including client ID, requested scope, and redirect URI
///
/// # Returns
///
/// An `OwnerConsent` indicating that the authorization flow is still in progress,
/// with an HTML response containing the consent form
fn consent_form(
    _: &mut OAuthRequest<'_>,
    solicitation: Solicitation,
) -> OwnerConsent<OAuthResponse> {
    let output = consent_page_html("/authorize", solicitation);
    OwnerConsent::InProgress(OAuthResponse::new().body_html(&output).to_owned())
}

/// Process the user's consent decision
///
/// This function takes the user's decision (allow or deny) and returns
/// the appropriate `OwnerConsent` value to continue the OAuth flow.
///
/// # Parameters
///
/// * `allowed` - Whether the user granted permission (true) or denied it (false)
/// * `_` - The solicitation details (unused in this implementation)
/// * `username` - The authenticated user's username
///
/// # Returns
///
/// * `OwnerConsent::Authorized` - If the user granted permission
/// * `OwnerConsent::Denied` - If the user denied permission
///
/// # Note
///
/// In a production system, this would typically identify the actual user
/// instead of using "dummy user" as the owner ID.
fn consent_decision<'r>(
    allowed: bool,
    _: Solicitation,
    username: String,
) -> OwnerConsent<OAuthResponse> {
    if allowed {
        OwnerConsent::Authorized(username)
    } else {
        OwnerConsent::Denied
    }
}

/// Generate the HTML for the OAuth consent page
///
/// This function generates the HTML for the consent page shown to users
/// during the OAuth authorization flow. It creates a simple form with
/// Accept and Deny buttons, showing the client ID, redirect URI, and
/// requested permissions.
///
/// # Parameters
///
/// * `route` - The route that will handle the consent form submission
/// * `solicitation` - Contains information about the authorization request
///
/// # Returns
///
/// A string containing the HTML for the consent page
///
fn consent_page_html(route: &str, solicitation: Solicitation) -> String {
    let mut handlebars = Handlebars::new();

    // Register the consent template
    handlebars
        .register_template_string("consent", include_str!("../../resources/forms/consent.hbs"))
        .expect("Failed to register consent template");

    let grant = solicitation.pre_grant();
    let state = solicitation.state();

    // Preserve all required OAuth parameters in the form submission
    let mut extra = vec![
        ("response_type", "code"),
        ("client_id", grant.client_id.as_str()),
        ("redirect_uri", grant.redirect_uri.as_str()),
    ];

    // Include state parameter if it was provided in the original request
    if let Some(state) = state {
        extra.push(("state", state));
    }

    let query_params = serde_urlencoded::to_string(extra).unwrap_or_default();
    let formatted_scopes = format_scopes(&grant.scope.to_string());

    let data = json!({
        "client_id": grant.client_id.as_str(),
        "redirect_uri": grant.redirect_uri.as_str(),
        "formatted_scopes": formatted_scopes,
        "route": route,
        "query_params": query_params
    });

    handlebars
        .render("consent", &data)
        .expect("Failed to render consent template")
}

/// Generate a login form for user authentication
fn login_page_html(
    response_type: String,
    client_id: String,
    redirect_uri: String,
    state: Option<String>,
    scope: Option<String>,
    error_msg: Option<&str>,
) -> String {
    let mut handlebars = Handlebars::new();

    // Register the template
    handlebars
        .register_template_string("login", include_str!("../../resources/forms/login.hbs"))
        .expect("Failed to register login template");

    let data = json!({
        "client_id": client_id,
        "error_msg": error_msg,
        "response_type": response_type,
        "redirect_uri": redirect_uri,
        "state": state,
        "scope": scope
    });

    handlebars
        .render("login", &data)
        .expect("Failed to render login template")
}

/// Format scope string into HTML list items
fn format_scopes(scope: &str) -> String {
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
            format!("<div class=\"scope-item\">{} {}</div>", icon, description)
        })
        .collect::<Vec<String>>()
        .join("\n")
}
