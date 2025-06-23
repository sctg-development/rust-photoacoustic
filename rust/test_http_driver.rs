use rust_photoacoustic::processing::computing_nodes::display_drivers::{
    DisplayData, DisplayDriver, HttpsCallbackActionDriver,
};
use std::collections::HashMap;
use std::time::Duration;
use std::time::SystemTime;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    println!("Testing HTTP Display Driver...");

    // Create HTTP driver
    let mut driver = HttpsCallbackActionDriver::new(
        "https://httpbin.org/post".to_string(), // Using httpbin.org as a test endpoint
        None,
        None,
        2000,
        1,
        false,
    );

    // Create test display data
    let display_data = DisplayData {
        concentration_ppm: 456.78,
        source_node_id: "test_node".to_string(),
        peak_amplitude: 0.75,
        peak_frequency: 1000.0,
        timestamp: SystemTime::now(),
        metadata: HashMap::new(),
    };

    println!("Sending HTTP request to httpbin.org...");

    // Send the data
    match driver.update_display(&display_data).await {
        Ok(()) => println!("✅ HTTP request sent successfully!"),
        Err(e) => println!("❌ HTTP request failed: {}", e),
    }

    // Let's also test with a local endpoint that doesn't exist to see error handling
    println!("\nTesting with non-existent endpoint...");
    let mut local_driver = HttpsCallbackActionDriver::new(
        "https://localhost:8080/api/test/web_dashboard_action".to_string(),
        None,
        None,
        1000,
        1,
        false,
    );

    match local_driver.update_display(&display_data).await {
        Ok(()) => println!("✅ Local request sent successfully!"),
        Err(e) => println!("❌ Local request failed (expected): {}", e),
    }

    Ok(())
}
