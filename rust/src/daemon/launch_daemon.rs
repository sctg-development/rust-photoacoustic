// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (    /// * `config` - Application configuration as `Arc<RwLock<Config>>` for shared access
///   across all daemon components, enabling dynamic configuration support.e LICENSE.md for details).
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
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time;

use crate::acquisition::record_consumer::RecordConsumer;
use crate::acquisition::{
    get_default_realtime_audio_source, get_realtime_audio_source_from_device,
    get_realtime_audio_source_from_file, get_realtime_simulated_photoacoustic_source,
    RealTimeAcquisitionDaemon, SharedAudioStream,
};
use crate::processing::computing_nodes::SharedComputingState;
use crate::processing::nodes::StreamingNodeRegistry;
use crate::processing::{ProcessingConsumer, ProcessingGraph};
use crate::thermal_regulation::{
    create_shared_thermal_state, SharedThermalState, ThermalRegulationSystemDaemon,
};
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
/// ### Fields
///
/// * `tasks` - Collection of handles to running tasks for management and cleanup
/// * `running` - Atomic flag shared between tasks to coordinate shutdown
/// * `config` - Shared configuration (`Arc<RwLock<Config>>`) providing dynamic access for all components
///
/// ### Thread Safety
///
/// The `running` flag is wrapped in an `Arc` (Atomic Reference Counter) to allow
/// safe sharing between multiple tasks. Each task checks this flag periodically
/// to determine if it should continue running or gracefully terminate.
///
/// The `config` is also wrapped in an `Arc` to enable efficient sharing of configuration
/// data across all daemon components while supporting future dynamic configuration updates.
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
    /// Shared configuration for dynamic configuration support
    /// This is the single source of truth for all configuration across the application
    config: Arc<RwLock<crate::config::Config>>,
    /// Thermal regulation system daemon for PID temperature control
    thermal_regulation_daemon: Option<ThermalRegulationSystemDaemon>,
    /// Shared thermal regulation state for historical data and monitoring
    thermal_regulation_state: SharedThermalState,
    /// Shared computing state for analytical results from computing nodes
    computing_state: SharedComputingState,
}

impl Default for Daemon {
    fn default() -> Self {
        Self::new()
    }
}

impl Daemon {
    /// Create a new daemon instance
    ///
    /// Initializes a new daemon manager with an empty task list, the running flag
    /// set to `true`, and a default configuration that will be replaced when
    /// `launch()` is called with the actual configuration.
    ///
    /// ### Returns
    ///
    /// A new `Daemon` instance ready to have tasks added to it
    ///
    /// ### Examples
    ///
    /// ```
    /// use rust_photoacoustic::daemon::launch_daemon::Daemon;
    ///
    /// let daemon = Daemon::new();
    /// // Daemon is now ready to launch tasks with a configuration
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
            config: Arc::new(RwLock::new(crate::config::Config::default())),
            thermal_regulation_daemon: None,
            thermal_regulation_state: create_shared_thermal_state(),
            computing_state: Arc::new(RwLock::new(
                crate::processing::computing_nodes::ComputingSharedData::default(),
            )),
        }
    }

    /// Launch all configured tasks based on configuration
    ///
    /// Starts the various daemon services according to the provided configuration.
    /// The configuration is stored internally as `Arc<RwLock<Config>>` and shared with all
    /// spawned tasks to enable dynamic configuration access. Only services that are
    /// enabled in the configuration will be started. Each service runs as a separate
    /// asynchronous task.
    ///
    /// The following services may be started:
    /// * Visualization web server - If `config.visualization.enabled` is `true`
    /// * Data acquisition - If `config.acquisition.enabled` is `true`
    /// * Processing consumer - If `config.processing.enabled` is `true`
    /// * Modbus server - If `config.modbus.enabled` is `true`
    /// * Record consumer - If `config.photoacoustic.record_consumer` is `true`
    /// * Heartbeat monitoring - Always started for system health monitoring
    ///
    /// ### Parameters
    ///
    /// * `config` - Application configuration as `Arc<Config>` for shared access
    ///   across all daemon components, enabling dynamic configuration support.
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success if all tasks started successfully, or error details
    ///
    /// ### Errors
    ///
    /// This function can fail if any of the services fail to start, such as:
    /// * The web server fails to bind to the specified port
    /// * Certificate decoding fails for TLS
    /// * Data acquisition initialization fails
    /// * Processing graph configuration is invalid
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::{config::Config, daemon::launch_daemon::Daemon};
    /// use std::sync::Arc;
    /// use tokio::sync::RwLock;
    ///
    /// async fn start_daemon() -> anyhow::Result<Daemon> {
    ///     let config = Config::from_file("config.yaml")?;
    ///     let config_arc = Arc::new(RwLock::new(config));
    ///     let mut daemon = Daemon::new();
    ///     daemon.launch(config_arc).await?;
    ///     Ok(daemon)
    /// }
    /// ```
    pub async fn launch(&mut self, config: Arc<RwLock<Config>>) -> Result<()> {
        // Store the config as a shared Arc<RwLock<Config>> for dynamic configuration support
        self.config = config;

        // Démarrer l'acquisition audio AVANT le serveur web
        self.start_audio_acquisition().await?;

        // Start record consumer if enabled
        if self.config.read().await.photoacoustic.record_consumer {
            self.start_record_consumer().await?;
        }

        // Start processing consumer if enabled
        if self.config.read().await.processing.enabled {
            self.start_processing_consumer().await?;
        }

        // Start web server if enabled
        if self.config.read().await.visualization.enabled {
            self.start_visualization_server().await?;
        }

        // Start data acquisition task if enabled
        if self.config.read().await.acquisition.enabled {
            self.start_auxiliary_data_acquisition().await?;
        }

        // Start modbus server if enabled
        if self.config.read().await.modbus.enabled {
            self.start_modbus_server().await?;
        }

        // Start thermal regulation system if enabled
        if self.config.read().await.thermal_regulation.enabled {
            self.start_thermal_regulation_system().await?;
        }

        // Add additional tasks here as needed

        // Start heartbeat task for monitoring
        self.start_heartbeat()?;

        Ok(())
    }

    /// Start the Rocket web server for visualization
    ///
    /// Initializes and launches a Rocket web server for the visualization interface.
    /// The server is configured according to the shared `Arc<Config>` stored in the daemon,
    /// including address, port, and optional TLS settings.
    ///
    /// This method spawns an asynchronous task that runs the web server in the background.
    /// The server will continue running until the daemon's `running` flag is set to `false`.
    /// The server has access to the shared configuration for dynamic updates.
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success if the server started successfully, or error details
    ///
    /// ### Errors
    ///
    /// This function can fail if:
    /// * TLS certificate decoding fails
    /// * The server fails to bind to the specified address/port
    /// * The Rocket server fails to initialize for any other reason
    async fn start_visualization_server(&mut self) -> Result<()> {
        // Use the shared config from the daemon
        let config = Arc::clone(&self.config);

        // Extract all needed values in one scope and immediately release the lock
        let (
            visualization_address,
            visualization_port,
            visualization_name,
            session_secret,
            rs256_public_key,
            rs256_private_key,
            visualization_cert,
            visualization_key,
            hmac_secret,
            enable_compression,
        ) = {
            let config_read = config.read().await;
            (
                config_read.visualization.address.clone(),
                config_read.visualization.port,
                config_read.visualization.name.clone(),
                config_read.visualization.session_secret.clone(),
                config_read.visualization.rs256_public_key.clone(),
                config_read.visualization.rs256_private_key.clone(),
                config_read.visualization.cert.clone(),
                config_read.visualization.key.clone(),
                config_read.visualization.hmac_secret.clone(),
                config_read.visualization.enable_compression,
            )
        };

        info!(
            "Starting web server on {}:{}",
            visualization_address, visualization_port
        );

        let mut figment = rocket::Config::figment()
            .merge(("ident", visualization_name))
            .merge(("limits", Limits::new().limit("json", 2.mebibytes())))
            .merge(("address", visualization_address))
            .merge(("port", visualization_port))
            .merge(("log_level", LogLevel::Normal))
            .merge(("secret_key", session_secret));

        // Add RS256 keys to figment
        if !rs256_public_key.is_empty() && !rs256_private_key.is_empty() {
            debug!("RS256 keys found in configuration");
            figment = figment
                .merge(("rs256_public_key", rs256_public_key))
                .merge(("rs256_private_key", rs256_private_key));
        }

        // Configure TLS if certificates are provided
        if let (Some(cert), Some(key)) = (&visualization_cert, &visualization_key) {
            debug!("SSL certificates found in configuration, enabling TLS");

            // Decode base64 certificates
            let cert_data = BASE64_STANDARD.decode(cert)?;
            let key_data = BASE64_STANDARD.decode(key)?;

            // Configure TLS
            figment = figment
                .merge(("tls.certs", cert_data))
                .merge(("tls.key", key_data));

            // Add the hmac secret to the figment
            figment = figment.merge(("hmac_secret", hmac_secret));

            info!("TLS enabled for web server");
        }

        let rocket = build_rocket(
            figment,
            Arc::clone(&config),
            self.audio_stream.clone(),
            Some(Arc::clone(&self.visualization_state)),
            Some(Arc::clone(&self.streaming_registry)),
            Some(self.thermal_regulation_state.clone()),
            Some(self.computing_state.clone()),
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

    /// Start the data acquisition task for collecting auxiliary measurements
    ///
    /// Initializes and launches a background task that periodically acquires data
    /// from the configured sensors. This task runs on a fixed interval and continues
    /// until the daemon's `running` flag is set to `false`. The task uses the shared
    /// `Arc<Config>` for accessing acquisition settings.
    ///
    /// In a complete implementation, this would integrate with hardware sensors
    /// via the acquisition module to collect real-time photoacoustic data.
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success if the task started successfully, or error details
    ///
    /// ### Errors
    ///
    /// This function can fail if:
    /// * The acquisition hardware is not available
    /// * Sensor initialization fails
    /// * Task spawning fails
    async fn start_auxiliary_data_acquisition(&mut self) -> Result<()> {
        info!("Starting auxliary data acquisition task");
        // Use the shared config from the daemon
        let config = Arc::clone(&self.config);

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

    /// **DEPRECATED**: Start photoacoustic computation task
    ///
    /// This method is obsolete and should not be used. The photoacoustic computation
    /// is now handled by the ProcessingConsumer and computing nodes (like peak_finder)
    /// which provide real-time data processing and update the SharedComputingState.
    ///
    /// This old implementation was writing fake data to PhotoacousticDataSource
    /// which was interfering with the real data from the processing graph.
    #[allow(dead_code)]
    async fn start_photoacoustic_computation(&mut self) -> Result<()> {
        info!("Starting photoacoustic computation task");
        // Use the shared config from the daemon
        let config = Arc::clone(&self.config);
        let interval_ms = config.read().await.acquisition.interval_ms;

        let running = self.running.clone();
        let data_source_clone = self.data_source.clone();

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
                time::sleep(Duration::from_millis(interval_ms)).await;
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
    /// ### Returns
    ///
    /// * `Result<()>` - Success if the heartbeat task started successfully, or error details
    ///
    /// ### Errors
    ///
    /// This function can fail if task spawning fails
    ///
    /// ### Monitoring
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
    /// configured according to the shared `Arc<Config>` stored in the daemon,
    /// including address and port settings.
    ///
    /// This method spawns an asynchronous task that runs the Modbus server in the background.
    /// The server will continue running until the daemon's `running` flag is set to `false`.
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success if the server started successfully, or error details
    ///
    /// ### Errors
    ///
    /// This function can fail if:
    /// * The server fails to bind to the specified address/port
    /// * The socket address is invalid
    /// * The Modbus server fails to initialize for any other reason
    async fn start_modbus_server(&mut self) -> Result<()> {
        // Use the shared config from the daemon
        let config = Arc::clone(&self.config);
        let config_read = config.read().await;

        info!(
            "Starting modbus server on {}:{}",
            config_read.modbus.address, config_read.modbus.port
        );

        let socket_addr_str = format!("{}:{}", config_read.modbus.address, config_read.modbus.port);
        drop(config_read); // Release the read lock

        let running = self.running.clone();
        // Get a reference to the shared computing state
        let computing_state = Arc::clone(&self.computing_state);

        let task = tokio::spawn(async move {
            let socket_addr: SocketAddr = socket_addr_str.parse().expect("Invalid socket address");
            let listener = TcpListener::bind(socket_addr).await?;

            let server = Server::new(listener);

            // Use a single shared service instance for all connections
            // This might be sufficient because on modbus specifications only one
            // Modbus master can connect to a Modbus slave at a time

            // Create a new Modbus server instance
            let on_connected = move |stream, socket_addr| {
                // Clone the Arc to avoid moving the original
                let computing_state_clone = computing_state.clone();

                // Log current data from computing state
                if let Ok(state) = computing_state_clone.try_read() {
                    if let (Some(freq), Some(amp), Some(conc)) = (
                        state.peak_frequency,
                        state.peak_amplitude,
                        state.concentration_ppm,
                    ) {
                        debug!(
                            "Computing state contains - frequency:{} amplitude:{} concentration:{}",
                            freq, amp, conc
                        );
                    } else {
                        debug!("Computing state contains no measurement data yet");
                    }
                } else {
                    debug!("Could not read computing state");
                }

                async move {
                    accept_tcp_connection(stream, socket_addr, move |_socket_addr| {
                        // Use the cloned Arc in this inner closure
                        Ok(Some(PhotoacousticModbusServer::with_computing_state(
                            &computing_state_clone,
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

            // Monitor the running flag and shutdown when requested
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
    /// The task uses the shared `Arc<Config>` for accessing audio acquisition settings.
    ///
    /// This new implementation uses the RealTimeAudioSource trait which provides native
    /// streaming capabilities, eliminating the batching issues present in the previous
    /// AudioSource-based implementation.
    ///
    /// ### Audio Source Priority
    ///
    /// The function selects the audio source based on configuration priority:
    /// 1. **Simulated source** - If `config.photoacoustic.simulated_source` is configured
    /// 2. **File source** - If `config.photoacoustic.input_file` is specified
    /// 3. **Device source** - If `config.photoacoustic.input_device` is specified  
    /// 4. **Default source** - Uses the system's default audio input device
    ///
    /// ### Real-Time Architecture
    ///
    /// This function creates and orchestrates the new real-time components:
    /// - **RealTimeAudioSource**: Handles low-level audio input with native streaming
    /// - **SharedAudioStream**: Thread-safe streaming buffer for real-time data sharing
    /// - **RealTimeAcquisitionDaemon**: Core acquisition manager with direct streaming
    /// - **Background Task**: Async task for non-blocking operation
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success if the acquisition started successfully, or error details
    ///
    /// ### Errors
    ///
    /// This function can fail if:
    /// * **Audio source initialization fails**
    /// * **Real-time daemon creation fails**
    /// * **Task spawning fails**
    async fn start_audio_acquisition(&mut self) -> Result<()> {
        // Use the shared config from the daemon
        let config = Arc::clone(&self.config);
        let config_read = config.read().await;

        // Early return if acquisition is disabled in configuration
        if !config_read.acquisition.enabled {
            drop(config_read);
            info!("Audio acquisition is disabled in configuration");
            return Ok(());
        }

        info!("Starting real-time audio acquisition system");

        // === PHASE 1: Real-Time Audio Source Selection ===
        // Clone the necessary data from config before dropping the read lock
        let photoacoustic_config = config_read.photoacoustic.clone();
        let buffer_size: usize = config_read.photoacoustic.frame_size.into();
        drop(config_read);

        // Select and initialize the appropriate real-time audio source based on configuration
        let audio_source = if let Some(ref simulated_config) = photoacoustic_config.simulated_source
        {
            // Simulated photoacoustic source for testing and advanced simulation
            info!(
                "Using simulated photoacoustic source with type: {}",
                simulated_config.source_type
            );
            get_realtime_simulated_photoacoustic_source(photoacoustic_config.clone())?
        } else if let Some(ref file_path) = photoacoustic_config.input_file {
            // File-based real-time audio source for testing and playback scenarios
            info!("Using real-time file audio source: {}", file_path);
            get_realtime_audio_source_from_file(photoacoustic_config.clone())?
        } else if let Some(ref device_name) = photoacoustic_config.input_device {
            // Named device source for specific hardware targeting
            info!("Using real-time device audio source: {}", device_name);
            get_realtime_audio_source_from_device(photoacoustic_config.clone())?
        } else {
            // Default system audio input as fallback
            info!("Using default real-time audio source");
            get_default_realtime_audio_source(photoacoustic_config.clone())?
        };

        // === PHASE 2: Real-Time Acquisition Daemon Creation ===
        // Create the real-time acquisition daemon with the selected source
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
    /// The task uses the shared `Arc<Config>` for accessing record consumer settings.
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success if record consumer started successfully, or error details
    ///
    /// ### Requirements
    ///
    /// This method requires that `start_audio_acquisition` has been called first to
    /// establish the audio stream. If no audio stream is available, this method will
    /// return an error.
    ///
    /// ### Configuration
    ///
    /// The record consumer is controlled by the `config.photoacoustic.record_consumer` flag.
    /// When enabled, it will:
    /// - Start consuming audio frames after audio acquisition begins
    /// - Save audio stream to WAV file with same precision and sample rate as producer
    /// - Generate debug log messages for studying consumer behavior
    /// - Track throughput statistics (FPS, frame delays)
    ///
    /// ### Examples
    ///
    /// ```no_run,ignore
    /// use rust_photoacoustic::daemon::launch_daemon::Daemon;
    /// use rust_photoacoustic::config::Config;
    /// use std::sync::Arc;
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let mut daemon = Daemon::new();
    ///     let config = Config::load("config.yaml")?;
    ///     let config_arc = Arc::new(config);
    ///
    ///     // Launch daemon with config (starts audio acquisition internally)
    ///     daemon.launch(config_arc).await?;
    ///     
    ///     Ok(())
    /// }
    /// ```
    async fn start_record_consumer(&mut self) -> Result<()> {
        info!("Starting record consumer daemon for validation");
        // Use the shared config from the daemon
        let config = Arc::clone(&self.config);
        let record_file = config.read().await.photoacoustic.record_file.clone();

        // Ensure audio stream is available
        let audio_stream = self.audio_stream.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Audio stream not available. Start audio acquisition first.")
        })?;

        // Create record consumer daemon
        let record_consumer = RecordConsumer::new(audio_stream.clone(), record_file);

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
    /// The task uses the shared `Arc<Config>` for accessing processing configuration.
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success if the processing consumer started successfully
    ///
    /// ### Errors
    ///
    /// This function can fail if:
    /// * Audio stream is not available (acquisition must be started first)
    /// * Processing graph configuration is invalid
    /// * Processing consumer fails to initialize
    async fn start_processing_consumer(&mut self) -> Result<()> {
        info!("Starting processing consumer daemon");
        // Use the shared config from the daemon
        let config = Arc::clone(&self.config);
        let (processing_config, default_graph, photoacoustic_config) = {
            let config_read = config.read().await;
            (
                config_read.processing.clone(),
                config_read.processing.default_graph.clone(),
                config_read.photoacoustic.clone(),
            )
        };

        // Ensure audio stream is available
        let audio_stream = self.audio_stream.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Audio stream not available. Start audio acquisition first.")
        })?;

        // Validate processing configuration
        processing_config
            .validate()
            .map_err(|e| anyhow::anyhow!("Invalid processing configuration: {}", e))?;

        // Create processing graph from configuration with streaming registry, photoacoustic parameters, and computing state
        let processing_graph = ProcessingGraph::from_config_with_all_params(
            &default_graph,
            Some((*self.streaming_registry).clone()),
            &photoacoustic_config,
            Some(self.computing_state.clone()),
        )
        .map_err(|e| anyhow::anyhow!("Failed to create processing graph: {}", e))?;

        // Create processing consumer daemon with shared visualization state and config
        let processing_consumer = ProcessingConsumer::new_with_visualization_state_and_config(
            audio_stream.clone(),
            processing_graph,
            Arc::clone(&self.visualization_state),
            Arc::clone(&self.config),
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
        // Note: We don't create a second processing graph to avoid duplicating streaming nodes
        // in the registry. The actual processing graph is already created and running in the task.
        self.processing_consumer_daemon = None;

        // Register the task for lifecycle management and graceful shutdown
        self.tasks.push(task);
        info!("Processing consumer daemon started successfully");
        Ok(())
    }

    /// Start the thermal regulation system daemon
    ///
    /// Initializes and starts the thermal regulation system with multiple independent
    /// PID controllers, each running in its own thread with individual sampling frequencies.
    /// The system provides precise temperature control for photoacoustic applications.
    ///
    /// This method creates a thermal regulation daemon that manages all configured
    /// thermal regulators according to the shared configuration. Each regulator
    /// operates independently with its own:
    /// - PID controller parameters (Kp, Ki, Kd)
    /// - Sampling frequency (sampling_frequency_hz)
    /// - Target temperature setpoint
    /// - Hardware driver (mock, native, or CP2112)
    ///
    /// The thermal regulation system maintains a shared state with historical data
    /// (up to 3600 data points per regulator) that can be accessed by the web interface
    /// and other system components.
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success if the thermal regulation system started successfully
    ///
    /// ### Errors
    ///
    /// This function can fail if:
    /// * Hardware initialization fails
    /// * Configuration is invalid
    /// * Thread spawning fails
    /// * Driver creation fails
    async fn start_thermal_regulation_system(&mut self) -> Result<()> {
        info!("Starting thermal regulation system");

        // Extract thermal regulation configuration
        let thermal_config = {
            let config = self.config.read().await;
            config.thermal_regulation.clone()
        };

        if !thermal_config.enabled {
            info!("Thermal regulation system is disabled in configuration");
            return Ok(());
        }

        if thermal_config.regulators.is_empty() {
            warn!("No thermal regulators configured");
            return Ok(());
        }

        // Create thermal regulation system daemon
        let mut thermal_daemon = ThermalRegulationSystemDaemon::new(
            thermal_config,
            self.thermal_regulation_state.clone(),
            self.running.clone(),
        );

        // Start the thermal regulation system
        thermal_daemon.start().await?;

        info!(
            "Thermal regulation system started successfully with {} regulators",
            thermal_daemon
                .get_shared_state()
                .read()
                .await
                .get_regulator_ids()
                .len()
        );

        // Store the daemon for later management
        self.thermal_regulation_daemon = Some(thermal_daemon);

        Ok(())
    }

    /// Get thermal regulation shared state for external access
    ///
    /// Returns a reference to the shared thermal regulation state that contains
    /// historical data, current status, and PID parameters for all thermal regulators.
    /// This can be used by the web interface and API endpoints to provide
    /// real-time monitoring and control capabilities.
    ///
    /// ### Returns
    ///
    /// Reference to the shared thermal regulation state
    pub fn get_thermal_regulation_state(&self) -> &SharedThermalState {
        &self.thermal_regulation_state
    }

    /// Update PID parameters for a specific thermal regulator
    ///
    /// Allows dynamic updating of PID controller parameters for a specific regulator
    /// without requiring a system restart. This enables real-time tuning and
    /// optimization of thermal control performance.
    ///
    /// ### Arguments
    ///
    /// * `regulator_id` - Unique identifier of the thermal regulator
    /// * `kp` - Proportional gain
    /// * `ki` - Integral gain  
    /// * `kd` - Derivative gain
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success if parameters were updated successfully
    ///
    /// ### Errors
    ///
    /// This function can fail if:
    /// * The specified regulator is not found
    /// * The thermal regulation system is not running
    pub async fn update_thermal_regulator_pid_parameters(
        &mut self,
        regulator_id: &str,
        kp: f64,
        ki: f64,
        kd: f64,
    ) -> Result<()> {
        if let Some(ref mut thermal_daemon) = self.thermal_regulation_daemon {
            thermal_daemon
                .update_regulator_pid_parameters(regulator_id, kp, ki, kd)
                .await
        } else {
            Err(anyhow::anyhow!("Thermal regulation system is not running"))
        }
    }

    /// Update setpoint temperature for a specific thermal regulator
    ///
    /// Allows dynamic updating of the target temperature setpoint for a specific
    /// regulator without requiring a system restart. This enables real-time control
    /// of thermal regulation targets.
    ///
    /// ### Arguments
    ///
    /// * `regulator_id` - Unique identifier of the thermal regulator
    /// * `setpoint_celsius` - New target temperature in degrees Celsius
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success if setpoint was updated successfully
    ///
    /// ### Errors
    ///
    /// This function can fail if:
    /// * The specified regulator is not found
    /// * The thermal regulation system is not running
    /// * The setpoint is outside safety limits
    pub async fn update_thermal_regulator_setpoint(
        &mut self,
        regulator_id: &str,
        setpoint_celsius: f64,
    ) -> Result<()> {
        if let Some(ref mut thermal_daemon) = self.thermal_regulation_daemon {
            thermal_daemon
                .update_regulator_setpoint(regulator_id, setpoint_celsius)
                .await
        } else {
            Err(anyhow::anyhow!("Thermal regulation system is not running"))
        }
    }

    /// Get the shared data source
    ///
    /// ### Returns
    ///
    /// A reference to the shared data source that can be used by other components
    pub fn get_data_source(&self) -> Arc<PhotoacousticDataSource> {
        self.data_source.clone()
    }

    /// Get the shared computing state
    ///
    /// Returns a clone of the `Arc<RwLock<ComputingSharedData>>` for sharing the
    /// computing state with other components that need access to real-time measurement data.
    ///
    /// ### Returns
    ///
    /// A cloned `Arc` pointing to the shared computing state
    pub fn get_computing_state(&self) -> SharedComputingState {
        Arc::clone(&self.computing_state)
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
    /// the flag becomes `false`. Tasks have access to the shared configuration through
    /// their `Arc<Config>` for any shutdown-specific configuration needs.
    ///
    /// This method only signals the tasks to stop; it does not wait for them to complete.
    /// To wait for all tasks to finish, call `join()` after this method.
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::daemon::launch_daemon::Daemon;
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let mut daemon = Daemon::new();
    ///     // Signal all tasks to stop
    ///     daemon.shutdown();
    ///
    ///     // Wait for all tasks to complete
    ///     daemon.join().await?;
    ///     Ok(())
    /// }
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
    /// All tasks will have had access to the shared `Arc<Config>` during their execution.
    ///
    /// If any task panics, the error is logged but this method will still wait for
    /// all other tasks to complete.
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success if all tasks completed without errors
    ///
    /// ### Errors
    ///
    /// This method logs task panics but does not fail because of them.
    /// It may fail due to other async runtime issues.
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::{config::Config, daemon::launch_daemon::Daemon};
    /// use std::sync::Arc;
    /// use tokio::sync::RwLock;
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let config = Config::from_file("config.yaml")?;
    ///     let config_arc = Arc::new(RwLock::new(config));
    ///     let mut daemon = Daemon::new();
    ///     daemon.launch(config_arc).await?;
    ///     
    ///     // First signal shutdown
    ///     daemon.shutdown();
    ///
    ///     // Then wait for all tasks to finish
    ///     daemon.join().await?;
    ///     println!("All daemon tasks have completed");
    ///     Ok(())
    /// }
    /// ```
    pub async fn join(mut self) -> Result<()> {
        // Stop thermal regulation system if running
        if let Some(ref mut thermal_daemon) = self.thermal_regulation_daemon {
            info!("Stopping thermal regulation system");
            if let Err(e) = thermal_daemon.stop().await {
                error!("Failed to stop thermal regulation system: {}", e);
            }
        }

        // Stop other daemons
        if let Some(ref record_consumer) = self.record_consumer_daemon {
            info!("Stopping record consumer");
            record_consumer.stop();
        }

        if let Some(ref processing_consumer) = self.processing_consumer_daemon {
            info!("Stopping processing consumer");
            processing_consumer.stop().await;
        }

        // Wait for all tasks to complete
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

    /// Update configuration for processing graph nodes dynamically
    ///
    /// This method enables dynamic configuration updates for processing nodes without
    /// requiring a full restart of the processing consumer. It follows the hot-reload
    /// strategy defined in the audit documentation.
    ///
    /// ### Arguments
    ///
    /// * `node_id` - The ID of the node to update
    /// * `parameters` - New configuration parameters as JSON value
    ///
    /// ### Returns
    ///
    /// * `Ok(true)` - Configuration updated successfully (hot-reload)
    /// * `Ok(false)` - Configuration requires node reconstruction
    /// * `Err(anyhow::Error)` - Update failed or processing consumer not available
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::daemon::launch_daemon::Daemon;
    /// use serde_json::json;
    ///
    /// # async fn example(daemon: &Daemon) -> anyhow::Result<()> {
    /// // Update gain parameter for a gain node
    /// let result = daemon.update_processing_node_config("gain_amp", &json!({"gain_db": 6.0})).await;
    /// if result? {
    ///     println!("Gain updated with hot-reload");
    /// } else {
    ///     println!("Node requires reconstruction");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_processing_node_config(
        &self,
        node_id: &str,
        parameters: &serde_json::Value,
    ) -> Result<bool> {
        if let Some(ref processing_consumer) = self.processing_consumer_daemon {
            debug!(
                "Daemon: Updating processing node '{}' configuration",
                node_id
            );
            processing_consumer
                .update_node_config(node_id, parameters)
                .await
        } else {
            warn!("Daemon: Cannot update processing node configuration - processing consumer not available");
            Err(anyhow::anyhow!("Processing consumer not available"))
        }
    }

    /// Update configuration for multiple processing graph nodes
    ///
    /// This method allows batch updates of multiple nodes in the processing graph,
    /// which is more efficient than updating them individually.
    ///
    /// ### Arguments
    ///
    /// * `node_configs` - Map of node ID to new configuration parameters
    ///
    /// ### Returns
    ///
    /// A HashMap where:
    /// * key = node_id
    /// * value = Result<bool> indicating success and whether hot-reload was possible
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::daemon::launch_daemon::Daemon;
    /// use serde_json::json;
    /// use std::collections::HashMap;
    ///
    /// # async fn example(daemon: &Daemon) -> anyhow::Result<()> {
    /// let mut updates = HashMap::new();
    /// updates.insert("gain1".to_string(), json!({"gain_db": 6.0}));
    /// updates.insert("gain2".to_string(), json!({"gain_db": -3.0}));
    ///
    /// let results = daemon.update_multiple_processing_node_configs(&updates).await;
    /// for (node_id, result) in results {
    ///     match result {
    ///         Ok(true) => println!("Node {} updated with hot-reload", node_id),
    ///         Ok(false) => println!("Node {} requires reconstruction", node_id),
    ///         Err(e) => println!("Node {} update failed: {}", node_id, e),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_multiple_processing_node_configs(
        &self,
        node_configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, Result<bool>> {
        if let Some(ref processing_consumer) = self.processing_consumer_daemon {
            debug!(
                "Daemon: Updating configuration for {} processing nodes",
                node_configs.len()
            );
            processing_consumer
                .update_multiple_node_configs(node_configs)
                .await
        } else {
            // Return error for all nodes
            let mut results = std::collections::HashMap::new();
            for node_id in node_configs.keys() {
                results.insert(
                    node_id.clone(),
                    Err(anyhow::anyhow!("Processing consumer not available")),
                );
            }
            warn!("Daemon: Cannot update processing node configurations - processing consumer not available");
            results
        }
    }

    /// Apply configuration changes from the shared config
    ///
    /// This method detects changes in the shared configuration and applies them
    /// to the appropriate components. It follows the audit strategy for determining
    /// which components need restart vs hot-reload.
    ///
    /// ### Arguments
    ///
    /// * `config_changes` - Map of configuration section to changed parameters
    ///
    /// ### Returns
    ///
    /// * `Result<()>` - Success if all applicable changes were processed
    ///
    /// ### Implementation Notes
    ///
    /// This method implements the strategy defined in `audit_impact_reload_daemon.md`:
    /// - Processing node parameters: Attempt hot-reload via ProcessingConsumer
    /// - Structural changes: Log requirement for daemon restart
    /// - Operational parameters: Apply dynamically where possible
    pub async fn apply_configuration_changes(
        &self,
        config_changes: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        debug!(
            "Daemon: Applying {} configuration changes",
            config_changes.len()
        );

        for (section, changes) in config_changes {
            match section.as_str() {
                "processing.default_graph.nodes" => {
                    // Processing node parameter changes
                    if let serde_json::Value::Object(node_changes) = changes {
                        let mut node_configs = std::collections::HashMap::new();

                        for (node_id, node_params) in node_changes {
                            if let serde_json::Value::Object(params) = node_params {
                                if let Some(parameters) = params.get("parameters") {
                                    node_configs.insert(node_id.clone(), parameters.clone());
                                }
                            }
                        }

                        if !node_configs.is_empty() {
                            let results = self
                                .update_multiple_processing_node_configs(&node_configs)
                                .await;

                            // Log results and identify nodes that need reconstruction
                            let mut reconstruction_needed = Vec::new();
                            for (node_id, result) in results {
                                match result {
                                    Ok(true) => {
                                        info!(
                                            "Node '{}' configuration updated with hot-reload",
                                            node_id
                                        );
                                    }
                                    Ok(false) => {
                                        reconstruction_needed.push(node_id.clone());
                                    }
                                    Err(e) => {
                                        error!(
                                            "Failed to update node '{}' configuration: {}",
                                            node_id, e
                                        );
                                    }
                                }
                            }

                            if !reconstruction_needed.is_empty() {
                                warn!("The following nodes require daemon restart for configuration changes: {:?}", 
                                      reconstruction_needed);
                            }
                        }
                    }
                }
                "visualization" => {
                    // Visualization server changes typically require restart
                    warn!(
                        "Visualization configuration changes require daemon restart to take effect"
                    );
                }
                "acquisition" => {
                    // Acquisition changes typically require restart
                    warn!(
                        "Acquisition configuration changes require daemon restart to take effect"
                    );
                }
                "modbus" => {
                    // Modbus server changes typically require restart
                    warn!("Modbus configuration changes require daemon restart to take effect");
                }
                _ => {
                    debug!("Configuration section '{}' changes noted but no specific handler implemented", section);
                }
            }
        }

        Ok(())
    }
}
