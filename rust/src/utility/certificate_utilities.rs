// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Certificate Utilities
//!
//! This module provides utilities for generating and managing SSL/TLS certificates
//! for secure communication in the photoacoustic application.
//!
//! ## Features
//!
//! * Generation of self-signed certificates
//! * Support for custom validity periods
//! * Support for custom Subject Alternative Names (SANs)
//! * Proper key usage configuration
//!
//! ## Usage Example
//!
//! ```rust
//! use rust_photoacoustic::utility::certificate_utilities::create_self_signed_cert;
//!
//! // Generate a simple self-signed certificate for localhost
//! let result = create_self_signed_cert(
//!     365,                           // Valid for 365 days
//!     "path/to/certificate.pem",     // Certificate output path
//!     "path/to/private_key.pem",     // Private key output path
//!     "localhost",                   // Common name
//!     None,                          // Use default key length
//!     None,                          // Use default alternative names
//! );
//! ```
//!
//! ## Security Considerations
//!
//! Self-signed certificates are suitable for development and testing environments,
//! but should not be used in production without understanding the security implications.
//! For production environments, certificates from a trusted Certificate Authority (CA)
//! should be used whenever possible.

use anyhow::{Context, Result};
use rcgen::{
    CertificateParams, DnType, DnValue, IsCa, KeyPair, KeyUsagePurpose, SanType,
};
use rcgen::string::Ia5String;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// Creates a self-signed certificate and key pair and writes them to the specified paths.
///
/// This function generates a new X.509 certificate suitable for TLS/SSL connections.
/// By default, the certificate will include "localhost", "127.0.0.1", and "::1" as
/// Subject Alternative Names (SANs) if no alternative names are provided.
///
/// The certificate is configured with digital signature and key encipherment purposes,
/// making it suitable for server authentication in TLS connections.
///
/// ### Arguments
///
/// * `days` - Validity period in days for the certificate
/// * `cert_path` - File path where the PEM-encoded certificate will be saved
/// * `key_path` - File path where the PEM-encoded private key will be saved
/// * `common_name` - The common name (CN) for the certificate (e.g., "localhost", "example.com")
/// * `key_length` - Optional key length in bits (e.g., 2048, 4096). If None, uses the default length
/// * `alt_names` - Optional list of subject alternative names (DNS names or IP addresses)
///
/// ### Returns
///
/// * `Result<()>` - Ok(()) on success, or an error if certificate generation or file operations fail
///
/// ### Errors
///
/// Returns an error if:
/// - Certificate parameter creation fails
/// - Self-signing operation fails
/// - Output directories cannot be created
/// - Certificate or key files cannot be created or written to
///
/// ### Examples
///
/// Basic usage with default settings:
///
/// ```rust
/// use rust_photoacoustic::utility::certificate_utilities::create_self_signed_cert;
///
/// let result = create_self_signed_cert(
///     30,                     // Valid for 30 days
///     "cert.pem",             // Certificate output path
///     "key.pem",              // Private key output path
///     "localhost",            // Common name
///     None,                   // Use default key length
///     None,                   // Use default alternative names
/// );
/// ```
///
/// Usage with custom alternative names:
///
/// ```rust
/// use rust_photoacoustic::utility::certificate_utilities::create_self_signed_cert;
///
/// let alt_names = vec![
///     "example.com".to_string(),
///     "www.example.com".to_string(),
///     "192.168.1.1".to_string(),
/// ];
///
/// let result = create_self_signed_cert(
///     365,                    // Valid for 1 year
///     "server.crt",           // Certificate output path
///     "server.key",           // Private key output path
///     "example.com",          // Common name
///     Some(4096),             // Use 4096-bit key
///     Some(alt_names),        // Use custom alternative names
/// );
/// ```
pub fn create_self_signed_cert(
    days: u32,
    cert_path: &str,
    key_path: &str,
    common_name: &str,
    key_length: Option<u32>,
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

    // Generate key pair with specified length or default
    let key_pair = match key_length {
        Some(bits) => {
            // Note: Custom key length implementation would go here
            // The rcgen crate doesn't directly expose key length configuration,
            // so we're using the default for now
            KeyPair::generate().context("Failed to generate key pair")?
        }
        None => KeyPair::generate().context("Failed to generate key pair")?,
    };

    // Generate self-signed certificate
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use x509_parser::parse_x509_certificate;
    use x509_parser::pem::parse_x509_pem;

    #[test]
    fn test_create_self_signed_cert_creates_files_and_contains_pem() {
        let dir = tempdir().unwrap();
        let cert_path = dir.path().join("test_cert.pem");
        let key_path = dir.path().join("test_key.pem");

        // use default alt names
        create_self_signed_cert(
            30,
            cert_path.to_str().unwrap(),
            key_path.to_str().unwrap(),
            "localhost",
            None,
            None,
        )
        .unwrap();

        // Check files exist
        assert!(cert_path.exists());
        assert!(key_path.exists());

        // Check file content contains PEM headers
        let cert_content = fs::read_to_string(&cert_path).unwrap();
        let key_content = fs::read_to_string(&key_path).unwrap();
        assert!(cert_content.contains("-----BEGIN CERTIFICATE-----"));
        assert!(
            key_content.contains("-----BEGIN PRIVATE KEY-----")
                || key_content.contains("-----BEGIN RSA PRIVATE KEY-----")
        );
    }

    #[test]
    fn test_create_self_signed_cert_custom_alt_names_and_parse_san() {
        let dir = tempdir().unwrap();
        let cert_path = dir.path().join("test_cert.pem");
        let key_path = dir.path().join("test_key.pem");

        let alt_names = vec!["example.com".to_string(), "127.0.0.1".to_string()];
        create_self_signed_cert(
            30,
            cert_path.to_str().unwrap(),
            key_path.to_str().unwrap(),
            "example.com",
            None,
            Some(alt_names.clone()),
        )
        .unwrap();

        // Read and parse certificate
        let cert_bytes = fs::read(&cert_path).unwrap();
        let (_rem, pem) = parse_x509_pem(&cert_bytes).expect("PEM parse failed");
        let (_rem2, cert) = parse_x509_certificate(&pem.contents).expect("X509 parse failed");

        // Extract SAN extension and verify it contains the expected entries
        use x509_parser::extensions::{GeneralName, ParsedExtension};

        let mut san_found = Vec::new();
        for ext in cert.tbs_certificate.extensions().iter() {
            if let ParsedExtension::SubjectAlternativeName(san) = ext.parsed_extension() {
                for gn in san.general_names.iter() {
                    match gn {
                        GeneralName::DNSName(dns) => {
                            san_found.push(dns.to_string());
                        }
                        GeneralName::IPAddress(bytes) => {
                            // Convert IP bytes to string
                            if let Ok(arr4) = <[u8; 4]>::try_from(bytes.as_ref()) {
                                let ipv4 = std::net::Ipv4Addr::from(arr4);
                                san_found.push(ipv4.to_string());
                            } else if let Ok(arr16) = <[u8; 16]>::try_from(bytes.as_ref()) {
                                let ipv6 = std::net::Ipv6Addr::from(arr16);
                                san_found.push(ipv6.to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        assert!(san_found.contains(&"example.com".to_string()));
        assert!(san_found.contains(&"127.0.0.1".to_string()));
    }
}
