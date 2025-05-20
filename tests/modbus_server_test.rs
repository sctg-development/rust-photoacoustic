// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Tests for the PhotoacousticModbusServer implementation
//!
//! These tests validate the Modbus server functionality by starting a server
//! instance and connecting to it via a Modbus client. Various Modbus operations
//! are tested including reading input registers, reading holding registers,
//! writing to holding registers, and testing error conditions.

use tokio::net::TcpListener;
use tokio_modbus::{
    prelude::*,
    server::tcp::{accept_tcp_connection, Server},
};
use std::time::Duration;
use std::{net::SocketAddr, sync::Arc};
use std::str::FromStr;
use tokio::time;

use rust_photoacoustic::modbus::PhotoacousticModbusServer;

// This allows us to use #[tokio::test]
extern crate tokio;

/// Test utility function to start a Modbus server in the background
async fn start_test_server() -> Result<(SocketAddr, tokio::task::JoinHandle<()>), Box<dyn std::error::Error>> {
    // Use port 0 to let the OS assign an available port
    let socket_addr = SocketAddr::from_str("127.0.0.1:0").unwrap();
    let listener = TcpListener::bind(socket_addr).await?;
    
    // Get the assigned port
    let socket_addr = listener.local_addr()?;
    println!("Test server started on: {}", socket_addr);
    
    let server = Server::new(listener);
    let photoacoustic_modbus_service = |_socket_addr| {
        Ok(Some(PhotoacousticModbusServer::new()))
    };
    
    let on_connected = move |stream, socket_addr| async move {
        accept_tcp_connection(stream, socket_addr, photoacoustic_modbus_service)
    };
    
    let on_process_error = |err| {
        eprintln!("Server error: {}", err);
    };
    
    // Start the server in a background task
    let handle = tokio::spawn(async move {
        if let Err(e) = server.serve(&on_connected, on_process_error).await {
            eprintln!("Server error: {}", e);
        }
    });
    
    // Give the server a moment to start
    time::sleep(Duration::from_millis(100)).await;
    
    Ok((socket_addr, handle))
}

#[tokio::test]
async fn test_read_input_registers() -> Result<(), Box<dyn std::error::Error>> {
    let (socket_addr, _server_handle) = start_test_server().await?;
    
    // Connect a client to the server
    let mut ctx = tcp::connect(socket_addr).await?;
    
    // From our implementation in modbus_server.rs, we know input registers 0 and 1 contain test values
    let data = ctx.read_input_registers(0, 6).await??;
    
    // The default values with scaling
    assert_eq!(data.len(), 6);
    assert_eq!(data[0], 1234 * 10);  // Frequency scaled by 10
    assert_eq!(data[1], 5678);       // Amplitude scaled by 1000
    assert_eq!(data[2], 1000 * 10);  // Concentration scaled by 10
    assert_eq!(data[5], 0);          // Status code should be 0 (normal)
    
    // Clean up
    ctx.disconnect().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_read_holding_registers() -> Result<(), Box<dyn std::error::Error>> {
    let (socket_addr, _server_handle) = start_test_server().await?;
    
    // Connect a client to the server
    let mut ctx = tcp::connect(socket_addr).await?;
    
    // From our implementation, we know holding registers 0-3 contain test values
    let data = ctx.read_holding_registers(0, 4).await??;
    
    // The default values should be 10, 20, 30, 40
    assert_eq!(data.len(), 4);
    assert_eq!(data[0], 10);
    assert_eq!(data[1], 20);
    assert_eq!(data[2], 30);
    assert_eq!(data[3], 40);
    
    // Clean up
    ctx.disconnect().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_write_single_register() -> Result<(), Box<dyn std::error::Error>> {
    let (socket_addr, _server_handle) = start_test_server().await?;
    
    // Connect a client to the server
    let mut ctx = tcp::connect(socket_addr).await?;
    
    // Write a value to holding register 2
    ctx.write_single_register(2, 999).await??;
    
    // Read back the value to verify it was written
    let data = ctx.read_holding_registers(2, 1).await??;
    
    // The value should be updated to 999
    assert_eq!(data.len(), 1);
    assert_eq!(data[0], 999);
    
    // Clean up
    ctx.disconnect().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_write_multiple_registers() -> Result<(), Box<dyn std::error::Error>> {
    let (socket_addr, _server_handle) = start_test_server().await?;
    
    // Connect a client to the server
    let mut ctx = tcp::connect(socket_addr).await?;
    
    // Write multiple values to holding registers
    let values = vec![101, 202, 303];
    ctx.write_multiple_registers(1, &values).await??;
    
    // Read back the values to verify they were written
    let data = ctx.read_holding_registers(1, 3).await??;
    
    // The values should be updated
    assert_eq!(data.len(), 3);
    assert_eq!(data[0], 101);
    assert_eq!(data[1], 202);
    assert_eq!(data[2], 303);
    
    // Clean up
    ctx.disconnect().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_invalid_register_address() -> Result<(), Box<dyn std::error::Error>> {
    let (socket_addr, _server_handle) = start_test_server().await?;
    
    // Connect a client to the server
    let mut ctx = tcp::connect(socket_addr).await?;
    
    // Try to read from an invalid register address
    let result = ctx.read_input_registers(100, 1).await?;
    
    // We expect an IllegalDataAddress exception
    assert!(result.is_err());
    if let Err(error) = result {
        assert_eq!(error.to_string(), "Illegal data address");
    }
    
    // Clean up
    ctx.disconnect().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_unsupported_function() -> Result<(), Box<dyn std::error::Error>> {
    let (socket_addr, _server_handle) = start_test_server().await?;
    
    // Connect a client to the server
    let mut ctx = tcp::connect(socket_addr).await?;
    
    // Try to read coils which is not supported in our implementation
    let result = ctx.read_coils(0, 1).await?;
    
    // We expect an IllegalFunction exception
    assert!(result.is_err());
    if let Err(error) = result {
        assert_eq!(error.to_string(), "Illegal function");
    }
    
    // Clean up
    ctx.disconnect().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_multiple_clients() -> Result<(), Box<dyn std::error::Error>> {
    // Use a different register for this test to avoid conflicts
    let test_register = 5;  // Using register 5 instead of 0 
    let test_value = 888;
    
    let (socket_addr, _server_handle) = start_test_server().await?;
    
    // Connect the first client
    let mut client1 = tcp::connect(socket_addr).await?;
    
    // Connect the second client
    let mut client2 = tcp::connect(socket_addr).await?;
    
    // Let's first initialize the register with a known value
    client1.write_single_register(test_register, test_value).await??;
    
    // Create a small delay to ensure proper server handling (100ms should be enough)
    time::sleep(Duration::from_millis(100)).await;
    
    // Client 2 reads the register to verify the value is there
    let data = client2.read_holding_registers(test_register, 1).await??;
    
    // Verify client 2 sees the update from client 1
    assert_eq!(data.len(), 1);
    assert_eq!(data[0], test_value);
    
    // Clean up
    client1.disconnect().await?;
    client2.disconnect().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_real_world_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let (socket_addr, _server_handle) = start_test_server().await?;
    
    // Connect a client to the server
    let mut ctx = tcp::connect(socket_addr).await?;
    
    // In a real-world scenario, we might write sensor values to input registers
    // and configuration parameters to holding registers
    
    // First, read the current values
    let initial_input = ctx.read_input_registers(0, 2).await??;
    let initial_holding = ctx.read_holding_registers(0, 4).await??;
    
    // Verify initial values
    assert_eq!(initial_input[0], 1234);  // Input register 0 contains frequency
    assert_eq!(initial_input[1], 5678);  // Input register 1 contains amplitude
    
    assert_eq!(initial_holding[0], 10);  // Holding register 0 contains some config parameter
    assert_eq!(initial_holding[1], 20);  // Holding register 1 contains another config
    
    // Simulate updating a configuration (e.g., changing sampling interval)
    ctx.write_single_register(0, 50).await??;  // Change config to 50
    
    // Read back to verify the change
    let updated_holding = ctx.read_holding_registers(0, 1).await??;
    assert_eq!(updated_holding[0], 50);
    
    // In a real implementation, this might trigger some action in the server
    // such as changing the sampling rate of the photoacoustic sensor
    
    // Clean up
    ctx.disconnect().await?;
    
    Ok(())
}
