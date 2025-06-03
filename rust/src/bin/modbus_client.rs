// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use clap::Parser;
use std::{error::Error, net::SocketAddr};
use tokio_modbus::prelude::*;

/// Modbus client for reading input registers from a photoacoustic analyzer
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Modbus server address
    #[clap(long, default_value = "127.0.0.1")]
    address: String,

    /// Modbus server port
    #[clap(long, default_value = "502")]
    port: u16,

    /// Starting input register address
    #[clap(long, default_value = "0")]
    input_register: u16,

    /// Number of registers to read
    #[clap(long, default_value = "6")]
    quantity: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    // Parse command line arguments
    let args = Args::parse();

    // Format server address
    let socket_addr: SocketAddr = format!("{}:{}", args.address, args.port)
        .parse()
        .expect("Invalid socket address");
    println!("Connecting to Modbus server at {}", socket_addr);

    // Create TCP transport
    let mut ctx = tcp::connect_slave(socket_addr, Slave(1)).await?;

    // Read input registers
    println!(
        "Reading {} input registers starting at address {}",
        args.quantity, args.input_register
    );
    let response = ctx
        .read_input_registers(args.input_register, args.quantity)
        .await?;

    // Display raw results
    println!("Raw register values: {:?}", response);
    Ok(())
}
