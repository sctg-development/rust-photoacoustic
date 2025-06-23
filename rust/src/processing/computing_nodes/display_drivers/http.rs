//! HTTP/HTTPS callback display driver implementation
//!
//! This module implements a driver for sending display data to external HTTP endpoints via webhooks.
//! It's useful for integration with web applications, dashboards, or cloud services.

use anyhow::Result;
use async_trait::async_trait;
use log::{info, warn};
use reqwest::header::HeaderMap;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::SystemTime;

use super::{AlertData, DisplayData, DisplayDriver};

/// HTTP/HTTPS callback display driver
///
/// Sends display data to external HTTP endpoints via webhooks.
/// Useful for integration with web applications, dashboards, or cloud services.
#[derive(Debug)]
pub struct HttpsCallbackDisplayDriver {
    /// Target webhook URL
    url: String,
    /// Optional authentication token
    auth_token: Option<String>,
    /// HTTP client for making requests
    client: reqwest::Client,
    /// Number of retry attempts for failed requests
    retry_count: u32,
    /// Timeout for HTTP requests in seconds
    timeout_seconds: u64,
    /// Custom HTTP headers to include in every request
    headers: HashMap<String, String>,
    /// Last known connection status
    connection_status: String,
}

impl HttpsCallbackDisplayDriver {
    /// Create a new HTTPS callback driver
    ///
    /// # Arguments
    /// * `url` - Target webhook URL (http:// or https://)
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            auth_token: None,
            client: reqwest::Client::new(),
            retry_count: 3,
            timeout_seconds: 10,
            headers: HashMap::new(),
            connection_status: "Initializing".to_string(),
        }
    }

    /// Set authentication token for requests
    ///
    /// # Arguments
    /// * `token` - Auth token string
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Set retry count for failed requests
    ///
    /// # Arguments
    /// * `count` - Number of retry attempts (0-10)
    pub fn with_retry_count(mut self, count: u32) -> Self {
        self.retry_count = count.min(10); // Cap at 10 retries
        self
    }

    /// Set HTTP request timeout
    ///
    /// # Arguments
    /// * `seconds` - Timeout in seconds (1-60)
    pub fn with_timeout_seconds(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds.clamp(1, 60); // 1-60 second range
        self
    }

    /// Add custom HTTP header
    ///
    /// # Arguments
    /// * `key` - Header name
    /// * `value` - Header value
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    // Helper to send a payload with retry logic
    async fn send_with_retry(&mut self, payload: &serde_json::Value) -> Result<()> {
        let mut attempts = 0;
        let max_attempts = self.retry_count + 1;

        let mut headers = HeaderMap::new();
        if let Some(ref token) = self.auth_token {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", token).parse()?,
            );
        }

        // Add custom headers
        for (key, value) in &self.headers {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(key.as_bytes())?,
                value.parse()?,
            );
        }

        loop {
            attempts += 1;

            // Add retry info to payload
            let mut payload_with_retry = payload.as_object().unwrap().clone();
            if attempts > 1 {
                payload_with_retry.insert("retry_attempt".into(), attempts.into());
            }

            let result = self
                .client
                .post(&self.url)
                .headers(headers.clone())
                .json(&payload_with_retry)
                .timeout(std::time::Duration::from_secs(self.timeout_seconds))
                .send()
                .await;

            match result {
                Ok(response) => {
                    if response.status().is_success() {
                        self.connection_status = format!(
                            "Connected - Last success: {}",
                            chrono::Local::now().to_rfc3339()
                        );
                        return Ok(());
                    } else {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_default();
                        self.connection_status = format!("Error: HTTP {}", status);

                        if attempts >= max_attempts {
                            return Err(anyhow::anyhow!(
                                "HTTP request failed after {} attempts: {} - {}",
                                attempts,
                                status,
                                error_text
                            ));
                        }

                        warn!(
                            "HTTP request failed (attempt {}/{}): {} - {}",
                            attempts, max_attempts, status, error_text
                        );
                    }
                }
                Err(e) => {
                    self.connection_status = format!("Error: {}", e);

                    if attempts >= max_attempts {
                        return Err(anyhow::anyhow!(
                            "HTTP request failed after {} attempts: {}",
                            attempts,
                            e
                        ));
                    }

                    warn!(
                        "HTTP request failed (attempt {}/{}): {}",
                        attempts, max_attempts, e
                    );
                }
            }

            // Exponential backoff (50ms, 100ms, 200ms, etc.)
            let backoff_ms = 50 * (2_u64.pow(attempts as u32 - 1));
            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
        }
    }
}

#[async_trait]
impl DisplayDriver for HttpsCallbackDisplayDriver {
    async fn initialize(&mut self) -> Result<()> {
        // Validate URL
        if !self.url.starts_with("https://") && !self.url.starts_with("http://") {
            return Err(anyhow::anyhow!(
                "Invalid URL: must start with http:// or https://"
            ));
        }

        // Test connection with a health check
        let response = self
            .client
            .get(&self.url)
            .header("User-Agent", "rust-photoacoustic-display-driver/1.0")
            .timeout(std::time::Duration::from_secs(self.timeout_seconds))
            .send()
            .await;

        match response {
            Ok(_) => {
                info!(
                    "HttpsCallbackDisplayDriver: Successfully connected to {}",
                    self.url
                );
                self.connection_status = "Connected".to_string();
                Ok(())
            }
            Err(e) => {
                warn!(
                    "HttpsCallbackDisplayDriver: Connection test failed for {}: {}",
                    self.url, e
                );
                // Don't fail initialization - the endpoint might not support GET requests
                self.connection_status = format!("Warning: Initial connection test failed: {}", e);
                Ok(())
            }
        }
    }

    async fn update_display(&mut self, data: &DisplayData) -> Result<()> {
        let payload = json!({
            "type": "display_update",
            "concentration_ppm": data.concentration_ppm,
            "source_node_id": data.source_node_id,
            "peak_amplitude": data.peak_amplitude,
            "peak_frequency": data.peak_frequency,
            "timestamp": data.timestamp.duration_since(std::time::UNIX_EPOCH)?.as_secs(),
            "metadata": data.metadata
        });

        self.send_with_retry(&payload).await
    }

    async fn show_alert(&mut self, alert: &AlertData) -> Result<()> {
        let payload = json!({
            "type": "alert",
            "alert_type": alert.alert_type,
            "severity": alert.severity,
            "message": alert.message,
            "data": alert.data,
            "timestamp": alert.timestamp.duration_since(std::time::UNIX_EPOCH)?.as_secs()
        });

        self.send_with_retry(&payload).await
    }

    async fn clear_display(&mut self) -> Result<()> {
        let payload = json!({
            "type": "clear_display",
            "timestamp": SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs()
        });

        self.send_with_retry(&payload).await
    }

    async fn get_status(&self) -> Result<Value> {
        Ok(json!({
            "driver_type": self.driver_type(),
            "url": self.url,
            "timeout_seconds": self.timeout_seconds,
            "retry_count": self.retry_count,
            "connection_status": self.connection_status,
            "has_auth_token": self.auth_token.is_some(),
            "custom_headers": self.headers.keys().collect::<Vec<_>>()
        }))
    }

    fn driver_type(&self) -> &str {
        "https_callback"
    }
}
