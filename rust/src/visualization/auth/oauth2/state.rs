// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! OAuth state management
//!
//! This module contains the OxideState struct and its implementation,
//! which manages the OAuth 2.0 server state including client registrations,
//! authorization storage, and token issuance.

use std::sync::{Arc, Mutex};

use log::debug;
use oxide_auth::frontends::simple::endpoint::{Generic, Vacant};
use oxide_auth::primitives::prelude::*;
use oxide_auth::primitives::registrar::RegisteredUrl;
use rocket::figment::Figment;
use url::Url;

use crate::config::{AccessConfig, GenerixConfig};
use crate::visualization::jwt::JwtIssuer;

/// Main state container for the OAuth 2.0 server implementation
///
/// `OxideState` encapsulates all the components needed for the OAuth 2.0 server:
/// client registrations, authorization storage, and token issuance. It's designed
/// to be shared across multiple routes and threads using Rocket's state management.
///
/// This structure uses thread-safe wrappers (`Arc<Mutex<>>`) around the core
/// components to ensure safe concurrent access from multiple Rocket workers.
///
/// ### Components
///
/// * `registrar` - Stores registered OAuth clients
/// * `authorizer` - Manages authorization grants and codes
/// * `issuer` - JWT token issuer for generating access tokens
/// * `hmac_secret` - Shared secret for JWT token validation
///
/// ### Thread Safety
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

    /// Generix configuration for Oxide Auth
    pub generix_config: GenerixConfig,
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
            generix_config: self.generix_config.clone(),
        }
    }
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
    /// ### Parameters
    ///
    /// * `hmac_secret` - The secret key used for signing and validating JWT tokens
    ///
    /// ### Returns
    ///
    /// A preconfigured `OxideState` instance ready to use with Rocket
    ///
    /// ### Example
    ///
    /// ```no_run
    /// use rust_photoacoustic::visualization::auth::OxideState;
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

        // Create a ClientMap based on config::AccessConfig::clients
        // The client_id is mapped to the Client::client_id
        // The first string in the allowed_callbacks is the default callback
        // The rest are additional allowed callbacks
        let mut client_map: Vec<Client> = vec![];
        // Extract the AccessConfig from the figment
        let access_config = figment
            .extract_inner::<AccessConfig>("access_config")
            .unwrap_or_else(|_| {
                panic!("Missing access configuration");
            });

        // Create and configure the JWT issuer
        let mut jwt_issuer = JwtIssuer::new(jwt_secret);
        jwt_issuer
            .with_issuer(
                access_config
                    .clone()
                    .iss
                    .unwrap_or("LaserSmartServer".to_string()),
            ) // Set the issuer name
            .valid_for(chrono::Duration::hours(1)); // Tokens valid for 1 hour

        for client in access_config.clients {
            debug!("Adding client to oxide-auth: {:?}", client.client_id);
            let mut oauth_client = Client::public(
                client.client_id.as_str(),
                RegisteredUrl::Semantic(client.allowed_callbacks[0].parse::<Url>().unwrap()),
                client.default_scope.parse::<Scope>().unwrap(),
            );
            debug!("  - registered url: {:?}", client.allowed_callbacks[0]);
            // Add additional redirect URIs
            for callback in &client.allowed_callbacks[1..] {
                oauth_client =
                    oauth_client.with_additional_redirect_uris(vec![RegisteredUrl::Semantic(
                        callback.parse().unwrap(),
                    )]);
                debug!("  - additional redirect uri: {:?}", callback);
            }
            // For debuggin purposes, log the default from the Client
            debug!("  - default scope: {:?}", client.default_scope);
            client_map.push(oauth_client);
        }

        OxideState {
            registrar: Arc::new(Mutex::new(client_map.into_iter().collect::<ClientMap>())),
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
            // Initialize the generix configuration
            generix_config: GenerixConfig::default(),
        }
    }

    /// Create a preconfigured OxideState from application configuration
    ///
    /// This factory method creates an OxideState using the application's Config struct
    /// instead of extracting from figment. This is part of the dynamic configuration
    /// refactoring to use the managed Config state.
    ///
    /// ### Parameters
    ///
    /// * `config` - The application configuration containing all OAuth settings
    ///
    /// ### Returns
    ///
    /// A preconfigured `OxideState` instance ready to use with Rocket
    ///
    /// ### Example
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use rust_photoacoustic::{config::Config, visualization::auth::OxideState};
    ///
    /// // Create the OAuth state from config
    /// let config = Arc::new(Config::default());
    /// let state = OxideState::from_config(&config);
    /// ```
    pub fn from_config(config: &crate::config::Config) -> Self {
        // Use the HMAC secret from configuration
        let hmac_secret = &config.visualization.hmac_secret;
        let jwt_secret = hmac_secret.as_bytes();

        // Create a ClientMap based on config::AccessConfig::clients
        // The client_id is mapped to the Client::client_id
        // The first string in the allowed_callbacks is the default callback
        // The rest are additional allowed callbacks
        let mut client_map: Vec<Client> = vec![];
        // Use the AccessConfig from the config
        let access_config = &config.access;

        // Create and configure the JWT issuer
        let mut jwt_issuer = JwtIssuer::new(jwt_secret);
        jwt_issuer
            .with_issuer(
                access_config
                    .clone()
                    .iss
                    .unwrap_or("LaserSmartServer".to_string()),
            ) // Set the issuer name
            .valid_for(chrono::Duration::hours(1)); // Tokens valid for 1 hour

        for client in &access_config.clients {
            debug!("Adding client to oxide-auth: {:?}", client.client_id);
            let mut oauth_client = Client::public(
                client.client_id.as_str(),
                RegisteredUrl::Semantic(client.allowed_callbacks[0].parse::<Url>().unwrap()),
                client.default_scope.parse::<Scope>().unwrap(),
            );
            debug!("  - registered url: {:?}", client.allowed_callbacks[0]);
            // Add additional redirect URIs
            for callback in &client.allowed_callbacks[1..] {
                oauth_client =
                    oauth_client.with_additional_redirect_uris(vec![RegisteredUrl::Semantic(
                        callback.parse().unwrap(),
                    )]);
                debug!("  - additional redirect uri: {:?}", callback);
            }
            // For debugging purposes, log the default from the Client
            debug!("  - default scope: {:?}", client.default_scope);
            client_map.push(oauth_client);
        }

        OxideState {
            registrar: Arc::new(Mutex::new(client_map.into_iter().collect::<ClientMap>())),
            // Authorization tokens are 16 byte random keys to a memory hash map.
            authorizer: Arc::new(Mutex::new(AuthMap::new(RandomGenerator::new(16)))),
            // Use JWT issuer for access tokens
            // These tokens can be verified independently by the resource server
            // and contain user information embedded within them
            issuer: Arc::new(Mutex::new(jwt_issuer)),
            // Store the HMAC secret for validation elsewhere
            hmac_secret: hmac_secret.clone(),
            // Set RS256 keys from config
            rs256_private_key: config.visualization.rs256_private_key.clone(),
            rs256_public_key: config.visualization.rs256_public_key.clone(),
            // Use the access config from config
            access_config: access_config.clone(),
            // Use the generix configuration from config
            generix_config: config.generix.clone(),
        }
    }

    /// Create an OAuth endpoint with this state
    ///
    /// This method creates a new OAuth endpoint configured with this state's
    /// registrar, authorizer, and issuer. The endpoint can then be further
    /// customized with solicitors and scope validators before executing
    /// an OAuth flow.
    ///
    /// ### Returns
    ///
    /// A Generic OAuth endpoint ready to be configured for a specific flow
    ///
    /// ### Panics
    ///
    /// This method will panic if any of the internal mutexes are poisoned
    /// (which would indicate a thread panic while holding the lock).
    ///
    /// ### Example
    ///
    /// ```no_run
    /// use rust_photoacoustic::visualization::auth::OxideState;
    /// let figment = rocket::Config::figment().merge(("hmac_secret", "your-secret"));
    /// let state = OxideState::preconfigured(figment);
    /// // We don't need an oauth_request for this example
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
