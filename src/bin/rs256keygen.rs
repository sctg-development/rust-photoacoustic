// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey};
use rsa::{RsaPrivateKey, RsaPublicKey};

/// Generate RSA256 key pair for JWT tokens
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Output path for the public key PEM file
    #[clap(long, default_value = "./pub.key")]
    out_pub_key: PathBuf,

    /// Output path for the private key PEM file
    #[clap(long, default_value = "./private.key")]
    out_private_key: PathBuf,

    /// RSA key length in bits
    #[clap(long, default_value = "4096")]
    length: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("Generating RSA key pair with {} bits...", args.length);

    let mut rng = rsa::rand_core::OsRng;

    // Generate a new random RSA key pair with the specified bits
    let private_key =
        RsaPrivateKey::new(&mut rng, args.length).context("Failed to generate RSA private key")?;
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
