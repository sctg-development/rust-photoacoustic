use anyhow::Result;
use log::{debug, info};
use serde::de;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time;

use crate::config::Config;
use crate::visualization::server::build_rocket;
use base64::prelude::*;
use rocket::{
    config::LogLevel,
    data::{Limits, ToByteUnit},
};

/// Represents a daemon task that can be started and managed
pub struct Daemon {
    tasks: Vec<JoinHandle<Result<()>>>,
    running: Arc<AtomicBool>,
}

impl Daemon {
    /// Create a new daemon instance
    pub fn new() -> Self {
        Daemon {
            tasks: Vec::new(),
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Launch all configured tasks based on configuration
    pub async fn launch(&mut self, config: &Config) -> Result<()> {
        // Start web server if enabled
        if config.visualization.enabled {
            self.start_web_server(config).await?;
        }

        // Start data acquisition task if enabled
        if config.acquisition.enabled {
            self.start_data_acquisition(config)?;
        }

        // Add additional tasks here as needed

        // Start heartbeat task for monitoring
        self.start_heartbeat()?;

        Ok(())
    }

    /// Start the Rocket web server
    async fn start_web_server(&mut self, config: &Config) -> Result<()> {
        info!(
            "Starting web server on {}:{}",
            config.visualization.address, config.visualization.port
        );

        let mut figment = rocket::Config::figment()
            .merge(("ident", config.visualization.name.clone()))
            .merge(("limits", Limits::new().limit("json", 2.mebibytes())))
            .merge(("address", config.visualization.address.clone()))
            .merge(("port", config.visualization.port))
            .merge(("log_level", LogLevel::Normal));

        // Configure TLS if certificates are provided
        if let (Some(cert), Some(key)) = (&config.visualization.cert, &config.visualization.key) {
            debug!("SSL certificates found in configuration, enabling TLS");

            // Decode base64 certificates
            let cert_data = BASE64_STANDARD.decode(cert)?;
            let key_data = BASE64_STANDARD.decode(key)?;

            // Configure TLS
            figment = figment
                .merge(("tls.certs", cert_data))
                .merge(("tls.key", key_data));

            info!("TLS enabled for web server");
        }

        let rocket = build_rocket(figment, &config.visualization.hmac_secret).await;

        let _running = self.running.clone();
        let task = tokio::spawn(async move {
            let ignited = rocket.ignite().await?;
            ignited.launch().await?;
            Ok(())
        });

        self.tasks.push(task);
        Ok(())
    }

    /// Start the data acquisition task
    fn start_data_acquisition(&mut self, config: &Config) -> Result<()> {
        info!("Starting data acquisition task");

        let running = self.running.clone();
        let task = tokio::spawn(async move {
            while running.load(Ordering::SeqCst) {
                // Perform data acquisition
                // This would integrate with our acquisition module

                // Example: wait 60 second between acquisitions
                debug!("Acquiring data...");
                time::sleep(Duration::from_secs(60)).await;
            }
            Ok(())
        });

        self.tasks.push(task);
        Ok(())
    }

    /// Start a heartbeat task that logs system status periodically
    fn start_heartbeat(&mut self) -> Result<()> {
        debug!("Starting heartbeat monitor");

        let running = self.running.clone();
        let task = tokio::spawn(async move {
            while running.load(Ordering::SeqCst) {
                debug!("Daemon heartbeat: running");
                time::sleep(Duration::from_secs(60)).await;
            }
            Ok(())
        });

        self.tasks.push(task);
        Ok(())
    }

    /// Stop all running tasks
    pub fn shutdown(&self) {
        info!("Shutting down daemon tasks");
        self.running.store(false, Ordering::SeqCst);
        // Tasks should check the running flag and terminate gracefully
    }

    /// Wait for all tasks to complete
    pub async fn join(self) -> Result<()> {
        for task in self.tasks {
            if let Err(e) = task.await {
                log::error!("Task panicked: {}", e);
            }
        }
        Ok(())
    }
}
