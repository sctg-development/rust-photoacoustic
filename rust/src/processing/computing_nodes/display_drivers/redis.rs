//! Redis display driver implementation
//!
//! This module implements a driver for sending display data to Redis.
//! It supports both publishing to channels and storing key-value pairs.

use anyhow::Result;
use async_trait::async_trait;
use log::{error, info, warn};
use redis::AsyncCommands;
use redis::{aio::MultiplexedConnection, Client};
use serde_json::{json, Value};
use std::time::SystemTime;

use super::{AlertData, DisplayData, DisplayDriver};

/// Redis display driver modes
#[derive(Debug, Clone)]
pub enum RedisDriverMode {
    /// Publish to a Redis channel (pub/sub)
    PubSub,
    /// Store as key-value pairs
    KeyValue,
}

/// Redis display driver
///
/// Sends display data to Redis using either pub/sub channels or key-value storage.
/// Useful for real-time dashboards and data-sharing between services.
#[derive(Debug)]
pub struct RedisDisplayDriver {
    /// Redis connection URL
    url: String,
    /// Redis channel or key prefix
    channel_or_prefix: String,
    /// Operation mode (pub/sub or key-value)
    mode: RedisDriverMode,
    /// Redis client
    client: Option<Client>,
    /// Redis connection
    connection: Option<MultiplexedConnection>,
    /// Value expiration time in seconds (for key-value mode)
    expiration_seconds: Option<u64>,
    /// Connection status
    connection_status: String,
}

impl RedisDisplayDriver {
    /// Create a new Redis driver in pub/sub mode
    ///
    /// # Arguments
    /// * `url` - Redis connection URL (e.g., "redis://127.0.0.1:6379")
    /// * `channel` - Redis channel to publish to
    pub fn new_pubsub(url: impl Into<String>, channel: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            channel_or_prefix: channel.into(),
            mode: RedisDriverMode::PubSub,
            client: None,
            connection: None,
            expiration_seconds: None,
            connection_status: "Initializing".to_string(),
        }
    }

    /// Create a new Redis driver in key-value mode
    ///
    /// # Arguments
    /// * `url` - Redis connection URL (e.g., "redis://127.0.0.1:6379")
    /// * `key_prefix` - Prefix for Redis keys
    pub fn new_key_value(url: impl Into<String>, key_prefix: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            channel_or_prefix: key_prefix.into(),
            mode: RedisDriverMode::KeyValue,
            client: None,
            connection: None,
            expiration_seconds: None,
            connection_status: "Initializing".to_string(),
        }
    }

    /// Set expiration time for key-value pairs (key-value mode only)
    ///
    /// # Arguments
    /// * `seconds` - Expiration time in seconds (0 = no expiration)
    pub fn with_expiration_seconds(mut self, seconds: u64) -> Self {
        if seconds > 0 {
            self.expiration_seconds = Some(seconds);
        } else {
            self.expiration_seconds = None;
        }
        self
    }

    // Helper method to get a valid Redis connection
    async fn get_connection(&mut self) -> Result<&mut MultiplexedConnection> {
        if self.connection.is_none() {
            if self.client.is_none() {
                self.client = Some(redis::Client::open(self.url.clone())?);
                self.connection_status = "Client created".to_string();
            }

            let client = self.client.as_ref().unwrap();
            match client.get_multiplexed_async_connection().await {
                Ok(conn) => {
                    self.connection = Some(conn);
                    self.connection_status =
                        format!("Connected - {}", chrono::Local::now().to_rfc3339());
                }
                Err(e) => {
                    self.connection_status = format!("Connection error: {}", e);
                    error!("Redis connection error: {}", e);
                    return Err(anyhow::anyhow!("Redis connection error: {}", e));
                }
            }
        }

        // Safe to unwrap now because we just created it if it didn't exist
        Ok(self.connection.as_mut().unwrap())
    }
}

#[async_trait]
impl DisplayDriver for RedisDisplayDriver {
    async fn initialize(&mut self) -> Result<()> {
        // Test Redis connection
        let conn = self.get_connection().await?;

        // Simple command to verify connection works (ECHO instead of PING)
        let echo_result: Result<String, redis::RedisError> = redis::cmd("ECHO")
            .arg("connection_test")
            .query_async(conn)
            .await;

        match echo_result {
            Ok(_) => {
                info!("RedisDisplayDriver: Successfully connected to Redis");
                self.connection_status = "Connected and verified".to_string();
                Ok(())
            }
            Err(e) => {
                warn!("RedisDisplayDriver: Connection test failed: {}", e);
                self.connection_status = format!("Connection test failed: {}", e);
                Err(anyhow::anyhow!("Redis connection test failed: {}", e))
            }
        }
    }

    async fn update_display(&mut self, data: &DisplayData) -> Result<()> {
        // Clone values that we'll need after borrowing self
        let mode = self.mode.clone();
        let channel_or_prefix = self.channel_or_prefix.clone();
        let expiration_seconds = self.expiration_seconds;

        // Create the payload first
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

        // Now get the connection
        let conn = self.get_connection().await?;

        match mode {
            RedisDriverMode::PubSub => {
                // Publish to Redis channel
                let _: () = conn.publish(&channel_or_prefix, &json_str).await?;
            }
            RedisDriverMode::KeyValue => {
                // Store with timestamp-based key
                let ts = data
                    .timestamp
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs();
                let key = format!(
                    "{}:display:{}:{}",
                    channel_or_prefix, data.source_node_id, ts
                );

                if let Some(exp_secs) = expiration_seconds {
                    let _: () = conn.set_ex(&key, &json_str, exp_secs).await?;
                } else {
                    let _: () = conn.set(&key, &json_str).await?;
                }

                // Also update a "latest" key
                let latest_key = format!("{}:latest:{}", channel_or_prefix, data.source_node_id);
                if let Some(exp_secs) = expiration_seconds {
                    let _: () = conn.set_ex(&latest_key, &json_str, exp_secs).await?;
                } else {
                    let _: () = conn.set(&latest_key, &json_str).await?;
                }
            }
        }

        Ok(())
    }

    async fn show_alert(&mut self, alert: &AlertData) -> Result<()> {
        // Clone values that we'll need after borrowing self
        let mode = self.mode.clone();
        let channel_or_prefix = self.channel_or_prefix.clone();
        let expiration_seconds = self.expiration_seconds;

        let payload = json!({
            "type": "alert",
            "alert_type": alert.alert_type,
            "severity": alert.severity,
            "message": alert.message,
            "data": alert.data,
            "timestamp": alert.timestamp.duration_since(std::time::UNIX_EPOCH)?.as_secs()
        });

        let json_str = serde_json::to_string(&payload)?;

        // Get the connection after preparing the payload
        let conn = self.get_connection().await?;

        match mode {
            RedisDriverMode::PubSub => {
                // Publish to Redis channel with alert prefix
                let alert_channel = format!("{}:alert", channel_or_prefix);
                let _: () = conn.publish(&alert_channel, &json_str).await?;
            }
            RedisDriverMode::KeyValue => {
                // Store with timestamp-based key
                let ts = alert
                    .timestamp
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs();
                let key = format!("{}:alert:{}", channel_or_prefix, ts);

                if let Some(exp_secs) = expiration_seconds {
                    let _: () = conn.set_ex(&key, &json_str, exp_secs).await?;
                } else {
                    let _: () = conn.set(&key, &json_str).await?;
                }

                // Also update a "latest_alert" key
                let latest_key = format!("{}:latest_alert", channel_or_prefix);
                if let Some(exp_secs) = expiration_seconds {
                    let _: () = conn.set_ex(&latest_key, &json_str, exp_secs).await?;
                } else {
                    let _: () = conn.set(&latest_key, &json_str).await?;
                }
            }
        }

        Ok(())
    }

    async fn clear_display(&mut self) -> Result<()> {
        // Clone values that we'll need after borrowing self
        let mode = self.mode.clone();
        let channel_or_prefix = self.channel_or_prefix.clone();
        let expiration_seconds = self.expiration_seconds;

        let payload = json!({
            "type": "clear_display",
            "timestamp": SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs()
        });

        let json_str = serde_json::to_string(&payload)?;

        // Get the connection
        let conn = self.get_connection().await?;

        match mode {
            RedisDriverMode::PubSub => {
                // Publish clear command to Redis channel
                let _: () = conn.publish(&channel_or_prefix, &json_str).await?;
            }
            RedisDriverMode::KeyValue => {
                // Only update the latest key with clear command
                let latest_key = format!("{}:latest", channel_or_prefix);
                if let Some(exp_secs) = expiration_seconds {
                    let _: () = conn.set_ex(&latest_key, &json_str, exp_secs).await?;
                } else {
                    let _: () = conn.set(&latest_key, &json_str).await?;
                }
            }
        }

        Ok(())
    }

    async fn get_status(&self) -> Result<Value> {
        Ok(json!({
            "driver_type": self.driver_type(),
            "url": self.url,
            "mode": match self.mode {
                RedisDriverMode::PubSub => "pub_sub",
                RedisDriverMode::KeyValue => "key_value",
            },
            "channel_or_prefix": self.channel_or_prefix,
            "expiration_seconds": self.expiration_seconds,
            "connection_status": self.connection_status,
            "is_connected": self.connection.is_some(),
        }))
    }

    fn driver_type(&self) -> &str {
        "redis"
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Redis connections are automatically closed when dropped
        self.connection = None;
        Ok(())
    }
}
