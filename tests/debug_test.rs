mod debug_test {
    #[test]
    fn test_debug_cert_without_key() -> anyhow::Result<()> {
        use rust_photoacoustic::config::Config;
        use std::fs;
        use tempfile::tempdir;

        // Create a temporary directory
        let temp_dir = tempdir()?;
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
"#;
        fs::write(&missing_key_path, missing_key_yaml)?;

        // This should fail validation with a specific error message
        println!("Missing key path: {:?}", missing_key_path);
        let result = Config::from_file(&missing_key_path);

        assert!(
            result.is_err(),
            "YAML with certificate but no key should fail validation"
        );

        match &result {
            Ok(_) => println!("Result: OK (unexpected)"),
            Err(e) => {
                println!("Result: Err with message: {}", e);
                assert!(
                    e.to_string()
                        .contains("SSL certificate provided without a key"),
                    "Expected error about missing key, got: {}",
                    e
                );
            }
        }

        Ok(())
    }
}
