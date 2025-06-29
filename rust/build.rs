// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use anyhow::Context;
use anyhow::Result;
use cargo_metadata::{MetadataCommand, Package};
use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey};
use rsa::rand_core::le;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::Deserialize;
use serde::Serialize;
use sha2::{Digest, Sha256};
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

#[derive(Debug, Serialize)]
struct PackageInfo {
    name: String,
    license: Option<String>,
    licenses: Vec<String>,
    authors: Vec<String>,
    repository: Option<String>,
    version: String,
}

/// For multiple licenses
fn split_license(license: &str) -> Vec<String> {
    // SPDX identifiers: letters, numbers, -, ., + (ex: Apache-2.0, LGPL-2.1-or-later, MIT, BSD-3-Clause, etc.)
    let re = regex::Regex::new(r"[A-Za-z0-9\.\-\+]+").unwrap();
    re.find_iter(license)
        .map(|m| m.as_str().to_string())
        .filter(|id| {
            let upper = id.to_ascii_uppercase();
            upper != "AND" && upper != "OR" && upper != "WITH"
        })
        .collect()
}

/// Get information about the packages in the current Cargo project
fn get_packages_info() -> Result<Vec<PackageInfo>> {
    // Use cargo_metadata to get package information
    let metadata = MetadataCommand::new()
        .exec()
        .context("Failed to execute cargo metadata")?;

    let mut packages_info = Vec::new();

    for package in metadata.packages {
        let info = PackageInfo {
            name: package.name.to_string(),
            license: package.license.clone(),
            licenses: package
                .license
                .as_ref()
                .map(|l| split_license(l))
                .unwrap_or_else(|| vec![]),
            authors: package.authors,
            version: package.version.to_string(),
            repository: package.repository.clone(),
        };
        packages_info.push(info);
    }

    Ok(packages_info)
}

/// Generates a rust file in the OUT_DIR
/// this files contains a constant with a string showing a license notice
/// This license notice is like:
/// ```text
/// This software is licensed under the SCTG Development Non-Commercial License v1.0.
/// For more information, see the LICENSE.md file in the root of this project.
/// (c) Ronan LE MEILLAT, SCTG Development
/// ---
/// This software contains Open Source Software (OSS) components.
/// - [crate_name] ([crate_version]) - [crate_license] - [crate_authors]
/// - [crate_name] ([crate_version]) - [crate_license] - [crate_authors]
/// ---
/// You can find the full text of the licenses used by the dependencies at the following URL:
/// - MIT: https://opensource.org/license/mit/
/// - Apache-2.0: https://opensource.org/license/apache-2-0/
/// ...
/// ```
fn generate_license_notice() -> Result<()> {
    // Get package information
    let packages_info = get_packages_info()?;

    // Create the license notice string
    let mut notice = String::new();
    let mut oss_licenses: Vec<String> = vec![];
    notice.push_str(
        "This software is licensed under the SCTG Development Non-Commercial License v1.0.\n",
    );
    notice.push_str("For more information, see the LICENSE.md file in the root of this project.\n");
    notice.push_str("© Ronan LE MEILLAT, SCTG Development\n");
    notice.push_str("---\n");
    notice.push_str("This software contains Open Source Software (OSS) components:\n");

    for package in packages_info {
        let authors = package.authors.join(", ");
        let license = package.license.unwrap_or_else(|| "Unknown".to_string());
        notice.push_str(&format!(
            "- {} ({}) - {} - {} - {}\n",
            package.name,
            package.version,
            license,
            authors,
            package
                .repository
                .unwrap_or_else(|| "No repository".to_string())
        ));
        oss_licenses.extend(package.licenses.iter().cloned());
    }

    // Remove duplicates from the licenses
    let oss_licenses: Vec<String> = oss_licenses
        .into_iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    notice.push_str("---\n");
    notice.push_str("You can find the full text of the licenses used by the dependencies at the following URLs:\n");
    for license in oss_licenses {
        match license.as_str() {
            "0BSD" => notice.push_str("- 0BSD: https://opensource.org/license/0bsd/\n"),
            "AGPL-3.0-only" => {
                notice.push_str("- AGPL-3.0-only: https://www.gnu.org/licenses/agpl-3.0.en.html\n")
            }
            "Apache-2.0" => {
                notice.push_str("- Apache-2.0: https://opensource.org/license/apache-2-0/\n")
            }
            "BSD-2-Clause" => {
                notice.push_str("- BSD-2-Clause: https://opensource.org/license/bsd-2-clause/\n")
            }
            "BSD-3-Clause" => {
                notice.push_str("- BSD-3-Clause: https://opensource.org/license/bsd-3-clause/\n")
            }
            "BSL-1.0" => notice.push_str("- BSL-1.0: https://opensource.org/license/bsl-1-0/\n"),
            "CC0-1.0" => {
                notice.push_str("- CC0-1.0: https://creativecommons.org/publicdomain/zero/1.0/\n")
            }
            "CDLA-Permissive-2.0" => {
                notice.push_str("- CDLA-Permissive-2.0: https://cdla.io/permissive-2-0/\n")
            }
            "ISC" => notice.push_str("- ISC: https://opensource.org/license/isc/\n"),
            "LGPL-2.1-or-later" => notice.push_str(
                "- LGPL-2.1-or-later: https://www.gnu.org/licenses/old-licenses/lgpl-2.1.en.html\n",
            ),
            "LLVM-exception" => {
                notice.push_str("- LLVM-exception: https://spdx.org/licenses/LLVM-exception.html\n")
            }
            "MIT" => notice.push_str("- MIT: https://opensource.org/license/mit/\n"),
            "MIT-0" => notice.push_str("- MIT-0: https://opensource.org/license/mit-0/\n"),
            "OpenSSL" => {
                notice.push_str("- OpenSSL: https://www.openssl.org/source/license.html\n")
            }
            "Unlicense" => notice.push_str("- Unlicense: https://unlicense.org/\n"),
            "Unicode-3.0" => {
                notice.push_str("- Unicode-3.0: https://opensource.org/license/unicode-3-0/\n")
            }
            "Zlib" => notice.push_str("- Zlib: https://opensource.org/license/zlib/\n"),
            _ => notice.push_str(&format!("- {}: Unknown license URL\n", license)),
        }
    }
    notice.push_str("---\n");
    notice.push_str("Please note that this software is an original work and does not constitute a derivative work of any of its dependencies.\n");
    // Write the notice to a file in OUT_DIR
    let out_dir = env::var("OUT_DIR")?;
    let file_path = PathBuf::from(out_dir).join("license_notice.rs");
    let mut file = File::create(file_path)?;
    writeln!(file, "pub const LICENSE_NOTICE: &str = r#\"{}\"#;", notice)?;

    Ok(())
}

/// Run generate_license_notice only if the Cargo.lock file has changed
/// It generates a hash of the Cargo.lock file and compares it to a stored hash
/// If the hashes are different, it runs the function and updates the stored hash
/// # Returns true if the function was run, false if it was skipped
fn run_generate_license_notice_if_needed() -> Result<bool> {
    // Get the path to the Cargo.lock file
    let cargo_lock_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("Cargo.lock");

    // Check if the Cargo.lock file exists
    if !cargo_lock_path.exists() {
        return Ok(false);
    }

    // Read the contents of the Cargo.lock file
    let cargo_lock_content = fs::read_to_string(&cargo_lock_path)?;

    // Calculate a hash of the Cargo.lock content
    let mut hasher = Sha256::new();
    hasher.update(cargo_lock_content.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    // Get the path to the stored hash file
    let hash_file_path = PathBuf::from(env::var("OUT_DIR")?).join("cargo_lock_hash.txt");

    // Check if the hash file exists and read its content
    let mut previous_hash = String::new();
    if hash_file_path.exists() {
        previous_hash = fs::read_to_string(&hash_file_path)?;
    }

    // Compare the hashes
    if previous_hash.trim() == hash.trim() {
        println!("cargo:warning=No changes in Cargo.lock, skipping license notice generation");
        return Ok(false);
    }

    // Run the function to generate the license notice
    let _ = generate_license_notice()?;

    // Write the new hash to the hash file
    fs::write(&hash_file_path, hash)?;

    println!("cargo:warning=License notice generated successfully");
    Ok(true)
}

fn is_web_source_newer_than_dist(dist_path: &PathBuf, src_paths: &[PathBuf]) -> bool {
    // Get the most recent modification date of files in dist
    let dist_latest_mod = get_latest_modification_time(dist_path).unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 86400
    });

    let src_paths: Vec<PathBuf> = src_paths.iter().map(|p| p.to_path_buf()).collect();
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
/// ### Arguments
///
/// * `project_path` - The path to the Node.js project directory (should contain package.json)
///
/// ### Returns
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

    let needs_build = !dist_path.exists() || is_web_source_newer_than_dist(&dist_path, &src_paths);
    println!(
        "cargo:warning=build_web_console: needs_build: {}",
        needs_build
    );

    // If no rebuild is needed, exit early
    if !needs_build && !version_changed {
        // Delete generix.json if it exists, as it will be generated dynamically
        delete_dist_file(&dist_path, "generix.json")?;
        println!("cargo:warning=No changes detected in webconsole files, skipping build");
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

    // Checks if dist files already exist to avoid unnecessary rebuilds
    let dist_path = PathBuf::from("./resources/rapidoc_helper/dist");
    let web_path = PathBuf::from("./resources/rapidoc_helper");

    println!(
        "cargo:warning=build_rapidoc: dist_path exists: {}",
        dist_path.exists()
    );
    println!(
        "cargo:warning=build_rapidoc: web_path exists: {}",
        web_path.exists()
    );

    // Get the most recent modification date of source files
    let src_paths = [
        PathBuf::from("./resources/rapidoc_helper/package.json"),
        PathBuf::from("./resources/rapidoc_helper/openapi3.ts"),
        PathBuf::from("./resources/rapidoc_helper/webpack.config.cjs"),
        PathBuf::from("./resources/rapidoc_helper/index.ts"),
        // Add other files/directories to watch as needed
    ];

    let needs_build = !dist_path.exists() || is_web_source_newer_than_dist(&dist_path, &src_paths);
    println!("cargo:warning=build_rapidoc: needs_build: {}", needs_build);

    // If no rebuild is needed, exit early
    if !needs_build {
        println!("cargo:warning=No changes detected in rapidoc files, skipping build");
        return Ok(());
    }

    println!("cargo:warning=build_rapidoc: Calling build_node_project");

    // Use the build_node_project function
    match build_node_project(web_path) {
        Ok(()) => {
            println!("cargo:warning=build_rapidoc: build_node_project completed successfully")
        }
        Err(e) => {
            println!(
                "cargo:warning=build_rapidoc: build_node_project failed: {}",
                e
            );
            return Err(e);
        }
    }

    // Verify the dist folder was created
    println!("cargo:warning=build_rapidoc: Checking if dist directory was created");
    if !dist_path.exists() {
        return Err(anyhow::anyhow!(
            "Rapidoc build completed but dist directory was not created"
        ));
    }

    println!("cargo:warning=Rapidoc built successfully");
    Ok(())
}

/// Extract Git commit information for build metadata
fn get_git_info() -> Result<(String, String, String)> {
    // Get current commit hash (short)
    let commit_hash_short = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .context("Failed to execute git rev-parse --short HEAD")?;

    if !commit_hash_short.status.success() {
        return Err(anyhow::anyhow!(
            "Git command failed: {}",
            String::from_utf8_lossy(&commit_hash_short.stderr)
        ));
    }

    // Get current commit hash (full)
    let commit_hash_full = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .context("Failed to execute git rev-parse HEAD")?;

    if !commit_hash_full.status.success() {
        return Err(anyhow::anyhow!(
            "Git command failed: {}",
            String::from_utf8_lossy(&commit_hash_full.stderr)
        ));
    }

    // Get commit date
    let commit_date = Command::new("git")
        .args(&["log", "-1", "--format=%ci"])
        .output()
        .context("Failed to execute git log for commit date")?;

    if !commit_date.status.success() {
        return Err(anyhow::anyhow!(
            "Git command failed: {}",
            String::from_utf8_lossy(&commit_date.stderr)
        ));
    }

    // Check if working directory is dirty
    let git_status = Command::new("git")
        .args(&["status", "--porcelain"])
        .output()
        .context("Failed to execute git status")?;

    let is_dirty = !git_status.stdout.is_empty();

    let short_hash = String::from_utf8_lossy(&commit_hash_short.stdout)
        .trim()
        .to_string();
    let full_hash = String::from_utf8_lossy(&commit_hash_full.stdout)
        .trim()
        .to_string();
    let date = String::from_utf8_lossy(&commit_date.stdout)
        .trim()
        .to_string();

    // Add dirty marker if working directory has uncommitted changes
    let final_short_hash = if is_dirty {
        format!("{}-dirty", short_hash)
    } else {
        short_hash
    };

    let final_full_hash = if is_dirty {
        format!("{}-dirty", full_hash)
    } else {
        full_hash
    };

    Ok((final_short_hash, final_full_hash, date))
}

#[tokio::main]
async fn main() {
    // Tells Cargo to rerun build.rs if any files in the web folder change or certificate files change
    println!("cargo:rerun-if-changed=web");
    println!("cargo:rerun-if-changed=resources/cert.pem");
    println!("cargo:rerun-if-changed=resources/cert.key");
    // Rerun if .git directory changes (for commit hash updates)
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
    // Rerun if Cargo.lock changes (to regenerate license notice)
    println!("cargo:rerun-if-changed=Cargo.lock");

    run_generate_license_notice_if_needed();

    // Extract Git information and set environment variables
    match get_git_info() {
        Ok((short_hash, full_hash, commit_date)) => {
            println!("cargo:rustc-env=GIT_COMMIT_HASH_SHORT={}", short_hash);
            println!("cargo:rustc-env=GIT_COMMIT_HASH_FULL={}", full_hash);
            println!("cargo:rustc-env=GIT_COMMIT_DATE={}", commit_date);
            println!("cargo:warning=Git info: {} ({})", short_hash, commit_date);
        }
        Err(e) => {
            println!("cargo:warning=Failed to get Git information: {}", e);
            // Set fallback values
            println!("cargo:rustc-env=GIT_COMMIT_HASH_SHORT=unknown");
            println!("cargo:rustc-env=GIT_COMMIT_HASH_FULL=unknown");
            println!("cargo:rustc-env=GIT_COMMIT_DATE=unknown");
        }
    }

    // Set additional build information
    let build_timestamp = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", build_timestamp);

    // Get Rust compiler version
    let rustc_version = env::var("RUSTC_VERSION").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_RUSTC_VERSION={}", rustc_version);

    // Get target triple
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_TARGET={}", target);

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
        eprintln!("cargo:warning=build_web_console failed: {}", e);
        panic!("Failed to build web console: {}", e);
    }
    // Build the rapidoc helper
    if let Err(e) = build_rapidoc() {
        eprintln!("cargo:warning=build_rapidoc failed: {}", e);
        panic!("Failed to build rapidoc helper: {}", e);
    }

    // Extract and print Git information
    match get_git_info() {
        Ok((short_hash, full_hash, date)) => {
            println!("cargo:warning=Current Git short hash: {}", short_hash);
            println!("cargo:warning=Current Git full hash: {}", full_hash);
            println!("cargo:warning=Commit date: {}", date);
        }
        Err(e) => {
            println!("cargo:warning=Failed to get Git info: {}", e);
        }
    }
}
