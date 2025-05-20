// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Simulated Data Generator for Modbus Server Testing
//!
//! This example demonstrates how to:
//! 1. Start a Modbus server with simulated photoacoustic data
//! 2. Periodically update the data to simulate changing measurements
//!
//! Usage:
//!   cargo run --example modbus_simulation
//!
//! Then in another terminal, run the client:
//!   cargo run --example modbus_client

use rust_photoacoustic::modbus::PhotoacousticModbusServer;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time;
use tokio_modbus::server::tcp::{accept_tcp_connection, Server};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_addr = "127.0.0.1:502".parse().unwrap();
    let listener = TcpListener::bind(socket_addr).await?;
    println!("Starting Modbus server at {}", socket_addr);
    
    let server = Server::new(listener);
    
    // Create our Modbus server with default values
    let modbus_server = Arc::new(PhotoacousticModbusServer::new());
    let modbus_server_clone = modbus_server.clone();
    
    // Create a simulation task that updates the data
    let simulation_handle = tokio::spawn(async move {
        let mut time_counter: f32 = 0.0;
        let mut running = true;
        
        println!("Starting data simulation...");
        
        while running {
            // Simulate a resonance frequency that varies with time
            let frequency = 1000.0 + 100.0 * time_counter.sin();
            
            // Simulate amplitude varying with time but in different phase
            let amplitude = 0.5 + 0.3 * (time_counter + 1.0).sin();
            
            // Simulate concentration varying more slowly
            let concentration = 500.0 + 300.0 * (time_counter * 0.2).sin();
            
            // Update the modbus server with our simulated data
            println!("Updating with: frequency={:.1}Hz, amplitude={:.3}, concentration={:.1}ppm",
                     frequency, amplitude, concentration);
            modbus_server_clone.update_measurement_data(frequency, amplitude, concentration);
            
            // Wait for 1 second
            time::sleep(Duration::from_secs(1)).await;
            
            // Update time counter
            time_counter += 0.1;
            
            // Stop after 10 minutes (600 seconds)
            if time_counter > 600.0 {
                running = false;
            }
        }
    });
    
    // Set up the server
    let server_instance = modbus_server.clone();
    let photoacoustic_modbus_service = move |_socket_addr| {
        Ok(Some(server_instance.as_ref().clone()))
    };
    
    let on_connected = |stream, socket_addr| async move {
        println!("Client connected: {}", socket_addr);
        accept_tcp_connection(stream, socket_addr, photoacoustic_modbus_service)
    };
    
    let on_process_error = |err| {
        eprintln!("Server error: {}", err);
    };
    
    // Serve modbus requests until ctrl+c is pressed
    println!("Press Ctrl+C to stop the server");
    
    // Create a signal handler for graceful shutdown
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.serve(&on_connected, on_process_error).await {
            eprintln!("Server error: {}", e);
        }
    });
    
    // Wait for Ctrl+C
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            println!("Shutting down server...");
        }
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
        }
    }
    
    // Cleanup and exit
    server_handle.abort();
    simulation_handle.abort();
    
    Ok(())
}
