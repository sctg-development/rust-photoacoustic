// Test Redis reconnection logic
use rust_photoacoustic::processing::computing_nodes::display_drivers::redis::{RedisActionDriver, RedisDriverMode};
use rust_photoacoustic::processing::computing_nodes::display_drivers::{DisplayData, DisplayDriver};
use std::time::SystemTime;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    println!("Testing Redis reconnection logic...");
    
    // Create Redis driver
    let mut driver = RedisActionDriver::new(
        "redis://localhost:6379".to_string(),
        RedisDriverMode::KeyValue,
        "test_reconnection".to_string(),
    ).with_expiration_seconds(30);
    
    // Try to initialize
    match driver.initialize().await {
        Ok(()) => println!("✓ Initial connection successful"),
        Err(e) => {
            println!("✗ Initial connection failed: {}", e);
            println!("Make sure Redis is running on localhost:6379");
            return Err(e.into());
        }
    }
    
    // Send initial data
    let test_data = DisplayData {
        concentration_ppm: 123.45,
        source_node_id: "test_node".to_string(),
        peak_amplitude: 0.75,
        peak_frequency: 440.0,
        timestamp: SystemTime::now(),
        metadata: json!({"test": true}),
    };
    
    match driver.update_display(&test_data).await {
        Ok(()) => println!("✓ Initial data send successful"),
        Err(e) => println!("✗ Initial data send failed: {}", e),
    }
    
    // Check status
    if let Ok(status) = driver.get_status().await {
        println!("Status: {}", serde_json::to_string_pretty(&status)?);
    }
    
    println!("\nTest completed. Now manually restart your Redis server to test reconnection:");
    println!("1. Stop Redis server (docker stop redis or systemctl stop redis)");
    println!("2. Start Redis server again");
    println!("3. The driver should automatically reconnect on next operation");
    
    // Keep trying to send data in a loop to test reconnection
    for i in 1..=10 {
        println!("\nAttempt {}/10 - Sending test data...", i);
        
        let test_data = DisplayData {
            concentration_ppm: 123.45 + i as f64,
            source_node_id: format!("test_node_{}", i),
            peak_amplitude: 0.75,
            peak_frequency: 440.0,
            timestamp: SystemTime::now(),
            metadata: json!({"test": true, "attempt": i}),
        };
        
        match driver.update_display(&test_data).await {
            Ok(()) => println!("✓ Data send #{} successful", i),
            Err(e) => println!("✗ Data send #{} failed: {}", i, e),
        }
        
        // Wait 2 seconds between attempts
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    
    println!("\nReconnection test completed!");
    Ok(())
}
