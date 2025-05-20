use clap::Parser;
use std::error::Error;
use tokio::time::Duration;
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
    let socket_addr = format!("{}:{}", args.address, args.port);
    println!("Connecting to Modbus server at {}", socket_addr);

    // Create TCP transport
    let mut ctx = tcp::connect_slave(&socket_addr, Slave(1)).await?;

    // Set request timeout
    ctx.set_timeout(Duration::from_secs(1));

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

    // Display formatted results based on our register map
    // This matches the register map defined in your modbus_server.rs
    for (i, value) in response.iter().enumerate() {
        let register = args.input_register + i as u16;
        match register {
            0 => println!(
                "Register 0: Resonance Frequency = {:.1} Hz",
                *value as f32 / 10.0
            ),
            1 => println!(
                "Register 1: Signal Amplitude = {:.3}",
                *value as f32 / 1000.0
            ),
            2 => println!(
                "Register 2: Water Vapor Concentration = {:.1} ppm",
                *value as f32 / 10.0
            ),
            3 => println!("Register 3: Timestamp Low Word = {}", value),
            4 => println!("Register 4: Timestamp High Word = {}", value),
            5 => {
                let status = match value {
                    0 => "Normal",
                    1 => "Warning",
                    2 => "Error",
                    _ => "Unknown",
                };
                println!("Register 5: Status Code = {} ({})", value, status);
            }
            _ => println!("Register {}: Value = {}", register, value),
        }
    }

    // If we read both timestamp registers, compute the full timestamp
    if args.input_register <= 3 && args.input_register + args.quantity > 4 {
        let low_word_idx = 3 - args.input_register;
        let high_word_idx = 4 - args.input_register;

        if low_word_idx < response.len() as u16 && high_word_idx < response.len() as u16 {
            let low_word = response[low_word_idx as usize] as u32;
            let high_word = response[high_word_idx as usize] as u32;
            let timestamp = (high_word << 16) | low_word;

            // Format timestamp as human-readable date/time
            let datetime = chrono::NaiveDateTime::from_timestamp_opt(timestamp as i64, 0)
                .unwrap_or_else(|| chrono::NaiveDateTime::from_timestamp_opt(0, 0).unwrap());
            println!("Full Timestamp: {} ({})", timestamp, datetime);
        }
    }

    Ok(())
}
