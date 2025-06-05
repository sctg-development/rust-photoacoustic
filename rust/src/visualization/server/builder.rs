// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Rocket server builder and configuration
//!
//! This module provides functions to build and configure the Rocket server
//! instance with all necessary routes, fairings, and state management.

use super::cors::CORS;
use super::handlers::*;
use crate::acquisition::SharedAudioStream;
use crate::config::{AccessConfig, GenerixConfig, VisualizationConfig};
use crate::include_png_as_base64;
use crate::processing::nodes::streaming_registry::StreamingNodeRegistry;
use crate::visualization::auth::{
    authorize, oauth2::authorize_consent, oauth2::login, oauth2::userinfo, refresh, token,
    OxideState,
};
use crate::visualization::oidc::{jwks, openid_configuration};
use crate::visualization::shared_state::SharedVisualizationState;
use crate::visualization::streaming::AudioStreamState;
use crate::visualization::vite_dev_proxy;
use anyhow::Context;
use base64::Engine;
use log::{debug, info, warn};
use rocket::figment::Figment;
use rocket::routes;
use rocket::{Build, Rocket};
use rocket_async_compression::Compression;
use rocket_okapi::{openapi_get_routes, rapidoc::*, settings::UrlObject};
use std::sync::Arc;

/// Build a configured Rocket server instance
///
/// This function creates and configures a Rocket server instance with all
/// necessary routes, fairings, and state management for the photoacoustic
/// visualization application.
///
/// # Parameters
///
/// * `figment` - The Rocket configuration figment containing server settings
/// * `audio_stream` - Optional shared audio stream for real-time audio endpoints
///
/// # Returns
///
/// A configured Rocket instance ready to be launched
///
/// # Panics
///
/// This function will exit the process if:
/// * The JWT validator cannot be initialized with the provided secret
///
/// # Example
///
/// ```
/// use rocket::figment::Figment;
/// use rust_photoacoustic::visualization::server;
///
/// async fn example() {
///     let config = Figment::from(rocket::Config::default());
///     let rocket = server::build_rocket(config, None, None).await;
///     // Launch the server
///     // rocket.launch().await.expect("Failed to launch");
/// }
/// ```
pub async fn build_rocket(
    figment: Figment,
    audio_stream: Option<Arc<SharedAudioStream>>,
    visualization_state: Option<Arc<SharedVisualizationState>>,
) -> Rocket<Build> {
    let hmac_secret = figment
        .extract_inner::<String>("hmac_secret")
        .context("Missing HMAC secret in config")
        .unwrap();
    let access_config: AccessConfig = figment
        .extract_inner::<AccessConfig>("access_config")
        .context("Missing access in figment")
        .unwrap();
    let compression_config = figment
        .extract_inner::<VisualizationConfig>("visualization_config")
        .context("Missing visualization_config in figment")
        .unwrap()
        .enable_compression;

    // Create OAuth2 state with the HMAC secret from config
    let mut oxide_state = OxideState::preconfigured(figment.clone());

    // Extract RS256 keys from figment if present
    if let Ok(private_key) = figment.extract_inner::<String>("rs256_private_key") {
        oxide_state.rs256_private_key = private_key;
    }

    // Extract the Generix configuration from the figment key generix_config
    if let Ok(generix_config) = figment.extract_inner::<GenerixConfig>("generix_config") {
        oxide_state.generix_config = generix_config;
    }

    if let Ok(public_key) = figment.extract_inner::<String>("rs256_public_key") {
        oxide_state.rs256_public_key = public_key;

        // If we have RS256 keys, update the JWT issuer
        if !oxide_state.rs256_public_key.is_empty() && !oxide_state.rs256_private_key.is_empty() {
            if let Ok(decoded_private) =
                base64::engine::general_purpose::STANDARD.decode(&oxide_state.rs256_private_key)
            {
                if let Ok(decoded_public) =
                    base64::engine::general_purpose::STANDARD.decode(&oxide_state.rs256_public_key)
                {
                    // Create a new JWT issuer with RS256 keys
                    if let Ok(mut jwt_issuer) = crate::visualization::jwt::JwtIssuer::with_rs256_pem(
                        &decoded_private,
                        &decoded_public,
                    ) {
                        let duration: chrono::TimeDelta = chrono::TimeDelta::seconds(
                            access_config.duration.or(Some(86400)).unwrap(),
                        );
                        jwt_issuer.valid_for(duration);
                        oxide_state.issuer = std::sync::Arc::new(std::sync::Mutex::new(jwt_issuer));
                    }
                }
            }
        }
    }

    // Extract user access configuration from figment
    if let Ok(access_config) = figment.extract_inner::<AccessConfig>("access_config") {
        oxide_state.access_config = access_config;
    } else {
        log::error!("Access config not found in figment");
    }

    // Initialize JWT validator - try to use RS256 if keys are available, otherwise use HMAC secret
    let rs256_public_key_bytes = if !oxide_state.rs256_public_key.is_empty() {
        match base64::engine::general_purpose::STANDARD.decode(&oxide_state.rs256_public_key) {
            Ok(decoded) => Some(decoded),
            Err(e) => {
                eprintln!("Warning: Failed to decode RS256 public key: {}", e);
                None
            }
        }
    } else {
        None
    };

    let access_config = figment
        .extract_inner::<AccessConfig>("access_config")
        .expect("Failed to extract config from Rocket");
    // Initialize JWT validator with RS256 public key if available, otherwise use HMAC
    let jwt_validator = match crate::visualization::api_auth::init_jwt_validator(
        hmac_secret.clone().as_str(),
        rs256_public_key_bytes.as_deref(),
        access_config,
    ) {
        Ok(validator) => std::sync::Arc::new(validator),
        Err(e) => {
            eprintln!("Failed to initialize JWT validator: {}", e);
            std::process::exit(1);
        }
    };

    let rocket_builder = rocket::custom(figment).attach(CORS);

    // Add visualization state if available (before mounting routes that need it)
    let rocket_builder = if let Some(vis_state) = visualization_state {
        debug!("Adding SharedVisualizationState to Rocket state management");
        // Extract the value from Arc to match the expected type for State<SharedVisualizationState>
        let shared_state = (*vis_state).clone();
        rocket_builder.manage(shared_state).mount(
            "/api",
            routes![crate::visualization::api::get::graph_statistics::get_graph_statistics,],
        )
    } else {
        debug!("No visualization state provided, API will return 404 for statistics");
        rocket_builder
    };

    let rocket_builder = rocket_builder
        .mount(
            "/",
            openapi_get_routes![webclient_index, webclient_index_html, options],
        )
        .mount(
            "/",
            routes![
                favicon,
                webclient,
                authorize,
                authorize_consent,
                login,
                userinfo,
                token,
                refresh,
                crate::visualization::introspection::introspect,
                openid_configuration,
                jwks,
                get_generix_config,
            ],
        )
        .mount("/", vite_dev_proxy::get_vite_dev_routes())
        .mount(
            "/api",
            routes![
                crate::visualization::api_auth::get_profile,
                crate::visualization::api_auth::get_data,
                crate::visualization::api::test_api,
            ],
        )
        .manage(oxide_state)
        .manage(jwt_validator);

    // Attach compression fairing if enabled in config
    let rocket_builder = if compression_config {
        info!("Compression is enabled in configuration");
        if cfg!(debug_assertions) {
            warn!("Compression is enabled in debug mode, this may affect performance");
        }
        rocket_builder.attach(Compression::fairing())
    } else {
        debug!("Compression is disabled in configuration");
        rocket_builder
    };

    // Add audio streaming routes and state if audio stream is available
    if let Some(stream) = audio_stream {
        let registry = StreamingNodeRegistry::new();
        let audio_state = AudioStreamState { stream, registry };
        rocket_builder
            .mount(
                "/api",
                crate::visualization::streaming::get_audio_streaming_routes(),
            )
            .manage(audio_state)
    } else {
        debug!("No audio stream provided, skipping audio routes");
        rocket_builder
    }
        .mount(
            "/api/doc/",
            make_rapidoc(&RapiDocConfig {
                title: Some("SCTG rust-photoacoustic API Doc".to_owned()),
                custom_html: Some(include_str!("../../../resources/rapidoc_helper/index.html").to_owned()),
                slots: SlotsConfig{
                    logo: Some(include_png_as_base64!("../../../resources/rapidoc_helper/logo.png")),
                    footer: Some(r#"© 2025 <a style="color: #ffffff; text-decoration: none;" href='https://sctg.eu.org/'>SCTG</a>. All rights reserved. <a style="color: #ffffff; text-decoration: none;" href="https://github.com/sctg-development/sctgdesk-server">sctgdesk-server <svg style="height:1.25em" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 496 512"><path d="M165.9 397.4c0 2-2.3 3.6-5.2 3.6-3.3 .3-5.6-1.3-5.6-3.6 0-2 2.3-3.6 5.2-3.6 3-.3 5.6 1.3 5.6 3.6zm-31.1-4.5c-.7 2 1.3 4.3 4.3 4.9 2.6 1 5.6 0 6.2-2s-1.3-4.3-4.3-5.2c-2.6-.7-5.5 .3-6.2 2.3zm44.2-1.7c-2.9 .7-4.9 2.6-4.6 4.9 .3 2 2.9 3.3 5.9 2.6 2.9-.7 4.9-2.6 4.6-4.6-.3-1.9-3-3.2-5.9-2.9zM244.8 8C106.1 8 0 113.3 0 252c0 110.9 69.8 205.8 169.5 239.2 12.8 2.3 17.3-5.6 17.3-12.1 0-6.2-.3-40.4-.3-61.4 0 0-70 15-84.7-29.8 0 0-11.4-29.1-27.8-36.6 0 0-22.9-15.7 1.6-15.4 0 0 24.9 2 38.6 25.8 21.9 38.6 58.6 27.5 72.9 20.9 2.3-16 8.8-27.1 16-33.7-55.9-6.2-112.3-14.3-112.3-110.5 0-27.5 7.6-41.3 23.6-58.9-2.6-6.5-11.1-33.3 2.6-67.9 20.9-6.5 69 27 69 27 20-5.6 41.5-8.5 62.8-8.5s42.8 2.9 62.8 8.5c0 0 48.1-33.6 69-27 13.7 34.7 5.2 61.4 2.6 67.9 16 17.7 25.8 31.5 25.8 58.9 0 96.5-58.9 104.2-114.8 110.5 9.2 7.9 17 22.9 17 46.4 0 33.7-.3 75.4-.3 83.6 0 6.5 4.6 14.4 17.3 12.1C428.2 457.8 496 362.9 496 252 496 113.3 383.5 8 244.8 8zM97.2 352.9c-1.3 1-1 3.3 .7 5.2 1.6 1.6 3.9 2.3 5.2 1 1.3-1 1-3.3-.7-5.2-1.6-1.6-3.9-2.3-5.2-1zm-10.8-8.1c-.7 1.3 .3 2.9 2.3 3.9 1.6 1 3.6 .7 4.3-.7 .7-1.3-.3-2.9-2.3-3.9-2-.6-3.6-.3-4.3 .7zm32.4 35.6c-1.6 1.3-1 4.3 1.3 6.2 2.3 2.3 5.2 2.6 6.5 1 1.3-1.3 .7-4.3-1.3-6.2-2.2-2.3-5.2-2.6-6.5-1zm-11.4-14.7c-1.6 1-1.6 3.6 0 5.9 1.6 2.3 4.3 3.3 5.6 2.3 1.6-1.3 1.6-3.9 0-6.2-1.4-2.3-4-3.3-5.6-2z"/></svg></a>"#.to_owned()),
                    ..Default::default()
                },
                general: GeneralConfig {
                    spec_urls: vec![UrlObject::new("General", "../../openapi.json")],
                    ..Default::default()
                },
                hide_show: HideShowConfig {
                    allow_spec_url_load: false,
                    allow_spec_file_load: false,
                    allow_spec_file_download: true,
                    show_curl_before_try: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
        )
}

#[cfg(test)]
/// Build a Rocket instance configured specifically for testing
///
/// This function creates a Rocket instance with settings optimized for
/// automated testing. It uses a random port to avoid conflicts with
/// other running services and disables logging for cleaner test output.
///
/// # Returns
///
/// A configured Rocket instance ready for testing
///
/// # Panics
///
/// This function will exit the process if:
/// * The JWT validator cannot be initialized with the test secret
///
/// # Note
///
/// This function is only available when compiled with the `test` configuration
/// and is primarily intended for internal unit and integration tests.
pub fn build_rocket_test_instance() -> Rocket<Build> {
    use rocket::Config;

    use crate::visualization::introspection::introspect;

    // Create a test configuration
    let config = Config::figment()
        .merge(("address", "localhost"))
        .merge(("port", 0)) // Random port for tests
        .merge(("log_level", rocket::config::LogLevel::Off));

    let access_config = AccessConfig::default();

    // Use a test HMAC secret
    let test_hmac_secret = "test-hmac-secret-key-for-testing";
    // Add the test HMAC secret to the configuration
    let config = config.merge(("hmac_secret", test_hmac_secret.to_string()));

    // Create OAuth2 state with the test secret
    let oxide_state = OxideState::preconfigured(config.clone());

    // Initialize JWT validator with the test secret
    let jwt_validator = match crate::visualization::api_auth::init_jwt_validator(
        test_hmac_secret,
        None,
        access_config,
    ) {
        Ok(validator) => std::sync::Arc::new(validator),
        Err(e) => {
            eprintln!("Failed to initialize JWT validator: {}", e);
            std::process::exit(1);
        }
    };

    // Build Rocket instance for tests
    rocket::custom(config)
        .attach(CORS)
        .mount(
            "/",
            routes![
                // Routes for OAuth tests
                authorize,
                authorize_consent,
                token,
                refresh,
                openid_configuration, // Add OIDC configuration endpoint
                jwks,                 // Add JWKS endpoint
                introspect,           //Add introspection endpoint once fixed
                get_generix_config,   // Add generix.json endpoint
            ],
        )
        .mount(
            "/api",
            routes![
                crate::visualization::api_auth::get_profile,
                crate::visualization::api_auth::get_data,
            ],
        )
        .manage(oxide_state)
        .manage(jwt_validator)
}

use rocket::{get, http::Status, serde::json::Json};

#[get("/client/generix.json", rank = 1)]
/// Endpoint to retrieve the Generix configuration
pub async fn get_generix_config(
    generix_config: GenerixConfig,
) -> Result<Json<GenerixConfig>, Status> {
    // This endpoint returns the Generix configuration as a JSON string
    Ok(Json(generix_config))
}
