// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

mod cli;

use cli::CliArgs;
use rust_photoacoustic::config::Config;
use rust_photoacoustic::utility::jwt_token::{
    ConfigLoader, JwtAlgorithm, TokenCreationError, TokenCreationParams, TokenCreator,
};
use std::process;
use std::str::FromStr;

#[tokio::main]
async fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<(), TokenCreationError> {
    // Parse CLI arguments
    let args = CliArgs::parse();

    // Load and validate configuration
    let config = Config::from_file(&args.config_path)
        .map_err(|e| TokenCreationError::ConfigError { source: e.into() })?;

    let config_loader = ConfigLoader::from_config(&config)?;

    // Validate user and client
    let _user = config_loader.find_user(&args.user)?;
    let _client = config_loader.find_client(&args.client)?;

    // Prepare creation parameters
    let algorithm = JwtAlgorithm::from_str(&args.algorithm)?;
    let duration = args.duration_override.unwrap_or(86400); // Default 24 hours

    let params = TokenCreationParams {
        user_id: args.user.clone(),
        client_id: args.client.clone(),
        algorithm,
        duration_seconds: duration,
    };

    // Create the token
    let token_creator = TokenCreator::new(&config_loader)?;
    let result = token_creator.create_token(&params)?;

    // Display results
    if args.quiet {
        print_short_results(&result);
    } else {
        print_full_results(&result);
    }

    Ok(())
}

fn print_full_results(result: &rust_photoacoustic::utility::jwt_token::TokenCreationResult) {
    println!("✅ Token created successfully!");
    println!("👤 User: {}", result.user_id);
    println!("🔐 Algorithm: {}", result.algorithm);
    println!("⏱️  Duration: {} seconds", result.duration_seconds);
    println!("🔑 Permissions: {}", result.permissions.join(", "));
    println!("🎫 Token: {}", result.token);
}

fn print_short_results(result: &rust_photoacoustic::utility::jwt_token::TokenCreationResult) {
    print!("{}", result.token);
}
