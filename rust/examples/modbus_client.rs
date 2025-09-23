// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Simple Modbus client example for the photoacoustic water vapor analyzer
//!
//! This example demonstrates how to connect to the photoacoustic Modbus server
//! and read measurement data. It can be used as a template for integrating
//! the analyzer with SCADA systems, PLCs, or other industrial automation equipment.
//!
//! ## Usage
//!
//! First, start the photoacoustic daemon with Modbus enabled:
//! ```bash
//! cargo run -- --config config.yaml daemon
//! ```
//!
//! Then run this client example:
//! ```bash
//! cargo run --example modbus_client
//! ```
//!
//! ## Register Map
//!
//! ### Input Registers (Read-Only - Measurement Data)
//! - Register 0: Resonance frequency (Hz × 10, 0.1 Hz resolution)
//! - Register 1: Signal amplitude (dB × 1000, 0.001 dB resolution)  
//! - Register 2: Water vapor concentration (ppm × 10, 0.1 ppm resolution)
//! - Register 3: Timestamp low word (UNIX epoch seconds)
//! - Register 4: Timestamp high word (UNIX epoch seconds)
//! - Register 5: Status code (0=normal, 1=warning, 2=error)
//!
//! ### Holding Registers (Read-Write - Configuration Data)
//! - Register 0: Measurement interval (seconds), default: 10
//! - Register 1: Averaging count (samples), default: 20
//! - Register 2: Gain setting, default: 30
//! - Register 3: Filter strength, default: 40

use std::time::{Duration, UNIX_EPOCH};
use tokio::time;
use tokio_modbus::client::{tcp::connect, Reader, Writer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Modbus server configuration (should match config.yaml)
    let server_address = "127.0.0.1:1502"; // Non-privileged port for security

    println!("🔌 Photoacoustic Modbus Client");
    println!("=====================================");
    println!("Connecting to Modbus server at {}", server_address);

    // Parse socket address
    let socket_addr: std::net::SocketAddr = server_address.parse()?;

    // Connect to the Modbus server
    let mut ctx = match connect(socket_addr).await {
        Ok(ctx) => {
            println!("✅ Successfully connected to Modbus server");
            ctx
        }
        Err(e) => {
            eprintln!("❌ Failed to connect to Modbus server: {}", e);
            eprintln!("💡 Make sure the photoacoustic daemon is running with Modbus enabled");
            eprintln!("   Example: cargo run -- --config config.yaml daemon");
            return Err(e.into());
        }
    };

    println!("\n📊 Reading measurement data (Input Registers):");
    println!("===============================================");

    // Read input registers (measurement data)
    match ctx.read_input_registers(0, 6).await {
        Ok(Ok(data)) => {
            // Decode frequency (register 0)
            let freq_raw = data[0];
            let frequency = freq_raw as f32 / 10.0;
            println!(
                "🌊 Resonance Frequency: {} Hz (raw: {})",
                frequency, freq_raw
            );

            // Decode amplitude (register 1)
            let amp_raw = data[1];
            let amplitude = amp_raw as f32 / 1000.0;
            println!("📈 Signal Amplitude: {} dB (raw: {})", amplitude, amp_raw);

            // Decode concentration (register 2)
            let conc_raw = data[2];
            let concentration = conc_raw as f32 / 10.0;
            println!(
                "💧 Gas Concentration: {} ppm (raw: {})",
                concentration, conc_raw
            );

            // Decode timestamp (registers 3-4)
            let timestamp_low = data[3] as u32;
            let timestamp_high = data[4] as u32;
            let timestamp = timestamp_low | (timestamp_high << 16);

            let current_time = std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs() as u32;
            let age_seconds = current_time.saturating_sub(timestamp);

            println!(
                "⏰ Measurement Timestamp: {} (age: {} seconds)",
                timestamp, age_seconds
            );

            // Decode status (register 5)
            let status = data[5];
            let status_text = match status {
                0 => "Normal",
                1 => "Warning",
                2 => "Error",
                _ => "Unknown",
            };
            println!("📊 System Status: {} ({})", status_text, status);
        }
        Ok(Err(e)) => {
            eprintln!("❌ Modbus exception when reading input registers: {:?}", e);
        }
        Err(e) => {
            eprintln!("❌ Failed to read input registers: {}", e);
        }
    }

    println!("\n⚙️  Reading configuration data (Holding Registers):");
    println!("====================================================");

    // Read holding registers (configuration data)
    match ctx.read_holding_registers(0, 4).await {
        Ok(Ok(data)) => {
            println!("⏱️  Measurement Interval: {} seconds", data[0]);
            println!("🔢 Averaging Count: {} samples", data[1]);
            println!("📈 Gain Setting: {}", data[2]);
            println!("🎛️  Filter Strength: {}", data[3]);
        }
        Ok(Err(e)) => {
            eprintln!(
                "❌ Modbus exception when reading holding registers: {:?}",
                e
            );
        }
        Err(e) => {
            eprintln!("❌ Failed to read holding registers: {}", e);
        }
    }

    println!("\n✏️  Testing configuration write (Holding Registers):");
    println!("=====================================================");

    // Example: Change measurement interval to 15 seconds
    match ctx.write_single_register(0, 15).await {
        Ok(_) => {
            println!("✅ Successfully set measurement interval to 15 seconds");

            // Read back to confirm
            match ctx.read_holding_registers(0, 1).await {
                Ok(Ok(data)) => {
                    println!(
                        "✅ Confirmed: Measurement interval is now {} seconds",
                        data[0]
                    );
                }
                Ok(Err(e)) => {
                    eprintln!(
                        "❌ Modbus exception when reading back configuration: {:?}",
                        e
                    );
                }
                Err(e) => {
                    eprintln!("❌ Failed to read back configuration: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to write configuration: {}", e);
        }
    }

    // Example: Write multiple configuration values
    let new_config = [10, 25, 35, 45]; // interval, averaging, gain, filter
    match ctx.write_multiple_registers(0, &new_config).await {
        Ok(_) => {
            println!("✅ Successfully updated multiple configuration values");
            println!(
                "   Interval: {} sec, Averaging: {} samples, Gain: {}, Filter: {}",
                new_config[0], new_config[1], new_config[2], new_config[3]
            );
        }
        Err(e) => {
            eprintln!("❌ Failed to write multiple registers: {}", e);
        }
    }

    println!("\n🔄 Continuous monitoring (press Ctrl+C to stop):");
    println!("=================================================");

    // Continuous monitoring loop
    for i in 1..=5 {
        println!("\n📊 Reading #{}", i);

        match ctx.read_input_registers(0, 3).await {
            Ok(Ok(data)) => {
                let frequency = data[0] as f32 / 10.0;
                let amplitude = data[1] as f32 / 1000.0;
                let concentration = data[2] as f32 / 10.0;

                println!(
                    "  Freq: {} Hz | Amp: {} dB | Conc: {} ppm",
                    frequency, amplitude, concentration
                );
            }
            Ok(Err(e)) => {
                eprintln!("❌ Modbus exception when reading measurement data: {:?}", e);
            }
            Err(e) => {
                eprintln!("❌ Failed to read measurement data: {}", e);
            }
        }

        time::sleep(Duration::from_secs(2)).await;
    }

    println!("\n🎉 Modbus client example completed!");
    println!("💡 This demonstrates how to integrate the photoacoustic analyzer");
    println!("   with SCADA systems, PLCs, or other industrial automation equipment");

    Ok(())
}
