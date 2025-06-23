// Test direct du RedisActionDriver en mode key_value
use rust_photoacoustic::processing::computing_nodes::display_drivers::redis::RedisActionDriver;
use rust_photoacoustic::processing::computing_nodes::display_drivers::{
    DisplayData, DisplayDriver,
};
use serde_json::json;
use std::time::SystemTime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🧪 Testing RedisActionDriver in key_value mode...");

    // Create Redis driver in key_value mode (same as config)
    let mut driver = RedisActionDriver::new_key_value(
        "redis://localhost:6379",
        "photoacoustic:realtime:sensor_data",
    )
    .with_expiration_seconds(3600);

    // Initialize
    println!("📡 Initializing Redis driver...");
    match driver.initialize().await {
        Ok(()) => println!("✅ Driver initialized successfully"),
        Err(e) => {
            println!("❌ Driver initialization failed: {}", e);
            return Err(e.into());
        }
    }

    // Get status
    if let Ok(status) = driver.get_status().await {
        println!("📊 Driver status:");
        println!("{}", serde_json::to_string_pretty(&status)?);
    }

    // Create test data
    let test_data = DisplayData {
        concentration_ppm: 25.67,
        source_node_id: "concentration_calculator".to_string(),
        peak_amplitude: 62.3,
        peak_frequency: 2150.0,
        timestamp: SystemTime::now(),
        metadata: json!({"test": true, "node": "test_driver"}),
    };

    // Send data
    println!("📤 Sending test data...");
    match driver.update_display(&test_data).await {
        Ok(()) => println!("✅ Data sent successfully"),
        Err(e) => {
            println!("❌ Failed to send data: {}", e);
            return Err(e.into());
        }
    }

    // Send a second data point with different timestamp
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let test_data2 = DisplayData {
        concentration_ppm: 26.12,
        source_node_id: "final_peak_detector".to_string(),
        peak_amplitude: 63.8,
        peak_frequency: 2200.0,
        timestamp: SystemTime::now(),
        metadata: json!({"test": true, "node": "test_driver_2"}),
    };

    println!("📤 Sending second test data...");
    match driver.update_display(&test_data2).await {
        Ok(()) => println!("✅ Second data sent successfully"),
        Err(e) => {
            println!("❌ Failed to send second data: {}", e);
            return Err(e.into());
        }
    }

    println!("\n🔍 Test completed. Check Redis with:");
    println!(
        "./target/release/redis_viewer --pattern \"photoacoustic:realtime:sensor_data*\" --json"
    );

    Ok(())
}
