// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

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

// Define the structure with Arc for shared resources
pub struct OxideState {
    registrar: Arc<Mutex<ClientMap>>,
    authorizer: Arc<Mutex<AuthMap<RandomGenerator>>>,
    pub issuer: Arc<Mutex<JwtIssuer>>, // Wrap JwtIssuer in Arc<Mutex<>> for shared mutability
}

// Implement Clone for OxideState
impl Clone for OxideState {
    fn clone(&self) -> Self {
        OxideState {
            registrar: Arc::clone(&self.registrar),
            authorizer: Arc::clone(&self.authorizer),
            issuer: Arc::clone(&self.issuer),
        }
    }
}

#[get("/authorize")]
pub fn authorize<'r>(
    oauth: OAuthRequest<'r>,
    state: &State<OxideState>,
) -> Result<OAuthResponse, OAuthFailure> {
    state
        .endpoint()
        .with_solicitor(FnSolicitor(consent_form))
        .authorization_flow()
        .execute(oauth)
        .map_err(|err| err.pack::<OAuthFailure>())
}

#[post("/authorize?<allow>")]
pub fn authorize_consent<'r>(
    oauth: OAuthRequest<'r>,
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
    pub fn preconfigured() -> Self {
        // Secret key for JWT token signing - in a real app this should be loaded from a secure source
        // For development, we use a simple hard-coded key
        let jwt_secret = b"my-super-secret-jwt-key-for-photoacoustic-app";

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
        }
    }

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

fn consent_form<'r>(
    _: &mut OAuthRequest<'r>,
    solicitation: Solicitation,
) -> OwnerConsent<OAuthResponse> {
    let output = consent_page_html("/authorize", solicitation);
    OwnerConsent::InProgress(OAuthResponse::new().body_html(&output).to_owned())
}

fn consent_decision<'r>(allowed: bool, _: Solicitation) -> OwnerConsent<OAuthResponse> {
    if allowed {
        OwnerConsent::Authorized("dummy user".into())
    } else {
        OwnerConsent::Denied
    }
}

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

    let mut extra = vec![
        ("response_type", "code"),
        ("client_id", grant.client_id.as_str()),
        ("redirect_uri", grant.redirect_uri.as_str()),
    ];

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
