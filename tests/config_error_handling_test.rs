use anyhow::Result;
use rust_photoacoustic::config::Config;
use std::fs;
use std::path::Path;
use tempfile::tempdir;
use std::sync::Once;
use env_logger;

static INIT: Once = Once::new();

// Setup logger for tests
fn setup() {
    INIT.call_once(|| {
        env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .init();
    });
}

#[test]
fn test_config_deserialization_error_creates_sample_file() -> Result<()> {
    // Initialize the logger
    setup();
    println!("Starting deserialization error test");
    // Create a temporary directory for the test
    let temp_dir = tempdir()?;
    let config_path = temp_dir.path().join("config.yaml");

    // Create an invalid config file for deserialization (valid YAML but wrong structure)
    let invalid_yaml = r#"
visualization:
  port: "not-an-integer"  # Integer field with string value (type mismatch)
  address: 12345          # String field with number value
  enabled: "true"         # Boolean field with string value
  hmac_secret: 12345      # String field with number value
  rs256_private_key: []   # Array instead of string
  rs256_public_key: {}    # Object instead of string
"#;

    // Write the invalid config to the file
    fs::write(&config_path, invalid_yaml)?;

    // Try to load the config, which should fail but create a sample file
    let result = Config::from_file(&config_path);
    
    // Assert loading failed
    assert!(result.is_err(), "Config loading should have failed");
    
    // Assert sample file was created
    let sample_path = config_path.with_extension("sample.yaml");
    assert!(Path::new(&sample_path).exists(), "Sample config file was not created");

    // Load and verify the sample file is valid
    let sample_config = Config::from_file(&sample_path)?;
    assert_eq!(sample_config.visualization.port, 8080); // Default value

    Ok(())
}

#[test]
fn test_config_validation_error_creates_sample_file() -> Result<()> {
    // Create a temporary directory for the test
    let temp_dir = tempdir()?;
    let config_path = temp_dir.path().join("config.yaml");

    // Create a config with valid YAML but invalid value (port out of range)
    let invalid_config = r#"
visualization:
  port: 99999  # Port out of range (valid range is 1-65534)
  address: "127.0.0.1"
  enabled: true
  hmac_secret: "test-secret"
  rs256_private_key: "valid-key-format"
  rs256_public_key: "valid-key-format"
"#;

    // Write the invalid config to the file
    fs::write(&config_path, invalid_config)?;

    // Try to load the config, which should fail but create a sample file
    let result = Config::from_file(&config_path);
    
    // Assert loading failed
    assert!(result.is_err(), "Config loading should have failed");
    
    // Assert sample file was created
    let sample_path = config_path.with_extension("sample.yaml");
    assert!(Path::new(&sample_path).exists(), "Sample config file was not created");

    // Load and verify the sample file is valid
    let sample_config = Config::from_file(&sample_path)?;
    assert_eq!(sample_config.visualization.port, 8080); // Default value

    Ok(())
}
