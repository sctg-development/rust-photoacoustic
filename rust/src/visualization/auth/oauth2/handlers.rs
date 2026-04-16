// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! OAuth 2.0 endpoint handlers
//!
//! This module contains the Rocket route handlers for various OAuth 2.0 endpoints
//! including authorization, token exchange, refresh, and user info.

use std::collections::HashMap;
use std::sync::Arc;

use log::debug;
use oxide_auth::endpoint::{Solicitation, WebRequest};
use oxide_auth::frontends::simple::endpoint::FnSolicitor;
use oxide_auth_rocket::{OAuthFailure, OAuthRequest, OAuthResponse};
use rocket::form::Form;
use rocket::http::{Cookie, CookieJar, Status};
use rocket::serde::json::Json;
use rocket::time::Duration;
use rocket::{get, post, State};
use tokio::sync::RwLock;

use super::consent::{consent_decision, consent_form};
use super::forms::{encode_user_session, login_page_html, AuthForm, AuthenticatedUser};
use super::state::OxideState;
use crate::config::Config;
use crate::visualization::auth::oauth2::validate_user;
use crate::visualization::auth::OAuthBearer;
use crate::visualization::user_info_reponse::UserInfoResponse;

/// OAuth 2.0 authorization endpoint
///
/// This Rocket handler implements the OAuth 2.0 authorization endpoint,
/// which is the entry point for the authorization code flow. It presents
/// a consent form to the user, allowing them to authorize or deny the
/// client's request for access.
///
/// ### URL
///
/// `GET /authorize`
///
/// ### Query Parameters
///
/// Standard OAuth 2.0 parameters:
/// - `response_type`: Must be "code"
/// - `client_id`: The client identifier
/// - `redirect_uri`: Where to send the authorization code
/// - `scope`: Requested permission scopes
/// - `state`: Optional state for CSRF protection
///
/// ### Returns
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

    // Try to extract query parameters first to debug potential parsing issues
    let query_result = oauth.query();
    debug!("OAuth query parsing result: {:?}", query_result.is_ok());
    if let Err(ref err) = query_result {
        debug!("OAuth query parsing error: {:?}", err);
        return Err(OAuthFailure::from(
            oxide_auth::endpoint::OAuthError::BadRequest,
        ));
    }

    // If user is already authenticated, proceed to consent
    if authenticated_user.is_some() {
        debug!("User is authenticated, proceeding to consent form");
        let debug_info = match oauth.query() {
            Ok(query) => {
                let client_id = query.unique_value("client_id").map(|v| v.to_string());
                let redirect_uri = query.unique_value("redirect_uri").map(|v| v.to_string());
                let scope = query.unique_value("scope").map(|v| v.to_string());
                let code_challenge = query.unique_value("code_challenge").map(|v| v.to_string());
                let code_challenge_method = query
                    .unique_value("code_challenge_method")
                    .map(|v| v.to_string());
                Some((
                    client_id,
                    redirect_uri,
                    scope,
                    code_challenge,
                    code_challenge_method,
                ))
            }
            Err(_) => None,
        };
        return state
            .endpoint()
            .with_solicitor(FnSolicitor(consent_form))
            .authorization_flow()
            .execute(oauth)
            .map_err(|err| {
                debug!("OAuth authorization flow error occurred");
                match err {
                    oxide_auth::frontends::simple::endpoint::Error::OAuth(oauth_error) => {
                        match oauth_error {
                            oxide_auth::endpoint::OAuthError::BadRequest => {
                                debug!("Bad request error in authorization flow");
                                OAuthFailure::from(oxide_auth::endpoint::OAuthError::BadRequest)
                            }
                            oxide_auth::endpoint::OAuthError::DenySilently => {
                                debug!("Deny silently error in authorization flow - For example, this response is given when an incorrect client has been provided in the authorization request in order to avoid potential indirect denial of service vulnerabilities.");
                                if let Some((client_id, redirect_uri, scope, code_challenge, code_challenge_method)) = &debug_info {
                                    debug!("Requested parameters:");
                                    if let Some(cid) = client_id {
                                        debug!("  client_id: {}", cid);
                                    }
                                    if let Some(ruri) = redirect_uri {
                                        debug!("  redirect_uri: {}", ruri);
                                    }
                                    if let Some(s) = scope {
                                        debug!("  scope: {}", s);
                                    }
                                    if let Some(cc) = code_challenge {
                                        debug!("  code_challenge: {}", cc);
                                    }
                                    if let Some(ccm) = code_challenge_method {
                                        debug!("  code_challenge_method: {}", ccm);
                                    }

                                }
                                OAuthFailure::from(oxide_auth::endpoint::OAuthError::DenySilently)
                            }
                            oxide_auth::endpoint::OAuthError::PrimitiveError => {
                                debug!("Primitive error in authorization flow - server component failed");
                                OAuthFailure::from(oxide_auth::endpoint::OAuthError::PrimitiveError)
                            }
                        }
                    }
                    _ => {
                        debug!("Other authorization flow error");
                        OAuthFailure::from(oxide_auth::endpoint::OAuthError::PrimitiveError)
                    }
                }
            });
    }

    // Otherwise show login form
    let query = query_result.unwrap_or_default();

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

    // Extract PKCE parameters
    let code_challenge = query.unique_value("code_challenge").map(|s| s.to_string());
    let code_challenge_method = query
        .unique_value("code_challenge_method")
        .map(|s| s.to_string());

    let output = login_page_html(
        response_type.to_string(),
        client_id.to_string(),
        redirect_uri.to_string(),
        state_param,
        scope,
        code_challenge,
        code_challenge_method,
        Some("Error: You must be logged in to authorize this client."),
    );

    Ok(OAuthResponse::new()
        .body_html(&output)
        .set_status(Status::Ok)
        .clone())
}

/// Handles user login credentials and sets session if valid
///
/// The access configuration (users and credentials) is read live from the shared
/// `Arc<RwLock<Config>>` so that credential changes take effect immediately without
/// restarting the server.
#[post("/login", data = "<form>")]
pub async fn login(
    form: Form<AuthForm>,
    state: &State<OxideState>,
    config: &State<Arc<RwLock<Config>>>,
    cookies: &CookieJar<'_>,
) -> Result<OAuthResponse, OAuthFailure> {
    debug!("Login form data: {:?}", form);
    // Read live access config from the shared config state
    let access_config = config.read().await.access.clone();

    // Validate user credentials
    if let Some(user) = validate_user(&form.username, &form.password, &access_config) {
        // Set authenticated session cookie
        let mut cookie = Cookie::new("user_session", encode_user_session(user.clone()));
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

        // Preserve PKCE parameters
        if let Some(code_challenge) = &form.code_challenge {
            query_params.insert("code_challenge", code_challenge.clone());
        }

        if let Some(code_challenge_method) = &form.code_challenge_method {
            query_params.insert("code_challenge_method", code_challenge_method.clone());
        }

        let query_string =
            serde_urlencoded::to_string(&query_params).unwrap_or_else(|_| String::new());
        let redirect_url = format!("/authorize?{}", query_string);

        Ok(OAuthResponse::new()
            .set_status(Status::Found)
            .set_location(Some(&redirect_url))
            .clone())
    } else {
        // Invalid credentials, show login form with error
        let output = login_page_html(
            form.response_type.clone(),
            form.client_id.clone(),
            form.redirect_uri.clone(),
            form.state.clone(),
            form.scope.clone(),
            form.code_challenge.clone(),
            form.code_challenge_method.clone(),
            Some("Invalid username or password."),
        );

        Ok(OAuthResponse::new()
            .body_html(&output)
            .set_status(Status::Unauthorized)
            .clone())
    }
}

/// OAuth 2.0 authorization consent handling endpoint
///
/// This Rocket handler processes the user's consent decision from the
/// authorization form. It completes the authorization flow by either
/// granting or denying the client's request based on user input.
///
/// ### URL
///
/// `POST /authorize?allow=[true|false]`
///
/// ### Query Parameters
///
/// - `allow`: Boolean flag indicating user consent (true) or denial (false)
/// - Standard OAuth parameters carried over from the authorize request
///
/// ### Returns
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

/// OIDC end-session (logout) endpoint
///
/// Clears the server-side `user_session` cookie, effectively invalidating the
/// user's authentication session.  After clearing the cookie the user is
/// redirected to the `post_logout_redirect_uri` query parameter (if provided
/// and relative), or to the application root `/`.
///
/// This endpoint is advertised as `end_session_endpoint` in the OIDC discovery
/// document so that `oidc-client-ts` (frontend) can invoke it automatically
/// when `userManager.signoutRedirect()` is called.
///
/// ### URL
///
/// `GET /logout?post_logout_redirect_uri=<uri>`
///
/// ### Query Parameters
///
/// - `post_logout_redirect_uri` *(optional)*: URL to redirect to after logout
///
/// ### Returns
///
/// HTTP 302 redirect to the post-logout URI
#[get("/logout?<post_logout_redirect_uri>")]
pub fn logout(cookies: &CookieJar<'_>, post_logout_redirect_uri: Option<String>) -> OAuthResponse {
    // Remove the private session cookie — this is the key step that prevents
    // the consent handler from re-authenticating a logged-out user.
    cookies.remove_private("user_session");
    debug!("User session cookie removed (logout)");

    // Redirect to the requested post-logout URI, falling back to "/".
    // Only allow relative URIs to prevent open-redirect attacks.
    let redirect_to = post_logout_redirect_uri
        .filter(|uri| {
            uri.starts_with('/')
                || uri.starts_with("https://localhost")
                || uri.starts_with("https://127.0.0.1")
        })
        .unwrap_or_else(|| "/".to_string());

    OAuthResponse::new()
        .set_status(Status::Found)
        .set_location(Some(&redirect_to))
        .clone()
}

/// OAuth 2.0 token endpoint
///
/// This Rocket handler implements the OAuth 2.0 token endpoint, which
/// exchanges an authorization code for an access token and optional
/// refresh token. It validates the authorization code and client
/// credentials before issuing tokens.
///
/// ### URL
///
/// `POST /token`
///
/// ### Request Body
///
/// Form-encoded with standard OAuth 2.0 parameters:
/// - `grant_type`: Must be "authorization_code"
/// - `code`: The authorization code from the authorize endpoint
/// - `redirect_uri`: Must match the original authorization request
/// - `client_id`: The client identifier
///
/// ### Returns
///
/// - On success: A JSON response with access_token, token_type, expires_in, and refresh_token
/// - On error: An OAuth error response
#[post("/token", data = "<oauth>")]
pub async fn token<'r>(
    mut oauth: OAuthRequest<'r>,
    state: &State<OxideState>,
    authenticated_user: Option<AuthenticatedUser>,
) -> Result<OAuthResponse, OAuthFailure> {
    // Extract all values from body as owned Strings before any `.await`.
    // `Cow<dyn QueryParameter>` is `!Sync` and cannot be held across await points.
    let (grant_type, refresh_token_for_claims) = {
        let body = oauth.urlbody()?;
        let gt = body.unique_value("grant_type").map(|v| v.into_owned());
        let rt = body.unique_value("refresh_token").map(|v| v.into_owned());
        (gt, rt)
    };
    debug!("grant_type: {:?}", grant_type);

    // If user is authenticated via Bearer token, inject their claims for the access_token flow.
    if let Some(authenticated_user) = authenticated_user {
        let username = authenticated_user.0.username;
        if let Ok(mut issuer) = state.issuer.lock() {
            issuer.add_user_claims(&username, &authenticated_user.0.permissions);
        }
    }

    if grant_type.as_deref() == Some("refresh_token") {
        // Before executing the refresh flow, inject the user's *current* permissions
        // from the live AccessConfig so that any changes to config.yaml (e.g. removing
        // write:api) are reflected immediately in the newly issued token.
        if let Some(refresh_token_value) = refresh_token_for_claims {
            let maybe_owner_id = state
                .issuer
                .lock()
                .ok()
                .and_then(|issuer| issuer.get_refresh_token_owner(&refresh_token_value));

            if let Some(owner_id) = maybe_owner_id {
                let current_permissions = {
                    let access = state.access_config.read().await;
                    access
                        .users
                        .iter()
                        .find(|u| u.user == owner_id)
                        .map(|u| u.permissions.clone())
                        .unwrap_or_default()
                };
                if let Ok(mut issuer) = state.issuer.lock() {
                    issuer.add_user_claims(&owner_id, &current_permissions);
                }
                debug!(
                    "Refresh flow: updated permissions for '{}': {:?}",
                    owner_id, current_permissions
                );
            }
        }

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
/// ### URL
///
/// `POST /refresh`
///
/// ### Request Body
///
/// Form-encoded with standard OAuth 2.0 parameters:
/// - `grant_type`: Must be "refresh_token"
/// - `refresh_token`: The refresh token from a previous token response
/// - `client_id`: The client identifier
///
/// ### Returns
///
/// - On success: A JSON response with a new access_token, token_type, expires_in, and refresh_token
/// - On error: An OAuth error response
#[post("/refresh", data = "<oauth>")]
pub async fn refresh<'r>(
    mut oauth: OAuthRequest<'r>,
    state: &State<OxideState>,
) -> Result<OAuthResponse, OAuthFailure> {
    // Extract refresh token as owned String before any `.await` (Cow<dyn QueryParameter> is !Sync).
    let refresh_token_for_claims = oauth
        .urlbody()
        .ok()
        .and_then(|body| body.unique_value("refresh_token").map(|v| v.into_owned()));

    // Inject current permissions from live AccessConfig before reissuing the token,
    // so that config.yaml permission changes take effect immediately on refresh.
    if let Some(refresh_token_value) = refresh_token_for_claims {
        let maybe_owner_id = state
            .issuer
            .lock()
            .ok()
            .and_then(|issuer| issuer.get_refresh_token_owner(&refresh_token_value));

        if let Some(owner_id) = maybe_owner_id {
            let current_permissions = {
                let access = state.access_config.read().await;
                access
                    .users
                    .iter()
                    .find(|u| u.user == owner_id)
                    .map(|u| u.permissions.clone())
                    .unwrap_or_default()
            };
            if let Ok(mut issuer) = state.issuer.lock() {
                issuer.add_user_claims(&owner_id, &current_permissions);
            }
            debug!(
                "Refresh endpoint: updated permissions for '{}': {:?}",
                owner_id, current_permissions
            );
        }
    }

    state
        .endpoint()
        .refresh_flow()
        .execute(oauth)
        .map_err(|err| err.pack::<OAuthFailure>())
}

/// Openid userinfo endpoint
/// Accessed via `GET /userinfo`
/// This endpoint returns user information based on the access token provided in the Authorization header.
/// It requires a valid JWT access token to be present in the request Authorization header.
#[get("/userinfo")]
pub async fn userinfo(
    auth_bearer: OAuthBearer,
    state: &State<OxideState>,
) -> Result<Json<UserInfoResponse>, OAuthFailure> {
    // Return the authenticated user's information
    debug!("Userinfo endpoint accessed with bearer token");
    let user = auth_bearer.user_info.clone(); // This is the client ID of the authenticated user

    Ok(Json(
        UserInfoResponse {
            sub: user.user_id,
            name: user.name,
            email: user.email,
            permissions: None,
            given_name: None,
            family_name: None,
            middle_name: None,
            nickname: None,
            preferred_username: None,
            profile: None,
            picture: None,
            website: None,
            gender: None,
            birthdate: None,
            zoneinfo: None,
            locale: None,
            updated_at: None,
            email_verified: None,
            phone_number: None,
            phone_number_verified: None,
            address: None,
        }, // Permissions from the OAuthBearer
    ))
}
