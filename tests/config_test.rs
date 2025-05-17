use anyhow::Result;
use rust_photoacoustic::config::{Config, VisualizationConfig};
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
fn test_config_validation() -> Result<()> {
    // Valid config
    let valid_config = Config {
        visualization: VisualizationConfig {
            port: 8080,
            address: "127.0.0.1".to_string(),
            name: "TestServer".to_string(),
            cert: None,
            key: None,
        },
    };
    assert!(valid_config.validate().is_ok());

    // Invalid port (outside allowed range)
    let invalid_port_config = Config {
        visualization: VisualizationConfig {
            port: -1, // Invalid port (schema specifies minimum as 1)
            address: "127.0.0.1".to_string(),
            name: "TestServer".to_string(),
            cert: None,
            key: None,
        },
    };
    assert!(invalid_port_config.validate().is_err());

    // Invalid certificate configuration (cert without key)
    let invalid_cert_config = Config {
        visualization: VisualizationConfig {
            port: 8080,
            address: "127.0.0.1".to_string(),
            name: "TestServer".to_string(),
            cert: Some("SGVsbG8gV29ybGQ=".to_string()), // Base64 "Hello World"
            key: None,
        },
    };
    assert!(invalid_cert_config.validate().is_err());

    Ok(())
}
