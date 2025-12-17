// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
// Tool to remove "auto-initialize" feature from pyo3 dependencies in Cargo.toml
// auto-initialize feature should not be used while using a static libpython build

use std::env;
use std::fs;
use std::process;

/// Recursively removes the "auto-initialize" feature from a pyo3 dependency in a TOML value.
///
/// # Arguments
/// * `value` - A mutable reference to a TOML value (typically a pyo3 dependency table)
///
/// # Returns
/// `true` if the feature was found and removed, `false` otherwise
///
/// # Example
/// This function handles pyo3 entries like:
/// ```toml
/// pyo3 = { version = "0.27.2", optional = true, features = ["auto-initialize", "extension-module"] }
/// ```
fn remove_auto_initialize_from_value(value: &mut toml::Value) -> bool {
    if let toml::Value::Table(table) = value {
        if let Some(features_value) = table.get_mut("features") {
            if let toml::Value::Array(features) = features_value {
                let original_len = features.len();
                features.retain(|item| {
                    if let toml::Value::String(s) = item {
                        s != "auto-initialize"
                    } else {
                        true
                    }
                });
                return features.len() < original_len;
            }
        }
    }
    false
}

/// Main entry point for the `remove-auto-init` tool.
///
/// This utility removes the "auto-initialize" feature from pyo3 dependencies in a Cargo.toml file.
/// It handles multiple scenarios:
/// - Single-line feature declarations: `features = ["auto-initialize"]`
/// - Multi-line feature arrays with indentation
/// - Features with or without spacing
/// - Dependencies in both `[dependencies]` and `[workspace.dependencies]` sections
///
/// # Arguments
/// The program expects exactly one command-line argument: the path to the Cargo.toml file to modify.
///
/// # Example
/// ```bash
/// remove-auto-init ./Cargo.toml
/// ```
///
/// # Exit Codes
/// - 0: Successfully processed the file
/// - 1: Error reading/writing file or invalid arguments
fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <path_to_cargo_toml>", args[0]);
        process::exit(1);
    }

    let cargo_toml_path = &args[1];

    // Read the Cargo.toml file
    let content = match fs::read_to_string(cargo_toml_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {}: {}", cargo_toml_path, e);
            process::exit(1);
        }
    };

    // Parse the TOML using native TOML parser for robust handling of all format variations
    let mut doc: toml::Value = match toml::from_str(&content) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error parsing TOML: {}", e);
            process::exit(1);
        }
    };

    let mut modified = false;

    // Check [workspace.dependencies] section for pyo3 with auto-initialize feature
    println!("DEBUG: Checking [workspace.dependencies]...");
    if let Some(workspace) = doc.get_mut("workspace") {
        if let Some(dependencies) = workspace.get_mut("dependencies") {
            if let Some(pyo3) = dependencies.get_mut("pyo3") {
                println!("✓ Found pyo3 in [workspace.dependencies]");
                if remove_auto_initialize_from_value(pyo3) {
                    println!("✓ Removed 'auto-initialize' from [workspace.dependencies].pyo3");
                    modified = true;
                }
            }
        }
    }

    // Check [dependencies] section for pyo3 with auto-initialize feature
    println!("DEBUG: Checking [dependencies]...");
    if let Some(dependencies) = doc.get_mut("dependencies") {
        if let Some(pyo3) = dependencies.get_mut("pyo3") {
            println!("✓ Found pyo3 in [dependencies]");
            if remove_auto_initialize_from_value(pyo3) {
                println!("✓ Removed 'auto-initialize' from [dependencies].pyo3");
                modified = true;
            }
        }
    }

    if !modified {
        println!("⚠ WARNING: 'auto-initialize' feature was not found or already removed");
    }

    // Serialize the modified TOML document back to string and write to file
    let output = toml::to_string_pretty(&doc).expect("Failed to serialize TOML");
    if let Err(e) = fs::write(cargo_toml_path, output) {
        eprintln!("Error writing {}: {}", cargo_toml_path, e);
        process::exit(1);
    }

    println!("✓ Successfully updated {}", cargo_toml_path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn test_remove_feature(input: &str, expected_contains: &str, expected_not_contains: &str) {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_Cargo.toml");

        // Write test file
        fs::write(&file_path, input).unwrap();

        // Parse and modify
        let content = fs::read_to_string(&file_path).unwrap();
        let mut doc: toml::Value = toml::from_str(&content).unwrap();

        // Remove from dependencies
        if let Some(dependencies) = doc.get_mut("dependencies") {
            if let Some(pyo3) = dependencies.get_mut("pyo3") {
                remove_auto_initialize_from_value(pyo3);
            }
        }

        let output = toml::to_string_pretty(&doc).unwrap();
        println!("Output:\n{}", output);

        assert!(
            output.contains(expected_contains),
            "Output should contain: {}",
            expected_contains
        );
        assert!(
            !output.contains(expected_not_contains),
            "Output should NOT contain: {}",
            expected_not_contains
        );
    }

    /// Test helper function that creates a temporary Cargo.toml, modifies it, and verifies the output.
    ///
    /// # Arguments
    /// * `input` - The TOML content to test
    /// * `expected_contains` - String that must be present in the output
    /// * `expected_not_contains` - String that must NOT be present in the output

    #[test]
    fn test_multiline_features_array() {
        let input = r#"
[dependencies]
pyo3 = { version = "0.27.2", optional = true, features = [
    "auto-initialize",
], default-features = false }
"#;
        test_remove_feature(input, "optional = true", "auto-initialize");
    }

    /// Test removing auto-initialize from a multi-line features array with proper indentation.

    #[test]
    fn test_single_line_features() {
        let input = r#"
[dependencies]
pyo3 = { version = "0.27.2", optional = true, features = ["auto-initialize"], default-features = false }
"#;
        test_remove_feature(input, "optional = true", "auto-initialize");
    }

    /// Test removing auto-initialize from a single-line features array.

    #[test]
    fn test_features_no_spaces() {
        let input = r#"
[dependencies]
pyo3={version="0.27.2",optional=true,features=["auto-initialize"],default-features=false}
"#;
        test_remove_feature(input, "optional", "auto-initialize");
    }

    /// Test removing auto-initialize from a features array with no extra spacing.

    #[test]
    fn test_no_auto_initialize_feature() {
        let input = r#"
[dependencies]
pyo3 = { version = "0.27.2", optional = true, features = [
    "extension-module",
], default-features = false }
"#;
        // Should not contain auto-initialize
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_Cargo.toml");
        fs::write(&file_path, input).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        let mut doc: toml::Value = toml::from_str(&content).unwrap();

        if let Some(dependencies) = doc.get_mut("dependencies") {
            if let Some(pyo3) = dependencies.get_mut("pyo3") {
                remove_auto_initialize_from_value(pyo3);
            }
        }

        let output = toml::to_string_pretty(&doc).unwrap();
        assert!(output.contains("extension-module"));
        assert!(!output.contains("auto-initialize"));
    }

    /// Test that files without the auto-initialize feature are left unchanged.

    #[test]
    fn test_multiple_features() {
        let input = r#"
[dependencies]
pyo3 = { version = "0.27.2", optional = true, features = [
    "auto-initialize",
    "extension-module",
    "abi3",
], default-features = false }
"#;
        test_remove_feature(input, "extension-module", "auto-initialize");
    }
}
