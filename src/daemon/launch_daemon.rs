//! # Daemon Management Module
//!
//! This module provides functionality for running and managing long-running background
//! tasks (daemons) in the photoacoustic application. It handles the lifecycle of various
//! services including:
//!
//! - Web server for visualization
//! - Data acquisition from sensors
//! - System health monitoring (heartbeat)
//!
//! The daemon system allows for graceful startup and shutdown of these services,
//! with proper error handling and task coordination.
//!
//! ## Architecture
//!
//! The daemon system uses Tokio's asynchronous runtime to manage concurrent tasks.
//! Each service runs as an independent task, and the main daemon structure tracks
//! and coordinates these tasks.
//!
//! ## Usage
//!
//! ```no_run
//! use rust_photoacoustic::{config::Config, daemon::Daemon};
//!
//! async fn example() -> anyhow::Result<()> {
//!     let config = Config::from_file("config.yaml")?;
//!     
//!     // Create and launch daemon with all enabled services
//!     let mut daemon = Daemon::new();
//!     daemon.launch(&config).await?;
//!     
//!     // Later, trigger a graceful shutdown
//!     daemon.shutdown();
//!     
//!     // Wait for all tasks to complete
//!     daemon.join().await?;
//!     
//!     Ok(())
//! }
//! ```

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

/// Represents a daemon task manager that coordinates multiple background services
///
/// This structure maintains a collection of asynchronous tasks and provides methods
/// to start, stop, and monitor them. It handles the coordination between different
/// services like web servers, data acquisition systems, and health monitors.
///
/// # Fields
///
/// * `tasks` - Collection of handles to running tasks for management and cleanup
/// * `running` - Atomic flag shared between tasks to coordinate shutdown
///
/// # Thread Safety
///
/// The `running` flag is wrapped in an `Arc` (Atomic Reference Counter) to allow
/// safe sharing between multiple tasks. Each task checks this flag periodically
/// to determine if it should continue running or gracefully terminate.
pub struct Daemon {
    tasks: Vec<JoinHandle<Result<()>>>,
    running: Arc<AtomicBool>,
}

impl Daemon {
    /// Create a new daemon instance
    ///
    /// Initializes a new daemon manager with an empty task list and the
    /// running flag set to `true`.
    ///
    /// # Returns
    ///
    /// A new `Daemon` instance ready to have tasks added to it
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_photoacoustic::daemon::Daemon;
    ///
    /// let daemon = Daemon::new();
    /// // Daemon is now ready to launch tasks
    /// ```
    pub fn new() -> Self {
        Daemon {
            tasks: Vec::new(),
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Launch all configured tasks based on configuration
    ///
    /// Starts the various daemon services according to the provided configuration.
    /// Only services that are enabled in the configuration will be started.
    /// Each service runs as a separate asynchronous task.
    ///
    /// The following services may be started:
    /// * Visualization web server - If `config.visualization.enabled` is `true`
    /// * Data acquisition - If `config.acquisition.enabled` is `true`
    /// * Heartbeat monitoring - Always started for system health monitoring
    ///
    /// # Parameters
    ///
    /// * `config` - Application configuration containing service settings
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success if all tasks started successfully, or error details
    ///
    /// # Errors
    ///
    /// This function can fail if any of the services fail to start, such as:
    /// * The web server fails to bind to the specified port
    /// * Certificate decoding fails for TLS
    /// * Data acquisition initialization fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::{config::Config, daemon::Daemon};
    ///
    /// async fn start_daemon() -> anyhow::Result<Daemon> {
    ///     let config = Config::from_file("config.yaml")?;
    ///     let mut daemon = Daemon::new();
    ///     daemon.launch(&config).await?;
    ///     Ok(daemon)
    /// }
    /// ```
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

    /// Start the Rocket web server for visualization
    ///
    /// Initializes and launches a Rocket web server for the visualization interface.
    /// The server is configured according to the provided configuration, including
    /// address, port, and optional TLS settings.
    ///
    /// This method spawns an asynchronous task that runs the web server in the background.
    /// The server will continue running until the daemon's `running` flag is set to `false`.
    ///
    /// # Parameters
    ///
    /// * `config` - Application configuration containing web server settings
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success if the server started successfully, or error details
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// * TLS certificate decoding fails
    /// * The server fails to bind to the specified address/port
    /// * The Rocket server fails to initialize for any other reason
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

    /// Start the data acquisition task for collecting photoacoustic measurements
    ///
    /// Initializes and launches a background task that periodically acquires data
    /// from the configured sensors. This task runs on a fixed interval and continues
    /// until the daemon's `running` flag is set to `false`.
    ///
    /// In a complete implementation, this would integrate with hardware sensors
    /// via the acquisition module to collect real-time photoacoustic data.
    ///
    /// # Parameters
    ///
    /// * `config` - Application configuration containing acquisition settings
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success if the task started successfully, or error details
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// * The acquisition hardware is not available
    /// * Sensor initialization fails
    /// * Task spawning fails
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
    ///
    /// Initializes and launches a background task that periodically emits a heartbeat
    /// log message. This serves as a health monitoring system to ensure the daemon
    /// is still running correctly.
    ///
    /// The heartbeat task runs every 60 seconds and continues until the daemon's
    /// `running` flag is set to `false`.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success if the heartbeat task started successfully, or error details
    ///
    /// # Errors
    ///
    /// This function can fail if task spawning fails
    ///
    /// # Monitoring
    ///
    /// In a production environment, these heartbeat messages could be monitored by
    /// an external system to detect if the daemon has stopped functioning properly.
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

    /// Stop all running tasks gracefully
    ///
    /// Signals all spawned tasks to terminate by setting the shared `running` flag to `false`.
    /// Each task should periodically check this flag and perform a clean shutdown when
    /// the flag becomes `false`.
    ///
    /// This method only signals the tasks to stop; it does not wait for them to complete.
    /// To wait for all tasks to finish, call `join()` after this method.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::daemon::Daemon;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// # let daemon = Daemon::new();
    /// // Signal all tasks to stop
    /// daemon.shutdown();
    ///
    /// // Wait for all tasks to complete
    /// daemon.join().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn shutdown(&self) {
        info!("Shutting down daemon tasks");
        self.running.store(false, Ordering::SeqCst);
        // Tasks should check the running flag and terminate gracefully
    }

    /// Wait for all tasks to complete
    ///
    /// Consumes the daemon and waits for all spawned tasks to finish execution.
    /// This method should be called after `shutdown()` to ensure a clean application exit.
    ///
    /// If any task panics, the error is logged but this method will still wait for
    /// all other tasks to complete.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success if all tasks completed without errors
    ///
    /// # Errors
    ///
    /// This method logs task panics but does not fail because of them.
    /// It may fail due to other async runtime issues.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::daemon::Daemon;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// # let daemon = Daemon::new();
    /// // First signal shutdown
    /// daemon.shutdown();
    ///
    /// // Then wait for all tasks to finish
    /// daemon.join().await?;
    /// println!("All daemon tasks have completed");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn join(self) -> Result<()> {
        for task in self.tasks {
            if let Err(e) = task.await {
                log::error!("Task panicked: {}", e);
            }
        }
        Ok(())
    }
}
