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
//! let state = OxideState::preconfigured("your-secret-key");
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

use oxide_auth::endpoint::{OwnerConsent, Solicitation};
use oxide_auth::frontends::simple::endpoint::{FnSolicitor, Generic, Vacant};
use oxide_auth::primitives::prelude::*;
use oxide_auth::primitives::registrar::RegisteredUrl;
use oxide_auth_rocket;
use oxide_auth_rocket::{OAuthFailure, OAuthRequest, OAuthResponse};
use rocket::State;
use rocket::{get, post};

use super::jwt::JwtIssuer;

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
/// * `registrar` - Stores registered OAuth clients and their configurations
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
    oauth: OAuthRequest<'_>,
    state: &State<OxideState>,
) -> Result<OAuthResponse, OAuthFailure> {
    state
        .endpoint()
        .with_solicitor(FnSolicitor(consent_form))
        .authorization_flow()
        .execute(oauth)
        .map_err(|err| err.pack::<OAuthFailure>())
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
    state: &State<OxideState>,
) -> Result<OAuthResponse, OAuthFailure> {
    let allowed = allow.unwrap_or(false);
    state
        .endpoint()
        .with_solicitor(FnSolicitor(move |_: &mut _, grant: Solicitation<'_>| {
            consent_decision(allowed, grant)
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
    oauth: OAuthRequest<'r>,
    state: &State<OxideState>,
) -> Result<OAuthResponse, OAuthFailure> {
    state
        .endpoint()
        .access_token_flow()
        .execute(oauth)
        .map_err(|err| err.pack::<OAuthFailure>())
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
    /// let state = OxideState::preconfigured("your-secret-key-here");
    /// ```
    pub fn preconfigured(hmac_secret: &str) -> Self {
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
    /// # use rust_photoacoustic::visualization::oxide_auth::OxideState;
    /// # let state = OxideState::preconfigured("secret");
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
fn consent_decision<'r>(allowed: bool, _: Solicitation) -> OwnerConsent<OAuthResponse> {
    if allowed {
        OwnerConsent::Authorized("dummy user".into())
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
/// # Example HTML Output
///
/// ```html
/// <html>'client123' (at http://example.com/callback) is requesting permission for 'read:api write:api'
/// <form method="post">
///     <input type="submit" value="Accept" formaction="/authorize?response_type=code&client_id=client123&redirect_uri=http%3A%2F%2Fexample.com%2Fcallback&allow=true">
///     <input type="submit" value="Deny" formaction="/authorize?response_type=code&client_id=client123&redirect_uri=http%3A%2F%2Fexample.com%2Fcallback&deny=true">
/// </form>
/// </html>
/// ```
fn consent_page_html(route: &str, solicitation: Solicitation) -> String {
    macro_rules! template {
        () => {
            "<html>'{0:}' (at {1:}) is requesting permission for '{2:}'
<form method=\"post\">
    <input type=\"submit\" value=\"Accept\" formaction=\"{4:}?{3:}&allow=true\">
    <input type=\"submit\" value=\"Deny\" formaction=\"{4:}?{3:}&deny=true\">
</form>
</html>"
        };
    }

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

    format!(
        template!(),
        grant.client_id,
        grant.redirect_uri,
        grant.scope,
        serde_urlencoded::to_string(extra).unwrap(),
        &route,
    )
}
