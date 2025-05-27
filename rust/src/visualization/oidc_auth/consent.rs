// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! OAuth consent form handling
//!
//! This module provides functionality for generating and processing
//! OAuth consent forms during the authorization flow.

use handlebars::Handlebars;
use log::debug;
use oxide_auth::endpoint::{OwnerConsent, Solicitation};
use oxide_auth_rocket::OAuthResponse;
use serde_json::json;

use super::forms::format_scopes;

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
pub fn consent_form(
    _: &mut oxide_auth_rocket::OAuthRequest<'_>,
    solicitation: Solicitation,
) -> OwnerConsent<OAuthResponse> {
    let output = consent_page_html("/authorize", solicitation);
    debug!("Consent form HTML {}", output);
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
pub fn consent_decision<'r>(
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
pub fn consent_page_html(route: &str, solicitation: Solicitation) -> String {
    let mut handlebars = Handlebars::new();

    // Register the consent template
    handlebars
        .register_template_string(
            "consent",
            include_str!("../../../resources/forms/consent.hbs"),
        )
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
