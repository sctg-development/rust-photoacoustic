// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use anyhow::Context;
use anyhow::Result;
use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey};
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::str;
use std::time::{SystemTime, UNIX_EPOCH};

const RS256_KEY_LENGTH: usize = 4096; // Default key length for RS256
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
        PathBuf::from("../web/src"),
        PathBuf::from("../web/public"),
        PathBuf::from("../web/index.html"),
        PathBuf::from("../web/package.json"),
        PathBuf::from("../web/tsconfig.json"),
        PathBuf::from("../web/vite.config.ts"),
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

fn create_rs256_key_pair_if_needed() -> Result<()> {
    let pub_key_path = "resources/pub.key";
    let priv_key_path = "resources/private.key";

    // Check if both key files already exist
    let pub_key_exists = std::path::Path::new(pub_key_path).exists();
    let priv_key_exists = std::path::Path::new(priv_key_path).exists();

    if pub_key_exists && priv_key_exists {
        println!("cargo:warning=RS256 key pair files already exist, skipping generation");
        return Ok(());
    }

    println!("cargo:warning=Generating RS256 key pair for JWT signing");
    let mut rng = rsa::rand_core::OsRng;

    // Generate a new random RSA key pair with the specified bits
    let private_key = RsaPrivateKey::new(&mut rng, RS256_KEY_LENGTH)
        .context("Failed to generate RSA private key")?;
    let public_key = RsaPublicKey::from(&private_key);

    // Create resources directory if it doesn't exist
    let resources_dir = std::path::Path::new("resources");
    if !resources_dir.exists() {
        std::fs::create_dir_all(resources_dir)?;
    }
    // Convert keys to PKCS#1 PEM format
    let private_pem = private_key
        .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
        .context("Failed to encode private key to PEM")?;
    let public_pem = public_key
        .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
        .context("Failed to encode public key to PEM")?;

    // Write private key to file
    let mut private_file = File::create(priv_key_path)
        .with_context(|| format!("Failed to create private key file at {:?}", priv_key_path))?;
    private_file
        .write_all(private_pem.as_bytes())
        .context("Failed to write private key to file")?;

    // Write public key to file
    let mut public_file = File::create(pub_key_path)
        .with_context(|| format!("Failed to create public key file at {:?}", pub_key_path))?;
    public_file
        .write_all(public_pem.as_bytes())
        .context("Failed to write public key to file")?;

    println!("cargo:warning=RS256 key pair generated successfully");
    Ok(())
}

/// Build a Node.js project at the specified path
///
/// This function handles the complete build process for a Node.js project:
/// - Installs npm dependencies
/// - Runs the build command
/// - Validates that the build completed successfully
///
/// # Arguments
///
/// * `project_path` - The path to the Node.js project directory (should contain package.json)
///
/// # Returns
///
/// * `Ok(())` if the build completed successfully
/// * `Err(anyhow::Error)` if any step of the build process failed
fn build_node_project(project_path: PathBuf) -> Result<()> {
    // Validate that the project path exists and contains package.json
    if !project_path.exists() {
        return Err(anyhow::anyhow!(
            "Project path does not exist: {}",
            project_path.display()
        ));
    }

    let package_json_path = project_path.join("package.json");
    if !package_json_path.exists() {
        return Err(anyhow::anyhow!(
            "No package.json found at: {}",
            package_json_path.display()
        ));
    }

    let is_windows = cfg!(target_os = "windows");

    println!(
        "cargo:warning=Building Node.js project at: {}",
        project_path.display()
    );

    // Platform-specific npm install
    println!("cargo:warning=Starting npm install...");
    if is_windows {
        let install_output = Command::new("cmd")
            .args(["/C", "npm install --force"])
            .current_dir(&project_path)
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute npm install in {}",
                    project_path.display()
                )
            })?;

        println!(
            "cargo:warning=npm install exit code: {:?}",
            install_output.status.code()
        );
        println!(
            "cargo:warning=npm install stdout: {}",
            str::from_utf8(&install_output.stdout).unwrap_or("")
        );
        println!(
            "cargo:warning=npm install stderr: {}",
            str::from_utf8(&install_output.stderr).unwrap_or("")
        );

        if !install_output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to install npm dependencies in {}: {}\n{}",
                project_path.display(),
                str::from_utf8(&install_output.stdout).unwrap_or(""),
                str::from_utf8(&install_output.stderr).unwrap_or("")
            ));
        }
    } else {
        let install_output = Command::new("npm")
            .args(["install", "--force"])
            .current_dir(&project_path)
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute npm install in {}",
                    project_path.display()
                )
            })?;

        println!(
            "cargo:warning=npm install exit code: {:?}",
            install_output.status.code()
        );
        println!(
            "cargo:warning=npm install stdout: {}",
            str::from_utf8(&install_output.stdout).unwrap_or("")
        );
        println!(
            "cargo:warning=npm install stderr: {}",
            str::from_utf8(&install_output.stderr).unwrap_or("")
        );

        if !install_output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to install npm dependencies in {}: {}\n{}",
                project_path.display(),
                str::from_utf8(&install_output.stdout).unwrap_or(""),
                str::from_utf8(&install_output.stderr).unwrap_or("")
            ));
        }
    }

    // Determine which build command to use by checking package.json
    let build_command = determine_build_command(&package_json_path)?;
    println!("cargo:warning=Using build command: {}", build_command);

    // Platform-specific npm build
    println!("cargo:warning=Starting npm build...");
    if is_windows {
        let build_output = Command::new("cmd")
            .args(["/C", &format!("npm run {}", build_command)])
            .current_dir(&project_path)
            .output()
            .with_context(|| {
                format!("Failed to execute npm build in {}", project_path.display())
            })?;

        println!(
            "cargo:warning=npm build exit code: {:?}",
            build_output.status.code()
        );
        println!(
            "cargo:warning=npm build stdout: {}",
            str::from_utf8(&build_output.stdout).unwrap_or("")
        );
        println!(
            "cargo:warning=npm build stderr: {}",
            str::from_utf8(&build_output.stderr).unwrap_or("")
        );

        if !build_output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to build Node.js project in {}: {}\n{}",
                project_path.display(),
                str::from_utf8(&build_output.stdout).unwrap_or(""),
                str::from_utf8(&build_output.stderr).unwrap_or("")
            ));
        }
    } else {
        let build_output = Command::new("npm")
            .args(["run", &build_command])
            .current_dir(&project_path)
            .output()
            .with_context(|| {
                format!("Failed to execute npm build in {}", project_path.display())
            })?;

        println!(
            "cargo:warning=npm build exit code: {:?}",
            build_output.status.code()
        );
        println!(
            "cargo:warning=npm build stdout: {}",
            str::from_utf8(&build_output.stdout).unwrap_or("")
        );
        println!(
            "cargo:warning=npm build stderr: {}",
            str::from_utf8(&build_output.stderr).unwrap_or("")
        );

        if !build_output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to build Node.js project in {}: {}\n{}",
                project_path.display(),
                str::from_utf8(&build_output.stdout).unwrap_or(""),
                str::from_utf8(&build_output.stderr).unwrap_or("")
            ));
        }
    }

    println!(
        "cargo:warning=Node.js project built successfully at: {}",
        project_path.display()
    );

    Ok(())
}

/// Determine the appropriate build command from package.json
///
/// Checks the scripts section of package.json to find the appropriate build command.
/// Prioritizes build:env over build over other variants.
fn determine_build_command(package_json_path: &PathBuf) -> Result<String> {
    let package_content = fs::read_to_string(package_json_path).with_context(|| {
        format!(
            "Failed to read package.json at {}",
            package_json_path.display()
        )
    })?;

    let package: PackageJson = serde_json::from_str(&package_content).with_context(|| {
        format!(
            "Failed to parse package.json at {}",
            package_json_path.display()
        )
    })?;

    // Priority order for build commands
    let build_commands = ["build", "compile", "dist", "build:env"];

    for command in &build_commands {
        if package.scripts.contains_key(*command) {
            return Ok(command.to_string());
        }
    }

    // If no standard build command is found, default to "build"
    println!(
        "cargo:warning=No standard build command found in package.json, defaulting to 'build'"
    );
    Ok("build".to_string())
}

/// Build the web console by calling the Node.js build process
fn build_web_console(version_changed: bool) -> Result<()> {
    println!("cargo:warning=build_web_console: Starting function");

    // Checks if dist files already exist to avoid unnecessary rebuilds
    let dist_path = PathBuf::from("../web/dist");
    let web_path = PathBuf::from("../web");

    println!(
        "cargo:warning=build_web_console: dist_path exists: {}",
        dist_path.exists()
    );
    println!(
        "cargo:warning=build_web_console: web_path exists: {}",
        web_path.exists()
    );
    println!(
        "cargo:warning=build_web_console: version_changed: {}",
        version_changed
    );

    let needs_build = !dist_path.exists() || is_web_source_newer_than_dist(&dist_path);
    println!(
        "cargo:warning=build_web_console: needs_build: {}",
        needs_build
    );

    // If no rebuild is needed, exit early
    if !needs_build && !version_changed {
        // Delete generix.json if it exists, as it will be generated dynamically
        delete_dist_file(&dist_path, "generix.json")?;
        println!("cargo:warning=No changes detected in web files, skipping build");
        return Ok(());
    }

    println!("cargo:warning=build_web_console: Calling build_node_project");

    // Use the new build_node_project function
    match build_node_project(web_path) {
        Ok(()) => {
            // Before ending, remove web/dist/generix.json if it exists because it will be generated dynamically
            delete_dist_file(&dist_path, "generix.json")?;
            // If the build was successful, print a success message
            println!("cargo:warning=build_web_console: build_node_project completed successfully")
        }
        Err(e) => {
            println!(
                "cargo:warning=build_web_console: build_node_project failed: {}",
                e
            );
            return Err(e);
        }
    }

    // Verify the dist folder was created
    println!("cargo:warning=build_web_console: Checking if dist directory was created");
    if !dist_path.exists() {
        return Err(anyhow::anyhow!(
            "Web build completed but dist directory was not created"
        ));
    }

    println!("cargo:warning=Web console built successfully");
    Ok(())
}

fn delete_dist_file(dist_path: &PathBuf, file: &str) -> Result<(), anyhow::Error> {
    let dist_file_path = dist_path.join(file);
    Ok(if dist_file_path.exists() {
        fs::remove_file(&dist_file_path)
            .with_context(|| format!("Failed to remove {}", dist_file_path.display()))?;
        println!(
            "cargo:warning=build_web_console: Removed existing generix.json at {}",
            dist_file_path.display()
        );
    })
}

/// Biuld the rapidoc by calling the Node.js build process
fn build_rapidoc() -> Result<()> {
    println!("cargo:warning=build_rapidoc: Starting function");
    let dist_path = PathBuf::from("./resources/rapidoc_helper/dist");
    let web_path = PathBuf::from("./resources/rapidoc_helper");

    // Check if dist files already exist to avoid unnecessary rebuilds
    if !dist_path.exists() || is_web_source_newer_than_dist(&dist_path) {
        println!("cargo:warning=build_rapidoc: Calling build_node_project");
        build_node_project(web_path)?;
    }

    // Verify the dist folder was created
    if !dist_path.exists() {
        return Err(anyhow::anyhow!(
            "Rapidoc build completed but dist directory was not created"
        ));
    }

    println!("cargo:warning=Rapidoc built successfully");
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

    // Generate RS256 key pair if it doesn't exist
    if let Err(e) = create_rs256_key_pair_if_needed() {
        println!("cargo:warning=Failed to generate RS256 key pair: {}", e);
    }

    // Process package.json to check for version changes
    let data = fs::read_to_string("../web/package.json").unwrap();
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
        fs::write("../web/package.json", serialized).unwrap();
    }

    // Build the web console
    if let Err(e) = build_web_console(version_changed) {
        println!("cargo:warning={}", e);
    }
    // Build the rapidoc helper
    if let Err(e) = build_rapidoc() {
        println!("cargo:warning={}", e);
    }
}
