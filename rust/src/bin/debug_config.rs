// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

// Debug test for certificate validation
use anyhow::Result;
use clap::Parser;
use rust_photoacoustic::config::Config;
use std::path::{Path, PathBuf};
#[derive(Debug, Parser)]
#[command(author, version, about = "Check config.yaml for detecting errors", long_about = None)]
struct Args {
    /// Input file path (.yaml)
    ///
    /// The path where the configuration file is located.
    /// should be .yaml or .yml format.
    #[arg(short, long, value_name = "FILE")]
    input: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Check if input file exists
    if !Path::new(&args.input).exists() {
        eprintln!(
            "Error: Input file '{}' does not exist",
            args.input.display()
        );
        std::process::exit(1);
    }

    let path = Path::new(args.input.as_path());

    println!("Testing file: {:?}", path);
    println!("File exists: {}", path.exists());

    let result = Config::from_file(path);

    match result {
        Ok(_) => println!("Validation succeeded for file: {:?}", path),
        Err(e) => println!("Validation failed: {}", e),
    }

    Ok(())
}
