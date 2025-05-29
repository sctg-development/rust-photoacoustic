// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use clap::{Arg, ArgMatches, Command};
use std::path::PathBuf;

/// Structure for handling command-line arguments
#[derive(Debug, Clone)]
pub struct CliArgs {
    pub config_path: PathBuf,
    pub algorithm: String,
    pub user: String,
    pub client: String,
    pub duration_override: Option<u64>,
    pub quiet: bool,
}

impl CliArgs {
    /// Parse command-line arguments
    pub fn parse() -> Self {
        let matches = Self::build_cli().get_matches();
        Self::from_matches(&matches)
    }

    /// Build the CLI interface
    fn build_cli() -> Command {
        Command::new("create_token")
            .version("1.0")
            .about("Create JWT access tokens manually")
            .arg(
                Arg::new("config")
                    .short('c')
                    .long("config")
                    .value_name("FILE")
                    .help("Path to configuration file")
                    .default_value("config.yaml"),
            )
            .arg(
                Arg::new("algorithm")
                    .short('a')
                    .long("algorithm")
                    .value_name("ALGORITHM")
                    .help("JWT signing algorithm")
                    .value_parser(["HS256", "RS256"])
                    .default_value("RS256"),
            )
            .arg(
                Arg::new("duration")
                    .short('d')
                    .long("duration")
                    .value_name("SECONDS")
                    .help("Token duration in seconds (overrides config)")
                    .value_parser(clap::value_parser!(u64)),
            )
            .arg(
                Arg::new("user")
                    .short('u')
                    .long("user")
                    .value_name("USERNAME")
                    .help("Username (must exist in config)")
                    .required(true),
            )
            .arg(
                Arg::new("client")
                    .short('i')
                    .long("client")
                    .value_name("CLIENT")
                    .help("Client (must exist in config)")
                    .required(true),
            )
            .arg(
                Arg::new("quiet")
                    .short('q')
                    .long("quiet")
                    .value_name("QUIET")
                    .help("Suppress output messages, only token is printed")
                    .action(clap::ArgAction::SetTrue),
            )
    }

    /// Extract arguments from matches
    fn from_matches(matches: &ArgMatches) -> Self {
        Self {
            config_path: PathBuf::from(matches.get_one::<String>("config").unwrap()),
            algorithm: matches.get_one::<String>("algorithm").unwrap().clone(),
            user: matches.get_one::<String>("user").unwrap().clone(),
            client: matches.get_one::<String>("client").unwrap().clone(),
            duration_override: matches.get_one::<u64>("duration").copied(),
            quiet: matches.get_one::<bool>("quiet").copied().unwrap_or(false),
        }
    }
}
