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
pub struct RedisActionDriver {
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

impl RedisActionDriver {
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

    // Helper method to get a valid Redis connection with reconnection logic
    async fn get_connection(&mut self) -> Result<&mut MultiplexedConnection> {
        // First, check if we have a connection and if it's still valid
        let connection_is_valid = if let Some(ref mut conn) = self.connection {
            // Test the connection with a simple command
            let ping_result: Result<String, redis::RedisError> =
                redis::cmd("PING").query_async(conn).await;

            ping_result.is_ok()
        } else {
            false
        };

        if connection_is_valid {
            // Connection is still valid, return it
            return Ok(self.connection.as_mut().unwrap());
        } else {
            // Connection is broken or doesn't exist
            if self.connection.is_some() {
                warn!("RedisActionDriver: Connection lost, attempting to reconnect...");
                self.connection_status = "Connection lost, reconnecting...".to_string();
            }
            self.connection = None;
        }

        // Create client if needed
        if self.client.is_none() {
            self.client = Some(redis::Client::open(self.url.clone())?);
            self.connection_status = "Client created".to_string();
        }

        // Attempt to connect/reconnect
        let client = self.client.as_ref().unwrap();
        match client.get_multiplexed_async_connection().await {
            Ok(conn) => {
                self.connection = Some(conn);
                self.connection_status =
                    format!("Connected - {}", chrono::Local::now().to_rfc3339());
                info!("RedisActionDriver: Successfully reconnected to Redis");
            }
            Err(e) => {
                self.connection_status = format!("Connection error: {}", e);
                error!("Redis connection error: {}", e);
                return Err(anyhow::anyhow!("Redis connection error: {}", e));
            }
        }

        // Safe to unwrap now because we just created it
        Ok(self.connection.as_mut().unwrap())
    }
}

#[async_trait]
impl DisplayDriver for RedisActionDriver {
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
                info!("RedisActionDriver: Successfully connected to Redis");
                self.connection_status = "Connected and verified".to_string();
                Ok(())
            }
            Err(e) => {
                warn!("RedisActionDriver: Connection test failed: {}", e);
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

        // Try to send the data with automatic reconnection
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 2;

        loop {
            // Get the connection (this will reconnect if needed)
            match self.get_connection().await {
                Ok(conn) => {
                    let result = match mode {
                        RedisDriverMode::PubSub => {
                            // Publish to Redis channel
                            let publish_result: Result<(), redis::RedisError> =
                                conn.publish(&channel_or_prefix, &json_str).await;
                            publish_result
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

                            let set_result: Result<(), redis::RedisError> =
                                if let Some(exp_secs) = expiration_seconds {
                                    conn.set_ex(&key, &json_str, exp_secs).await
                                } else {
                                    conn.set(&key, &json_str).await
                                };

                            if set_result.is_ok() {
                                // Also update a "latest" key
                                let latest_key =
                                    format!("{}:latest:{}", channel_or_prefix, data.source_node_id);
                                if let Some(exp_secs) = expiration_seconds {
                                    let _: Result<(), redis::RedisError> =
                                        conn.set_ex(&latest_key, &json_str, exp_secs).await;
                                } else {
                                    let _: Result<(), redis::RedisError> =
                                        conn.set(&latest_key, &json_str).await;
                                }
                            }
                            set_result
                        }
                    };

                    match result {
                        Ok(_) => return Ok(()), // Success!
                        Err(e) => {
                            retry_count += 1;
                            if retry_count >= MAX_RETRIES {
                                return Err(anyhow::anyhow!(
                                    "Redis operation failed after {} retries: {}",
                                    MAX_RETRIES,
                                    e
                                ));
                            }

                            warn!(
                                "Redis operation failed (attempt {}/{}), retrying: {}",
                                retry_count, MAX_RETRIES, e
                            );
                            // Mark connection as invalid to force reconnection on next attempt
                            self.connection = None;
                            continue;
                        }
                    }
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= MAX_RETRIES {
                        return Err(anyhow::anyhow!(
                            "Redis connection failed after {} retries: {}",
                            MAX_RETRIES,
                            e
                        ));
                    }

                    warn!(
                        "Redis connection failed (attempt {}/{}), retrying: {}",
                        retry_count, MAX_RETRIES, e
                    );
                    // Small delay before retry
                    tokio::time::sleep(std::time::Duration::from_millis(100 * retry_count as u64))
                        .await;
                    continue;
                }
            }
        }
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

        // Try to send the alert with automatic reconnection
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 2;

        loop {
            // Get the connection (this will reconnect if needed)
            match self.get_connection().await {
                Ok(conn) => {
                    let result = match mode {
                        RedisDriverMode::PubSub => {
                            // Publish to Redis channel with alert prefix
                            let alert_channel = format!("{}:alert", channel_or_prefix);
                            let publish_result: Result<(), redis::RedisError> =
                                conn.publish(&alert_channel, &json_str).await;
                            publish_result
                        }
                        RedisDriverMode::KeyValue => {
                            // Store with timestamp-based key
                            let ts = alert
                                .timestamp
                                .duration_since(std::time::UNIX_EPOCH)?
                                .as_secs();
                            let key = format!("{}:alert:{}", channel_or_prefix, ts);

                            let set_result: Result<(), redis::RedisError> =
                                if let Some(exp_secs) = expiration_seconds {
                                    conn.set_ex(&key, &json_str, exp_secs).await
                                } else {
                                    conn.set(&key, &json_str).await
                                };

                            if set_result.is_ok() {
                                // Also update a "latest_alert" key
                                let latest_key = format!("{}:latest_alert", channel_or_prefix);
                                if let Some(exp_secs) = expiration_seconds {
                                    let _: Result<(), redis::RedisError> =
                                        conn.set_ex(&latest_key, &json_str, exp_secs).await;
                                } else {
                                    let _: Result<(), redis::RedisError> =
                                        conn.set(&latest_key, &json_str).await;
                                }
                            }
                            set_result
                        }
                    };

                    match result {
                        Ok(_) => return Ok(()), // Success!
                        Err(e) => {
                            retry_count += 1;
                            if retry_count >= MAX_RETRIES {
                                return Err(anyhow::anyhow!(
                                    "Redis alert operation failed after {} retries: {}",
                                    MAX_RETRIES,
                                    e
                                ));
                            }

                            warn!(
                                "Redis alert operation failed (attempt {}/{}), retrying: {}",
                                retry_count, MAX_RETRIES, e
                            );
                            // Mark connection as invalid to force reconnection on next attempt
                            self.connection = None;
                            continue;
                        }
                    }
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= MAX_RETRIES {
                        return Err(anyhow::anyhow!(
                            "Redis connection failed for alert after {} retries: {}",
                            MAX_RETRIES,
                            e
                        ));
                    }

                    warn!(
                        "Redis connection failed for alert (attempt {}/{}), retrying: {}",
                        retry_count, MAX_RETRIES, e
                    );
                    // Small delay before retry
                    tokio::time::sleep(std::time::Duration::from_millis(100 * retry_count as u64))
                        .await;
                    continue;
                }
            }
        }
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

        // Try to send the clear command with automatic reconnection
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 2;

        loop {
            // Get the connection (this will reconnect if needed)
            match self.get_connection().await {
                Ok(conn) => {
                    let result: Result<(), redis::RedisError> = match mode {
                        RedisDriverMode::PubSub => {
                            // Publish clear command to Redis channel
                            conn.publish(&channel_or_prefix, &json_str).await
                        }
                        RedisDriverMode::KeyValue => {
                            // Only update the latest key with clear command
                            let latest_key = format!("{}:latest", channel_or_prefix);
                            if let Some(exp_secs) = expiration_seconds {
                                conn.set_ex(&latest_key, &json_str, exp_secs).await
                            } else {
                                conn.set(&latest_key, &json_str).await
                            }
                        }
                    };

                    match result {
                        Ok(_) => return Ok(()), // Success!
                        Err(e) => {
                            retry_count += 1;
                            if retry_count >= MAX_RETRIES {
                                return Err(anyhow::anyhow!(
                                    "Redis clear operation failed after {} retries: {}",
                                    MAX_RETRIES,
                                    e
                                ));
                            }

                            warn!(
                                "Redis clear operation failed (attempt {}/{}), retrying: {}",
                                retry_count, MAX_RETRIES, e
                            );
                            // Mark connection as invalid to force reconnection on next attempt
                            self.connection = None;
                            continue;
                        }
                    }
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= MAX_RETRIES {
                        return Err(anyhow::anyhow!(
                            "Redis connection failed for clear after {} retries: {}",
                            MAX_RETRIES,
                            e
                        ));
                    }

                    warn!(
                        "Redis connection failed for clear (attempt {}/{}), retrying: {}",
                        retry_count, MAX_RETRIES, e
                    );
                    // Small delay before retry
                    tokio::time::sleep(std::time::Duration::from_millis(100 * retry_count as u64))
                        .await;
                    continue;
                }
            }
        }
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
