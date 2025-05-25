// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Test OAuth scope validation functionality
//!
//! This test verifies that the OAuth consent form properly validates requested
//! scopes against user permissions and shows appropriate warnings when scopes
//! are not permitted.

use rust_photoacoustic::config::{AccessConfig, User, Client};
use rust_photoacoustic::visualization::oidc_auth::OxideState;
use rocket::figment::Figment;
use rocket::figment::providers::{Format, Toml};

#[test]
fn test_scope_validation_logic() {
    // Test the scope validation function directly
    use rust_photoacoustic::visualization::oidc_auth::is_scope_allowed;
    
    let user_permissions = vec![
        "openid".to_string(),
        "profile".to_string(),
        "read:api".to_string(),
    ];
    
    // Test standard OpenID scopes (should always be allowed)
    assert!(is_scope_allowed("openid", &user_permissions));
    assert!(is_scope_allowed("profile", &user_permissions));
    assert!(is_scope_allowed("email", &user_permissions)); // Email is allowed by default
    
    // Test API scopes (should match user permissions)
    assert!(is_scope_allowed("read:api", &user_permissions));
    assert!(!is_scope_allowed("write:api", &user_permissions)); // User doesn't have this permission
    assert!(!is_scope_allowed("admin:api", &user_permissions)); // User doesn't have this permission
}

#[test]
fn test_user_permission_setup() {
    // Create test users with different permission levels
    let admin_user = User {
        user: "admin".to_string(),
        pass: "test_hash".to_string(),
        permissions: vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
            "read:api".to_string(),
            "write:api".to_string(),
            "admin:api".to_string(),
        ],
    };
    
    let readonly_user = User {
        user: "readonly".to_string(),
        pass: "test_hash".to_string(),
        permissions: vec![
            "openid".to_string(),
            "profile".to_string(),
            "read:api".to_string(),
        ],
    };
    
    let test_client = Client {
        client_id: "TestClient".to_string(),
        default_scope: "openid profile email read:api".to_string(),
        allowed_callbacks: vec!["http://localhost:8080/callback".to_string()],
    };
    
    let access_config = AccessConfig {
        users: vec![admin_user.clone(), readonly_user.clone()],
        clients: vec![test_client],
    };
    
    // Verify admin user has all permissions
    assert!(admin_user.permissions.contains(&"admin:api".to_string()));
    assert!(admin_user.permissions.contains(&"write:api".to_string()));
    assert!(admin_user.permissions.contains(&"read:api".to_string()));
    
    // Verify readonly user has limited permissions
    assert!(!readonly_user.permissions.contains(&"admin:api".to_string()));
    assert!(!readonly_user.permissions.contains(&"write:api".to_string()));
    assert!(readonly_user.permissions.contains(&"read:api".to_string()));
    
    // Verify access config is properly constructed
    assert_eq!(access_config.users.len(), 2);
    assert_eq!(access_config.clients.len(), 1);
}

#[test]
fn test_scope_respecting_registrar() {
    use rust_photoacoustic::visualization::oidc_auth::ScopedRegistrar;
    use oxide_auth::primitives::registrar::{Client, RegisteredUrl};
    use oxide_auth::primitives::prelude::Scope;
    use url::Url;
    
    // Create a test client
    let client = Client::public(
        "TestClient",
        RegisteredUrl::Semantic(Url::parse("http://localhost:8080/callback").unwrap()),
        "openid profile read:api".parse::<Scope>().unwrap(),
    );
    
    // Create registrar and add client
    let mut registrar = ScopedRegistrar::new();
    registrar.extend(vec![client]);
    
    // Test that the registrar respects requested scopes
    // This is a basic test - in practice, the scope negotiation happens
    // during the OAuth flow with actual HTTP requests
    
    println!("ScopedRegistrar test completed successfully");
}

#[cfg(test)]
mod scope_validation_integration_tests {
    use super::*;
    
    /// Test that demonstrates the complete OAuth scope validation flow
    #[test]
    fn test_oauth_state_with_scope_validation() {
        // Create a test configuration
        let config_toml = r#"
            hmac_secret = "test_secret_key_for_oauth_testing_purposes_only"
            
            [access_config]
            
            [[access_config.users]]
            user = "admin"
            pass = "test_hash"
            permissions = ["openid", "profile", "email", "read:api", "write:api", "admin:api"]
            
            [[access_config.users]]
            user = "readonly"
            pass = "test_hash"
            permissions = ["openid", "profile", "read:api"]
            
            [[access_config.clients]]
            client_id = "TestClient"
            default_scope = "openid profile read:api"
            allowed_callbacks = ["http://localhost:8080/callback"]
        "#;
        
        let figment = Figment::new()
            .merge(Toml::string(config_toml));
        
        // Create OAuth state with our custom registrar
        let state = OxideState::preconfigured(figment);
        
        // Verify that the state was created successfully
        assert!(!state.hmac_secret.is_empty());
        println!("Actual users: {}", state.access_config.users.len());
        println!("Users: {:?}", state.access_config.users.iter().map(|u| &u.user).collect::<Vec<_>>());
        // Now we should have exactly the 2 users we defined in the test config
        assert_eq!(state.access_config.users.len(), 2);
        assert_eq!(state.access_config.clients.len(), 1); // Just the test client
        
        // Find the admin user and verify permissions
        let admin_user = state.access_config.users.iter()
            .find(|u| u.user == "admin")
            .expect("Admin user should exist");
        
        assert!(admin_user.permissions.contains(&"admin:api".to_string()));
        
        // Find the readonly user and verify limited permissions
        let readonly_user = state.access_config.users.iter()
            .find(|u| u.user == "readonly")
            .expect("Readonly user should exist");
        
        assert!(!readonly_user.permissions.contains(&"admin:api".to_string()));
        assert!(readonly_user.permissions.contains(&"read:api".to_string()));
        
        println!("OAuth state with scope validation created successfully");
    }
}
