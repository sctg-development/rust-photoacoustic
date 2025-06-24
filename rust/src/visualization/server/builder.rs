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
use crate::config::{Config, GenerixConfig};
use crate::include_png_as_base64;
use crate::processing::computing_nodes::SharedComputingState;
use crate::processing::nodes::streaming_registry::StreamingNodeRegistry;
use crate::thermal_regulation::SharedThermalState;
use crate::visualization::api::action::get_action_routes;
use crate::visualization::api::graph::graph::*;
use crate::visualization::api::*;
use crate::visualization::auth::{
    authorize, oauth2::authorize_consent, oauth2::login, oauth2::userinfo, refresh, token,
    OxideState,
};
use crate::visualization::oidc::{jwks, openid_configuration};
use crate::visualization::shared_state::SharedVisualizationState;
use crate::visualization::streaming::AudioStreamState;
use crate::visualization::streaming::*;
use crate::visualization::vite_dev_proxy;
use base64::Engine;
use log::{debug, info, warn};
use rocket::figment::Figment;
use rocket::{routes, Route};
use rocket::{Build, Rocket};
use rocket_async_compression::CachedCompression;
use rocket_okapi::okapi::merge::marge_spec_list;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::settings::OpenApiSettings;
use rocket_okapi::{get_openapi_route, openapi_get_routes_spec};
use rocket_okapi::{rapidoc::*, settings::UrlObject};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Build a configured Rocket server instance
///
/// This function creates and configures a Rocket server instance with all
/// necessary routes, fairings, and state management for the photoacoustic
/// visualization application.
///
/// ### Parameters
///
/// * `figment` - The Rocket configuration figment containing server settings
/// * `config` - The application configuration (for future dynamic configuration support)
/// * `audio_stream` - Optional shared audio stream for real-time audio endpoints
/// * `visualization_state` - Optional shared visualization state for statistics
/// * `streaming_registry` - Optional streaming node registry for audio processing
/// * `thermal_state` - Optional shared thermal regulation state for temperature control
/// * `computing_state` - Optional shared computing state for analytical results from computing nodes
///
/// ### Returns
///
/// A configured Rocket instance ready to be launched
///
/// ### Panics
///
/// This function will exit the process if:
/// * The JWT validator cannot be initialized with the provided secret
///
/// ### Example
///
/// ```
/// use rocket::figment::Figment;
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
/// use rust_photoacoustic::{config::Config, visualization::server};
///
/// async fn example() {
///     let figment = Figment::from(rocket::Config::default());
///     let config = Arc::new(RwLock::new(Config::default()));
///     let rocket = server::build_rocket(figment, config, None, None, None, None, None).await;
///     // Launch the server
///     // rocket.launch().await.expect("Failed to launch");
/// }
/// ```
pub async fn build_rocket(
    figment: Figment,
    config: Arc<RwLock<Config>>,
    audio_stream: Option<Arc<SharedAudioStream>>,
    visualization_state: Option<Arc<SharedVisualizationState>>,
    streaming_registry: Option<Arc<StreamingNodeRegistry>>,
    thermal_state: Option<SharedThermalState>,
    computing_state: Option<SharedComputingState>,
) -> Rocket<Build> {
    // Load hmac secret from config
    let config_read = config.read().await;
    let hmac_secret = config_read.visualization.hmac_secret.clone();

    // Load access configuration from config
    let access_config = config_read.access.clone();
    let compression_config = config_read.visualization.enable_compression;
    drop(config_read);

    // Create OAuth2 state from config (improved dynamic configuration approach)
    let mut oxide_state = OxideState::from_config(&config).await;

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
                    let duration: chrono::TimeDelta =
                        chrono::TimeDelta::seconds(access_config.duration.or(Some(86400)).unwrap());
                    jwt_issuer.valid_for(duration);
                    oxide_state.issuer = std::sync::Arc::new(std::sync::Mutex::new(jwt_issuer));
                }
            }
        }
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

    // Initialize OpenAPI specification accumulator with proper version
    let mut openapi_spec = OpenApi::default();
    openapi_spec.openapi = "3.0.0".to_string(); // Set the version to match other specs

    // Add config routes
    let (openapi_routes_config, openapi_spec_config) = get_config_routes();

    // Merge config OpenAPI spec
    if let Err(e) = rocket_okapi::okapi::merge::merge_specs(
        &mut openapi_spec,
        &"/".to_string(),
        &openapi_spec_config,
    ) {
        warn!("Failed to merge config OpenAPI spec: {}", e);
    }

    let rocket_builder = rocket_builder.mount("/", openapi_routes_config);

    // Add action routes for node history
    let (openapi_routes_action, openapi_spec_action) = get_action_routes();

    // Merge action OpenAPI spec
    if let Err(e) = rocket_okapi::okapi::merge::merge_specs(
        &mut openapi_spec,
        &"/".to_string(),
        &openapi_spec_action,
    ) {
        warn!("Failed to merge action OpenAPI spec: {}", e);
    }

    let rocket_builder = rocket_builder.mount("/", openapi_routes_action);

    // Add visualization routes if state is available
    let rocket_builder =
        add_visualization_routes(rocket_builder, visualization_state, &mut openapi_spec);

    // Add test routes for API testing
    let rocket_builder = add_test_routes(rocket_builder, &mut openapi_spec);

    let (openapi_routes_base, openapi_spec_base) =
        openapi_get_routes_spec![webclient_index, webclient_index_html, options,];

    // Merge base routes OpenAPI spec
    if let Err(e) = rocket_okapi::okapi::merge::merge_specs(
        &mut openapi_spec,
        &"/".to_string(),
        &openapi_spec_base,
    ) {
        warn!("Failed to merge base routes OpenAPI spec: {}", e);
    }

    let rocket_builder = rocket_builder
        .mount("/", openapi_routes_base)
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
                helper_min_js,
                crate::visualization::api_auth::get_profile,
                crate::visualization::api_auth::get_data,
            ],
        )
        .mount("/", vite_dev_proxy::get_vite_dev_routes())
        .manage(oxide_state)
        .manage(jwt_validator)
        .manage(config.clone()); // Add config as managed state for future dynamic configuration

    // Add computing routes and state if available
    let rocket_builder =
        add_computing_routes(rocket_builder, computing_state.clone(), &mut openapi_spec);

    // Add thermal regulation state if available
    let rocket_builder = add_thermal_routes(rocket_builder, thermal_state, &mut openapi_spec);

    // Add compression fairing if enabled in configuration
    let rocket_builder = add_compression(rocket_builder, compression_config);

    // Add audio streaming routes and state if audio stream is available
    let rocket_builder = add_audio_routes(
        rocket_builder,
        audio_stream,
        streaming_registry,
        &mut openapi_spec,
    );

    // Add OpenAPI documentation routes
    add_openapi_documentation(rocket_builder, openapi_spec)
}

use rocket::{get, http::Status, serde::json::Json, State};

#[get("/client/generix.json", rank = 1)]
/// Endpoint to retrieve the Generix configuration
///
/// This endpoint demonstrates Phase 1 of the dynamic configuration evolution.
/// It accesses the GenerixConfig through the managed Config state instead of
/// the GenerixConfig request guard, preparing for future dynamic configuration.
pub async fn get_generix_config(
    config: &State<Arc<RwLock<Config>>>,
) -> Result<Json<GenerixConfig>, Status> {
    // Access the generix config through the managed Config state
    Ok(Json(config.read().await.generix.clone()))
}

/// Add test routes for API testing
///
/// This function mounts the test routes for API testing and merges their OpenAPI specification
fn add_test_routes(rocket_builder: Rocket<Build>, openapi_spec: &mut OpenApi) -> Rocket<Build> {
    // Add test routes for API testing
    let (openapi_routes_test, openapi_spec_test) = get_test_routes();
    // Merge OpenAPI specs into the main spec
    if let Err(e) =
        rocket_okapi::okapi::merge::merge_specs(openapi_spec, &"/".to_string(), &openapi_spec_test)
    {
        warn!("Failed to merge graph OpenAPI spec: {}", e);
    }
    rocket_builder.mount("/", openapi_routes_test)
}

/// Add visualization and graph routes if state is available
///
/// Updates the OpenAPI specification with graph and system routes
fn add_visualization_routes(
    rocket_builder: Rocket<Build>,
    visualization_state: Option<Arc<SharedVisualizationState>>,
    openapi_spec: &mut OpenApi,
) -> Rocket<Build> {
    if let Some(vis_state) = visualization_state {
        debug!("Adding SharedVisualizationState to Rocket state management");
        // Extract the value from Arc to match the expected type for State<SharedVisualizationState>
        let shared_state = (*vis_state).clone();
        let (openapi_routes_graph, openapi_spec_graph) = get_graph_routes();
        let (openapi_routes_system, openapi_spec_system) = get_system_routes();

        // Merge OpenAPI specs into the main spec
        if let Err(e) = rocket_okapi::okapi::merge::merge_specs(
            openapi_spec,
            &"/".to_string(),
            &openapi_spec_graph,
        ) {
            warn!("Failed to merge graph OpenAPI spec: {}", e);
        }
        if let Err(e) = rocket_okapi::okapi::merge::merge_specs(
            openapi_spec,
            &"/".to_string(),
            &openapi_spec_system,
        ) {
            warn!("Failed to merge system OpenAPI spec: {}", e);
        }

        rocket_builder
            .manage(shared_state)
            .mount("/", openapi_routes_graph)
            .mount("/", openapi_routes_system)
    } else {
        debug!("No visualization state provided, API will return 404 for statistics");
        rocket_builder
    }
}

/// Add thermal regulation routes if state is available
///
/// Updates the OpenAPI specification with thermal routes
fn add_thermal_routes(
    rocket_builder: Rocket<Build>,
    thermal_state: Option<SharedThermalState>,
    openapi_spec: &mut OpenApi,
) -> Rocket<Build> {
    if let Some(thermal_state) = thermal_state {
        debug!("Adding SharedThermalState to Rocket state management");
        let (openapi_routes_thermal, openapi_spec_thermal) = get_thermal_routes();

        // Merge the thermal spec only when thermal state is available
        if let Err(e) = rocket_okapi::okapi::merge::merge_specs(
            openapi_spec,
            &"/".to_string(),
            &openapi_spec_thermal,
        ) {
            warn!("Failed to merge thermal OpenAPI spec: {}", e);
        }

        rocket_builder
            .manage(thermal_state)
            .mount("/", openapi_routes_thermal)
    } else {
        debug!("No thermal state provided, skipping thermal routes");
        rocket_builder
    }
}

/// Add audio streaming routes if stream is available
///
/// Updates the OpenAPI specification with audio routes
fn add_audio_routes(
    rocket_builder: Rocket<Build>,
    audio_stream: Option<Arc<SharedAudioStream>>,
    streaming_registry: Option<Arc<StreamingNodeRegistry>>,
    openapi_spec: &mut OpenApi,
) -> Rocket<Build> {
    if let Some(stream) = audio_stream {
        let registry = streaming_registry.unwrap_or_else(|| Arc::new(StreamingNodeRegistry::new()));
        let audio_state = AudioStreamState { stream, registry };
        let (openapi_routes_audio, openapi_spec_audio) = get_audio_streaming_routes();

        // Merge audio OpenAPI spec
        if let Err(e) = rocket_okapi::okapi::merge::merge_specs(
            openapi_spec,
            &"/".to_string(),
            &openapi_spec_audio,
        ) {
            warn!("Failed to merge audio OpenAPI spec: {}", e);
        }

        rocket_builder
            .mount("/", openapi_routes_audio)
            .manage(audio_state)
    } else {
        debug!("No audio stream provided, skipping audio routes");
        rocket_builder
    }
}

/// Add Computing routes if computing state is available
///
/// This function mounts the computing routes and merges their OpenAPI specification
fn add_computing_routes(
    rocket_builder: Rocket<Build>,
    computing_state: Option<SharedComputingState>,
    openapi_spec: &mut OpenApi,
) -> Rocket<Build> {
    if let Some(computing_state) = computing_state {
        debug!("Adding SharedComputingState to Rocket state management");
        let (openapi_routes_computing, openapi_spec_computing) = get_computing_routes();

        // Merge computing OpenAPI spec
        if let Err(e) = rocket_okapi::okapi::merge::merge_specs(
            openapi_spec,
            &"/".to_string(),
            &openapi_spec_computing,
        ) {
            warn!("Failed to merge computing OpenAPI spec: {}", e);
        }

        rocket_builder
            .mount("/", openapi_routes_computing)
            .manage(computing_state)
    } else {
        debug!("No computing state provided, skipping computing routes");
        rocket_builder
    }
}

/// Adds OpenAPI documentation routes to the Rocket instance.
/// This function mounts the openapi.json endpoint and RapiDoc interface.
fn add_openapi_documentation(
    rocket_builder: Rocket<Build>,
    openapi_spec: OpenApi,
) -> Rocket<Build> {
    let openapi_settings = OpenApiSettings::default();
    let rocket_builder = rocket_builder.mount(
        "/",
        vec![get_openapi_route(openapi_spec, &openapi_settings)],
    );

    rocket_builder
        .mount(
            "/api/doc/",
            make_rapidoc(&RapiDocConfig {
                title: Some("SCTG rust-photoacoustic API Doc".to_owned()),
                custom_html: Some(include_str!("../../../resources/rapidoc_helper/index.html").to_owned()),
                slots: SlotsConfig{
                    logo: Some(include_png_as_base64!("../../../resources/rapidoc_helper/logo.png")),
                    footer: Some(r#"© 2025 <a style="color: #ffffff; text-decoration: none;" href='https://sctg.eu.org/'>SCTG</a>. All rights reserved. <a style="color: #ffffff; text-decoration: none;" href="https://github.com/sctg-development/rust-photoacoustic">rust-photoacoustic <svg style="height:1.25em" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 496 512"><path d="M165.9 397.4c0 2-2.3 3.6-5.2 3.6-3.3 .3-5.6-1.3-5.6-3.6 0-2 2.3-3.6 5.2-3.6 3-.3 5.6 1.3 5.6 3.6zm-31.1-4.5c-.7 2 1.3 4.3 4.3 4.9 2.6 1 5.6 0 6.2-2s-1.3-4.3-4.3-5.2c-2.6-.7-5.5 .3-6.2 2.3zm44.2-1.7c-2.9 .7-4.9 2.6-4.6 4.9 .3 2 2.9 3.3 5.9 2.6 2.9-.7 4.9-2.6 4.6-4.6-.3-1.9-3-3.2-5.9-2.9zM244.8 8C106.1 8 0 113.3 0 252c0 110.9 69.8 205.8 169.5 239.2 12.8 2.3 17.3-5.6 17.3-12.1 0-6.2-.3-40.4-.3-61.4 0 0-70 15-84.7-29.8 0 0-11.4-29.1-27.8-36.6 0 0-22.9-15.7 1.6-15.4 0 0 24.9 2 38.6 25.8 21.9 38.6 58.6 27.5 72.9 20.9 2.3-16 8.8-27.1 16-33.7-55.9-6.2-112.3-14.3-112.3-110.5 0-27.5 7.6-41.3 23.6-58.9-2.6-6.5-11.1-33.3 2.6-67.9 20.9-6.5 69 27 69 27 20-5.6 41.5-8.5 62.8-8.5s42.8 2.9 62.8 8.5c0 0 48.1-33.6 69-27 13.7 34.7 5.2 61.4 2.6 67.9 16 17.7 25.8 31.5 25.8 58.9 0 96.5-58.9 104.2-114.8 110.5 9.2 7.9 17 22.9 17 46.4 0 33.7-.3 75.4-.3 83.6 0 6.5 4.6 14.4 17.3 12.1C428.2 457.8 496 362.9 496 252 496 113.3 383.5 8 244.8 8zM97.2 352.9c-1.3 1-1 3.3 .7 5.2 1.6 1.6 3.9 2.3 5.2 1 1.3-1 1-3.3-.7-5.2-1.6-1.6-3.9-2.3-5.2-1zm-10.8-8.1c-.7 1.3 .3 2.9 2.3 3.9 1.6 1 3.6 .7 4.3-.7 .7-1.3-.3-2.9-2.3-3.9-2-.6-3.6-.3-4.3 .7zm32.4 35.6c-1.6 1.3-1 4.3 1.3 6.2 2.3 2.3 5.2 2.6 6.5 1 1.3-1.3 .7-4.3-1.3-6.2-2.2-2.3-5.2-2.6-6.5-1zm-11.4-14.7c-1.6 1-1.6 3.6 0 5.9 1.6 2.3 4.3 3.3 5.6 2.3 1.6-1.3 1.6-3.9 0-6.2-1.4-2.3-4-3.3-5.6-2z"/></svg></a>"#.to_owned()),
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

/// Add compression fairing if enabled in configuration
///
/// This function attaches the CachedCompression fairing to the Rocket instance
/// if compression is enabled in the configuration and the EXTERNAL_WEB_CLIENT
/// environment variable is not set (which is used for Vite dev server proxying).
fn add_compression(rocket_builder: Rocket<Build>, compression_enabled: bool) -> Rocket<Build> {
    // Attach compression fairing if enabled in config and not using EXTERNAL_WEB_CLIENT
    // EXTERNAL_WEB_CLIENT is an environment variable for proxying Vite dev server
    if compression_enabled && !std::env::var("EXTERNAL_WEB_CLIENT").is_ok() {
        info!("Compression is enabled in configuration");
        if cfg!(debug_assertions) {
            warn!("Compression is enabled in debug mode, this may affect performance");
        }
        rocket_builder.attach(CachedCompression {
            cached_path_prefixes: vec!["/api/doc/".to_owned(), "/client/".to_owned()],
            cached_path_suffixes: vec![
                ".otf".to_owned(),
                ".js".to_owned(),
                ".css".to_owned(),
                ".html".to_owned(),
                ".json".to_owned(),
                ".wasm".to_owned(),
                ".svg".to_owned(),
                ".map".to_owned(),
            ],
            ..Default::default()
        })
    } else {
        debug!("Compression is disabled in configuration");
        rocket_builder
    }
}

#[cfg(test)]
/// Build a Rocket instance configured specifically for testing
///
/// This function creates a Rocket instance with settings optimized for
/// automated testing. It uses a random port to avoid conflicts with
/// other running services and disables logging for cleaner test output.
///
/// ### Returns
///
/// A configured Rocket instance ready for testing
///
/// ### Panics
///
/// This function will exit the process if:
/// * The JWT validator cannot be initialized with the test secret
///
/// ### Note
///
/// This function is only available when compiled with the `test` configuration
/// and is primarily intended for internal unit and integration tests.
pub fn build_rocket_test_instance() -> Rocket<Build> {
    use rocket::Config;
    use std::sync::Arc;

    use crate::visualization::introspection::introspect;

    // Create a test configuration
    let rocket_config = Config::figment()
        .merge(("address", "localhost"))
        .merge(("port", 0)) // Random port for tests
        .merge(("log_level", rocket::config::LogLevel::Off));

    let access_config = crate::config::AccessConfig::default();

    // Use a test HMAC secret
    let test_hmac_secret = "test-hmac-secret-key-for-testing";
    // Add the test HMAC secret to the configuration
    let rocket_config = rocket_config.merge(("hmac_secret", test_hmac_secret.to_string()));

    // Create OAuth2 state with the test secret
    let oxide_state = OxideState::preconfigured(rocket_config.clone());

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

    // Create a test application config
    let app_config = Arc::new(crate::config::Config::default());

    // Build Rocket instance for tests
    rocket::custom(rocket_config)
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
        .manage(app_config) // Add config as managed state
}
