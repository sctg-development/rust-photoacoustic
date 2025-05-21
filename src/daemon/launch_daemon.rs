//! # Daemon Management Module
//!
//! This module provides functionality for running and manageing background
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
use log::{debug, error, info, warn};
use std::time::{Duration, SystemTime};
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::task::JoinHandle;
use tokio::time;

use crate::utility::PhotoacousticDataSource;
use crate::visualization::server::build_rocket;
use crate::{config::Config, modbus::PhotoacousticModbusServer};
use base64::prelude::*;
use rocket::{
    config::LogLevel,
    data::{Limits, ToByteUnit},
};
use tokio::net::TcpListener;
use tokio_modbus::server::tcp::{accept_tcp_connection, Server};

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
///
/// The `data_source` provides measurement data that can be accessed by multiple
/// components including the web API, visualizations, and Modbus server.
pub struct Daemon {
    tasks: Vec<JoinHandle<Result<()>>>,
    running: Arc<AtomicBool>,
    data_source: Arc<PhotoacousticDataSource>,
    modbus_server: Option<Arc<PhotoacousticModbusServer>>,
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
            data_source: Arc::new(PhotoacousticDataSource::new()),
            modbus_server: None,
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

        // Start modbus server if enabled
        if config.modbus.enabled {
            self.start_modbus_server(config).await?;
        }

        // Start computation task if enabled
        if true {
            self.start_computation(config)?;
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
        let config = config.clone();
        let task = tokio::spawn(async move {
            while running.load(Ordering::SeqCst) {
                // Perform data acquisition
                // This would integrate with our acquisition module
                debug!("Acquiring data... currently nothing");
                time::sleep(Duration::from_millis(1000 * 60)).await;
            }
            Ok(())
        });

        self.tasks.push(task);
        Ok(())
    }

    fn start_computation(&mut self, config: &Config) -> Result<()> {
        info!("Starting computation task");

        let running = self.running.clone();
        let data_source_clone = self.data_source.clone();
        let config = config.clone();

        let task = tokio::spawn(async move {
            let now = SystemTime::now();
            while running.load(Ordering::SeqCst) {
                // Perform computation
                // This would integrate with our computation module

                debug!("Performing computation... currently simulated");
                // Simulate computation
                // Simulate data acquisition
                let timestamp = SystemTime::now()
                    .duration_since(now)
                    .expect("Time went backwards")
                    .as_secs();
                data_source_clone.update_data(
                    (1234 + timestamp) as f32,
                    (5678 + timestamp) as f32,
                    (1000 + timestamp) as f32,
                );
                time::sleep(Duration::from_millis(config.acquisition.interval_ms)).await;
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
        info!("Starting heartbeat monitor");

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

    /// Launch the modbus server daemon
    ///
    /// Initializes and launches a Modbus TCP server that allows external systems
    /// to access photoacoustic data using the Modbus protocol. The server is
    /// configured according to the provided configuration, including address and port.
    ///
    /// This method spawns an asynchronous task that runs the Modbus server in the background.
    /// The server will continue running until the daemon's `running` flag is set to `false`.
    ///
    /// # Parameters
    ///
    /// * `config` - Application configuration containing Modbus server settings
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success if the server started successfully, or error details
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// * The server fails to bind to the specified address/port
    /// * The socket address is invalid
    /// * The Modbus server fails to initialize for any other reason
    async fn start_modbus_server(&mut self, config: &Config) -> Result<()> {
        info!(
            "Starting modbus server on {}:{}",
            config.modbus.address, config.modbus.port
        );
        let config = config.clone();
        let running = self.running.clone();
        // Create a clone of the data source to share with the server
        let data_source = self.get_data_source();
        // Create another clone for the server task

        let task = tokio::spawn(async move {
            let socket_addr: SocketAddr =
                format!("{}:{}", config.modbus.address, config.modbus.port)
                    .parse()
                    .expect("Invalid socket address");
            let listener = TcpListener::bind(socket_addr).await?;

            let server = Server::new(listener);

            // Use a single shared service instance for all connections
            // This might be sufficient because on modbus specifications only one
            // Modbus master can connect to a Modbus slave at a time

            // Create a new Modbus server instance
            let on_connected = move |stream, socket_addr| {
                // Clone the Arc to avoid moving the original
                let data_source_clone = data_source.clone();
                let current_data_clone = data_source_clone.get_latest_data().unwrap();
                debug!(
                    "Data are now frequency:{} amplitude:{} concentration:{}",
                    current_data_clone.frequency,
                    current_data_clone.amplitude,
                    current_data_clone.concentration
                );

                async move {
                    accept_tcp_connection(stream, socket_addr, move |_socket_addr| {
                        // Use the cloned Arc in this inner closure
                        Ok(Some(PhotoacousticModbusServer::with_data_source(
                            &data_source_clone,
                        )))
                    })
                }
            };

            let on_process_error = |err| {
                error!("Modbus server error: {err}");
            };

            // Start the server in a separate task
            let server_handle = tokio::spawn(async move {
                if let Err(e) = server.serve(&on_connected, on_process_error).await {
                    error!("Modbus server error: {}", e);
                }
            });

            // Create a cancellation token for the server task
            let running_clone = running.clone();

            // Periodically update the modbus server with latest measurement data
            while running.load(Ordering::SeqCst) {
                // Check every second if we should continue running
                time::sleep(Duration::from_secs(1)).await;
            }

            // The running flag is now false, which means we need to shut down
            info!("Shutting down Modbus server...");

            // Explicitly abort the server task if it's still running
            server_handle.abort();

            // Wait for the server to shut down with a timeout
            match tokio::time::timeout(Duration::from_secs(5), server_handle).await {
                Ok(_) => info!("Modbus server shut down successfully"),
                Err(_) => {
                    // If it times out, just log and continue - we don't want to block shutdown
                    warn!("Modbus server shutdown timed out, forcing termination");
                }
            }

            Ok(())
        });

        self.tasks.push(task);
        info!("Modbus server started");
        Ok(())
    }

    /// Get the shared data source
    ///
    /// # Returns
    ///
    /// A reference to the shared data source that can be used by other components
    pub fn get_data_source(&self) -> Arc<PhotoacousticDataSource> {
        self.data_source.clone()
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
            match tokio::time::timeout(Duration::from_secs(10), task).await {
                Ok(result) => {
                    if let Err(e) = result {
                        log::error!("Task panicked: {}", e);
                    }
                }
                Err(_) => {
                    // Task didn't complete within timeout
                    log::warn!("Task did not complete within timeout period, may be hung");
                }
            }
        }
        Ok(())
    }
}
