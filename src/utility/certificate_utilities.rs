// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Certificate utilities for generating self-signed certificates

use anyhow::{Context, Result};
use rcgen::{
    Certificate, CertificateParams, DnType, DnValue, Ia5String, IsCa, KeyPair, KeyUsagePurpose, SanType, PKCS_ECDSA_P256_SHA256
};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// Creates a self-signed certificate and key pair and writes them to the specified paths
///
/// # Arguments
///
/// * `days` - Validity period in days
/// * `cert_path` - Path to save the certificate
/// * `key_path` - Path to save the private key
/// * `common_name` - The common name for the certificate (e.g., "localhost")
/// * `key_length` - Key length (default: 2048)
/// * `alt_names` - Optional list of subject alternative names
///
/// # Returns
///
/// * `Result<()>` - Success or error
///
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
                params.subject_alt_names.push(SanType::DnsName(Ia5String::try_from(name).unwrap()));
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
    let cert = params.self_signed(&key_pair).context("Failed to generate certificate")?;

    // Get the certificate and private key in PEM format
    let cert_pem = cert
        .pem();
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
