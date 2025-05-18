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
  rs256_private_key: LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQpNSUdyQWdFQUFpRUFyd0FZcXAvdGVvaUE4N2FWQStJTjQ1U1RvMTdMUVZPbGRUT3FJeHhQeElNQ0F3RUFBUUlnCldRVlpodUpYOGE4aXVkYzFNb1o1dldYcmxwdFlEUTQ3RXUwa2pNVVA2T0VDRVFEUjEyN0RsZWNKNU80V3B2VEcKdnQ1YkFoRUExWDZ4ZVVzUXpISkZTYlV4eXZEYStRSVJBS2Z1b05ZbHdTQko5Y0JySExseFJzRUNFUURCcTBOZApqNTMyaUxhWURablV5amNwQWhCSG9CU1JSTS9ESVA5dWE1MDhYMEtOCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tCg==
  rs256_public_key: LS0tLS1CRUdJTiBSU0EgUFVCTElDIEtFWS0tLS0tCk1DZ0NJUUN2QUJpcW4rMTZpSUR6dHBVRDRnM2psSk9qWHN0QlU2VjFNNm9qSEUvRWd3SURBUUFCCi0tLS0tRU5EIFJTQSBQVUJMSUMgS0VZLS0tLS0K
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
