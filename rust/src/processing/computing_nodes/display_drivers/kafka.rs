//! Kafka display driver implementation
//!
//! This module implements a driver for sending display data to Apache Kafka.
//! It allows publishing concentration and alert data to Kafka topics.

use anyhow::Result;
use async_trait::async_trait;
use log::{error, info};
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
    ClientConfig,
};
use serde_json::{json, Value};
use std::fmt;
use std::time::{Duration, SystemTime};

use super::{AlertData, DisplayData, DisplayDriver};

/// Kafka display driver
///
/// Sends display data to Apache Kafka topics.
/// Useful for integrating with data processing pipelines and event streams.
// Note: FutureProducer doesn't implement Debug, so we manually implement Debug for the struct
pub struct KafkaDisplayDriver {
    /// Kafka broker list (comma separated)
    brokers: String,
    /// Topic to publish display updates to
    display_topic: String,
    /// Topic to publish alerts to
    alert_topic: String,
    /// Kafka producer for sending messages
    producer: Option<FutureProducer>,
    /// Client ID for Kafka connection
    client_id: String,
    /// Message timeout in milliseconds
    timeout_ms: u64,
    /// Connection status
    connection_status: String,
}

// Manually implement Debug for KafkaDisplayDriver since FutureProducer doesn't implement Debug
impl fmt::Debug for KafkaDisplayDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KafkaDisplayDriver")
            .field("brokers", &self.brokers)
            .field("display_topic", &self.display_topic)
            .field("alert_topic", &self.alert_topic)
            .field("producer", &self.producer.is_some())
            .field("client_id", &self.client_id)
            .field("timeout_ms", &self.timeout_ms)
            .field("connection_status", &self.connection_status)
            .finish()
    }
}

impl KafkaDisplayDriver {
    /// Create a new Kafka display driver
    ///
    /// # Arguments
    /// * `brokers` - Kafka brokers list (e.g., "localhost:9092,localhost:9093")
    /// * `display_topic` - Topic for concentration updates
    /// * `alert_topic` - Topic for alerts
    pub fn new(
        brokers: impl Into<String>,
        display_topic: impl Into<String>,
        alert_topic: impl Into<String>,
    ) -> Self {
        Self {
            brokers: brokers.into(),
            display_topic: display_topic.into(),
            alert_topic: alert_topic.into(),
            producer: None,
            client_id: format!("photoacoustic-driver-{}", uuid::Uuid::new_v4()),
            timeout_ms: 5000, // Default 5 seconds
            connection_status: "Initializing".to_string(),
        }
    }

    /// Set message timeout in milliseconds
    ///
    /// # Arguments
    /// * `timeout_ms` - Timeout in milliseconds
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set client ID for Kafka connection
    ///
    /// # Arguments
    /// * `client_id` - Client ID string
    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = client_id.into();
        self
    }

    // Helper method to create a producer if it doesn't exist
    fn ensure_producer(&mut self) -> Result<&FutureProducer> {
        if self.producer.is_none() {
            // Create Kafka producer
            let producer: FutureProducer = ClientConfig::new()
                .set("bootstrap.servers", &self.brokers)
                .set("client.id", &self.client_id)
                .set("message.timeout.ms", &self.timeout_ms.to_string())
                .create()?;

            self.producer = Some(producer);
            self.connection_status = "Producer created".to_string();
        }

        Ok(self.producer.as_ref().unwrap())
    }

    // Helper to send a message to a topic
    async fn send_to_topic(&mut self, topic: &str, key: &str, payload: &str) -> Result<()> {
        // Store timeout_ms in a local variable to avoid borrowing self later
        let timeout_ms = self.timeout_ms;
        let producer = self.ensure_producer()?;

        let record = FutureRecord::to(topic).key(key).payload(payload);

        match producer
            .send(record, Timeout::After(Duration::from_millis(timeout_ms)))
            .await
        {
            Ok((_partition, _offset)) => {
                self.connection_status = format!(
                    "Connected - Last message sent: {}",
                    chrono::Local::now().to_rfc3339()
                );
                Ok(())
            }
            Err((kafka_error, _)) => {
                let error_msg = format!("Kafka send error: {}", kafka_error);
                self.connection_status = format!("Error: {}", error_msg);
                error!("{}", error_msg);
                Err(anyhow::anyhow!(error_msg))
            }
        }
    }
}

#[async_trait]
impl DisplayDriver for KafkaDisplayDriver {
    async fn initialize(&mut self) -> Result<()> {
        // Create a producer to test connection
        self.ensure_producer()?;

        info!(
            "KafkaDisplayDriver: Producer created for brokers: {}",
            self.brokers
        );
        self.connection_status = "Producer initialized".to_string();

        Ok(())
    }

    async fn update_display(&mut self, data: &DisplayData) -> Result<()> {
        // Clone the data we need to avoid borrowing self
        let display_topic = self.display_topic.clone();

        let payload = json!({
            "type": "display_update",
            "concentration_ppm": data.concentration_ppm,
            "source_node_id": data.source_node_id,
            "peak_amplitude": data.peak_amplitude,
            "peak_frequency": data.peak_frequency,
            "timestamp": data.timestamp.duration_since(std::time::UNIX_EPOCH)?.as_secs(),
            "metadata": data.metadata
        });

        let json_str = serde_json::to_string(&payload)?;
        let key = data.source_node_id.clone();

        self.send_to_topic(&display_topic, &key, &json_str).await
    }

    async fn show_alert(&mut self, alert: &AlertData) -> Result<()> {
        // Clone the data we need to avoid borrowing self
        let alert_topic = self.alert_topic.clone();

        let payload = json!({
            "type": "alert",
            "alert_type": alert.alert_type,
            "severity": alert.severity,
            "message": alert.message,
            "data": alert.data,
            "timestamp": alert.timestamp.duration_since(std::time::UNIX_EPOCH)?.as_secs()
        });

        let json_str = serde_json::to_string(&payload)?;
        let key = alert.alert_type.clone();

        self.send_to_topic(&alert_topic, &key, &json_str).await
    }

    async fn clear_display(&mut self) -> Result<()> {
        // Clone the data we need to avoid borrowing self
        let display_topic = self.display_topic.clone();

        let payload = json!({
            "type": "clear_display",
            "timestamp": SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs()
        });

        let json_str = serde_json::to_string(&payload)?;

        self.send_to_topic(&display_topic, "clear", &json_str).await
    }

    async fn get_status(&self) -> Result<Value> {
        Ok(json!({
            "driver_type": self.driver_type(),
            "brokers": self.brokers,
            "display_topic": self.display_topic,
            "alert_topic": self.alert_topic,
            "client_id": self.client_id,
            "timeout_ms": self.timeout_ms,
            "connection_status": self.connection_status,
            "is_connected": self.producer.is_some(),
        }))
    }

    fn driver_type(&self) -> &str {
        "kafka"
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Kafka producer is dropped automatically
        self.producer = None;
        Ok(())
    }
}
