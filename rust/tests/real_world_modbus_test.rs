// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Real-world integration test for Modbus TCP server
//!
//! This test creates a full server instance using the example configuration
//! with Modbus enabled. It tests the Modbus TCP server functionality including
//! reading input registers (measurement data) and holding registers (configuration data),
//! as well as writing configuration values.

use anyhow::Result;
use rust_photoacoustic::{
    config::Config,
    daemon::launch_daemon::Daemon,
    utility::jwt_token::{ConfigLoader, JwtAlgorithm, TokenCreationParams, TokenCreator},
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{sync::RwLock, time::sleep};
use tokio_modbus::client::{tcp::connect, Reader, Writer};

/// Integration test that starts a real server with Modbus enabled
/// and tests the Modbus TCP server functionality with live data processing
#[tokio::test]
async fn test_real_world_modbus_server() -> Result<()> {
    // Initialize logging for debugging
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    // Load the example configuration and enable Modbus
    let config_path = PathBuf::from("config.example.yaml");
    let mut config = Config::from_file(&config_path)?;

    // Enable Modbus server for testing
    config.modbus.enabled = true;

    println!("Loaded configuration from: {:?}", config_path);
    println!("Modbus enabled: {}", config.modbus.enabled);
    println!("Modbus port: {}", config.modbus.port);
    println!("Modbus address: {}", config.modbus.address);
    println!("Processing enabled: {}", config.processing.enabled);
    println!(
        "Simulated source type: {:?}",
        config
            .photoacoustic
            .simulated_source
            .as_ref()
            .map(|s| &s.source_type)
    );

    // Create shared configuration for the daemon
    let config_arc = Arc::new(RwLock::new(config.clone()));

    // Create and launch the daemon (this starts all services including Modbus)
    let mut daemon = Daemon::new();

    println!("Starting daemon with Modbus server and processing graph...");
    daemon.launch(config_arc.clone()).await?;

    // Wait for the system to initialize and start processing some frames
    // The processing graph needs time to start and accumulate data
    println!("Waiting for processing system and Modbus server to initialize...");
    sleep(Duration::from_secs(5)).await;

    // Test Modbus TCP client connections and register operations
    let modbus_address = format!("{}:{}", config.modbus.address, config.modbus.port);
    println!("Testing Modbus server at: {}", modbus_address);

    // Parse socket address for Modbus connection
    let socket_addr: std::net::SocketAddr = modbus_address.parse()?;

    // Connect to the Modbus server
    let mut ctx = match connect(socket_addr).await {
        Ok(ctx) => {
            println!("✓ Successfully connected to Modbus server");
            ctx
        }
        Err(e) => {
            eprintln!("❌ Failed to connect to Modbus server: {}", e);
            daemon.shutdown();
            daemon.join().await?;
            return Err(e.into());
        }
    };

    // Test 1: Read input registers (measurement data)
    println!("\n=== Testing Input Registers (Measurement Data) ===");

    match ctx.read_input_registers(0, 6).await {
        Ok(Ok(data)) => {
            println!("✓ Successfully read input registers");

            // Decode frequency (register 0)
            let freq_raw = data[0];
            let frequency = freq_raw as f32 / 10.0;
            println!(
                "🌊 Resonance Frequency: {} Hz (raw: {})",
                frequency, freq_raw
            );

            // Verify frequency is in expected range (should be around 2100 Hz for simulated source)
            assert!(
                frequency >= 2050.0 && frequency <= 2150.0,
                "Frequency should be in reasonable range: {}",
                frequency
            );

            // Decode amplitude (register 1)
            let amp_raw = data[1];
            let amplitude = amp_raw as f32 / 1000.0;
            println!("📈 Signal Amplitude: {} dB (raw: {})", amplitude, amp_raw);

            // Verify amplitude is in expected range
            assert!(
                amplitude >= -100.0 && amplitude <= 100.0,
                "Amplitude should be in reasonable range: {}",
                amplitude
            );

            // Decode concentration (register 2)
            let conc_raw = data[2];
            let concentration = conc_raw as f32 / 10.0;
            println!(
                "💧 Gas Concentration: {} ppm (raw: {})",
                concentration, conc_raw
            );

            // Verify concentration is in expected range
            assert!(
                concentration >= 0.0 && concentration <= 10000.0,
                "Concentration should be in reasonable range: {}",
                concentration
            );

            // Decode timestamp (registers 3-4)
            let timestamp_low = data[3] as u32;
            let timestamp_high = data[4] as u32;
            let timestamp = timestamp_low | (timestamp_high << 16);

            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as u32;
            let age_seconds = current_time.saturating_sub(timestamp);

            println!(
                "⏰ Measurement Timestamp: {} (age: {} seconds)",
                timestamp, age_seconds
            );

            // Verify timestamp is recent (within last 10 seconds)
            assert!(
                age_seconds <= 10,
                "Timestamp should be recent, age: {} seconds",
                age_seconds
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

            // Verify status is valid
            assert!(status <= 2, "Status should be valid (0-2): {}", status);
        }
        Ok(Err(e)) => {
            eprintln!("❌ Modbus exception when reading input registers: {:?}", e);
            daemon.shutdown();
            daemon.join().await?;
            return Err(anyhow::anyhow!("Modbus exception: {:?}", e));
        }
        Err(e) => {
            eprintln!("❌ Failed to read input registers: {}", e);
            daemon.shutdown();
            daemon.join().await?;
            return Err(e.into());
        }
    }

    // Test 2: Read holding registers (configuration data)
    println!("\n=== Testing Holding Registers (Configuration Data) ===");

    match ctx.read_holding_registers(0, 4).await {
        Ok(Ok(data)) => {
            println!("✓ Successfully read holding registers");
            println!("⏱️  Measurement Interval: {} seconds", data[0]);
            println!("🔢 Averaging Count: {} samples", data[1]);
            println!("📈 Gain Setting: {}", data[2]);
            println!("🎛️  Filter Strength: {}", data[3]);

            // Verify configuration values are reasonable
            assert!(
                data[0] >= 1 && data[0] <= 3600,
                "Measurement interval should be reasonable: {}",
                data[0]
            );
            assert!(
                data[1] >= 1 && data[1] <= 1000,
                "Averaging count should be reasonable: {}",
                data[1]
            );
            assert!(
                data[2] >= 0 && data[2] <= 100,
                "Gain setting should be reasonable: {}",
                data[2]
            );
            assert!(
                data[3] >= 0 && data[3] <= 100,
                "Filter strength should be reasonable: {}",
                data[3]
            );
        }
        Ok(Err(e)) => {
            eprintln!(
                "❌ Modbus exception when reading holding registers: {:?}",
                e
            );
            daemon.shutdown();
            daemon.join().await?;
            return Err(anyhow::anyhow!("Modbus exception: {:?}", e));
        }
        Err(e) => {
            eprintln!("❌ Failed to read holding registers: {}", e);
            daemon.shutdown();
            daemon.join().await?;
            return Err(e.into());
        }
    }

    // Test 3: Write single holding register (configuration write)
    println!("\n=== Testing Single Register Write ===");

    // Read current measurement interval
    let initial_interval = match ctx.read_holding_registers(0, 1).await {
        Ok(Ok(data)) => data[0],
        _ => {
            eprintln!("❌ Failed to read initial interval");
            daemon.shutdown();
            daemon.join().await?;
            return Err(anyhow::anyhow!("Failed to read initial interval"));
        }
    };

    println!("Initial measurement interval: {} seconds", initial_interval);

    // Write new measurement interval (15 seconds)
    let new_interval = 15;
    match ctx.write_single_register(0, new_interval).await {
        Ok(_) => {
            println!(
                "✓ Successfully wrote measurement interval to {} seconds",
                new_interval
            );

            // Read back to confirm
            match ctx.read_holding_registers(0, 1).await {
                Ok(Ok(data)) => {
                    println!(
                        "✓ Confirmed: Measurement interval is now {} seconds",
                        data[0]
                    );
                    assert_eq!(
                        data[0], new_interval,
                        "Written value should match read value: {} != {}",
                        data[0], new_interval
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
            daemon.shutdown();
            daemon.join().await?;
            return Err(e.into());
        }
    }

    // Test 4: Write multiple holding registers
    println!("\n=== Testing Multiple Register Write ===");

    let new_config = [10, 25, 35, 45]; // interval, averaging, gain, filter
    match ctx.write_multiple_registers(0, &new_config).await {
        Ok(_) => {
            println!("✓ Successfully updated multiple configuration values");
            println!(
                "   Interval: {} sec, Averaging: {} samples, Gain: {}, Filter: {}",
                new_config[0], new_config[1], new_config[2], new_config[3]
            );

            // Read back to verify all values
            match ctx.read_holding_registers(0, 4).await {
                Ok(Ok(data)) => {
                    println!("✓ Confirmed multiple register write:");
                    for i in 0..4 {
                        println!(
                            "   Register {}: {} (expected: {})",
                            i, data[i], new_config[i]
                        );
                        assert_eq!(
                            data[i], new_config[i],
                            "Register {} value should match: {} != {}",
                            i, data[i], new_config[i]
                        );
                    }
                }
                _ => {
                    eprintln!("❌ Failed to read back multiple registers");
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to write multiple registers: {}", e);
            daemon.shutdown();
            daemon.join().await?;
            return Err(e.into());
        }
    }

    // Test 5: Continuous monitoring to verify live data updates
    println!("\n=== Testing Continuous Data Updates ===");

    let mut previous_timestamp = 0u32;

    for i in 1..=3 {
        println!("📊 Reading #{}", i);

        match ctx.read_input_registers(0, 6).await {
            Ok(Ok(data)) => {
                let frequency = data[0] as f32 / 10.0;
                let amplitude = data[1] as f32 / 1000.0;
                let concentration = data[2] as f32 / 10.0;

                // Decode timestamp to check for updates
                let timestamp_low = data[3] as u32;
                let timestamp_high = data[4] as u32;
                let timestamp = timestamp_low | (timestamp_high << 16);

                println!(
                    "  Freq: {} Hz | Amp: {} dB | Conc: {} ppm | Time: {}",
                    frequency, amplitude, concentration, timestamp
                );

                // Verify data is being updated (timestamp should advance)
                if i > 1 {
                    assert!(
                        timestamp >= previous_timestamp,
                        "Timestamp should advance or stay same: {} >= {}",
                        timestamp,
                        previous_timestamp
                    );
                }
                previous_timestamp = timestamp;
            }
            Ok(Err(e)) => {
                eprintln!("❌ Modbus exception when reading measurement data: {:?}", e);
            }
            Err(e) => {
                eprintln!("❌ Failed to read measurement data: {}", e);
            }
        }

        sleep(Duration::from_secs(2)).await;
    }

    // Restore original configuration before shutdown
    println!("\n=== Restoring Original Configuration ===");
    match ctx.write_single_register(0, initial_interval).await {
        Ok(_) => {
            println!(
                "✓ Restored original measurement interval: {} seconds",
                initial_interval
            );
        }
        Err(e) => {
            eprintln!("⚠ Failed to restore original configuration: {}", e);
        }
    }

    // Clean shutdown
    println!("\n=== Shutting down daemon ===");
    daemon.shutdown();
    daemon.join().await?;

    println!("✓ Modbus integration test completed successfully");
    println!("🎉 All Modbus operations (read input registers, read/write holding registers) working correctly");

    Ok(())
}

/// Create a JWT token for the administrator user from the example configuration
/// (Not used in this test but kept for potential future API + Modbus combined tests)
#[allow(dead_code)]
fn create_admin_jwt_token(config: &Config) -> Result<String> {
    let config_loader = ConfigLoader::from_config(config)?;
    let token_creator = TokenCreator::new(&config_loader)?;

    let params = TokenCreationParams {
        user_id: "administrator".to_string(), // From config.example.yaml
        client_id: "LaserSmartClient".to_string(),
        algorithm: JwtAlgorithm::RS256,
        duration_seconds: 300, // 5 minutes should be enough for the test
    };

    let result = token_creator.create_token(&params)?;
    Ok(result.token)
}
