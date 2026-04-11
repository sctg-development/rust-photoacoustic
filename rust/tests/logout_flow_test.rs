// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Integration tests for the OIDC logout / end-session flow
//!
//! These tests cover the bug where, after logout, clicking "Accept" on the
//! OAuth2 consent screen re-authenticated the user without a password.
//!
//! ## Root cause (fixed)
//!
//! The server had no `/logout` endpoint, so the `user_session` private cookie
//! was never removed.  The OIDC discovery document did not advertise
//! `end_session_endpoint`, so `oidc-client-ts` could not ask the server to
//! clear the cookie.  On page refresh the cookie was still present, and the
//! consent handler accepted it as valid authentication.
//!
//! ## Fix
//!
//! * `GET /logout` now removes `user_session` via `CookieJar::remove_private`.
//! * `/.well-known/openid-configuration` now includes `end_session_endpoint`.
//!
//! ## Covered scenarios
//!
//! | Test | What it verifies |
//! |---|---|
//! | [`test_oidc_discovery_exposes_end_session_endpoint`] | Discovery doc carries the new field |
//! | [`test_logout_without_session_redirects_to_root`] | Idempotent: no cookie → 302 to `/` |
//! | [`test_logout_redirects_to_valid_relative_uri`] | Relative `post_logout_redirect_uri` is honoured |
//! | [`test_logout_blocks_open_redirect`] | External host in redirect param is ignored |
//! | [`test_login_shows_consent_form_when_session_active`] | After login the consent form is served |
//! | [`test_authorize_shows_login_form_after_logout`] | After logout `/authorize` shows the login form |
//! | [`test_consent_post_rejected_after_logout`] | **REGRESSION**: `POST /authorize?allow=true` fails without session |
//! | [`test_login_again_after_logout_succeeds`] | A second login after logout works normally |

use rocket::config::LogLevel;
use rocket::http::{ContentType, Status};
use rust_photoacoustic::config::{AccessConfig, Config, VisualizationConfig};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

// ─── Constants ────────────────────────────────────────────────────────────────

const TEST_HMAC_SECRET: &str = "test-hmac-secret-key-for-testing";

/// Redirect URI registered in `Client::default()` (used in all OAuth requests)
const REDIRECT_URI_ENCODED: &str = "https%3A%2F%2Flocalhost%3A8080%2Fclient%2F";

/// Standard OAuth query-string appended to `/authorize` in every test
const AUTHORIZE_QS: &str = "response_type=code\
    &client_id=LaserSmartClient\
    &redirect_uri=https%3A%2F%2Flocalhost%3A8080%2Fclient%2F\
    &scope=read%3Aapi";

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn test_figment() -> rocket::figment::Figment {
    rocket::Config::figment()
        .merge(("port", 0))
        .merge(("address", "127.0.0.1"))
        .merge(("log_level", LogLevel::Off))
        .merge(("hmac_secret", TEST_HMAC_SECRET.to_string()))
        .merge(("secret_key", "/qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis="))
        .merge(("access_config", AccessConfig::default()))
        .merge(("visualization_config", VisualizationConfig::default()))
}

async fn build_test_rocket(config: Arc<RwLock<Config>>) -> rocket::local::asynchronous::Client {
    let rocket = rust_photoacoustic::visualization::server::build_rocket(
        test_figment(),
        config,
        None,
        None,
        None,
        None,
        None,
    )
    .await;
    rocket::local::asynchronous::Client::tracked(rocket)
        .await
        .expect("valid rocket instance")
}

/// URL-encoded form body for `POST /login` with the default admin credentials.
///
/// All OAuth PKCE parameters are omitted; only what the `AuthForm` requires is
/// included so the handler can build its redirect back to `/authorize`.
fn admin_login_body() -> String {
    format!(
        "username=admin\
        &password=admin123\
        &response_type=code\
        &client_id=LaserSmartClient\
        &redirect_uri={}\
        &scope=read%3Aapi",
        REDIRECT_URI_ENCODED
    )
}

// ─── Discovery tests ──────────────────────────────────────────────────────────

/// The OIDC discovery document must advertise `end_session_endpoint` so that
/// `oidc-client-ts` can call it during `userManager.signoutRedirect()`.
///
/// Without this field the frontend library clears only its local session storage
/// but never asks the server to remove the `user_session` cookie.
#[rocket::async_test]
async fn test_oidc_discovery_exposes_end_session_endpoint() {
    let config = Arc::new(RwLock::new(Config::default()));
    let client = build_test_rocket(config).await;

    let response = client
        .get("/.well-known/openid-configuration")
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    let body: Value =
        serde_json::from_str(&response.into_string().await.expect("body")).expect("valid JSON");

    assert!(
        body.get("end_session_endpoint").is_some(),
        "end_session_endpoint must be present in the discovery document"
    );

    let endpoint = body["end_session_endpoint"].as_str().unwrap_or("");
    assert!(
        endpoint.ends_with("/logout"),
        "end_session_endpoint must point to /logout, got: {endpoint}"
    );
}

// ─── Logout endpoint behaviour ────────────────────────────────────────────────

/// Calling `GET /logout` without any session cookie is safe and returns a
/// redirect to `/` (idempotent — double-logout must not error).
#[rocket::async_test]
async fn test_logout_without_session_redirects_to_root() {
    let config = Arc::new(RwLock::new(Config::default()));
    let client = build_test_rocket(config).await;

    let response = client.get("/logout").dispatch().await;

    assert_eq!(
        response.status(),
        Status::Found,
        "logout without a session must return 302"
    );

    let location = response
        .headers()
        .get_one("Location")
        .unwrap_or("")
        .to_string();
    assert_eq!(
        location, "/",
        "logout without post_logout_redirect_uri must redirect to /"
    );
}

/// A relative `post_logout_redirect_uri` is honoured — the client is sent there
/// after the session cookie is cleared.
#[rocket::async_test]
async fn test_logout_redirects_to_valid_relative_uri() {
    let config = Arc::new(RwLock::new(Config::default()));
    let client = build_test_rocket(config).await;

    let response = client
        .get("/logout?post_logout_redirect_uri=%2Fclient%2F")
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Found);

    let location = response
        .headers()
        .get_one("Location")
        .unwrap_or("")
        .to_string();
    assert_eq!(
        location, "/client/",
        "a relative post_logout_redirect_uri must be used as the redirect target"
    );
}

/// An absolute URI pointing to an external host must be **ignored** to prevent
/// open-redirect attacks.  The server must fall back to `/`.
#[rocket::async_test]
async fn test_logout_blocks_open_redirect() {
    let config = Arc::new(RwLock::new(Config::default()));
    let client = build_test_rocket(config).await;

    // An attacker-controlled URI
    let response = client
        .get("/logout?post_logout_redirect_uri=https%3A%2F%2Fevil.example.com%2Fsteal")
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Found);

    let location = response
        .headers()
        .get_one("Location")
        .unwrap_or("")
        .to_string();
    assert_ne!(
        location, "https://evil.example.com/steal",
        "external redirect must be blocked"
    );
    assert_eq!(location, "/", "blocked open-redirect must fall back to /");
}

// ─── Session / consent regression tests ─────────────────────────────────────

/// After a successful `POST /login`, `GET /authorize` must display the **consent
/// form** (not the login form), confirming that the `user_session` cookie was set
/// and is being read by the `AuthenticatedUser` guard.
#[rocket::async_test]
async fn test_login_shows_consent_form_when_session_active() {
    let config = Arc::new(RwLock::new(Config::default()));
    let client = build_test_rocket(config).await;

    // Step 1 — login: sets user_session cookie, responds with redirect
    let login_resp = client
        .post("/login")
        .header(ContentType::Form)
        .body(admin_login_body())
        .dispatch()
        .await;
    assert_eq!(
        login_resp.status(),
        Status::Found,
        "POST /login with valid credentials must redirect (302)"
    );

    // Step 2 — authorize: cookie is tracked; must get consent form
    let auth_resp = client
        .get(format!("/authorize?{}", AUTHORIZE_QS))
        .dispatch()
        .await;
    assert_eq!(
        auth_resp.status(),
        Status::Ok,
        "/authorize with active session must return 200"
    );

    let body = auth_resp.into_string().await.expect("body");
    assert!(
        body.contains("Authorization Request"),
        "response must contain the consent form, not the login form; \
         got: {:.200}",
        body
    );
    assert!(
        !body.contains("Photoacoustic Login"),
        "login form must NOT be shown when session is active"
    );
}

/// **REGRESSION TEST** — After logout, `GET /authorize` must show the **login
/// form** instead of the consent screen.
///
/// Before the fix the `user_session` cookie persisted across logout, so the
/// consent handler treated the logged-out user as authenticated and displayed
/// the consent screen.  A single "Accept" click then re-issued an auth code
/// without ever prompting for a password.
#[rocket::async_test]
async fn test_authorize_shows_login_form_after_logout() {
    let config = Arc::new(RwLock::new(Config::default()));
    let client = build_test_rocket(config).await;

    // Step 1 — login
    let login_resp = client
        .post("/login")
        .header(ContentType::Form)
        .body(admin_login_body())
        .dispatch()
        .await;
    assert_eq!(login_resp.status(), Status::Found, "login must redirect");

    // Step 2 — verify session is active (consent form is shown)
    let before_logout = client
        .get(format!("/authorize?{}", AUTHORIZE_QS))
        .dispatch()
        .await;
    assert_eq!(before_logout.status(), Status::Ok);
    let body_before = before_logout.into_string().await.expect("body");
    assert!(
        body_before.contains("Authorization Request"),
        "consent form must be shown before logout"
    );

    // Step 3 — logout: clears the user_session cookie
    let logout_resp = client.get("/logout").dispatch().await;
    assert_eq!(
        logout_resp.status(),
        Status::Found,
        "GET /logout must return 302"
    );

    // Step 4 — authorize again: cookie gone → must show login form, NOT consent
    let after_logout = client
        .get(format!("/authorize?{}", AUTHORIZE_QS))
        .dispatch()
        .await;
    assert_eq!(
        after_logout.status(),
        Status::Ok,
        "/authorize after logout must return 200 (login page)"
    );

    let body_after = after_logout.into_string().await.expect("body");
    assert!(
        body_after.contains("Photoacoustic Login"),
        "login form must be shown after logout; got: {:.300}",
        body_after
    );
    assert!(
        !body_after.contains("Authorization Request"),
        "consent form must NOT be shown after logout"
    );
}

/// **REGRESSION TEST** — After logout, `POST /authorize?allow=true` must be
/// **rejected** (not issue an authorization code).
///
/// This is the exact user-facing bug: clicking "Accept" on the stale consent
/// screen (after a logout + page refresh) must no longer succeed.
#[rocket::async_test]
async fn test_consent_post_rejected_after_logout() {
    let config = Arc::new(RwLock::new(Config::default()));
    let client = build_test_rocket(config).await;

    // Step 1 — login + authorize flow to prime oxide-auth's pending grant
    let _login = client
        .post("/login")
        .header(ContentType::Form)
        .body(admin_login_body())
        .dispatch()
        .await;

    // Trigger the authorization flow so oxide-auth stores a pending grant
    let _authorize = client
        .get(format!("/authorize?{}", AUTHORIZE_QS))
        .dispatch()
        .await;

    // Step 2 — logout
    let logout = client.get("/logout").dispatch().await;
    assert_eq!(logout.status(), Status::Found, "logout must redirect");

    // Step 3 — POST /authorize?allow=true WITHOUT a session cookie.
    //
    // Before the fix: the stale cookie was still present → this returned 302
    //   with an auth code, logging the user in without a password.
    // After the fix: no cookie → AuthenticatedUser guard is None → the handler
    //   returns an error (not a redirect carrying an auth code).
    let consent_post = client
        .post(format!("/authorize?allow=true&{}", AUTHORIZE_QS))
        .dispatch()
        .await;

    // Must NOT be a 302 redirect (which would indicate a successful auth code grant)
    assert_ne!(
        consent_post.status(),
        Status::Found,
        "POST /authorize?allow=true must NOT issue an auth code after logout \
         (regression: cookie must have been cleared)"
    );
}

/// A complete login → logout → login cycle must work: the second login must
/// succeed and again display the consent form, proving that logout does not
/// permanently break the session machinery.
#[rocket::async_test]
async fn test_login_again_after_logout_succeeds() {
    let config = Arc::new(RwLock::new(Config::default()));
    let client = build_test_rocket(config).await;

    // First login
    let first_login = client
        .post("/login")
        .header(ContentType::Form)
        .body(admin_login_body())
        .dispatch()
        .await;
    assert_eq!(
        first_login.status(),
        Status::Found,
        "first login must redirect"
    );

    // Logout
    let logout = client.get("/logout").dispatch().await;
    assert_eq!(logout.status(), Status::Found, "logout must redirect");

    // Second login — must work exactly like the first
    let second_login = client
        .post("/login")
        .header(ContentType::Form)
        .body(admin_login_body())
        .dispatch()
        .await;
    assert_eq!(
        second_login.status(),
        Status::Found,
        "second login after logout must also redirect (302)"
    );

    // Authorize after re-login — must show consent form again
    let after_relogin = client
        .get(format!("/authorize?{}", AUTHORIZE_QS))
        .dispatch()
        .await;
    assert_eq!(after_relogin.status(), Status::Ok);

    let body = after_relogin.into_string().await.expect("body");
    assert!(
        body.contains("Authorization Request"),
        "consent form must be shown after re-login"
    );
}
