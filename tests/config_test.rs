use anyhow::Result;
use rust_photoacoustic::config::{Config, VisualizationConfig};
use std::fs;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_config_load_and_save() -> Result<()> {
    // Create a temporary directory
    let temp_dir = tempdir()?;
    let config_path = temp_dir.path().join("config.yaml");

    // Create a custom config
    let config = Config {
        visualization: VisualizationConfig {
            port: 8081,
            address: "192.168.1.1".to_string(),
            name: "TestServer".to_string(),
            cert: None,
            key: None,
        },
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

    // Apply command-line arguments
    config.apply_args(9000, "192.168.0.1".to_string());

    // Verify values were overridden
    assert_eq!(config.visualization.port, 9000);
    assert_eq!(config.visualization.address, "192.168.0.1");

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
"#;
    fs::write(&bracketed_ipv6_path, bracketed_ipv6_yaml)?;

    // This should parse successfully
    let result = Config::from_file(&bracketed_ipv6_path);
    assert!(
        result.is_err(),
        "Valid YAML with bracketed IPv6 should not pass validation"
    );

    // Continue with existing tests...
    // Test 2: Invalid YAML - missing required field
    let missing_field_path = temp_dir.path().join("missing_field.yaml");
    let missing_field_yaml = r#"
# Missing required name field
visualization:
  port: 8080
  address: "127.0.0.1"
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
"#;
    fs::write(&missing_key_path, missing_key_yaml)?;

    // This should fail validation
    let result = Config::from_file(&missing_key_path);
    assert!(
        result.is_err(),
        "YAML with certificate but no key should fail validation"
    );

    // Test 8: Invalid IPv6 format (currently not detected by jsonschema v0.30.0)
    let invalid_ipv6_path = temp_dir.path().join("invalid_ipv6.yaml");
    let invalid_ipv6_yaml = r#"
# Invalid IPv6 format (too many segments)
visualization:
  port: 8080
  address: "2001:0db8:85a3:0000:0000:8a2e:0370:7334:5678"
  name: "TestServer"
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
