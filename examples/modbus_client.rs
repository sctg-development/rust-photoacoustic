// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Example Modbus client to connect to the PhotoacousticModbusServer
//!
//! This example demonstrates how to connect to the Modbus server and read/write registers.
//! Run it while the server is running to see how to interact with the server.

use tokio_modbus::prelude::*;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Connect to the Modbus server
    let socket_addr = "127.0.0.1:502".parse().unwrap();
    println!("Connecting to photoacoustic Modbus server at {}", socket_addr);
    
    let mut ctx = tcp::connect(socket_addr).await?;
    
    // Read input registers (read-only sensor values)
    println!("\n--- Reading Input Registers (sensor values) ---");
    let input_regs = ctx.read_input_registers(0, 6).await??;
    println!("Input register 0 (frequency): {:.1} Hz", input_regs[0] as f32 / 10.0);
    println!("Input register 1 (amplitude): {:.3}", input_regs[1] as f32 / 1000.0);
    println!("Input register 2 (concentration): {:.1} ppm", input_regs[2] as f32 / 10.0);
    
    // Read the timestamp (32-bit value split across two registers)
    let timestamp_low = input_regs[3] as u32;
    let timestamp_high = input_regs[4] as u32;
    let timestamp = (timestamp_high << 16) | timestamp_low;
    
    // Format the timestamp for display
    use chrono::{DateTime, TimeZone, Utc};
    let datetime: DateTime<Utc> = Utc.timestamp_opt(timestamp as i64, 0).unwrap();
    println!("Timestamp: {} ({})", timestamp, datetime.format("%Y-%m-%d %H:%M:%S"));
    
    // Display status
    let status_codes = ["Normal", "Warning", "Error"];
    let status = input_regs[5] as usize;
    let status_str = status_codes.get(status).unwrap_or(&"Unknown");
    println!("Status: {} (code: {})", status_str, status);
    
    // Read holding registers (configuration values that can be modified)
    println!("\n--- Reading Holding Registers (configuration values) ---");
    let holding_regs = ctx.read_holding_registers(0, 4).await??;
    println!("Holding register 0: {}", holding_regs[0]);
    println!("Holding register 1: {}", holding_regs[1]);
    println!("Holding register 2: {}", holding_regs[2]);
    println!("Holding register 3: {}", holding_regs[3]);
    
    // Write to a holding register
    println!("\n--- Writing to Holding Register ---");
    let new_value = 42;
    println!("Writing value {} to holding register 2", new_value);
    ctx.write_single_register(2, new_value).await??;
    
    // Read back the value to verify
    let updated_regs = ctx.read_holding_registers(2, 1).await??;
    println!("Updated holding register 2: {}", updated_regs[0]);
    assert_eq!(updated_regs[0], new_value);
    
    // Write to multiple holding registers
    println!("\n--- Writing to Multiple Holding Registers ---");
    let new_values = vec![100, 200, 300];
    println!("Writing values {:?} to holding registers 1-3", new_values);
    ctx.write_multiple_registers(1, &new_values).await??;
    
    // Read back the values to verify
    let updated_regs = ctx.read_holding_registers(1, 3).await??;
    println!("Updated holding registers 1-3: {:?}", &updated_regs);
    assert_eq!(updated_regs, new_values);
    
    // Clean up
    println!("\nDisconnecting from server");
    ctx.disconnect().await?;
    
    println!("Done!");
    Ok(())
}
