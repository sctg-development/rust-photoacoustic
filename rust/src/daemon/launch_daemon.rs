// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

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
//! use rust_photoacoustic::{config::Config, daemon::launch_daemon::Daemon};
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

use crate::acquisition::record_consumer::RecordConsumer;
use crate::acquisition::{
    get_default_realtime_audio_source, get_realtime_audio_source_from_device,
    get_realtime_audio_source_from_file, get_realtime_mock_audio_source,
    get_realtime_simulated_photoacoustic_source, RealTimeAcquisitionDaemon, SharedAudioStream,
};
use crate::processing::nodes::StreamingNodeRegistry;
use crate::processing::{ProcessingConsumer, ProcessingGraph};
use crate::utility::PhotoacousticDataSource;
use crate::visualization::server::build_rocket;
use crate::visualization::shared_state::SharedVisualizationState;
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
    #[allow(dead_code)]
    modbus_server: Option<Arc<PhotoacousticModbusServer>>,
    /// Shared audio stream for real-time streaming to web clients
    audio_stream: Option<Arc<SharedAudioStream>>,
    /// Real-time acquisition daemon for audio processing
    #[allow(dead_code)]
    realtime_acquisition_daemon: Option<RealTimeAcquisitionDaemon>,
    /// record consumer daemon for testing and validation
    record_consumer_daemon: Option<RecordConsumer>,
    /// processing consumer daemon for audio processing pipeline
    processing_consumer_daemon: Option<ProcessingConsumer>,
    /// Shared visualization state for statistics and runtime data
    visualization_state: Arc<SharedVisualizationState>,
    /// Streaming node registry for managing real-time audio streams
    streaming_registry: Arc<StreamingNodeRegistry>,
}

impl Default for Daemon {
    fn default() -> Self {
        Self::new()
    }
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
    /// use rust_photoacoustic::daemon::launch_daemon::Daemon;
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
            audio_stream: None,
            realtime_acquisition_daemon: None,
            record_consumer_daemon: None,
            processing_consumer_daemon: None,
            visualization_state: Arc::new(SharedVisualizationState::new()),
            streaming_registry: Arc::new(StreamingNodeRegistry::new()),
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
    /// use rust_photoacoustic::{config::Config, daemon::launch_daemon::Daemon};
    ///
    /// async fn start_daemon() -> anyhow::Result<Daemon> {
    ///     let config = Config::from_file("config.yaml")?;
    ///     let mut daemon = Daemon::new();
    ///     daemon.launch(&config).await?;
    ///     Ok(daemon)
    /// }
    /// ```
    pub async fn launch(&mut self, config: &Config) -> Result<()> {
        // Démarrer l'acquisition audio AVANT le serveur web
        self.start_audio_acquisition(config).await?;

        // Start record consumer if enabled
        if config.photoacoustic.record_consumer {
            self.start_record_consumer(config).await?;
        }

        // Start processing consumer if enabled
        if config.processing.enabled {
            self.start_processing_consumer(config).await?;
        }

        // Start web server if enabled
        if config.visualization.enabled {
            self.start_visualization_server(config).await?;
        }

        // Start data acquisition task if enabled
        if config.acquisition.enabled {
            self.start_auxiliary_data_acquisition(config)?;
        }

        // Start modbus server if enabled
        if config.modbus.enabled {
            self.start_modbus_server(config).await?;
        }

        // Start computation task if enabled
        if true {
            self.start_photoacoustic_computation(config)?;
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
    async fn start_visualization_server(&mut self, config: &Config) -> Result<()> {
        info!(
            "Starting web server on {}:{}",
            config.visualization.address, config.visualization.port
        );

        let mut figment = rocket::Config::figment()
            .merge(("ident", config.visualization.name.clone()))
            .merge(("limits", Limits::new().limit("json", 2.mebibytes())))
            .merge(("address", config.visualization.address.clone()))
            .merge(("port", config.visualization.port))
            .merge(("log_level", LogLevel::Normal))
            .merge(("secret_key", config.visualization.session_secret.clone()));

        // Add RS256 keys to figment
        if !config.visualization.rs256_public_key.is_empty()
            && !config.visualization.rs256_private_key.is_empty()
        {
            debug!("RS256 keys found in configuration");
            figment = figment
                .merge((
                    "rs256_public_key",
                    config.visualization.rs256_public_key.clone(),
                ))
                .merge((
                    "rs256_private_key",
                    config.visualization.rs256_private_key.clone(),
                ));
        }

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

            // Add the hmac secret to the figment
            figment = figment.merge(("hmac_secret", config.visualization.hmac_secret.clone()));

            info!("TLS enabled for web server");
        }
        // Add the access config to the figment
        figment = figment.merge(("access_config", config.access.clone()));

        // Add the Generix configuration to the figment
        figment = figment.merge(("generix_config", config.generix.clone()));

        // Add the visualization configuration to the figment
        figment = figment.merge(("visualization_config", config.visualization.clone()));

        let rocket = build_rocket(
            figment,
            self.audio_stream.clone(),
            Some(Arc::clone(&self.visualization_state)),
            Some(Arc::clone(&self.streaming_registry)),
        )
        .await;

        let _running = self.running.clone();
        let task = tokio::spawn(async move {
            let ignited = rocket.ignite().await?;
            ignited.launch().await?;
            Ok(())
        });

        self.tasks.push(task);
        Ok(())
    }

    /// Start the data acquisition task for collecting auxiliaries measurements
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
    fn start_auxiliary_data_acquisition(&mut self, config: &Config) -> Result<()> {
        info!("Starting auxliary data acquisition task");

        let running = self.running.clone();
        let _config = config.clone();
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

    fn start_photoacoustic_computation(&mut self, config: &Config) -> Result<()> {
        info!("Starting photoacoustic computation task");

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
            let _running_clone = running.clone();

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

    /// Start the real-time audio acquisition daemon
    ///
    /// Initializes and starts a background task for real-time audio acquisition from the
    /// configured source (microphone, file, or mock). The acquired audio data is streamed
    /// directly to the shared audio stream for real-time consumption by web clients via SSE endpoints.
    ///
    /// This new implementation uses the RealTimeAudioSource trait which provides native
    /// streaming capabilities, eliminating the batching issues present in the previous
    /// AudioSource-based implementation.
    ///
    /// # Audio Source Priority
    ///
    /// The function selects the audio source based on configuration priority:
    /// 1. **Mock source** - If `config.photoacoustic.mock_source` is enabled
    /// 2. **File source** - If `config.photoacoustic.input_file` is specified
    /// 3. **Device source** - If `config.photoacoustic.input_device` is specified  
    /// 4. **Default source** - Uses the system's default audio input device
    ///
    /// # Real-Time Architecture
    ///
    /// This function creates and orchestrates the new real-time components:
    /// - **RealTimeAudioSource**: Handles low-level audio input with native streaming
    /// - **SharedAudioStream**: Thread-safe streaming buffer for real-time data sharing
    /// - **RealTimeAcquisitionDaemon**: Core acquisition manager with direct streaming
    /// - **Background Task**: Async task for non-blocking operation
    ///
    /// # Parameters
    ///
    /// * `config` - Application configuration containing acquisition settings
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success if the acquisition started successfully, or error details
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// * **Audio source initialization fails**
    /// * **Real-time daemon creation fails**
    /// * **Task spawning fails**
    async fn start_audio_acquisition(&mut self, config: &Config) -> Result<()> {
        // Early return if acquisition is disabled in configuration
        if !config.acquisition.enabled {
            info!("Audio acquisition is disabled in configuration");
            return Ok(());
        }

        info!("Starting real-time audio acquisition system");

        // === PHASE 1: Real-Time Audio Source Selection ===
        // Select and initialize the appropriate real-time audio source based on configuration
        let audio_source = if let Some(ref simulated_config) = config.photoacoustic.simulated_source
        {
            // Simulated photoacoustic source for testing and advanced simulation
            info!(
                "Using simulated photoacoustic source with type: {}",
                simulated_config.source_type
            );
            get_realtime_simulated_photoacoustic_source(config.photoacoustic.clone())?
        } else if let Some(ref file_path) = config.photoacoustic.input_file {
            // File-based real-time audio source for testing and playback scenarios
            info!("Using real-time file audio source: {}", file_path);
            get_realtime_audio_source_from_file(config.photoacoustic.clone())?
        } else if let Some(ref device_name) = config.photoacoustic.input_device {
            // Named device source for specific hardware targeting
            info!("Using real-time device audio source: {}", device_name);
            get_realtime_audio_source_from_device(config.photoacoustic.clone())?
        } else {
            // Default system audio input as fallback
            info!("Using default real-time audio source");
            get_default_realtime_audio_source(config.photoacoustic.clone())?
        };

        // === PHASE 2: Real-Time Acquisition Daemon Creation ===
        // Create the real-time acquisition daemon with the selected source
        let buffer_size: usize = config.photoacoustic.frame_size.into();
        let mut realtime_daemon = RealTimeAcquisitionDaemon::new(audio_source, buffer_size);

        // === PHASE 3: Stream Connection ===
        // Get a reference to the daemon's internal stream for web server use
        let audio_stream = realtime_daemon.get_shared_stream();

        // === PHASE 4: State Management ===
        // Store the acquisition daemon's stream for access by web server components
        self.audio_stream = Some(audio_stream.clone());

        // === PHASE 5: Background Task Spawning ===
        // Start the real-time acquisition daemon in a dedicated async task
        let running = self.running.clone();
        let task = tokio::spawn(async move {
            info!("Real-time audio acquisition task started");

            // Start the real-time acquisition daemon
            match realtime_daemon.start().await {
                Ok(_) => {
                    info!("Real-time audio acquisition daemon started successfully");
                }
                Err(e) => {
                    error!("Failed to start real-time audio acquisition daemon: {}", e);
                    return Ok(());
                }
            }

            // Keep the daemon running until shutdown is signaled
            while running.load(Ordering::Relaxed) {
                // Check daemon status
                if !realtime_daemon.is_running() {
                    warn!("Real-time acquisition daemon stopped unexpectedly");
                    break;
                }

                // Wait a bit before checking again
                tokio::time::sleep(Duration::from_millis(1000)).await;
            }

            // Graceful shutdown
            info!("Stopping real-time audio acquisition daemon");
            if let Err(e) = realtime_daemon.stop().await {
                error!("Error stopping real-time acquisition daemon: {}", e);
            }

            info!("Real-time audio acquisition task stopped");
            Ok(())
        });

        // Register the task for lifecycle management and graceful shutdown
        self.tasks.push(task);
        info!("Real-time audio acquisition system started successfully");
        Ok(())
    }

    /// Start the record consumer daemon for validation and testing
    ///
    /// Creates and starts a RecordConsumerDaemon that consumes audio frames from the
    /// shared audio stream and saves them to a WAV file. This daemon is used for
    /// validating the producer/consumer system and studying consumer behavior.
    ///
    /// # Parameters
    ///
    /// * `config` - Application configuration containing record consumer settings
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success if record consumer started successfully, or error details
    ///
    /// # Requirements
    ///
    /// This method requires that `start_audio_acquisition` has been called first to
    /// establish the audio stream. If no audio stream is available, this method will
    /// return an error.
    ///
    /// # Configuration
    ///
    /// The record consumer is controlled by the `config.acquisition.record_consumer` flag.
    /// When enabled, it will:
    /// - Start consuming audio frames after audio acquisition begins
    /// - Save audio stream to WAV file with same precision and sample rate as producer
    /// - Generate debug log messages for studying consumer behavior
    /// - Track throughput statistics (FPS, frame delays)
    ///
    /// # Examples
    ///
    /// ```no_run,ignore
    /// use rust_photoacoustic::daemon::launch_daemon::Daemon;
    /// use rust_photoacoustic::config::Config;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut daemon = Daemon::new();
    /// let config = Config::load("config.yaml")?;
    ///
    /// // Start audio acquisition first
    /// daemon.start_audio_acquisition(&config).await?;
    ///
    /// // Start record consumer for validation
    /// daemon.start_record_consumer(&config).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn start_record_consumer(&mut self, config: &Config) -> Result<()> {
        info!("Starting record consumer daemon for validation");

        // Ensure audio stream is available
        let audio_stream = self.audio_stream.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Audio stream not available. Start audio acquisition first.")
        })?;

        // Create record consumer daemon
        let record_consumer = RecordConsumer::new(
            audio_stream.clone(),
            config.photoacoustic.record_file.clone(),
        );

        // Start the record consumer in a background task
        let mut record_consumer_for_task = record_consumer;
        let task = tokio::spawn(async move {
            info!("record consumer task started");

            // Start the record consumer daemon
            match record_consumer_for_task.start().await {
                Ok(_) => {
                    info!("record consumer daemon completed successfully");
                }
                Err(e) => {
                    error!("record consumer daemon failed: {}", e);
                }
            }

            info!("record consumer task stopped");
            Ok(())
        });

        // Store a placeholder for the record consumer daemon (already moved to task)
        self.record_consumer_daemon = Some(RecordConsumer::new(
            audio_stream.clone(),
            "placeholder".to_string(),
        ));

        // Register the task for lifecycle management and graceful shutdown
        self.tasks.push(task);
        info!("record consumer daemon started successfully");
        Ok(())
    }

    /// Start the processing consumer daemon
    ///
    /// Initializes and starts the processing consumer daemon which handles audio processing
    /// using a configurable processing graph. The daemon consumes audio data from the
    /// shared audio stream and processes it through the configured processing nodes.
    ///
    /// # Parameters
    ///
    /// * `config` - Application configuration containing processing settings
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success if the processing consumer started successfully
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// * Audio stream is not available (acquisition must be started first)
    /// * Processing graph configuration is invalid
    /// * Processing consumer fails to initialize
    async fn start_processing_consumer(&mut self, config: &Config) -> Result<()> {
        info!("Starting processing consumer daemon");

        // Ensure audio stream is available
        let audio_stream = self.audio_stream.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Audio stream not available. Start audio acquisition first.")
        })?;

        // Validate processing configuration
        config
            .processing
            .validate()
            .map_err(|e| anyhow::anyhow!("Invalid processing configuration: {}", e))?;

        // Create processing graph from configuration with streaming registry
        let processing_graph = ProcessingGraph::from_config_with_registry(
            &config.processing.default_graph,
            Some((*self.streaming_registry).clone()),
        )
        .map_err(|e| anyhow::anyhow!("Failed to create processing graph: {}", e))?;

        // Create processing consumer daemon with shared visualization state
        let processing_consumer = ProcessingConsumer::new_with_visualization_state(
            audio_stream.clone(),
            processing_graph,
            Arc::clone(&self.visualization_state),
        );

        // Start the processing consumer in a background task
        let mut processing_consumer_for_task = processing_consumer;

        let task = tokio::spawn(async move {
            info!("Processing consumer task started");

            // Start the processing consumer daemon
            match processing_consumer_for_task.start().await {
                Ok(_) => {
                    info!("Processing consumer daemon completed successfully");
                }
                Err(e) => {
                    error!("Processing consumer daemon failed: {}", e);
                }
            }

            info!("Processing consumer task stopped");
            Ok(())
        });

        // Store a placeholder for the processing consumer daemon (already moved to task)
        // We create a new instance for tracking purposes
        let processing_graph_placeholder = ProcessingGraph::from_config_with_registry(
            &config.processing.default_graph,
            Some((*self.streaming_registry).clone()),
        )
        .map_err(|e| anyhow::anyhow!("Failed to create placeholder processing graph: {}", e))?;

        self.processing_consumer_daemon = Some(ProcessingConsumer::new_with_visualization_state(
            audio_stream.clone(),
            processing_graph_placeholder,
            Arc::clone(&self.visualization_state),
        ));

        // Register the task for lifecycle management and graceful shutdown
        self.tasks.push(task);
        info!("Processing consumer daemon started successfully");
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

    /// Get a reference to the shared audio stream
    ///
    /// Returns the shared audio stream if acquisition is enabled and running.
    /// This is used by the web server to provide real-time streaming endpoints.
    #[allow(dead_code)]
    pub fn get_audio_stream(&self) -> Option<Arc<SharedAudioStream>> {
        self.audio_stream.clone()
    }

    /// Get the shared visualization state
    ///
    /// Returns the shared visualization state that contains runtime statistics
    /// and other data that needs to be accessed by web API endpoints.
    pub fn get_visualization_state(&self) -> Arc<SharedVisualizationState> {
        Arc::clone(&self.visualization_state)
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
    /// use rust_photoacoustic::daemon::launch_daemon::Daemon;
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
    /// use rust_photoacoustic::daemon::launch_daemon::Daemon;
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
            match tokio::time::timeout(Duration::from_secs(5), task).await {
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
