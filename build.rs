// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::str;
use std::time::{SystemTime, UNIX_EPOCH};

// Checks if any web source files are newer than the compiled files
fn is_web_source_newer_than_dist(dist_path: &PathBuf) -> bool {
    // Get the most recent modification date of files in dist
    let dist_latest_mod = get_latest_modification_time(dist_path).unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 86400
    });

    // Get the most recent modification date of source files
    let src_paths = [
        PathBuf::from("./web/src"),
        PathBuf::from("./web/public"),
        PathBuf::from("./web/index.html"),
        PathBuf::from("./web/package.json"),
        PathBuf::from("./web/tsconfig.json"),
        PathBuf::from("./web/vite.config.ts"),
        // Add other files/directories to watch as needed
    ];

    for path in &src_paths {
        if let Some(mod_time) = get_latest_modification_time(path) {
            if mod_time > dist_latest_mod {
                println!("cargo:warning=Modified file detected: {:?}", path);
                return true;
            }
        }
    }

    false
}

// Gets the most recent modification date in a directory (recursively)
// or for a single file
fn get_latest_modification_time(path: &PathBuf) -> Option<u64> {
    if !path.exists() {
        return None;
    }

    let mut latest = 0;

    if path.is_file() {
        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(since_epoch) = modified.duration_since(UNIX_EPOCH) {
                    return Some(since_epoch.as_secs());
                }
            }
        }
        return None;
    }

    // Recursive function to walk through directories
    fn visit_dir(dir: &PathBuf, latest: &mut u64) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(metadata) = fs::metadata(&path) {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(since_epoch) = modified.duration_since(UNIX_EPOCH) {
                                let secs = since_epoch.as_secs();
                                if secs > *latest {
                                    *latest = secs;
                                }
                            }
                        }
                    }
                } else if path.is_dir() {
                    visit_dir(&path, latest);
                }
            }
        }
    }

    visit_dir(path, &mut latest);

    if latest > 0 {
        Some(latest)
    } else {
        None
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageJson {
    name: String,
    private: Option<bool>,
    version: String,
    #[serde(rename = "type")]
    type_: Option<String>,
    scripts: HashMap<String, String>,
    dependencies: HashMap<String, String>,
    #[serde(rename = "devDependencies")]
    dev_dependencies: HashMap<String, String>,
}

impl PackageJson {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            private: None,
            version: String::new(),
            type_: None,
            scripts: HashMap::new(),
            dependencies: HashMap::new(),
            dev_dependencies: HashMap::new(),
        }
    }

    pub fn set_version(&mut self, version: &str) {
        self.version = version.to_string();
    }
}

// Implement a Default trait for PackageJson providing a default npm package.json
impl Default for PackageJson {
    fn default() -> Self {
        Self {
            name: "webconsole".to_string(),
            private: Some(true),
            version: "0.0.0".to_string(),
            type_: Some("module".to_string()),
            scripts: HashMap::new(),
            dependencies: HashMap::new(),
            dev_dependencies: HashMap::new(),
        }
    }
}

// We need to compile the utility module directly in the build script
// to be able to use it during compilation
mod certificate_utils {
    use anyhow::{Context, Result};
    use rcgen::{
        CertificateParams, DnType, DnValue, Ia5String, IsCa, KeyPair, KeyUsagePurpose, SanType,
    };
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;

    pub fn create_self_signed_cert(
        days: u32,
        cert_path: &str,
        key_path: &str,
        common_name: &str,
        alt_names: Option<Vec<String>>,
    ) -> Result<()> {
        // Create directory if it doesn't exist
        if let Some(parent) = Path::new(cert_path).parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = Path::new(key_path).parent() {
            fs::create_dir_all(parent)?;
        }

        // Set up certificate parameters
        let mut params = CertificateParams::new(vec![String::from(common_name)])?;
        params.not_before = time::OffsetDateTime::now_utc();
        params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(days as i64);
        params
            .distinguished_name
            .push(DnType::CommonName, DnValue::from(common_name));

        // Add Subject Alternative Names if provided
        if let Some(names) = alt_names {
            for name in names {
                if name.parse::<std::net::IpAddr>().is_ok() {
                    params
                        .subject_alt_names
                        .push(SanType::IpAddress(name.parse().unwrap()));
                } else {
                    params
                        .subject_alt_names
                        .push(SanType::DnsName(Ia5String::try_from(name).unwrap()));
                }
            }
        } else {
            // Default SAN entries
            params
                .subject_alt_names
                .push(SanType::DnsName(Ia5String::try_from("localhost").unwrap()));
            params
                .subject_alt_names
                .push(SanType::IpAddress("127.0.0.1".parse().unwrap()));
            params
                .subject_alt_names
                .push(SanType::IpAddress("::1".parse().unwrap()));
        }

        // Set to not be a CA certificate
        params.is_ca = IsCa::NoCa;

        // Set key usage
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];

        let key_pair = KeyPair::generate().unwrap();
        let cert = params
            .self_signed(&key_pair)
            .context("Failed to generate certificate")?;

        // Get the certificate and private key in PEM format
        let cert_pem = cert.pem();
        let key_pem = key_pair.serialize_pem();

        // Write certificate to file
        let mut cert_file = File::create(cert_path).context("Failed to create certificate file")?;
        cert_file
            .write_all(cert_pem.as_bytes())
            .context("Failed to write certificate to file")?;

        // Write private key to file
        let mut key_file = File::create(key_path).context("Failed to create key file")?;
        key_file
            .write_all(key_pem.as_bytes())
            .context("Failed to write key to file")?;

        Ok(())
    }
}

// Function to create a self-signed certificate if it doesn't exist
fn create_certificate_files_if_needed() -> Result<()> {
    let cert_path = "resources/cert.pem";
    let key_path = "resources/cert.key";

    // Check if both certificate files already exist
    let cert_exists = std::path::Path::new(cert_path).exists();
    let key_exists = std::path::Path::new(key_path).exists();

    if cert_exists && key_exists {
        println!("cargo:warning=Certificate and key files already exist, skipping generation");
        return Ok(());
    }

    println!("cargo:warning=Generating self-signed certificate for development");

    // Create resources directory if it doesn't exist
    let resources_dir = std::path::Path::new("resources");
    if !resources_dir.exists() {
        std::fs::create_dir_all(resources_dir)?;
    }

    // Use the utility function to create the certificates
    certificate_utils::create_self_signed_cert(
        365, // Valid for 365 days
        cert_path,
        key_path,
        "rust-photoacoustic.local",
        Some(vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "::1".to_string(),
        ]),
    )?;

    println!("cargo:warning=Self-signed certificate and key generated successfully");
    Ok(())
}

#[tokio::main]
async fn main() {
    // Tells Cargo to rerun build.rs if any files in the web folder change or certificate files change
    println!("cargo:rerun-if-changed=web");
    println!("cargo:rerun-if-changed=resources/cert.pem");
    println!("cargo:rerun-if-changed=resources/cert.key");

    // Generate certificate files if they don't exist
    if let Err(e) = create_certificate_files_if_needed() {
        println!("cargo:warning=Failed to generate certificate files: {}", e);
    }

    // Checks if dist files already exist to avoid unnecessary rebuilds
    let dist_path = PathBuf::from("./web/dist");
    let needs_build = !dist_path.exists() || is_web_source_newer_than_dist(&dist_path);

    let data = fs::read_to_string("./web/package.json").unwrap();
    let mut package: PackageJson = serde_json::from_str(&data).unwrap();

    // Build the path to the file in the temporary directory
    let tmp_dir = env::var("TMP")
        .or_else(|_| env::var("TEMP"))
        .or_else(|_| env::var("TMPDIR"))
        .unwrap_or_else(|_| "/tmp".to_string());
    let mut path = PathBuf::from(tmp_dir);
    path.push("version-1B282C00-C9CC-4C5F-890E-952D88623718.txt");
    // Read the version from the file
    let version =
        fs::read_to_string(&path).unwrap_or_else(|_| env::var("CARGO_PKG_VERSION").unwrap());

    // Check if the version has changed
    let version_changed = package.version != version;
    if version_changed {
        package.set_version(&version);
        let serialized = serde_json::to_string_pretty(&package).unwrap();
        fs::write("./web/package.json", serialized).unwrap();
    }

    // If no rebuild is needed, exit early
    if !needs_build && !version_changed {
        println!("cargo:warning=No changes detected in web files, skipping vite build");
        return;
    }

    let is_windows = cfg!(target_os = "windows");

    let (command, install_args, build_args) = if is_windows {
        (
            "cmd.exe",
            &["/C", "npm install --force"],
            &["/C", "npm run build"],
        )
    } else {
        ("npm", &["install", "--force"], &["run", "build"])
    };

    // Install npm dependencies for webconsole
    let output = Command::new(command)
        .current_dir("web")
        .args(install_args)
        .output()
        .expect("Failed to execute command");
    assert!(
        output.status.success(),
        "Failed to install npm dependencies: {}{}",
        str::from_utf8(&output.stdout).unwrap_or(""),
        str::from_utf8(&output.stderr).unwrap_or("")
    );

    // Build webconsole
    let output = Command::new(command)
        .current_dir("web")
        .args(build_args)
        .output()
        .expect("Failed to execute command");
    assert!(
        output.status.success(),
        "Failed to build web: {}{}",
        str::from_utf8(&output.stdout).unwrap_or(""),
        str::from_utf8(&output.stderr).unwrap_or("")
    );

    // // Install npm dependencies for rapidoc
    //     let output = Command::new(command)
    //     .current_dir("rapidoc")
    //     .args(install_args)
    //     .output()
    //     .expect("Failed to execute command");
    // assert!(
    //     output.status.success(),
    //     "Failed to install npm dependencies: {}{}",
    //     str::from_utf8(&output.stdout).unwrap_or(""),
    //     str::from_utf8(&output.stderr).unwrap_or("")
    // );

    // // Build rapidoc
    // let output = Command::new(command)
    //     .current_dir("rapidoc")
    //     .args(build_args)
    //     .output()
    //     .expect("Failed to execute command");
    // assert!(
    //     output.status.success(),
    //     "Failed to build rapidoc: {}{}",
    //     str::from_utf8(&output.stdout).unwrap_or(""),
    //     str::from_utf8(&output.stderr).unwrap_or("")
    // );
}
