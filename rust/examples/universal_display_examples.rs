//! Example usage of UniversalActionNode with different drivers
//!
//! This example demonstrates how to configure and use the UniversalActionNode
//! with various display drivers for different output scenarios.

use anyhow::Result;
use env_logger;
use rust_photoacoustic::processing::{
    computing_nodes::{
        display_drivers::{HttpsCallbackActionDriver, KafkaActionDriver, RedisActionDriver},
        UniversalActionNode,
    },
    ProcessingNode,
};

/// Example: HTTP callback driver for web dashboard integration
pub fn create_web_dashboard_display() -> Result<UniversalActionNode> {
    // Configure HTTP driver for remote dashboard
    let http_driver = HttpsCallbackActionDriver::new(
        "https://dashboard.mycompany.com/api/photoacoustic/display",
    )
    .with_auth_token("Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...")
    .with_timeout_seconds(5)
    .with_retry_count(3);

    let display_node = UniversalActionNode::new("web_dashboard".to_string())
        .with_history_buffer_capacity(300) // 5 minutes at 1Hz
        .with_driver(Box::new(http_driver))
        .with_concentration_threshold(1000.0) // Alert at 1000 ppm CO2
        .with_amplitude_threshold(0.85) // Alert at 85% amplitude
        .with_monitored_node("co2_concentration".to_string())
        .with_monitored_node("ch4_concentration".to_string())
        .with_update_interval(1000); // Update every second

    Ok(display_node)
}

/// Example: Redis driver for real-time data streaming
pub fn create_redis_stream_display() -> Result<UniversalActionNode> {
    // Configure Redis driver for real-time streaming
    let redis_driver = RedisActionDriver::new_pubsub(
        "redis://redis-cluster.company.com:6379",
        "photoacoustic:realtime:sensor_data",
    )
    .with_expiration_seconds(3600); // Data expires after 1 hour

    let display_node = UniversalActionNode::new("redis_stream".to_string())
        .with_history_buffer_capacity(100) // Minimal buffer for streaming
        .with_driver(Box::new(redis_driver))
        .with_concentration_threshold(500.0) // Lower threshold for streaming
        .with_monitored_node("real_time_concentration".to_string())
        .with_update_interval(500); // High-frequency updates

    Ok(display_node)
}

/// Example: Kafka driver for enterprise event streaming
pub fn create_kafka_event_display() -> Result<UniversalActionNode> {
    // Configure Kafka driver for enterprise integration
    let kafka_driver = KafkaActionDriver::new(
        "kafka1.company.com:9092,kafka2.company.com:9092",
        "industrial.sensors.photoacoustic.display",
        "industrial.sensors.photoacoustic.alerts", // Alert topic for important messages
    );

    let display_node = UniversalActionNode::new("kafka_events".to_string())
        .with_history_buffer_capacity(1000) // Longer history for events
        .with_driver(Box::new(kafka_driver))
        .with_concentration_threshold(750.0)
        .with_amplitude_threshold(0.9) // High threshold for events
        .with_monitored_node("primary_concentration".to_string())
        .with_monitored_node("backup_concentration".to_string())
        .with_update_interval(2000); // Event-driven updates

    Ok(display_node)
}

/// Example: Multiple display outputs with different drivers
pub async fn setup_multi_output_system() -> Result<Vec<UniversalActionNode>> {
    let mut displays = Vec::new();

    // Web dashboard for operators
    let web_display = create_web_dashboard_display()?;
    displays.push(web_display);

    // Redis stream for real-time monitoring
    let redis_display = create_redis_stream_display()?;
    displays.push(redis_display);

    // Kafka events for enterprise integration
    let kafka_display = create_kafka_event_display()?;
    displays.push(kafka_display);

    println!(
        "Multi-output display system initialized with {} outputs",
        displays.len()
    );

    Ok(displays)
}

/// Example: Configuration-driven driver selection
pub fn create_display_from_config(config: &DisplayConfig) -> Result<UniversalActionNode> {
    let driver: Box<
        dyn rust_photoacoustic::processing::computing_nodes::display_drivers::DisplayDriver,
    > = match config.driver_type.as_str() {
        "http" => Box::new(
            HttpsCallbackActionDriver::new(&config.endpoint)
                .with_timeout_seconds(config.timeout_ms.unwrap_or(5000) / 1000),
        ),
        "redis" => Box::new(RedisActionDriver::new_pubsub(
            &config.endpoint,
            &config
                .channel
                .clone()
                .unwrap_or_else(|| "photoacoustic:display".to_string()),
        )),
        "kafka" => {
            let display_topic = config
                .topic
                .clone()
                .unwrap_or_else(|| "photoacoustic.display".to_string());

            // If there's an explicit alert topic, use new(), otherwise use default alert topic
            if let Some(alert_topic) = &config.alert_topic {
                Box::new(KafkaActionDriver::new(
                    &config.endpoint,
                    &display_topic,
                    alert_topic,
                ))
            } else {
                // Default alert topic is display_topic + ".alerts"
                let default_alert_topic = format!("{}.alerts", display_topic);
                Box::new(KafkaActionDriver::new(
                    &config.endpoint,
                    &display_topic,
                    &default_alert_topic,
                ))
            }
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported driver type: {}",
                config.driver_type
            ))
        }
    };

    let display_node = UniversalActionNode::new(config.node_id.clone())
        .with_history_buffer_capacity(config.buffer_capacity.unwrap_or(100))
        .with_driver(driver)
        .with_concentration_threshold(config.concentration_threshold.unwrap_or(1000.0))
        .with_amplitude_threshold(config.amplitude_threshold.unwrap_or(0.8))
        .with_update_interval(config.update_interval_ms.unwrap_or(1000));

    Ok(display_node)
}

/// Configuration structure for driver selection
#[derive(Debug, Clone)]
pub struct DisplayConfig {
    pub node_id: String,
    pub driver_type: String,         // "http", "redis", "kafka"
    pub endpoint: String,            // URL, connection string, or bootstrap servers
    pub channel: Option<String>,     // Redis channel
    pub topic: Option<String>,       // Kafka topic
    pub alert_topic: Option<String>, // Kafka alert topic
    pub timeout_ms: Option<u64>,
    pub buffer_capacity: Option<usize>,
    pub concentration_threshold: Option<f64>,
    pub amplitude_threshold: Option<f32>,
    pub update_interval_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_web_dashboard_creation() {
        let display = create_web_dashboard_display().expect("Failed to create web dashboard");
        assert_eq!(display.node_id(), "web_dashboard");
        assert!(display.has_driver());
    }

    #[tokio::test]
    async fn test_redis_stream_creation() {
        let display = create_redis_stream_display().expect("Failed to create Redis stream");
        assert_eq!(display.node_id(), "redis_stream");
        assert!(display.has_driver());
    }

    #[tokio::test]
    async fn test_config_driven_creation() {
        let config = DisplayConfig {
            node_id: "test_display".to_string(),
            driver_type: "http".to_string(),
            endpoint: "https://test.example.com/api/display".to_string(),
            channel: None,
            topic: None,
            alert_topic: None, // New field
            timeout_ms: Some(3000),
            buffer_capacity: Some(50),
            concentration_threshold: Some(800.0),
            amplitude_threshold: Some(0.75),
            update_interval_ms: Some(2000),
        };

        let display = create_display_from_config(&config).expect("Failed to create from config");
        assert_eq!(display.node_id(), "test_display");
        assert!(display.has_driver());
    }
}

/// Main function to demonstrate the universal display examples
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    println!("Universal Display ActionNode Examples");
    println!("=====================================");

    // Example 1: Web Dashboard Display
    println!("\n1. Creating Web Dashboard Display...");
    let web_display = create_web_dashboard_display()?;
    println!(
        "   ✓ Web dashboard display created with ID: {}",
        web_display.node_id()
    );
    println!("   ✓ Driver configured: {}", web_display.has_driver());

    // Example 2: Redis Stream Display
    println!("\n2. Creating Redis Stream Display...");
    let redis_display = create_redis_stream_display()?;
    println!(
        "   ✓ Redis stream display created with ID: {}",
        redis_display.node_id()
    );
    println!("   ✓ Driver configured: {}", redis_display.has_driver());

    // Example 3: Kafka Events Display
    println!("\n3. Creating Kafka Events Display...");
    let kafka_display = create_kafka_event_display()?;
    println!(
        "   ✓ Kafka events display created with ID: {}",
        kafka_display.node_id()
    );
    println!("   ✓ Driver configured: {}", kafka_display.has_driver());

    // Example 4: Multi-output system
    println!("\n4. Setting up Multi-output System...");
    let displays = setup_multi_output_system().await?;
    println!(
        "   ✓ Multi-output system initialized with {} displays",
        displays.len()
    );

    // Example 5: Configuration-driven creation
    println!("\n5. Creating Display from Configuration...");
    let config = DisplayConfig {
        node_id: "config_driven_display".to_string(),
        driver_type: "http".to_string(),
        endpoint: "https://test.example.com/api/display".to_string(),
        channel: None,
        topic: None,
        alert_topic: None, // New field
        timeout_ms: Some(3000),
        buffer_capacity: Some(50),
        concentration_threshold: Some(800.0),
        amplitude_threshold: Some(0.75),
        update_interval_ms: Some(2000),
    };

    let config_display = create_display_from_config(&config)?;
    println!(
        "   ✓ Configuration-driven display created with ID: {}",
        config_display.node_id()
    );
    println!("   ✓ Driver type: {}", config.driver_type);

    println!("\n✅ All examples completed successfully!");
    println!("\nNext steps:");
    println!("- Initialize drivers with .initialize_driver().await");
    println!("- Add nodes to a ProcessingGraph for data processing");
    println!("- Configure monitoring and alerting thresholds");
    println!("- Connect to real data sources for live operation");

    Ok(())
}
