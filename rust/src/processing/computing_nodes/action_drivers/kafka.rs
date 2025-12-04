// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Kafka display driver implementation
//!
//! This module implements a driver for sending display data to Apache Kafka.
//! It allows publishing concentration and alert data to Kafka topics.

use anyhow::Result;
use async_trait::async_trait;
use log::{error, info};
use rdkafka::message::OwnedMessage;
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
    ClientConfig,
};
use serde_json::{json, Value};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use super::{ActionDriver, AlertData, MeasurementData};

/// Kafka display driver
///
/// Sends display data to Apache Kafka topics.
/// Useful for integrating with data processing pipelines and event streams.
// Note: FutureProducer doesn't implement Debug, so we manually implement Debug for the struct
pub struct KafkaActionDriver {
    /// Kafka broker list (comma separated)
    brokers: String,
    /// Topic to publish display updates to
    display_topic: String,
    /// Topic to publish alerts to
    alert_topic: String,
    /// Kafka producer wrapper for sending messages
    producer: Option<Arc<dyn ProducerLike>>,
    /// Client ID for Kafka connection
    client_id: String,
    /// Message timeout in milliseconds
    timeout_ms: u64,
    /// Connection status
    connection_status: String,
}

// Manually implement Debug for KafkaActionDriver since FutureProducer doesn't implement Debug
impl fmt::Debug for KafkaActionDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KafkaActionDriver")
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

impl KafkaActionDriver {
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
    fn ensure_producer(&mut self) -> Result<Arc<dyn ProducerLike>> {
        if self.producer.is_none() {
            // Create Kafka producer
            let producer: FutureProducer = ClientConfig::new()
                .set("bootstrap.servers", &self.brokers)
                .set("client.id", &self.client_id)
                .set("message.timeout.ms", &self.timeout_ms.to_string())
                .create()?;

            let real = RealProducer::new(producer);
            self.producer = Some(Arc::new(real));
            self.connection_status = "Producer created".to_string();
        }

        Ok(self.producer.as_ref().unwrap().clone())
    }

    // Helper to send a message to a topic
    async fn send_to_topic(&mut self, topic: &str, key: &str, payload: &str) -> Result<()> {
        // Store timeout_ms in a local variable to avoid borrowing self later
        let timeout_ms = self.timeout_ms;
        let producer = self.ensure_producer()?;

        match producer.send(topic, key, payload, timeout_ms).await {
            // rdkafka 0.38 returns a Delivery struct on success, older versions returned (partition, offset)
            Ok(_delivery) => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use rdkafka::error::KafkaError;
    use rdkafka::message::OwnedMessage;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::SystemTime;

    struct MockProducer {
        pub calls: Mutex<Vec<(String, String, String)>>,
    }

    #[async_trait]
    impl ProducerLike for MockProducer {
        async fn send(
            &self,
            topic: &str,
            key: &str,
            payload: &str,
            _timeout_ms: u64,
        ) -> Result<(), (KafkaError, OwnedMessage)> {
            self.calls.lock().unwrap().push((
                topic.to_string(),
                key.to_string(),
                payload.to_string(),
            ));
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_update_action_pub_and_status() {
        let mut driver = KafkaActionDriver::new("localhost:9092", "displays", "alerts");
        let mock = Arc::new(MockProducer {
            calls: Mutex::new(Vec::new()),
        });
        driver.set_producer_for_test(mock.clone());

        let mut metadata = HashMap::new();
        metadata.insert("k".to_string(), serde_json::json!("v"));

        let data = MeasurementData {
            concentration_ppm: 12.34,
            source_node_id: "node-1".to_string(),
            peak_amplitude: 0.5,
            peak_frequency: 1000.0,
            timestamp: SystemTime::now(),
            metadata,
        };

        let res = driver.update_action(&data).await;
        assert!(res.is_ok());
        assert!(driver
            .get_status()
            .await
            .unwrap()
            .get("is_connected")
            .unwrap()
            .as_bool()
            .unwrap());
        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "displays");
    }

    #[tokio::test]
    async fn test_show_and_clear_alert() {
        let mut driver = KafkaActionDriver::new("localhost:9092", "displays", "alerts");
        let mock = Arc::new(MockProducer {
            calls: Mutex::new(Vec::new()),
        });
        driver.set_producer_for_test(mock.clone());

        let alert = AlertData {
            alert_type: "test_alert".to_string(),
            severity: "info".to_string(),
            message: "testing".to_string(),
            data: HashMap::new(),
            timestamp: SystemTime::now(),
        };

        let res = driver.show_alert(&alert).await;
        assert!(res.is_ok());

        let res = driver.clear_action().await;
        assert!(res.is_ok());

        let calls = mock.calls.lock().unwrap();
        assert!(calls.iter().any(|call| call.0 == "alerts"));
        assert!(calls.iter().any(|call| call.0 == "displays"));
    }
}

/// Lightweight abstraction over a producer to allow test mocks
#[async_trait]
pub trait ProducerLike: Send + Sync {
    async fn send(
        &self,
        topic: &str,
        key: &str,
        payload: &str,
        timeout_ms: u64,
    ) -> Result<(), (rdkafka::error::KafkaError, OwnedMessage)>;
}

/// Real producer wrapper for actual rdkafka FutureProducer
pub struct RealProducer {
    inner: FutureProducer,
}

impl RealProducer {
    pub fn new(producer: FutureProducer) -> Self {
        Self { inner: producer }
    }
}

#[async_trait]
impl ProducerLike for RealProducer {
    async fn send(
        &self,
        topic: &str,
        key: &str,
        payload: &str,
        timeout_ms: u64,
    ) -> Result<(), (rdkafka::error::KafkaError, OwnedMessage)> {
        let record = FutureRecord::to(topic).key(key).payload(payload);
        match self
            .inner
            .send(record, Timeout::After(Duration::from_millis(timeout_ms)))
            .await
        {
            Ok(_) => Ok(()),
            Err((kafka_error, owned_msg)) => Err((kafka_error, owned_msg)),
        }
    }
}

impl KafkaActionDriver {
    /// Set a custom producer (used for tests/mocks)
    pub fn set_producer_for_test(&mut self, producer: Arc<dyn ProducerLike>) {
        self.producer = Some(producer);
    }
}

#[async_trait]
impl ActionDriver for KafkaActionDriver {
    async fn initialize(&mut self) -> Result<()> {
        // Create a producer to test connection
        self.ensure_producer()?;

        info!(
            "KafkaActionDriver: Producer created for brokers: {}",
            self.brokers
        );
        self.connection_status = "Producer initialized".to_string();

        Ok(())
    }

    async fn update_action(&mut self, data: &MeasurementData) -> Result<()> {
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

    async fn clear_action(&mut self) -> Result<()> {
        // Clone the data we need to avoid borrowing self
        let display_topic = self.display_topic.clone();

        let payload = json!({
            "type": "clear_action",
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
