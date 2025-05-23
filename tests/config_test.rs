use anyhow::Result;
use base64::Engine;
use rust_photoacoustic::config::{
    AccessConfig, AcquisitionConfig, Config, ModbusConfig, PhotoacousticConfig,
    VisualizationConfig, USER_SESSION_SEPARATOR,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_config_load_and_save() -> Result<()> {
    // Create a temporary directory
    let temp_dir = tempdir()?;
    let config_path = temp_dir.path().join("config.yaml");

    // Create a custom config
    let config = Config {
        visualization: VisualizationConfig {
            enabled: true,
            port: 8081,
            address: "192.168.1.1".to_string(),
            name: "TestServer".to_string(),
            cert: None,
            key: None,
            hmac_secret: "test-secret".to_string(),
            rs256_private_key: base64::engine::general_purpose::STANDARD
                .encode((include_str!("../resources/private.key")).as_bytes()),
            rs256_public_key: base64::engine::general_purpose::STANDARD
                .encode((include_str!("../resources/pub.key")).as_bytes()),
            session_secret: "session-secret".to_string(),
        },
        acquisition: AcquisitionConfig {
            enabled: false,
            interval_ms: 1000,
        },
        modbus: ModbusConfig {
            enabled: false,
            port: 502,
            address: "127.0.0.1".to_string(),
        },
        photoacoustic: PhotoacousticConfig::default(),
        access: AccessConfig::default(),
    };

    // Save config to file
    config.save_to_file(&config_path)?;

    // Load config from file
    let loaded_config = Config::from_file(&config_path)?;

    // Verify loaded config matches original
    assert_eq!(loaded_config.visualization.port, 8081);
    assert_eq!(loaded_config.visualization.address, "192.168.1.1");
    assert_eq!(loaded_config.visualization.name, "TestServer");

    // Test loading default config for non-existent file
    let non_existent_path = temp_dir.path().join("non_existent.yaml");
    let default_config = Config::from_file(&non_existent_path)?;

    // Verify default config was created
    assert!(non_existent_path.exists());
    assert_eq!(default_config.visualization.port, 8080);
    assert_eq!(default_config.visualization.address, "127.0.0.1");

    // Test apply_args method
    let mut config = Config::default();
    assert_eq!(config.visualization.port, 8080);
    assert_eq!(config.visualization.address, "127.0.0.1");
    assert_eq!(
        config.visualization.hmac_secret,
        "my-super-secret-jwt-key-for-photoacoustic-app"
    );

    // Apply command-line arguments without HMAC secret
    config.apply_args(
        Some(9000),
        Some("192.168.0.1".to_string()),
        None,
        true,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    // Verify values were overridden
    assert_eq!(config.visualization.port, 9000);
    assert_eq!(config.visualization.address, "192.168.0.1");
    assert_eq!(
        config.visualization.hmac_secret,
        "my-super-secret-jwt-key-for-photoacoustic-app"
    );

    // Apply command-line arguments with HMAC secret
    config.apply_args(
        Some(9000),
        Some("192.168.0.1".to_string()),
        Some("new-secret-key".to_string()),
        true,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    // Verify HMAC secret was overridden
    assert_eq!(config.visualization.hmac_secret, "new-secret-key");

    Ok(())
}

#[test]
fn test_yaml_validation() -> Result<()> {
    // Create a temporary directory for test files
    let temp_dir = tempdir()?;

    // Test 1: Valid YAML configuration with IPv4
    let valid_ipv4_path = temp_dir.path().join("valid_ipv4_config.yaml");
    let valid_ipv4_yaml = r#"
# Photoacoustic Water Vapor Analyzer Configuration with IPv4
visualization:
  port: 8080
  address: "127.0.0.1"
  name: "TestServer"
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&valid_ipv4_path, valid_ipv4_yaml)?;

    // This should parse successfully
    let result = Config::from_file(&valid_ipv4_path);
    assert!(
        result.is_ok(),
        "Valid YAML with IPv4 should pass validation"
    );

    // Test 1b: Valid YAML configuration with standard IPv6
    let valid_ipv6_path = temp_dir.path().join("valid_ipv6_config.yaml");
    let valid_ipv6_yaml = r#"
# Photoacoustic Water Vapor Analyzer Configuration with IPv6
visualization:
  port: 8080
  address: "2001:db8::1"
  name: "TestServer"
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&valid_ipv6_path, valid_ipv6_yaml)?;

    // This should parse successfully
    let result = Config::from_file(&valid_ipv6_path);
    assert!(
        result.is_ok(),
        "Valid YAML with standard IPv6 should pass validation"
    );

    // Test 1c: Valid YAML configuration with IPv6 localhost
    let ipv6_localhost_path = temp_dir.path().join("ipv6_localhost_config.yaml");
    let ipv6_localhost_yaml = r#"
# Photoacoustic Water Vapor Analyzer Configuration with IPv6 localhost
visualization:
  port: 8080
  address: "::1"
  name: "TestServer"
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&ipv6_localhost_path, ipv6_localhost_yaml)?;

    // This should parse successfully
    let result = Config::from_file(&ipv6_localhost_path);
    assert!(
        result.is_ok(),
        "Valid YAML with IPv6 localhost should pass validation"
    );

    // Test 1d: Valid YAML configuration with IPv6 any address
    let ipv6_any_path = temp_dir.path().join("ipv6_any_config.yaml");
    let ipv6_any_yaml = r#"
# Photoacoustic Water Vapor Analyzer Configuration with IPv6 any address
visualization:
  port: 8080
  address: "::"
  name: "TestServer"
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&ipv6_any_path, ipv6_any_yaml)?;

    // This should parse successfully
    let result = Config::from_file(&ipv6_any_path);
    assert!(
        result.is_ok(),
        "Valid YAML with IPv6 any address should pass validation"
    );

    // Test 1e: Valid YAML configuration with full IPv6 address
    let full_ipv6_path = temp_dir.path().join("full_ipv6_config.yaml");
    let full_ipv6_yaml = r#"
# Photoacoustic Water Vapor Analyzer Configuration with full IPv6
visualization:
  port: 8080
  address: "2001:0db8:85a3:0000:0000:8a2e:0370:7334"
  name: "TestServer"
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&full_ipv6_path, full_ipv6_yaml)?;

    // This should parse successfully
    let result = Config::from_file(&full_ipv6_path);
    assert!(
        result.is_ok(),
        "Valid YAML with full IPv6 should pass validation"
    );

    // Test 1f: Invalid YAML configuration with bracketed IPv6 address for port bindings (draft 2020-12)
    let bracketed_ipv6_path = temp_dir.path().join("bracketed_ipv6_config.yaml");
    let bracketed_ipv6_yaml = r#"
# Photoacoustic Water Vapor Analyzer Configuration with bracketed IPv6
visualization:
  port: 8080
  address: "[::0]"
  name: "TestServer"
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&bracketed_ipv6_path, bracketed_ipv6_yaml)?;

    // This should parse successfully
    let result = Config::from_file(&bracketed_ipv6_path);
    assert!(
        result.is_err(),
        "Valid YAML with bracketed IPv6 should not pass validation"
    );

    // Test 2: Invalid YAML - missing required field
    let missing_field_path = temp_dir.path().join("missing_field.yaml");
    let missing_field_yaml = r#"
# Missing required name field
visualization:
  port: 8080
  address: "127.0.0.1"
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&missing_field_path, missing_field_yaml)?;

    // This should fail validation
    let result = Config::from_file(&missing_field_path);
    assert!(
        result.is_err(),
        "YAML missing required field should fail validation"
    );

    // Test 3: Invalid YAML - incorrectly named field
    let wrong_field_path = temp_dir.path().join("wrong_field.yaml");
    let wrong_field_yaml = r#"
# Field "wrong_port" instead of "port"
visualization:
  wrong_port: 8080
  address: "127.0.0.1"
  name: "TestServer"
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&wrong_field_path, wrong_field_yaml)?;

    // This should fail validation
    let result = Config::from_file(&wrong_field_path);
    assert!(
        result.is_err(),
        "YAML with incorrect field names should fail validation"
    );

    // Test 4: Invalid YAML - value outside allowed range
    let invalid_range_path = temp_dir.path().join("invalid_range.yaml");
    let invalid_range_yaml = r#"
# Port outside allowed range (1-65535)
visualization:
  port: 70000
  address: "127.0.0.1"
  name: "TestServer"
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&invalid_range_path, invalid_range_yaml)?;

    // This should fail validation
    let result = Config::from_file(&invalid_range_path);
    assert!(
        result.is_err(),
        "YAML with value outside allowed range should fail validation"
    );

    // Test 5: Invalid YAML - invalid address format
    let invalid_address_path = temp_dir.path().join("invalid_address.yaml");
    let invalid_address_yaml = r#"
# Invalid IP address format
visualization:
  port: 8080
  address: "not-an-ip-address"
  name: "TestServer"
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&invalid_address_path, invalid_address_yaml)?;

    // This should issue a warning but still pass (based on your current implementation)
    let result = Config::from_file(&invalid_address_path);
    assert!(result.is_err(), "YAML with invalid address should not pass");

    // Test 6: Invalid YAML - malformed certificate
    let invalid_cert_path = temp_dir.path().join("invalid_cert.yaml");
    let invalid_cert_yaml = r#"
# Invalid base64 in certificate
visualization:
  port: 8080
  address: "127.0.0.1"
  name: "TestServer"
  cert: "this-is-not-valid-base64!"
  key: "SGVsbG8gV29ybGQ="
  hmac_secret: "test-secret"
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&invalid_cert_path, invalid_cert_yaml)?;

    // This should fail validation
    let result = Config::from_file(&invalid_cert_path);
    assert!(
        result.is_err(),
        "YAML with invalid base64 certificate should fail validation"
    );

    // Test 7: Invalid YAML - certificate without key
    let missing_key_path = temp_dir.path().join("missing_key.yaml");
    let missing_key_yaml = r#"
# Certificate provided without key
visualization:
  port: 8080
  address: "127.0.0.1"
  name: "TestServer"
  cert: "SGVsbG8gV29ybGQ="
  key: null
  hmac_secret: "test-secret"
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&missing_key_path, missing_key_yaml)?;

    // This should fail validation with a specific error message
    let result = Config::from_file(&missing_key_path);
    assert!(
        result.is_err(),
        "YAML with certificate but no key should fail validation"
    );
    if let Err(e) = result {
        assert!(
            e.to_string()
                .contains("SSL certificate provided without a key"),
            "Expected error about missing key, got: {}",
            e
        );
    }

    // Test 8: Invalid IPv6 format
    let invalid_ipv6_path = temp_dir.path().join("invalid_ipv6.yaml");
    let invalid_ipv6_yaml = r#"
# Invalid IPv6 format (too many segments)
visualization:
  port: 8080
  address: "2001:0db8:85a3:0000:0000:8a2e:0370:7334:5678"
  name: "TestServer"
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
"#;
    fs::write(&invalid_ipv6_path, invalid_ipv6_yaml)?;

    // This should fail validation since the IPv6 address is invalid
    let result = Config::from_file(&invalid_ipv6_path);
    assert!(
        result.is_err(),
        "YAML with invalid IPv6 format should fail validation"
    );

    Ok(())
}
#[test]
fn test_invalid_permission_separator() -> Result<()> {
    // Create a temporary directory
    let temp_dir = tempdir()?;
    let invalid_permission_path = temp_dir.path().join("invalid_permission.yaml");

    // Create a configuration with a user permission containing the USER_SESSION_SEPARATOR character
    let invalid_permission_yaml = format!(
        r#"
# Configuration with invalid permission containing the session separator character
visualization:
  port: 8080
  address: "127.0.0.1"
  name: "TestServer"
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
  enabled: true
  session_secret: /qCJ7RyQIugza05wgFNN6R+c2/afrKlG5jJfZ0oQPis=
access:
  - user: "admin"
    pass: "JDUkM2E2OUZwQW0xejZBbWV2QSRvMlhhN0lxcVdVU1VPTUh6UVJiM3JjRlRhZy9WYjdpSWJtZUJFaXA3Y1ZECg=="
    permissions: 
      - "read:api"
      - "write{}api"
      - "admin:api"
"#,
        USER_SESSION_SEPARATOR
    );

    fs::write(&invalid_permission_path, invalid_permission_yaml)?;

    // Attempt to load the configuration - this should fail with a specific error
    let result = Config::from_file(&invalid_permission_path);

    // Verify that the validation failed with the expected error message
    assert!(
        result.is_err(),
        "Config with invalid permission separator should fail validation"
    );

    Ok(())
}
