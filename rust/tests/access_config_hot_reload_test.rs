// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Integration tests for Phase 1 hot-reload of `AccessConfig`
//!
//! These tests verify that modifying `Config.access` in the shared `Arc<RwLock<Config>>`
//! is immediately visible to the `AuthenticatedUser` and `AccessConfig` request guards
//! without restarting the server.
//!
//! ## Covered scenarios
//!
//! ### Unit tests (pure Rust, no HTTP)
//! - [`test_get_user_info_succeeds_for_known_user`]                  — baseline
//! - [`test_get_user_info_fails_for_unknown_user`]                   — user not in config
//! - [`test_get_user_info_reflects_permission_changes`]              — new permissions visible immediately
//!
//! ### Integration tests (Rocket test client, HTTP round-trips)
//! - [`test_authenticated_user_guard_baseline`]                      — valid token → 200
//! - [`test_authenticated_user_guard_rejects_removed_user`]         — user removed → 401
//! - [`test_authenticated_user_guard_accepts_newly_added_user`]     — user added → 200
//! - [`test_access_config_guard_reflects_added_user`]               — AccessConfig guard is live
//! - [`test_access_config_guard_reflects_removed_user`]             — AccessConfig guard is live
//! - [`test_concurrent_config_mutation_is_safe`]                    — concurrent reads during write

use oxide_auth::primitives::grant::{Extensions, Grant};
use oxide_auth::primitives::issuer::Issuer;
use rocket::config::LogLevel;
use rocket::http::{Header, Status};
use rust_photoacoustic::config::{AccessConfig, Config, User, VisualizationConfig};
use rust_photoacoustic::visualization::api_auth::init_jwt_validator;
use rust_photoacoustic::visualization::auth::jwt::{JwtIssuer, JwtValidator};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

// ─── Constants ───────────────────────────────────────────────────────────────

const TEST_HMAC_SECRET: &str = "test-hmac-secret-key-for-testing";
/// Password hash for "admin123" — same as AccessConfig::default()
const ADMIN123_HASH: &str =
    "JDUkM2E2OUZwQW0xejZBbWV2QSRvMlhhN0lxcVdVU1VPTUh6UVJiM3JjRlRhZy9WYjdpSWJtZUJFaXA3Y1ZECg==";

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Build a minimal Rocket figment for tests
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

/// Build a `Config` with default access settings and the test HMAC secret.
fn test_config_with_access(access: AccessConfig) -> Config {
    let mut cfg = Config::default();
    cfg.visualization.hmac_secret = TEST_HMAC_SECRET.to_string();
    cfg.visualization.port = 0;
    cfg.visualization.address = "127.0.0.1".to_string();
    cfg.access = access;
    cfg
}

/// Issue a signed HS256 JWT for `username` using the test HMAC secret.
///
/// The token is signed with `TEST_HMAC_SECRET`, has issuer "LaserSmartServer"
/// and audience "LaserSmartClient" to match `init_jwt_validator` configuration.
fn issue_test_token(username: &str) -> String {
    let mut issuer = JwtIssuer::new(TEST_HMAC_SECRET.as_bytes());
    // add_user_claims so the token carries a realistic scope
    issuer.add_user_claims(username, &["read:api".to_string(), "admin:api".to_string()]);

    let grant = Grant {
        owner_id: username.to_string(),
        client_id: "LaserSmartClient".to_string(),
        scope: "read:api admin:api".parse().unwrap(),
        redirect_uri: "https://localhost/callback".parse().unwrap(),
        until: chrono::Utc::now() + chrono::Duration::hours(1),
        extensions: Extensions::new(),
    };

    let issued = issuer.issue(grant).expect("token issuance must not fail");
    issued.token
}

/// Create a `JwtValidator` configured for the test HMAC secret.
fn test_jwt_validator(access: AccessConfig) -> JwtValidator {
    init_jwt_validator(TEST_HMAC_SECRET, None, access).expect("validator creation must not fail")
}

/// Build a full Rocket instance around a shared `Arc<RwLock<Config>>`.
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

/// Build a `User` with a dummy password hash and given permissions.
/// The hash doesn't matter for JWT-path tests (login form is bypassed).
fn make_user(username: &str, permissions: &[&str]) -> User {
    User {
        user: username.to_string(),
        pass: ADMIN123_HASH.to_string(),
        permissions: permissions.iter().map(|s| s.to_string()).collect(),
        email: Some(format!("{}@example.com", username)),
        name: Some(username.to_string()),
    }
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

/// Baseline: `get_user_info` succeeds when the user exists in `AccessConfig`.
#[test]
fn test_get_user_info_succeeds_for_known_user() {
    let token = issue_test_token("admin");
    let access = AccessConfig::default(); // contains "admin"

    let validator = test_jwt_validator(access.clone());
    let result = validator.get_user_info(&token, access);

    assert!(
        result.is_ok(),
        "user 'admin' must be found: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().user_id, "admin");
}

/// `get_user_info` returns an error when the user is absent from `AccessConfig`.
///
/// This is the core invariant for hot-reload: removing a user from the live
/// config causes their subsequent requests to be rejected even if their token
/// signature is still valid.
#[test]
fn test_get_user_info_fails_for_unknown_user() {
    let token = issue_test_token("ghost");
    // AccessConfig::default() only contains "admin" — "ghost" is absent
    let access = AccessConfig::default();

    let validator = test_jwt_validator(access.clone());
    let result = validator.get_user_info(&token, access);

    assert!(
        result.is_err(),
        "user 'ghost' must NOT be found in default config"
    );
    assert!(
        result.unwrap_err().to_string().contains("not found"),
        "error message must mention 'not found'"
    );
}

/// Permission changes in `AccessConfig` are immediately visible via `get_user_info`.
///
/// The method joins JWT claims against the LIVE config to extract permissions,
/// so changing a user's permission list is effective on the next call.
#[test]
fn test_get_user_info_reflects_permission_changes() {
    let token = issue_test_token("admin");

    let validator = test_jwt_validator(AccessConfig::default());

    // --- Original permissions ---
    let access_original = AccessConfig::default();
    let info_before = validator
        .get_user_info(&token, access_original)
        .expect("must succeed");
    let perms_before = info_before.permissions.unwrap_or_default();
    assert!(
        perms_before.contains(&"admin:api".to_string()),
        "admin must have admin:api initially: {:?}",
        perms_before
    );

    // --- Reduced permissions (simulate config change) ---
    let mut modified_access = AccessConfig::default();
    modified_access.users[0].permissions = vec!["read:api".to_string()];

    let info_after = validator
        .get_user_info(&token, modified_access)
        .expect("must still succeed — user still exists");
    let perms_after = info_after.permissions.unwrap_or_default();

    assert!(
        !perms_after.contains(&"admin:api".to_string()),
        "admin:api must be gone after config change: {:?}",
        perms_after
    );
    assert!(
        perms_after.contains(&"read:api".to_string()),
        "read:api must remain: {:?}",
        perms_after
    );
}

/// A token for a user absent from the initial config, once that user is added,
/// the SAME token is accepted (unit-level check without HTTP).
#[test]
fn test_get_user_info_accepts_token_after_user_added() {
    let token = issue_test_token("newuser");
    let validator = test_jwt_validator(AccessConfig::default());

    // Before: "newuser" does not exist
    let result_before = validator.get_user_info(&token, AccessConfig::default());
    assert!(result_before.is_err(), "newuser must be rejected initially");

    // After: add "newuser" to the config
    let mut updated_access = AccessConfig::default();
    updated_access
        .users
        .push(make_user("newuser", &["read:api"]));

    let result_after = validator.get_user_info(&token, updated_access);
    assert!(
        result_after.is_ok(),
        "newuser must be accepted after being added to config: {:?}",
        result_after.err()
    );
}

// ─── Integration tests — AuthenticatedUser guard ─────────────────────────────

/// Baseline integration test: a valid token for the default "admin" user returns 200.
#[rocket::async_test]
async fn test_authenticated_user_guard_baseline() {
    let config = Arc::new(RwLock::new(
        test_config_with_access(AccessConfig::default()),
    ));
    let client = build_test_rocket(config).await;

    let token = issue_test_token("admin");
    let response = client
        .get("/api/profile")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;

    assert_eq!(
        response.status(),
        Status::Ok,
        "valid admin token must be accepted"
    );

    let body: Value =
        serde_json::from_str(&response.into_string().await.expect("body")).expect("valid JSON");
    assert_eq!(body["user_id"], "admin");
}

/// Hot-reload scenario: removing a user from the shared config causes subsequent
/// requests with that user's (still-valid) token to be rejected with 401.
#[rocket::async_test]
async fn test_authenticated_user_guard_rejects_removed_user() {
    let config = Arc::new(RwLock::new(
        test_config_with_access(AccessConfig::default()),
    ));
    let client = build_test_rocket(Arc::clone(&config)).await;

    let token = issue_test_token("admin");

    // --- Before removal: token is accepted ---
    let response_before = client
        .get("/api/profile")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(
        response_before.status(),
        Status::Ok,
        "admin token must be accepted before removal"
    );

    // --- Mutate the shared config: remove all users ---
    {
        let mut cfg = config.write().await;
        cfg.access.users.clear();
    }

    // --- After removal: same token must be rejected ---
    let response_after = client
        .get("/api/profile")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(
        response_after.status(),
        Status::Unauthorized,
        "admin token must be rejected after user is removed from config (same token, same server)"
    );
}

/// Hot-reload scenario: adding a new user to the shared config makes their token
/// accepted without server restart.
///
/// The user "alice" does not exist in the initial config. A JWT is issued for her
/// (valid signature, but user lookup fails). After adding Alice to the config,
/// the identical token succeeds.
#[rocket::async_test]
async fn test_authenticated_user_guard_accepts_newly_added_user() {
    let config = Arc::new(RwLock::new(
        test_config_with_access(AccessConfig::default()),
    ));
    let client = build_test_rocket(Arc::clone(&config)).await;

    let token = issue_test_token("alice");

    // --- Before addition: "alice" is unknown ---
    let response_before = client
        .get("/api/profile")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(
        response_before.status(),
        Status::Unauthorized,
        "alice must be rejected before being added to config"
    );

    // --- Add alice to the shared config ---
    {
        let mut cfg = config.write().await;
        cfg.access
            .users
            .push(make_user("alice", &["read:api", "write:api"]));
    }

    // --- After addition: same token is accepted ---
    let response_after = client
        .get("/api/profile")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch()
        .await;
    assert_eq!(
        response_after.status(),
        Status::Ok,
        "alice must be accepted after being added to config"
    );

    let body: Value = serde_json::from_str(&response_after.into_string().await.expect("body"))
        .expect("valid JSON");
    assert_eq!(body["user_id"], "alice");
}

/// Replacing the entire user list (e.g. admin replaced by a new user) is reflected
/// immediately: old token rejected, new-user token accepted.
#[rocket::async_test]
async fn test_authenticated_user_guard_swap_user_list() {
    let initial_access = AccessConfig::default(); // only "admin"
    let config = Arc::new(RwLock::new(test_config_with_access(initial_access)));
    let client = build_test_rocket(Arc::clone(&config)).await;

    let admin_token = issue_test_token("admin");
    let bob_token = issue_test_token("bob");

    // Initially: admin OK, bob rejected
    assert_eq!(
        client
            .get("/api/profile")
            .header(Header::new(
                "Authorization",
                format!("Bearer {}", admin_token)
            ))
            .dispatch()
            .await
            .status(),
        Status::Ok
    );
    assert_eq!(
        client
            .get("/api/profile")
            .header(Header::new(
                "Authorization",
                format!("Bearer {}", bob_token)
            ))
            .dispatch()
            .await
            .status(),
        Status::Unauthorized
    );

    // Swap: replace user list with only "bob"
    {
        let mut cfg = config.write().await;
        cfg.access.users = vec![make_user("bob", &["read:api"])];
    }

    // After swap: bob OK, admin rejected
    assert_eq!(
        client
            .get("/api/profile")
            .header(Header::new(
                "Authorization",
                format!("Bearer {}", bob_token)
            ))
            .dispatch()
            .await
            .status(),
        Status::Ok,
        "bob must be accepted after swap"
    );
    assert_eq!(
        client
            .get("/api/profile")
            .header(Header::new(
                "Authorization",
                format!("Bearer {}", admin_token)
            ))
            .dispatch()
            .await
            .status(),
        Status::Unauthorized,
        "admin must be rejected after swap"
    );
}

/// A request without any Authorization header is rejected with 401.
/// Ensures the guard doesn't bypass auth on empty input.
#[rocket::async_test]
async fn test_authenticated_user_guard_rejects_missing_token() {
    let config = Arc::new(RwLock::new(
        test_config_with_access(AccessConfig::default()),
    ));
    let client = build_test_rocket(config).await;

    let response = client.get("/api/profile").dispatch().await;
    assert_eq!(
        response.status(),
        Status::Unauthorized,
        "missing token must yield 401"
    );
}

/// An Authorization header with a malformed (non-JWT) value is rejected.
#[rocket::async_test]
async fn test_authenticated_user_guard_rejects_malformed_token() {
    let config = Arc::new(RwLock::new(
        test_config_with_access(AccessConfig::default()),
    ));
    let client = build_test_rocket(config).await;

    let response = client
        .get("/api/profile")
        .header(Header::new("Authorization", "Bearer not.a.valid.jwt"))
        .dispatch()
        .await;
    assert_eq!(
        response.status(),
        Status::Unauthorized,
        "malformed token must yield 401"
    );
}

// ─── Integration tests — AccessConfig request guard ──────────────────────────

/// Verify that the `AccessConfig` request guard (`impl FromRequest for AccessConfig`)
/// reflects a user added to the shared config.
///
/// Strategy: use the OIDC `openid-configuration` endpoint, which reads
/// `state.access_config.iss` — still frozen in OxideState for Phase 1. For
/// the `AccessConfig` guard itself we confirm via the `AuthenticatedUser` path
/// that the underlying live config is consulted: an `AccessConfig` extracted
/// inside `from_request` for a SECOND request (after config mutation) must
/// include the new user.
///
/// The test drives this through `/api/profile` which internally delegates to
/// the same Arc read-path as the `AccessConfig` guard.
#[rocket::async_test]
async fn test_access_config_guard_reflects_added_user() {
    // Start with no users at all
    let mut empty_access = AccessConfig::default();
    empty_access.users.clear();

    let config = Arc::new(RwLock::new(test_config_with_access(empty_access)));
    let client = build_test_rocket(Arc::clone(&config)).await;

    let token = issue_test_token("carol");

    // No users → 401
    assert_eq!(
        client
            .get("/api/profile")
            .header(Header::new("Authorization", format!("Bearer {}", token)))
            .dispatch()
            .await
            .status(),
        Status::Unauthorized,
        "carol must be rejected with empty user list"
    );

    // Add carol via the live config
    {
        let mut cfg = config.write().await;
        cfg.access.users.push(make_user("carol", &["read:api"]));
    }

    // Now carol's token is accepted
    assert_eq!(
        client
            .get("/api/profile")
            .header(Header::new("Authorization", format!("Bearer {}", token)))
            .dispatch()
            .await
            .status(),
        Status::Ok,
        "carol must be accepted after config update"
    );
}

/// Verify that removing a user via the live config causes the `AccessConfig`
/// guard to deny subsequent requests.
#[rocket::async_test]
async fn test_access_config_guard_reflects_removed_user() {
    let mut access = AccessConfig::default();
    access.users.push(make_user("dave", &["read:api"]));

    let config = Arc::new(RwLock::new(test_config_with_access(access)));
    let client = build_test_rocket(Arc::clone(&config)).await;

    let token = issue_test_token("dave");

    // Dave exists → 200
    assert_eq!(
        client
            .get("/api/profile")
            .header(Header::new("Authorization", format!("Bearer {}", token)))
            .dispatch()
            .await
            .status(),
        Status::Ok,
        "dave must be accepted initially"
    );

    // Remove dave from live config
    {
        let mut cfg = config.write().await;
        cfg.access.users.retain(|u| u.user != "dave");
    }

    // Same token → 401
    assert_eq!(
        client
            .get("/api/profile")
            .header(Header::new("Authorization", format!("Bearer {}", token)))
            .dispatch()
            .await
            .status(),
        Status::Unauthorized,
        "dave must be rejected after removal"
    );
}

// ─── Concurrency / safety tests ───────────────────────────────────────────────

/// Verifies that concurrent config mutations don't cause panics or data races.
///
/// A writer task continuously rotates the user list while multiple reader tasks
/// issue requests. The test passes if no panics occur and responses are always
/// either 200 or 401 (never 500).
#[rocket::async_test]
async fn test_concurrent_config_mutation_is_safe() {
    let config = Arc::new(RwLock::new(
        test_config_with_access(AccessConfig::default()),
    ));
    let client = Arc::new(build_test_rocket(Arc::clone(&config)).await);

    let writer_config = Arc::clone(&config);

    // Writer: rapidly toggles between "admin" and "other" user lists
    let writer = tokio::spawn(async move {
        for i in 0..20 {
            {
                let mut cfg = writer_config.write().await;
                if i % 2 == 0 {
                    cfg.access.users = vec![make_user("admin", &["read:api", "admin:api"])];
                } else {
                    cfg.access.users = vec![make_user("other", &["read:api"])];
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });

    let admin_token = issue_test_token("admin");
    let other_token = issue_test_token("other");

    // Readers: fire requests interleaved with writer
    let mut tasks = vec![];
    for _ in 0..10 {
        let c = Arc::clone(&client);
        let at = admin_token.clone();
        let ot = other_token.clone();
        tasks.push(tokio::spawn(async move {
            for _ in 0..5 {
                let r1 = c
                    .get("/api/profile")
                    .header(Header::new("Authorization", format!("Bearer {}", at)))
                    .dispatch()
                    .await;
                // Must be 200 or 401 — never 500
                assert_ne!(r1.status(), Status::InternalServerError);

                let r2 = c
                    .get("/api/profile")
                    .header(Header::new("Authorization", format!("Bearer {}", ot)))
                    .dispatch()
                    .await;
                assert_ne!(r2.status(), Status::InternalServerError);

                tokio::time::sleep(std::time::Duration::from_millis(3)).await;
            }
        }));
    }

    writer.await.expect("writer task panicked");
    for t in tasks {
        t.await.expect("reader task panicked");
    }
}
