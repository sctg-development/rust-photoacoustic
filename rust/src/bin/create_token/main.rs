// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

mod cli;
mod config_loader;
mod error;
mod token_creator;

use cli::CliArgs;
use config_loader::ConfigLoader;
use error::TokenCreationError;
use std::process;
use std::str::FromStr;
use token_creator::{JwtAlgorithm, TokenCreationParams, TokenCreator};

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
        process::exit(e.exit_code());
    }
}

async fn run() -> Result<(), TokenCreationError> {
    // Parse CLI arguments
    let args = CliArgs::parse();

    // Load and validate configuration
    let config_loader = ConfigLoader::load(&args.config_path)?;
    // Validate user and client
    let _user = config_loader.find_user(&args.user)?;
    let _client = config_loader.find_client(&args.client)?;

    // Prepare creation parameters
    let algorithm = JwtAlgorithm::from_str(&args.algorithm)?;
    let duration = config_loader.get_token_duration(args.duration_override);

    let params = TokenCreationParams {
        username: args.user.clone(),
        client_id: args.client.clone(),
        algorithm,
        duration,
    };

    // Create the token
    let token_creator = TokenCreator::new(config_loader);
    let result = token_creator.create_token(params).await?;

    // Display results
    if args.quiet {
        print_short_results(&result);
    } else {
        print_full_results(&result);
    }

    Ok(())
}

fn print_full_results(result: &token_creator::TokenCreationResult) {
    println!("✅ Token created successfully!");
    println!("👤 User: {}", result.username);
    println!("🔐 Algorithm: {}", result.algorithm);
    println!("⏱️  Duration: {} seconds", result.duration);
    println!("🔑 Permissions: {}", result.permissions.join(", "));
    println!("🎫 Token: {}", result.token);
}

fn print_short_results(result: &token_creator::TokenCreationResult) {
    println!("{}", result.token);
}
