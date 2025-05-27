// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # RS256 Key Generator
//!
//! This binary utility generates RSA key pairs for JWT token signing in RS256 format.
//! It creates both private and public keys in PEM format and writes them to specified files.
//!
//! ## Usage
//!
//! ```
//! rs256keygen [OPTIONS]
//! ```
//!
//! ## Options
//!
//! - `--out-pub-key <PATH>`: Path for the public key PEM file (default: "./pub.key")
//! - `--out-private-key <PATH>`: Path for the private key PEM file (default: "./private.key")
//! - `--length <BITS>`: RSA key length in bits (default: 4096)
//!
//! ## Examples
//!
//! Generate default 4096-bit keys in the current directory:
//! ```
//! rs256keygen
//! ```
//!
//! Generate 3072-bit keys with custom paths:
//! ```
//! rs256keygen --length 3072 --out-pub-key /path/to/public.pem --out-private-key /path/to/private.pem
//! ```
//!
//! ## Integration with Photoacoustic Configuration
//!
//! After generating the keys, they need to be Base64 encoded and added to the `config.yaml` file
//! under the `visualization.rs256_private_key` and `visualization.rs256_public_key` fields.

use std::fs::File;
use std::io::Write;
use std::io::{self};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey};
use rsa::{RsaPrivateKey, RsaPublicKey};

/// Command line arguments for the RSA256 key generation utility.
///
/// This struct defines the parameters that can be passed to the application
/// via command line arguments. The clap crate automatically handles parsing
/// and validation of these arguments.
#[derive(Parser, Debug)]
#[clap(
    author,
    version,
    about = "Generate RSA key pairs for JWT token signing in RS256 format"
)]
struct Args {
    /// Output path for the public key PEM file.
    ///
    /// This file will contain the public key in PKCS#1 PEM format, which can be
    /// used for JWT token verification. In the photoacoustic application, this key
    /// should be base64-encoded and added to the config.yaml file.
    #[clap(long, default_value = "./pub.key")]
    out_pub_key: PathBuf,

    /// Output path for the private key PEM file.
    ///
    /// This file will contain the private key in PKCS#1 PEM format, which can be
    /// used for JWT token signing. In the photoacoustic application, this key
    /// should be base64-encoded and added to the config.yaml file.
    #[clap(long, default_value = "./private.key")]
    out_private_key: PathBuf,

    /// RSA key length in bits.
    ///
    /// Common values are 2048, 3072, or 4096 bits. Longer keys provide more
    /// security but require more computational resources for signing and verification.
    /// The default of 4096 bits provides a high level of security suitable for most
    /// applications.
    #[clap(long, default_value = "4096")]
    length: usize,
}

/// Main entry point for the RS256 key generation utility.
///
/// This function:
/// 1. Parses command line arguments
/// 2. Generates an RSA key pair with the specified key length
/// 3. Encodes the keys in PKCS#1 PEM format
/// 4. Writes the keys to the specified output files
/// 5. Prints instructions for using the keys with the photoacoustic application
///
/// # Errors
///
/// Returns an error if:
/// - RSA key generation fails
/// - PEM encoding fails
/// - File creation or writing fails
fn main() -> Result<()> {
    let args = Args::parse();

    println!("Generating RSA key pair with {} bits...", args.length);

    // Flag to indicate when key generation is complete
    let generating = Arc::new(AtomicBool::new(true));
    let generating_clone = generating.clone();

    // Spawn a thread to display a spinner while generating keys
    let spinner_handle = thread::spawn(move || {
        let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let mut i = 0;
        while generating_clone.load(Ordering::Relaxed) {
            print!("\r{} Generating RSA key... ", spinner_chars[i]);
            io::stdout().flush().ok();
            i = (i + 1) % spinner_chars.len();
            thread::sleep(Duration::from_millis(100));
        }
        print!("\r                                  \r"); // Clear the spinner line
        io::stdout().flush().ok();
    });

    // Use OsRng directly to avoid dependency version conflicts
    let mut rng = rsa::rand_core::OsRng;

    // Generate a new random RSA key pair with the specified bits
    let private_key =
        RsaPrivateKey::new(&mut rng, args.length).context("Failed to generate RSA private key")?;

    // Signal that generation is complete
    generating.store(false, Ordering::Relaxed);
    // Wait for spinner thread to finish
    spinner_handle.join().ok();

    println!("RSA key pair generation completed successfully.");

    let public_key = RsaPublicKey::from(&private_key);

    // Convert keys to PKCS#1 PEM format
    let private_pem = private_key
        .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
        .context("Failed to encode private key to PEM")?;
    let public_pem = public_key
        .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
        .context("Failed to encode public key to PEM")?;

    // Write private key to file
    let mut private_file = File::create(&args.out_private_key).with_context(|| {
        format!(
            "Failed to create private key file at {:?}",
            args.out_private_key
        )
    })?;
    private_file
        .write_all(private_pem.as_bytes())
        .context("Failed to write private key to file")?;

    // Write public key to file
    let mut public_file = File::create(&args.out_pub_key)
        .with_context(|| format!("Failed to create public key file at {:?}", args.out_pub_key))?;
    public_file
        .write_all(public_pem.as_bytes())
        .context("Failed to write public key to file")?;

    println!("Private key written to: {:?}", args.out_private_key);
    println!("Public key written to: {:?}", args.out_pub_key);
    println!();
    println!("You can use these keys for JWT RS256 signing in your application.");
    println!("To use them in the config.yaml, you can Base64 encode them with:");
    println!("cat {} | base64", args.out_private_key.display());
    println!("cat {} | base64", args.out_pub_key.display());

    Ok(())
}
