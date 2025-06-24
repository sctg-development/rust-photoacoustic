// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Universal Display ActionNode with Pluggable Driver Architecture
//!
//! This module provides a universal, extensible action ActionNode that uses a pluggable
//! driver system for output operations. It demonstrates how to create a flexible ActionNode
//! that can interface with various action technologies and communication protocols through
//! a common driver abstraction.
//!
//! # Driver Architecture
//!
//! The UniversalActionNode uses the ActionDriver trait to abstract action outputs:
//!
//! ```text
//! UniversalActionNode
//!           ↓
//!    ActionDriver trait  
//!           ↓
//! ┌─────────────┬─────────────┬─────────────┬─────────────┐
//! │   HTTPS     │    Redis    │    Kafka    │  Physical   │
//! │  Callback   │   Driver    │   Driver    │   Drivers   │
//! │   Driver    │             │             │  (Future)   │
//! └─────────────┴─────────────┴─────────────┴─────────────┘
//! ```
//!
//! # Available Drivers
//!
//! ## Network/Cloud Drivers (Implemented)
//! - **HttpsCallbackActionDriver**: HTTP/HTTPS webhooks for remote dashboards
//! - **RedisActionDriver**: Redis pub/sub for real-time data streams
//! - **KafkaActionDriver**: Apache Kafka for scalable message streaming
//!
//! ## Physical Hardware Drivers (Planned)
//! - **USBDisplayDriver**: USB-connected actions and HID devices
//! - **SerialDisplayDriver**: RS232/RS485 serial communication actions
//! - **I2CDisplayDriver**: I2C OLED/LCD actions for embedded systems
//! - **LEDStripDriver**: Addressable LED strips and indicator arrays
//! - **GPIODisplayDriver**: Direct GPIO control for custom hardware
//!
//! # Usage Examples
//!
//! ```rust,ignore
//! use crate::processing::computing_nodes::{
//!     UniversalActionNode,
//!     action_drivers::{HttpsCallbackActionDriver, RedisActionDriver}
//! };
//!
//! // HTTP callback driver for web dashboard
//! let http_driver = HttpsCallbackActionDriver::new("https://dashboard.company.com/api/action".to_string())
//!     .with_auth_header("Authorization".to_string(), "Bearer your_api_token_here".to_string())
//!     .with_timeout_ms(5000);
//!
//! let web_display_node = UniversalActionNode::new("web_display".to_string())
//!     .with_history_buffer_capacity(100)
//!     .with_driver(Box::new(http_driver))
//!     .with_concentration_threshold(1000.0)
//!     .with_monitored_node("co2_concentration".to_string());
//!
//! // Redis pub/sub driver for real-time streaming
//! let redis_driver = RedisActionDriver::new_pubsub("redis://localhost:6379", "photoacoustic:realtime:action");
//!
//! let redis_display_node = UniversalActionNode::new("redis_stream".to_string())
//!     .with_history_buffer_capacity(50)
//!     .with_driver(Box::new(redis_driver))
//!     .with_amplitude_threshold(0.8);
//!     
//! // Redis key-value driver with expiration
//! let redis_kv_driver = RedisActionDriver::new_key_value("redis://localhost:6379", "photoacoustic")
//!     .with_expiration_seconds(3600); // 1 hour expiration
//!
//! let redis_kv_node = UniversalActionNode::new("redis_storage".to_string())
//!     .with_history_buffer_capacity(200)
//!     .with_driver(Box::new(redis_kv_driver))
//!     .with_concentration_threshold(500.0)
//!     .with_update_interval(2000); // Update every 2 seconds
//! ```
//!
//! # Driver Development
//!
//! To create a new action driver, implement the `ActionDriver` trait:
//!
//! ```rust,ignore
//! use async_trait::async_trait;
//! use anyhow::Result;
//! use serde_json::{json, Value};
//! use std::collections::HashMap;
//! use std::time::SystemTime;
//! use crate::processing::computing_nodes::action_drivers::{ActionDriver, MeasurementData, AlertData};
//!
//! #[derive(Debug)]
//! pub struct MyCustomDisplayDriver {
//!     endpoint_url: String,
//!     connection_timeout_ms: u64,
//!     is_connected: bool,
//!     // Add your driver-specific fields here
//! }
//!
//! impl MyCustomDisplayDriver {
//!     pub fn new(endpoint_url: String) -> Self {
//!         Self {
//!             endpoint_url,
//!             connection_timeout_ms: 5000,
//!             is_connected: false,
//!         }
//!     }
//!     
//!     pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
//!         self.connection_timeout_ms = timeout_ms;
//!         self
//!     }
//! }
//!
//! #[async_trait]
//! impl ActionDriver for MyCustomDisplayDriver {
//!     async fn initialize(&mut self) -> Result<()> {
//!         // Initialize your action hardware/service
//!         // Example: establish network connection, test hardware, etc.
//!         log::info!("Initializing MyCustomDisplayDriver");
//!         self.is_connected = true;
//!         Ok(())
//!     }
//!
//!     async fn update_display(&mut self, data: &MeasurementData) -> Result<()> {
//!         // Update your action with concentration data
//!         // data.concentration_ppm contains the current value
//!         // data.timestamp contains when the measurement was taken
//!         log::debug!("Updating action: {:.2} ppm", data.concentration_ppm);
//!         Ok(())
//!     }
//!
//!     async fn show_alert(&mut self, alert: &AlertData) -> Result<()> {
//!         // Show alert/alarm on your action
//!         // alert.severity: "info", "warning", "critical"
//!         // alert.message: human-readable alert text
//!         log::warn!("Showing {} alert: {}", alert.severity, alert.message);
//!         Ok(())
//!     }
//!     
//!     async fn clear_display(&mut self) -> Result<()> {
//!         // Clear the action and return to idle state
//!         log::debug!("Clearing action");
//!         Ok(())
//!     }
//!     
//!     async fn get_status(&self) -> Result<Value> {
//!         // Return status information for monitoring
//!         Ok(json!({
//!             "driver_type": self.driver_type(),
//!             "endpoint_url": self.endpoint_url,
//!             "is_connected": self.is_connected,
//!             "timeout_ms": self.connection_timeout_ms
//!         }))
//!     }
//!     
//!     fn driver_type(&self) -> &str {
//!         "my_custom_driver"
//!     }
//!     
//!     async fn shutdown(&mut self) -> Result<()> {
//!         // Clean up resources when shutting down
//!         log::info!("Shutting down MyCustomDisplayDriver");
//!         self.is_connected = false;
//!         Ok(())
//!     }
//! }
//! ```
//!
//! ## Required Dependencies
//!
//! Add these to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! async-trait = "0.1"
//! anyhow = "1.0"
//! serde_json = "1.0"
//! log = "0.4"
//! tokio = { version = "1.0", features = ["rt-multi-thread", "macros"] }
//! ```
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! // Create and configure your driver
//! let my_driver = MyCustomDisplayDriver::new("https://my-api.com/action".to_string())
//!     .with_timeout(10000);
//!
//! // Use the driver in an ActionNode
//! let display_node = UniversalActionNode::new("my_display".to_string())
//!     .with_history_buffer_capacity(100)
//!     .with_driver(Box::new(my_driver))
//!     .with_concentration_threshold(1000.0);
//! ```
//!
//! ## Driver Development Tips
//!
//! 1. **Error Handling**: Always use `anyhow::Result` and log errors appropriately
//! 2. **Async Operations**: Use `async/await` for I/O operations, avoid blocking calls
//! 3. **Configuration**: Implement builder pattern for flexible configuration
//! 4. **Status Monitoring**: Provide detailed status information for debugging
//! 5. **Resource Cleanup**: Implement proper shutdown to avoid resource leaks
//! 6. **Testing**: Create unit tests for your driver methods
//!
//! For a complete development guide, see: `docs/display_driver_development_guide.md`
//!
//! # ActionNode Template
//!
//! This UniversalActionNode serves as a comprehensive template for creating
//! specialized ActionNode instances with pluggable backends. The patterns demonstrated
//! here can be adapted for various industrial and IoT applications.
//!
//! # Key Features
//!
//! - **Pluggable Drivers**: Easy to extend with new action technologies
//! - **Async Driver Interface**: Non-blocking operations for network/hardware drivers
//! - **Synchronous ActionNode**: Compatible with existing synchronous pipeline
//! - **Comprehensive Monitoring**: Buffer management, performance tracking
//! - **Threshold-based Triggers**: Configurable alarm conditions
//! - **Builder Pattern Configuration**: Fluent API for setup and customization

use crate::processing::computing_nodes::{
    action_drivers::{ActionDriver, AlertData, MeasurementData},
    ActionHistoryEntry, ActionNode, ActionNodeHelper, ActionTrigger, CircularBuffer,
    ComputingSharedData, SharedComputingState,
};
use crate::processing::nodes::{ProcessingData, ProcessingNode};
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use serde_json::json;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;

/// Messages sent to the action processing thread
#[derive(Debug, Clone)]
enum ActionMessage {
    Update(MeasurementData),
    Alert(AlertData),
    Shutdown,
}

/// Universal Action Node with Pluggable Driver Architecture
///
/// This is a production-ready ActionNode that demonstrates the pluggable driver pattern
/// for flexible action output. Unlike simple example nodes, this implementation provides
/// a complete framework for interfacing with various action technologies through a
/// unified driver abstraction.
///
/// # Driver-Based Architecture
///
/// The node delegates all action operations to a configured ActionDriver, enabling:
/// - **HTTP/HTTPS Callbacks**: For web dashboards and cloud integration
/// - **Redis Pub/Sub**: For real-time data streaming and caching
/// - **Kafka Messaging**: For scalable event streaming architectures
/// - **Physical Hardware**: Future drivers for LCD, LED, OLED actions
/// - **Custom Protocols**: Easy extension for proprietary systems
///
/// # Core Responsibilities
/// - **Signal Pass-through**: Maintains pipeline integrity by passing data unchanged
/// - **Computing Data Monitoring**: Watches for updates from PeakFinder/Concentration nodes
/// - **Driver-based Output**: Routes action operations through configured driver
/// - **Threshold Monitoring**: Configurable triggers for automated alerts
/// - **Historical Tracking**: Maintains circular buffer of computing data history
/// - **Performance Monitoring**: Tracks operation counts and timing
///
/// # Configuration Example
///
/// ```rust,ignore
/// // Create with HTTP driver for remote dashboard
/// let http_driver = HttpsCallbackActionDriver::new()
///     .with_callback_url("https://api.company.com/photoacoustic/action")
///     .with_auth_header("Authorization", "Bearer your_token")
///     .build()?;
///
/// let display_node = UniversalActionNode::new("main_display".to_string())
///     .with_history_buffer_capacity(200)        // 200 entries of history
///     .with_driver(Box::new(http_driver))       // Use HTTP driver
///     .with_concentration_threshold(1000.0)     // Alert at 1000 ppm
///     .with_amplitude_threshold(0.8)            // Alert at 80% amplitude
///     .with_monitored_node("co2_concentration".to_string())
///     .with_update_interval(1000);              // Update every second
/// ```
///
/// # Driver Interface Benefits
///
/// - **Hot-swappable**: Change action technology without code changes
/// - **Testable**: Mock drivers for unit testing and simulation
/// - **Scalable**: From single actions to distributed dashboard networks
/// - **Future-proof**: Easy addition of new hardware without breaking changes
/// - **Configuration-driven**: Driver selection and configuration via YAML/JSON
///
/// # Extension Points for Custom ActionNodes
///
/// This implementation demonstrates patterns that can be adapted for:
/// - **EmailActionNode**: SMTP driver for email notifications
/// - **RelayActionNode**: GPIO drivers for industrial control
/// - **DatabaseActionNode**: SQL/NoSQL drivers for data logging
/// - **CloudActionNode**: AWS/Azure/GCP drivers for cloud integration
/// - **WebhookActionNode**: HTTP drivers for external system integration
///
/// The driver pattern shown here provides a template for creating modular,
/// extensible ActionNodes that can adapt to changing requirements without
/// architectural changes to the core processing pipeline.
#[derive(Debug)]
pub struct UniversalActionNode {
    /// Channel sender for sending messages to the action processing thread
    action_sender: Option<mpsc::Sender<ActionMessage>>,
    /// Handle to the action processing thread
    action_thread_handle: Option<thread::JoinHandle<()>>,
    /// Unique identifier for this action node
    /// REQUIRED: Every ActionNode must have a unique ID for monitoring and debugging
    id: String,

    /// Circular buffer for storing historical data
    /// REQUIRED: ActionNode trait mandates historical data tracking
    /// The buffer automatically manages size and removes old entries
    history_buffer: CircularBuffer<ActionHistoryEntry>,

    /// List of computing node IDs to monitor
    /// REQUIRED: Defines which PeakFinder/Concentration nodes this ActionNode watches
    /// Empty list means no monitoring (ActionNode becomes pass-through only)
    monitored_nodes: Vec<String>,

    /// Shared computing state for reading analytical results
    /// REQUIRED: Connection to the shared data structure containing computing results
    /// Set by ProcessingGraph when the node is added to the graph
    shared_computing_state: Option<SharedComputingState>,

    /// Configuration thresholds - CUSTOMIZABLE PATTERN
    /// These demonstrate how to add configurable trigger conditions
    /// Replace with your own threshold types for custom ActionNodes
    concentration_threshold: Option<f64>, // ppm threshold for concentration alerts
    amplitude_threshold: Option<f32>, // normalized amplitude threshold (0.0-1.0)

    /// Display configuration - HARDWARE-SPECIFIC PATTERN
    /// Replace this section with your own hardware/service configuration
    /// Examples: GPIO pin numbers, SMTP server config, webhook URLs, etc.
    action_update_interval_ms: u64, // How often to update action (throttling)

    /// Performance statistics - MONITORING PATTERN
    /// These fields demonstrate how to track ActionNode performance
    /// Useful for debugging and system monitoring
    processing_count: u64, // Total number of process() calls
    actions_triggered: u64,                 // Total number of actions executed
    last_update_time: Option<SystemTime>,   // When computing data was last processed
    last_action_update: Option<SystemTime>, // When action was last updated (hardware-specific)
}

impl UniversalActionNode {
    /// Create a new UniversalActionNode with required configuration
    ///
    /// # IMPORTANT: Buffer capacity must be explicitly configured
    ///
    /// This constructor creates an ActionNode with a minimal buffer that MUST be
    /// configured using with_history_buffer_capacity() to set the appropriate size
    /// for your use case. This ensures that buffer sizing is intentional.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this ActionNode instance
    ///
    /// # Returns
    /// A new ActionNode instance that REQUIRES buffer capacity configuration
    ///
    /// # Required Configuration
    /// ```rust,ignore
    /// let node = UniversalActionNode::new("action".to_string())
    ///     .with_history_buffer_capacity(200)  // REQUIRED: Set buffer size
    ///     .with_concentration_threshold(1000.0)
    ///     .with_monitored_node("concentration_calc".to_string());
    /// ```
    pub fn new(id: String) -> Self {
        Self {
            id,
            action_sender: None,                    // No thread started yet
            action_thread_handle: None,             // No thread started yet
            history_buffer: CircularBuffer::new(1), // Minimal buffer - MUST configure with with_history_buffer_capacity()
            monitored_nodes: Vec::new(),            // Empty: add nodes via with_monitored_node()
            shared_computing_state: None,           // Set later by ProcessingGraph
            concentration_threshold: Some(1000.0),  // Default: 1000 ppm CO2 alarm
            amplitude_threshold: Some(0.8),         // Default: 80% amplitude alarm
            action_update_interval_ms: 1000,        // Default: update every second
            processing_count: 0,                    // Performance counter
            actions_triggered: 0,                   // Action counter
            last_update_time: None,                 // No updates yet
            last_action_update: None,               // No action updates yet
        }
    }

    /// Create a new UniversalActionNode with shared computing state
    ///
    /// # PATTERN: Shared state constructor for ActionNode
    /// This constructor is used by the ProcessingGraph when creating ActionNodes
    /// that need access to shared computing data from PeakFinder and Concentration nodes.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this ActionNode instance
    /// * `shared_state` - Optional shared computing state. If None, creates a new one.
    ///
    /// # Returns
    /// A new ActionNode instance with the provided or new shared state
    ///
    /// # Example
    /// ```rust,ignore
    /// let shared_state = Some(Arc::new(RwLock::new(ComputingSharedData::default())));
    /// let node = UniversalActionNode::new_with_shared_state(
    ///     "action".to_string(),
    ///     shared_state
    /// ).with_history_buffer_capacity(100);
    /// ```
    pub fn new_with_shared_state(id: String, shared_state: Option<SharedComputingState>) -> Self {
        Self {
            id,
            action_sender: None,                    // No thread started yet
            action_thread_handle: None,             // No thread started yet
            history_buffer: CircularBuffer::new(1), // Minimal buffer - MUST configure with with_history_buffer_capacity()
            monitored_nodes: Vec::new(),            // Empty: add nodes via with_monitored_node()
            shared_computing_state: shared_state,   // Use provided shared state
            concentration_threshold: Some(1000.0),  // Default: 1000 ppm CO2 alarm
            amplitude_threshold: Some(0.8),         // Default: 80% amplitude alarm
            action_update_interval_ms: 1000,        // Default: update every second
            processing_count: 0,                    // Performance counter
            actions_triggered: 0,                   // Action counter
            last_update_time: None,                 // No updates yet
            last_action_update: None,               // No action updates yet
        }
    }

    /// Configure the history buffer capacity - REQUIRED METHOD
    ///
    /// # PATTERN: Explicit buffer capacity configuration
    ///
    /// This method must be called to set the appropriate buffer size for your use case.
    /// The buffer size determines how much historical data the ActionNode retains
    /// for analysis and debugging.
    ///
    /// # Buffer Size Guidelines
    ///
    /// - **Real-time Display**: 50-100 entries (a few minutes of data)
    /// - **Trend Analysis**: 200-500 entries (several hours of data)  
    /// - **Historical Logging**: 1000+ entries (days of data)
    /// - **Memory Constrained**: 10-50 entries (minimal memory usage)
    ///
    /// # Arguments
    /// * `capacity` - Number of historical entries to retain
    ///
    /// # Examples
    /// ```rust,ignore
    /// // For a real-time action updating every second
    /// let display_node = UniversalActionNode::new("action".to_string())
    ///     .with_history_buffer_capacity(60);  // 1 minute of history
    ///
    /// // For trend analysis over several hours
    /// let logger_node = UniversalActionNode::new("logger".to_string())
    ///     .with_history_buffer_capacity(3600); // 1 hour at 1Hz
    ///
    /// // For memory-constrained embedded systems
    /// let led_node = UniversalActionNode::new("led".to_string())
    ///     .with_history_buffer_capacity(10);   // Just recent data
    /// ```
    ///
    /// # Custom ActionNode Adaptation
    /// ```rust,ignore
    /// impl MyActionNode {
    ///     pub fn with_history_buffer_capacity(mut self, capacity: usize) -> Self {
    ///         self.history_buffer.resize(capacity);
    ///         self
    ///     }
    /// }
    /// ```
    pub fn with_history_buffer_capacity(mut self, capacity: usize) -> Self {
        self.history_buffer.resize(capacity);
        self
    }

    // ========================================================================
    // BUILDER PATTERN METHODS - CONFIGURATION API
    // ========================================================================
    // These methods demonstrate the builder pattern for ActionNode configuration.
    // When creating your own ActionNode, add similar methods for your specific
    // configuration parameters.

    /// Configure concentration threshold for automatic alerts
    ///
    /// # PATTERN: Builder method for threshold configuration
    /// Use this pattern for any configurable thresholds in your ActionNode.
    /// Always use Option<T> for thresholds to allow disabling them.
    ///
    /// # Arguments  
    /// * `threshold` - Concentration in ppm that triggers alerts
    ///
    /// # Example
    /// ```rust,ignore
    /// let node = UniversalActionNode::new("action".to_string())
    ///     .with_concentration_threshold(500.0);  // Alert at 500 ppm
    /// ```
    pub fn with_concentration_threshold(mut self, threshold: f64) -> Self {
        self.concentration_threshold = Some(threshold);
        self
    }

    /// Configure amplitude threshold for signal strength alerts
    ///
    /// # PATTERN: Similar builder method for different threshold type
    ///
    /// # Arguments
    /// * `threshold` - Normalized amplitude (0.0-1.0) that triggers alerts  
    pub fn with_amplitude_threshold(mut self, threshold: f32) -> Self {
        self.amplitude_threshold = Some(threshold);
        self
    }

    /// Add a computing node to the monitoring list
    ///
    /// # PATTERN: Builder method for adding monitored dependencies
    /// ActionNodes typically monitor one or more ComputingNodes (PeakFinder, Concentration).
    /// This method demonstrates how to build the monitoring list during configuration.
    ///
    /// # Arguments
    /// * `node_id` - ID of the PeakFinderNode or ConcentrationNode to monitor
    ///
    /// # Example
    /// ```rust,ignore
    /// let node = UniversalActionNode::new("action".to_string())
    ///     .with_monitored_node("concentration_co2".to_string())
    ///     .with_monitored_node("co2_concentration".to_string());
    /// ```
    pub fn with_monitored_node(mut self, node_id: String) -> Self {
        if !self.monitored_nodes.contains(&node_id) {
            self.monitored_nodes.push(node_id);
        }
        self
    }

    /// Set action update interval for throttling
    ///
    /// # PATTERN: Builder method for hardware-specific timing configuration
    /// Replace this with your own hardware/service timing parameters.
    /// Examples: email send intervals, relay activation delays, etc.
    ///
    /// # Arguments
    /// * `interval_ms` - Minimum milliseconds between action updates
    pub fn with_update_interval(mut self, interval_ms: u64) -> Self {
        self.action_update_interval_ms = interval_ms;
        self
    }

    /// Configure the action driver for output operations
    ///
    /// # PATTERN: Builder method for pluggable driver configuration
    /// This is the key extension point for the UniversalActionNode.
    /// Different drivers implement the ActionDriver trait to provide:
    /// - HTTP/HTTPS callback endpoints
    /// - Redis pub/sub messaging
    /// - Kafka message streaming  
    /// - Physical action control (LCD, LED, OLED)
    /// - Future hardware interfaces (GPIO, SPI, I2C, etc.)
    ///
    /// This method starts an internal processing thread that handles action operations
    /// asynchronously, maintaining compatibility with the synchronous ProcessingNode trait.
    ///
    /// # Arguments
    /// * `driver` - A boxed ActionDriver implementation
    ///
    /// # Example
    /// ```rust,ignore
    /// use crate::processing::computing_nodes::action_drivers::*;
    ///
    /// // HTTP callback driver
    /// let http_driver = HttpsCallbackActionDriver::new("https://myserver.com/action");
    ///
    /// let node = UniversalActionNode::new("action".to_string())
    ///     .with_history_buffer_capacity(100)
    ///     .with_driver(Box::new(http_driver));
    /// ```
    pub fn with_driver(mut self, mut driver: Box<dyn ActionDriver>) -> Self {
        // Create channel for communicating with the action thread
        let (sender, receiver) = mpsc::channel::<ActionMessage>();

        // Start the action processing thread
        let node_id = self.id.clone();
        let handle = thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    error!(
                        "Display thread [{}]: Failed to create tokio runtime: {}",
                        node_id, e
                    );
                    return;
                }
            };

            // Initialize the driver
            if let Err(e) = rt.block_on(driver.initialize()) {
                error!(
                    "Display thread [{}]: Failed to initialize driver: {}",
                    node_id, e
                );
                return;
            }

            info!(
                "Display thread [{}]: Driver initialized successfully",
                node_id
            );

            // Process messages
            while let Ok(message) = receiver.recv() {
                match message {
                    ActionMessage::Update(data) => {
                        if let Err(e) = rt.block_on(driver.update_action(&data)) {
                            error!(
                                "Display thread [{}]: Failed to update action: {}",
                                node_id, e
                            );
                        } else {
                            debug!(
                                "Display thread [{}]: Successfully updated action with {:.2} ppm",
                                node_id, data.concentration_ppm
                            );
                        }
                    }
                    ActionMessage::Alert(alert) => {
                        if let Err(e) = rt.block_on(driver.show_alert(&alert)) {
                            error!("Display thread [{}]: Failed to show alert: {}", node_id, e);
                        } else {
                            debug!(
                                "Display thread [{}]: Successfully showed alert: {}",
                                node_id, alert.message
                            );
                        }
                    }
                    ActionMessage::Shutdown => {
                        info!("Display thread [{}]: Shutting down", node_id);
                        break;
                    }
                }
            }

            info!("Display thread [{}]: Thread terminated", node_id);
        });

        self.action_sender = Some(sender);
        self.action_thread_handle = Some(handle);
        self
    }

    /// Initialize the configured driver
    ///
    /// # PATTERN: Driver initialization method
    /// This method should be called after configuring the node with a driver.
    /// It initializes the driver's connection and prepares it for operation.
    ///
    /// # Returns
    /// * `Ok(())` - Driver initialized successfully
    /// * `Err(anyhow::Error)` - Driver initialization failed
    ///
    /// # Example
    /// ```rust,ignore
    /// let mut node = UniversalActionNode::new("action".to_string())
    ///     .with_history_buffer_capacity(100)
    ///     .with_driver(Box::new(http_driver));
    ///
    /// // Initialize the driver before using the node
    /// node.initialize_driver().await?;
    /// ```
    /// Check if a driver is configured and thread is running
    pub fn has_driver(&self) -> bool {
        self.action_sender.is_some() && self.action_thread_handle.is_some()
    }

    /// Send a action update message to the processing thread
    fn send_action_update(&self, data: MeasurementData) {
        if let Some(ref sender) = self.action_sender {
            if let Err(e) = sender.send(ActionMessage::Update(data)) {
                error!("Failed to send action update to thread: {}", e);
            }
        }
    }

    /// Send an alert message to the processing thread
    fn send_alert(&self, alert: AlertData) {
        if let Some(ref sender) = self.action_sender {
            if let Err(e) = sender.send(ActionMessage::Alert(alert)) {
                error!("Failed to send alert to thread: {}", e);
            }
        }
    }

    // ========================================================================
    // HARDWARE INTERFACE METHODS - CUSTOMIZE FOR YOUR ACTIONNODE
    // ========================================================================
    // These methods simulate hardware interaction. Replace them with actual
    // hardware drivers, API calls, or service interfaces for your ActionNode.

    /// Simulate updating a physical action with concentration data
    ///
    /// # PATTERN: Hardware interaction method
    /// This method demonstrates how to interface with physical hardware or services.
    /// In your ActionNode, replace this with:
    /// - GPIO operations for LED/relay control
    /// - SPI/I2C for action communication  
    /// - HTTP requests for webhooks
    /// - SMTP for email notifications
    /// - Database operations for logging
    ///
    /// # Key Design Points
    /// - Always log the action for debugging
    /// - Update performance tracking fields
    /// - Return Result<()> for error handling
    /// - Keep hardware operations fast (non-blocking)
    ///
    /// # Arguments
    /// * `concentration` - Current concentration value to action
    /// * `source_node` - Node ID that provided this data (for debugging)
    ///
    /// # Returns
    /// * `Ok(())` - Display updated successfully
    /// * `Err(anyhow::Error)` - Hardware communication failed
    /// Update the action with new concentration data using the configured driver
    ///
    /// # PATTERN: Driver-based action update
    /// This method demonstrates how to use the pluggable driver system to update
    /// actions. The actual output mechanism depends on the configured driver:
    /// - HttpsCallbackActionDriver: HTTP POST to endpoint
    /// - RedisActionDriver: Publish to Redis channel
    /// - KafkaActionDriver: Send to Kafka topic
    /// - Future hardware drivers: GPIO, SPI, I2C, etc.
    ///
    /// Updates the action with concentration data - no longer a sync wrapper
    /// This version handles both sync and async contexts safely
    fn update_action_safely(&mut self, concentration: f64, source_node: &str) -> Result<()> {
        // Log the update
        info!(
            "Display Update Queued [{}]: {:.2} ppm from node '{}'",
            self.id, concentration, source_node
        );

        // Get peak amplitude and frequency from shared state
        let (peak_amplitude, peak_frequency) =
            if let Some(shared_state) = self.shared_computing_state.clone() {
                if let Ok(computing_data) = shared_state.try_read() {
                    (
                        computing_data.peak_amplitude.unwrap_or(0.0),
                        computing_data.peak_frequency.unwrap_or(0.0),
                    )
                } else {
                    (0.0, 0.0) // Fallback if state is locked
                }
            } else {
                (0.0, 0.0) // Fallback if no shared state
            };

        // Send action data to the processing thread
        let measurement_data = MeasurementData {
            concentration_ppm: concentration,
            source_node_id: source_node.to_string(),
            peak_amplitude,
            peak_frequency,
            timestamp: SystemTime::now(),
            metadata: HashMap::new(),
        };

        self.send_action_update(measurement_data);

        // Update the timestamp to prevent too frequent updates
        self.last_action_update = Some(SystemTime::now());

        Ok(())
    }

    /// Sends a flash action alert to the processing thread
    fn flash_action_safely(&mut self, reason: &str) -> Result<()> {
        // Log the alert
        warn!("Display Alarm Queued [{}]: {}", self.id, reason);

        // Send alert to the processing thread
        let alert = AlertData {
            alert_type: "threshold_exceeded".to_string(),
            severity: "warning".to_string(),
            message: reason.to_string(),
            data: HashMap::new(),
            timestamp: SystemTime::now(),
        };

        self.send_alert(alert);

        // Update counters
        self.actions_triggered += 1;

        Ok(())
    }

    // ========================================================================
    // UTILITY METHODS - REUSABLE PATTERNS
    // ========================================================================

    /// Check if enough time has passed since last action update (throttling)
    ///
    /// # PATTERN: Rate limiting for hardware operations
    /// This demonstrates how to implement throttling to prevent overwhelming
    /// hardware or external services. Essential for:
    /// - Display updates (prevent flicker)
    /// - Email notifications (prevent spam)
    /// - API calls (respect rate limits)
    /// - GPIO operations (prevent wear)
    ///
    /// # Returns
    /// * `true` - Enough time has passed, safe to update
    /// * `false` - Too soon, skip this update
    fn should_update_action(&self) -> bool {
        if let Some(last_update) = self.last_action_update {
            if let Ok(elapsed) = last_update.elapsed() {
                elapsed.as_millis() >= self.action_update_interval_ms as u128
            } else {
                true // If time calculation fails, allow update (safe default)
            }
        } else {
            true // First update always allowed
        }
    }

    /// Helper method to update from shared state without borrowing conflicts
    ///
    /// # PATTERN: Borrow-safe shared state access
    /// This method demonstrates the correct pattern for accessing shared computing
    /// state without running into Rust's borrow checker issues. Key techniques:
    ///
    /// 1. **Clone the Arc**: `self.shared_computing_state.clone()`
    /// 2. **try_read()**: Non-blocking read attempt
    /// 3. **Clone data**: Clone the data structure to release the lock quickly
    /// 4. **Explicit drop**: Release the read lock before calling mutable methods
    ///
    /// This pattern is CRITICAL for ActionNode implementations to avoid deadlocks
    /// and borrow checker errors in concurrent environments.
    ///
    /// # Why This Pattern Works
    /// - Cloning Arc is cheap (just reference counting)
    /// - try_read() won't block if computing nodes are writing
    /// - Cloning ComputingSharedData releases the lock immediately  
    /// - No mutable borrow conflicts when calling self methods
    fn try_update_from_shared_state(&mut self) {
        // PATTERN: Safe shared state access - copy this exactly in your ActionNode
        if let Some(shared_state) = self.shared_computing_state.clone() {
            if let Ok(computing_data) = shared_state.try_read() {
                // Clone the data to release the lock immediately - CRITICAL!
                let computing_data_clone = computing_data.clone();
                drop(computing_data); // Explicit lock release

                // Now safe to call mutable methods on self
                let _ = self.update_from_computing_data(&computing_data_clone);
            }
            // If try_read() fails, skip this update cycle (non-blocking behavior)
        }
    }

    /// Get measurement history from the action node's buffer
    ///
    /// This method retrieves measurement data stored in the action node's history buffer,
    /// allowing external systems to access historical data without creating dedicated logging nodes.
    ///
    /// # Arguments
    /// * `limit` - Optional maximum number of entries to return (newest first)
    ///
    /// # Returns
    /// Vector of MeasurementData entries in chronological order (newest first)
    ///
    /// # Example
    /// ```rust,ignore
    /// let action_node = UniversalActionNode::new("redis_stream".to_string())
    ///     .with_history_buffer_capacity(100);
    ///
    /// // Get last 50 measurements
    /// let recent_history = action_node.get_measurement_history(Some(50));
    ///
    /// // Get all available measurements
    /// let full_history = action_node.get_measurement_history(None);
    /// ```
    pub fn get_measurement_history(&self, limit: Option<usize>) -> Vec<MeasurementData> {
        let buffer_data = self.history_buffer.iter().collect::<Vec<_>>();

        // Convert ActionHistoryEntry to MeasurementData
        let measurements: Vec<MeasurementData> = buffer_data
            .into_iter()
            .rev() // Newest first
            .map(|entry| MeasurementData {
                concentration_ppm: entry
                    .concentration_data
                    .as_ref()
                    .map(|c| c.concentration_ppm)
                    .unwrap_or(0.0),
                source_node_id: entry.source_node_id.clone(),
                peak_amplitude: entry.peak_data.as_ref().map(|p| p.amplitude).unwrap_or(0.0),
                peak_frequency: entry.peak_data.as_ref().map(|p| p.frequency).unwrap_or(0.0),
                timestamp: entry.timestamp,
                metadata: entry
                    .metadata
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect(),
            })
            .take(limit.unwrap_or(usize::MAX))
            .collect();

        measurements
    }

    /// Get statistics about the action node's history buffer
    ///
    /// This method returns comprehensive statistics about the action node including
    /// buffer usage, configuration, and performance metrics.
    ///
    /// # Returns
    /// JSON object containing detailed statistics
    ///
    /// # Example
    /// ```rust,ignore
    /// let action_node = UniversalActionNode::new("web_dashboard".to_string())
    ///     .with_history_buffer_capacity(100);
    ///
    /// let stats = action_node.get_history_statistics();
    /// println!("Buffer capacity: {}", stats["history_buffer"]["capacity"]);
    /// ```
    pub fn get_history_statistics(&self) -> serde_json::Value {
        let buffer_data = self.history_buffer.iter().collect::<Vec<_>>();

        let oldest_timestamp = buffer_data.first().map(|entry| {
            entry
                .timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        });

        let newest_timestamp = buffer_data.last().map(|entry| {
            entry
                .timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        });

        serde_json::json!({
            "node_id": self.id,
            "node_type": "action_universal",
            "history_buffer": {
                "capacity": self.history_buffer.capacity(),
                "current_size": self.history_buffer.len(),
                "is_full": self.history_buffer.len() == self.history_buffer.capacity(),
                "oldest_entry_timestamp": oldest_timestamp,
                "newest_entry_timestamp": newest_timestamp
            },
            "configuration": {
                "monitored_nodes": self.monitored_nodes,
                "concentration_threshold": self.concentration_threshold,
                "amplitude_threshold": self.amplitude_threshold,
                "update_interval_ms": self.action_update_interval_ms
            },
            "driver_info": {
                "has_driver": self.has_driver(),
                "driver_type": if self.has_driver() { "configured" } else { "none" }
            },
            "performance": {
                "processing_count": self.processing_count,
                "actions_triggered": self.actions_triggered,
                "last_update_time": self.last_update_time.map(|t|
                    t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
                ),
                "last_action_update": self.last_action_update.map(|t|
                    t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
                )
            }
        })
    }
}

// ============================================================================
// PROCESSINGNODE IMPLEMENTATION - REQUIRED BOILERPLATE
// ============================================================================
// This implementation is mostly boilerplate that you can copy to your own
// ActionNode. The key method is process() which implements pass-through behavior.

impl ProcessingNode for UniversalActionNode {
    /// Process input data (pass-through behavior) - CORE ACTIONNODE PATTERN
    ///
    /// This method implements the fundamental ActionNode pattern:
    /// 1. **Count processing calls** for performance monitoring
    /// 2. **Update from computing data** to check for trigger conditions
    /// 3. **Return input unchanged** to maintain pipeline integrity
    ///
    /// # CRITICAL: Pass-through Behavior
    /// ActionNodes MUST return the input data unchanged. The signal processing
    /// pipeline depends on this. Actions are performed as side effects during
    /// the update process, not by modifying the signal data.
    ///
    /// # Error Handling Strategy  
    /// - Processing errors should NOT fail the entire pipeline
    /// - Log errors but continue signal processing
    /// - Use Result<()> for action methods, but don't propagate to process()
    ///
    /// # Threading Considerations
    /// - This method is called by the processing pipeline thread
    /// - Keep processing fast to avoid pipeline delays
    /// - Use try_read() for non-blocking shared state access
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Step 1: Track performance - useful for monitoring and debugging
        self.processing_count += 1;

        // Step 2: Update from computing data if available
        // This is where ActionNode reacts to analytical results
        self.try_update_from_shared_state();

        // Step 3: CRITICAL - Return input unchanged (pass-through behavior)
        // ActionNodes NEVER modify the signal data, they only react to it
        // The action updates are now handled asynchronously by the internal thread
        Ok(input)
    }

    // ========================================================================
    // STANDARD PROCESSINGNODE BOILERPLATE - COPY TO YOUR ACTIONNODE
    // ========================================================================
    // These methods are required by ProcessingNode but are mostly boilerplate.
    // Copy them to your ActionNode and adjust node_type() as needed.

    /// Get the node's unique identifier (required by ProcessingNode)
    fn node_id(&self) -> &str {
        &self.id
    }

    /// Get the node's type identifier (customize this for your ActionNode)
    ///
    /// # PATTERN: Node type naming convention
    /// Use "action_" prefix followed by your specific type:
    /// - "action_generic" for generic action management
    /// - "action_relay" for GPIO/relay control  
    /// - "action_email" for email notifications
    /// - "action_webhook" for HTTP callbacks
    /// - "action_database" for data logging
    fn node_type(&self) -> &str {
        "action_generic_example"
    }

    /// Check if this node can accept the given input type (required by ProcessingNode)
    ///
    /// # PATTERN: ActionNode input acceptance
    /// ActionNodes are pass-through, so they accept ANY input type.
    /// Always return true unless you have specific input requirements.
    fn accepts_input(&self, _input: &ProcessingData) -> bool {
        // ActionNode accepts any input type (pass-through behavior)
        true
    }

    /// Get the expected output type for the given input (required by ProcessingNode)
    ///
    /// # PATTERN: Pass-through type mapping
    /// ActionNodes return the same type as input since they don't modify data.
    /// Copy this implementation exactly for your ActionNode.
    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        // Pass-through: output type matches input
        match input {
            ProcessingData::AudioFrame(_) => Some("AudioFrame".to_string()),
            ProcessingData::SingleChannel { .. } => Some("SingleChannel".to_string()),
            ProcessingData::DualChannel { .. } => Some("DualChannel".to_string()),
            ProcessingData::PhotoacousticResult { .. } => Some("PhotoacousticResult".to_string()),
        }
    }

    fn reset(&mut self) {
        self.reset_action_state();
    }

    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        let mut cloned = UniversalActionNode::new(self.id.clone())
            .with_history_buffer_capacity(self.history_buffer.capacity()) // IMPORTANT: Preserve buffer capacity
            .with_update_interval(self.action_update_interval_ms);

        if let Some(threshold) = self.concentration_threshold {
            cloned = cloned.with_concentration_threshold(threshold);
        }

        if let Some(threshold) = self.amplitude_threshold {
            cloned = cloned.with_amplitude_threshold(threshold);
        }

        for node_id in &self.monitored_nodes {
            cloned = cloned.with_monitored_node(node_id.clone());
        }

        Box::new(cloned)
    }

    fn supports_hot_reload(&self) -> bool {
        true
    }

    fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
        let mut updated = false;

        if let Some(threshold) = parameters
            .get("concentration_threshold")
            .and_then(|v| v.as_f64())
        {
            self.concentration_threshold = Some(threshold);
            updated = true;
        }

        if let Some(threshold) = parameters
            .get("amplitude_threshold")
            .and_then(|v| v.as_f64())
        {
            self.amplitude_threshold = Some(threshold as f32);
            updated = true;
        }

        if let Some(interval) = parameters
            .get("update_interval_ms")
            .and_then(|v| v.as_u64())
        {
            self.action_update_interval_ms = interval;
            updated = true;
        }

        if let Some(nodes) = parameters.get("monitored_nodes").and_then(|v| v.as_array()) {
            let mut new_nodes = Vec::new();
            for node in nodes {
                if let Some(node_id) = node.as_str() {
                    new_nodes.push(node_id.to_string());
                }
            }
            self.monitored_nodes = new_nodes;
            updated = true;
        }

        Ok(updated)
    }

    fn set_shared_computing_state(&mut self, shared_state: Option<SharedComputingState>) {
        self.shared_computing_state = shared_state;
    }

    fn get_shared_computing_state(&self) -> Option<SharedComputingState> {
        self.shared_computing_state.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ActionNode for UniversalActionNode {
    fn buffer_size(&self) -> usize {
        self.history_buffer.capacity()
    }

    fn set_buffer_size(&mut self, new_size: usize) -> Result<()> {
        if new_size == 0 {
            return Err(anyhow!("Buffer size must be greater than 0"));
        }

        self.history_buffer.resize(new_size);
        debug!(
            "ActionNode '{}': Buffer size updated to {}",
            self.id, new_size
        );

        Ok(())
    }

    /// Update ActionNode from shared computing data (CRITICAL METHOD)
    ///
    /// # ACTIONNODE MONITORING PATTERN EXPLAINED
    ///
    /// This method demonstrates the key principle of ActionNode design:
    /// **ActionNodes should monitor CONCENTRATION nodes, not peak finder nodes directly**.
    ///
    /// ## Why Monitor Concentration Nodes?
    ///
    /// 1. **Clean abstraction**: Concentration nodes provide processed, calibrated results
    /// 2. **Automatic peak access**: ConcentrationResults contain `source_peak_finder_id`
    ///    field that allows indirect access to related peak data
    /// 3. **Follows client pattern**: The web client uses the same approach (see TypeScript
    ///    `getPeakResult` function in computing-node-modal.tsx)
    /// 4. **Better encapsulation**: Changes to peak finder IDs don't break ActionNodes
    ///
    /// ## Data Access Pattern (COPY THIS IN YOUR ACTIONNODE)
    ///
    /// ```rust,ignore
    /// // 1. Monitor concentration nodes in your ActionNode
    /// let concentration_node = self.with_monitored_node("concentration_co2".to_string());
    ///
    /// // 2. In update_from_computing_data, get concentration results first
    /// for node_id in &self.monitored_nodes {
    ///     let concentration_data = computing_data.get_concentration_result(node_id);
    ///     
    ///     // 3. Get related peak data using source_peak_finder_id
    ///     let peak_data = if let Some(conc_result) = &concentration_data {
    ///         computing_data.get_peak_result(&conc_result.source_peak_finder_id)
    ///     } else { None };
    ///     
    ///     // 4. Now you have both concentration and peak data for analysis
    ///     if let Some(conc) = concentration_data {
    ///         println!("CO2: {:.2} ppm", conc.concentration_ppm);
    ///         if let Some(peak) = peak_data {
    ///             println!("From peak: {:.1} Hz @ {:.3} mV",
    ///                      peak.frequency, peak.amplitude);
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// ## Example Configuration
    ///
    /// ```yaml
    /// action_nodes:
    ///   - id: "co2_display"
    ///     type: "action_generic_example"
    ///     monitored_nodes: ["concentration_co2"]  # NOT "peak_finder_co2"
    ///     concentration_threshold: 1000.0
    ///     amplitude_threshold: 0.8
    /// ```
    ///
    /// This pattern ensures your ActionNode automatically gets both concentration
    /// and related peak data without tight coupling to specific peak finder IDs.
    fn update_from_computing_data(&mut self, computing_data: &ComputingSharedData) -> Result<()> {
        self.last_update_time = Some(SystemTime::now());

        // Update history buffer with data from monitored concentration nodes
        for node_id in &self.monitored_nodes.clone() {
            // Get concentration data for this node
            let concentration_data = computing_data.get_concentration_result(node_id).cloned();

            // Get related peak data using the same pattern as the client app
            // The peak data comes from the computing_peak_finder_id stored in the concentration result
            let peak_data = if let Some(conc_result) = &concentration_data {
                computing_data
                    .get_peak_result(&conc_result.source_peak_finder_id)
                    .cloned()
            } else {
                None
            };

            if concentration_data.is_some() {
                let entry = ActionHistoryEntry {
                    timestamp: SystemTime::now(),
                    peak_data,
                    concentration_data,
                    source_node_id: node_id.to_string(),
                    metadata: HashMap::new(),
                };
                self.history_buffer.push(entry);
            }
        }

        // Check for trigger conditions manually
        let mut triggers = Vec::new();

        // Check concentration thresholds
        if let Some(threshold) = self.concentration_threshold {
            for (node_id, result) in &computing_data.concentration_results {
                if self.monitored_nodes.contains(node_id) && result.concentration_ppm > threshold {
                    triggers.push(ActionTrigger::ConcentrationThreshold {
                        value: result.concentration_ppm,
                        threshold,
                        source_node_id: node_id.clone(),
                    });
                }
            }
        }

        // Check amplitude thresholds using peak data from concentration nodes
        if let Some(threshold) = self.amplitude_threshold {
            for (node_id, conc_result) in &computing_data.concentration_results {
                if self.monitored_nodes.contains(node_id) {
                    // Get the corresponding peak data using the same pattern as the client
                    if let Some(peak_result) =
                        computing_data.get_peak_result(&conc_result.source_peak_finder_id)
                    {
                        if peak_result.amplitude > threshold {
                            triggers.push(ActionTrigger::AmplitudeThreshold {
                                value: peak_result.amplitude,
                                threshold,
                                source_node_id: node_id.clone(),
                            });
                        }
                    }
                }
            }
        }

        // Check for data timeouts (30 seconds default) for concentration nodes
        let timeout_seconds = 30;

        // Track the last time we've seen this node data in our history buffer
        let mut last_timestamp_map: HashMap<String, SystemTime> = HashMap::new();

        // First, build a map of the most recent data for each node from the history buffer
        // We can't use .rev() as CircularBuffer doesn't implement DoubleEndedIterator
        // So instead, we iterate normally and keep the most recent timestamp for each node
        for entry in self.history_buffer.iter() {
            let should_update = match last_timestamp_map.get(&entry.source_node_id) {
                Some(existing_time) => {
                    // Keep the most recent timestamp
                    if entry.timestamp > *existing_time {
                        true
                    } else {
                        false
                    }
                }
                None => true,
            };

            if should_update {
                last_timestamp_map.insert(entry.source_node_id.clone(), entry.timestamp);
            }
        }

        // Loop through all monitored nodes to check for timeouts
        'node_loop: for node_id in &self.monitored_nodes {
            // Check if we have any data in the history buffer
            if let Some(last_timestamp) = last_timestamp_map.get(node_id) {
                if let Ok(elapsed) = last_timestamp.elapsed() {
                    let elapsed_secs = elapsed.as_secs();

                    // If we have recent data in our history buffer, no timeout
                    if elapsed_secs <= timeout_seconds {
                        debug!(
                            "Node '{}' has data in history buffer ({} seconds old)",
                            node_id, elapsed_secs
                        );
                        continue 'node_loop;
                    }
                }
            }

            // Check current concentration data
            let concentration_data_available =
                if let Some(conc_result) = computing_data.get_concentration_result(node_id) {
                    if let Ok(elapsed) = conc_result.timestamp.elapsed() {
                        let elapsed_secs = elapsed.as_secs();

                        // Skip timeout check if we have recent concentration data
                        if elapsed_secs <= timeout_seconds {
                            debug!(
                                "Node '{}' has recent concentration data ({} seconds old)",
                                node_id, elapsed_secs
                            );
                            true
                        } else {
                            // Also check if there is a peak_result with more recent data
                            // source_peak_finder_id is already a String, not an Option<String>
                            let peak_finder_id = &conc_result.source_peak_finder_id;
                            if !peak_finder_id.is_empty() {
                                if let Some(peak_result) =
                                    computing_data.get_peak_result(peak_finder_id)
                                {
                                    if let Ok(peak_elapsed) = peak_result.timestamp.elapsed() {
                                        if peak_elapsed.as_secs() <= timeout_seconds {
                                            debug!(
                                                "Node '{}' has recent peak data ({} seconds old)",
                                                node_id,
                                                peak_elapsed.as_secs()
                                            );
                                            true
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

            if concentration_data_available {
                continue 'node_loop;
            }

            // No concentration data for this node, or it's too old
            // Check if there's any peak data related to this node
            let mut found_peak_data = false;

            // Try to find any peak data associated with this node ID
            for (peak_id, peak_result) in &computing_data.peak_results {
                if peak_id.contains(node_id) || node_id.contains(peak_id) {
                    if let Ok(peak_elapsed) = peak_result.timestamp.elapsed() {
                        if peak_elapsed.as_secs() <= timeout_seconds {
                            debug!(
                                "Node '{}' has recent related peak data ({} seconds old) from '{}'",
                                node_id,
                                peak_elapsed.as_secs(),
                                peak_id
                            );
                            found_peak_data = true;
                            break;
                        }
                    }
                }
            }

            if found_peak_data {
                continue 'node_loop;
            }

            // If we've exhausted all checks and still no recent data, trigger a timeout
            debug!(
                "Node '{}' data timeout: all data older than {} seconds",
                node_id, timeout_seconds
            );
            triggers.push(ActionTrigger::DataTimeout {
                elapsed_seconds: timeout_seconds + 1,
                timeout_seconds,
                source_node_id: node_id.clone(),
            });
        }
        // Process triggers
        for trigger in triggers {
            let _ = self.trigger_action(trigger);
        }

        // Update action if enough time has passed
        if self.should_update_action() {
            if let Some(latest_concentration) = computing_data.get_latest_concentration_result() {
                self.update_action_safely(
                    latest_concentration.concentration_ppm,
                    &latest_concentration.source_peak_finder_id,
                )?;
            }
        }

        Ok(())
    }

    /// Execute action based on trigger condition - MOST IMPORTANT CUSTOMIZATION POINT
    ///
    /// # THIS IS THE KEY METHOD TO CUSTOMIZE FOR YOUR ACTIONNODE
    ///
    /// This method defines what happens when threshold conditions are met.
    /// It's called automatically by the ActionNode framework when triggers fire.
    /// Most of your custom logic will go here.
    ///
    /// # Customization Examples
    ///
    /// ## Email Alert ActionNode
    /// ```rust,ignore
    /// fn trigger_action(&mut self, trigger: ActionTrigger) -> Result<bool> {
    ///     match trigger {
    ///         ActionTrigger::ConcentrationThreshold { value, threshold, source_node_id } => {
    ///             if value > threshold {
    ///                 let subject = format!("ALERT: CO2 level {:.2} ppm exceeds limit", value);
    ///                 let body = format!("Concentration {:.2} ppm from sensor {} exceeds threshold {:.2} ppm",
    ///                                   value, source_node_id, threshold);
    ///                 self.send_email(&subject, &body)?;
    ///                 Ok(true)
    ///             } else { Ok(false) }
    ///         }
    ///         // Handle other trigger types...
    ///         _ => Ok(false)
    ///     }
    /// }
    /// ```
    ///
    /// ## Relay Control ActionNode
    /// ```rust,ignore
    /// fn trigger_action(&mut self, trigger: ActionTrigger) -> Result<bool> {
    ///     match trigger {
    ///         ActionTrigger::ConcentrationThreshold { value, threshold, .. } => {
    ///             if value > threshold {
    ///                 self.activate_ventilation_relay()?;  // Turn on fan
    ///                 self.log_safety_event("ventilation_activated", value)?;
    ///                 Ok(true)
    ///             } else {
    ///                 self.deactivate_ventilation_relay()?;  // Turn off fan  
    ///                 Ok(true)
    ///             }
    ///         }
    ///         ActionTrigger::DataTimeout { source_node_id, .. } => {
    ///             self.activate_alarm_relay()?;  // Sound alarm for sensor failure
    ///             self.log_safety_event("sensor_timeout", source_node_id)?;
    ///             Ok(true)
    ///         }
    ///         _ => Ok(false)
    ///     }
    /// }
    /// ```
    ///
    /// ## LED Array ActionNode
    /// ```rust,ignore
    /// fn trigger_action(&mut self, trigger: ActionTrigger) -> Result<bool> {
    ///     match trigger {
    ///         ActionTrigger::ConcentrationThreshold { value, threshold, .. } => {
    ///             if value > threshold {
    ///                 self.set_led_color(LEDColor::Red)?;      // Danger color
    ///                 self.set_led_blink_rate(2.0)?;          // Fast blink
    ///             } else {
    ///                 self.set_led_color(LEDColor::Green)?;    // Safe color
    ///                 self.set_led_blink_rate(0.0)?;          // Solid on
    ///             }
    ///             Ok(true)
    ///         }
    ///         _ => Ok(false)
    ///     }
    /// }
    /// ```
    ///
    /// # Important Design Principles
    ///
    /// 1. **Fast Execution**: Keep actions quick to avoid blocking the pipeline
    /// 2. **Error Handling**: Return errors for hardware failures, but log and continue
    /// 3. **Return true/false**: Indicate whether action was actually triggered
    /// 4. **State Management**: Update internal state as needed
    /// 5. **Logging**: Always log actions for debugging and monitoring
    ///
    /// # Arguments
    /// * `trigger` - The trigger condition that fired (contains all context data)
    ///
    /// # Returns
    /// * `Ok(true)` - Action was triggered and executed successfully
    /// * `Ok(false)` - Trigger condition not met, no action taken
    /// * `Err(anyhow::Error)` - Action failed (hardware error, network failure, etc.)
    fn trigger_action(&mut self, trigger: ActionTrigger) -> Result<bool> {
        match trigger {
            ActionTrigger::ConcentrationThreshold {
                value,
                threshold,
                source_node_id,
            } => {
                if value > threshold {
                    self.flash_action_safely(&format!(
                        "Concentration threshold exceeded: {:.2} ppm > {:.2} ppm (from {})",
                        value, threshold, source_node_id
                    ))?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            ActionTrigger::AmplitudeThreshold {
                value,
                threshold,
                source_node_id,
            } => {
                if value > threshold {
                    self.flash_action_safely(&format!(
                        "Amplitude threshold exceeded: {:.3} > {:.3} (from {})",
                        value, threshold, source_node_id
                    ))?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            ActionTrigger::DataTimeout {
                elapsed_seconds,
                timeout_seconds,
                source_node_id,
            } => {
                if elapsed_seconds > timeout_seconds {
                    self.flash_action_safely(&format!(
                        "Data timeout from node '{}': {} seconds",
                        source_node_id, elapsed_seconds
                    ))?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            ActionTrigger::FrequencyDeviation {
                value,
                expected,
                tolerance,
                source_node_id,
            } => {
                let deviation = (value - expected).abs();
                if deviation > tolerance {
                    self.flash_action_safely(&format!(
                        "Frequency deviation from node '{}': {:.1} Hz (expected {:.1} ± {:.1})",
                        source_node_id, value, expected, tolerance
                    ))?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            ActionTrigger::Custom {
                trigger_id,
                data: _,
            } => {
                debug!(
                    "Custom trigger '{}' not handled by DisplayActionNode",
                    trigger_id
                );
                Ok(false)
            }
        }
    }

    fn get_history_buffer(&self) -> &CircularBuffer<ActionHistoryEntry> {
        &self.history_buffer
    }

    fn get_monitored_node_ids(&self) -> Vec<String> {
        self.monitored_nodes.clone()
    }

    fn add_monitored_node(&mut self, node_id: String) -> Result<()> {
        if self.monitored_nodes.contains(&node_id) {
            return Err(anyhow!("Node '{}' is already being monitored", node_id));
        }

        self.monitored_nodes.push(node_id.clone());
        debug!(
            "ActionNode '{}': Added monitoring for node '{}'",
            self.id, node_id
        );

        Ok(())
    }

    fn remove_monitored_node(&mut self, node_id: &str) -> Result<bool> {
        if let Some(pos) = self.monitored_nodes.iter().position(|x| x == node_id) {
            self.monitored_nodes.remove(pos);
            debug!(
                "ActionNode '{}': Removed monitoring for node '{}'",
                self.id, node_id
            );
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn get_status(&self) -> Result<serde_json::Value> {
        Ok(json!({
            "node_id": self.id,
            "node_type": self.node_type(),
            "buffer_utilization": {
                "current_size": self.history_buffer.len(),
                "capacity": self.history_buffer.capacity(),
                "utilization_percent": (self.history_buffer.len() as f64 / self.history_buffer.capacity() as f64) * 100.0
            },
            "monitoring": {
                "monitored_nodes": self.monitored_nodes,
                "node_count": self.monitored_nodes.len()
            },
            "thresholds": {
                "concentration_threshold": self.concentration_threshold,
                "amplitude_threshold": self.amplitude_threshold
            },
            "performance": {
                "processing_count": self.processing_count,
                "actions_triggered": self.actions_triggered,
                "last_update": self.last_update_time.map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()),
                "last_action_update": self.last_action_update.map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
            },
            "configuration": {
                "action_update_interval_ms": self.action_update_interval_ms
            }
        }))
    }

    fn reset_action_state(&mut self) {
        self.history_buffer.clear();
        self.processing_count = 0;
        self.actions_triggered = 0;
        self.last_update_time = None;
        self.last_action_update = None;

        info!("ActionNode '{}': State reset completed", self.id);
    }
}

// ============================================================================
// COMPREHENSIVE TEST SUITE FOR ACTIONNODE DEVELOPMENT
// ============================================================================
//
// These tests demonstrate best practices for testing ActionNode implementations.
// When creating your own ActionNode, copy and adapt these test patterns.
//
// # Key Testing Areas for ActionNodes
//
// 1. **Construction & Configuration**: Test builder pattern and validation
// 2. **Monitoring Management**: Test adding/removing monitored nodes
// 3. **Trigger Logic**: Test all trigger types and threshold conditions
// 4. **Buffer Management**: Test circular buffer operations and resizing
// 5. **Pass-through Behavior**: Test that data flows unchanged
// 6. **Performance Tracking**: Test statistics collection
// 7. **Error Handling**: Test graceful handling of invalid inputs
//
// # Test Organization Strategy
//
// - Group tests by functionality (construction, monitoring, triggers, etc.)
// - Use descriptive test names that explain the scenario
// - Test both success and failure cases
// - Include edge cases (empty buffers, invalid thresholds, etc.)
// - Mock hardware interfaces for consistent testing
//
// # Custom ActionNode Testing Template
//
// When testing your ActionNode, create similar test groups:
//
// ```rust,ignore
// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     // Construction tests
//     #[tokio::test]
//     async fn test_my_action_node_creation() { /* ... */ }
//
//     // Configuration tests
//     #[tokio::test]
//     async fn test_my_action_node_configuration() { /* ... */ }
//
//     // Trigger tests for your specific actions
//     #[tokio::test]
//     async fn test_email_sending_trigger() { /* ... */ }
//
//     #[tokio::test]
//     async fn test_relay_activation_trigger() { /* ... */ }
//
//     // Error handling tests
//     #[tokio::test]
//     async fn test_hardware_failure_handling() { /* ... */ }
// }
// ```

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processing::computing_nodes::{ConcentrationResult, PeakResult};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Test ActionNode construction and builder pattern configuration
    ///
    /// # TESTING PATTERN: Construction and Configuration
    ///
    /// This test demonstrates the standard pattern for testing ActionNode creation:
    /// 1. Test basic construction with default values
    /// 2. Test builder pattern methods for configuration
    /// 3. Verify all fields are set correctly
    /// 4. Test that ProcessingNode trait methods work
    ///
    /// # Custom ActionNode Testing Template
    ///
    /// For your own ActionNode, adapt this test:
    /// ```rust,ignore
    /// #[tokio::test]
    /// async fn test_my_action_node_creation() {
    ///     let node = MyActionNode::new("test_id".to_string())
    ///         .with_email_threshold(1000.0)
    ///         .with_smtp_server("smtp.example.com".to_string())
    ///         .with_monitored_node("sensor_1".to_string());
    ///
    ///     // Test identity
    ///     assert_eq!(node.node_id(), "test_id");
    ///     assert_eq!(node.node_type(), "email_alert_action");
    ///     
    ///     // Test configuration was applied
    ///     assert_eq!(node.email_threshold, Some(1000.0));
    ///     assert_eq!(node.smtp_server, "smtp.example.com");
    ///     
    ///     // Test ActionNode interface
    ///     assert_eq!(node.get_monitored_node_ids(), vec!["sensor_1"]);
    ///     assert!(node.buffer_size() > 0);
    /// }
    /// ```
    #[tokio::test]
    async fn test_example_action_node_creation() {
        let action_node = UniversalActionNode::new("test_display".to_string())
            .with_history_buffer_capacity(150) // REQUIRED: explicit buffer capacity
            .with_concentration_threshold(500.0)
            .with_amplitude_threshold(0.6)
            .with_monitored_node("concentration_co2".to_string())
            .with_update_interval(2000);

        assert_eq!(action_node.node_id(), "test_display");
        assert_eq!(action_node.node_type(), "action_generic_example");
        assert_eq!(action_node.buffer_size(), 150); // Test the configured capacity
        assert_eq!(
            action_node.get_monitored_node_ids(),
            vec!["concentration_co2"]
        );
        assert_eq!(action_node.concentration_threshold, Some(500.0));
        assert_eq!(action_node.amplitude_threshold, Some(0.6));
    }

    #[tokio::test]
    async fn test_action_node_monitoring() -> Result<()> {
        let mut action_node =
            UniversalActionNode::new("test_display".to_string()).with_history_buffer_capacity(50); // REQUIRED: explicit buffer capacity

        // Test adding monitored nodes
        action_node.add_monitored_node("node1".to_string())?;
        action_node.add_monitored_node("node2".to_string())?;

        assert_eq!(action_node.get_monitored_node_ids().len(), 2);
        assert!(action_node.is_monitoring_node("node1"));
        assert!(action_node.is_monitoring_node("node2"));

        // Test removing monitored node
        assert!(action_node.remove_monitored_node("node1")?);
        assert!(!action_node.remove_monitored_node("nonexistent")?);

        assert_eq!(action_node.get_monitored_node_ids().len(), 1);
        assert!(!action_node.is_monitoring_node("node1"));
        assert!(action_node.is_monitoring_node("node2"));

        Ok(())
    }

    /// Test ActionNode trigger logic and action execution
    ///
    /// # TESTING PATTERN: Trigger Logic Validation
    ///
    /// This test demonstrates how to thoroughly test trigger conditions:
    /// 1. Test triggers that should fire (above threshold)
    /// 2. Test triggers that shouldn't fire (below threshold)
    /// 3. Verify action counters are updated correctly
    /// 4. Test multiple trigger types
    ///
    /// # Custom ActionNode Trigger Testing
    ///
    /// For your own ActionNode, test all your trigger scenarios:
    /// ```rust,ignore
    /// #[tokio::test]
    /// async fn test_my_action_node_triggers() -> Result<()> {
    ///     let mut node = MyActionNode::new("test".to_string())
    ///         .with_email_threshold(1000.0)
    ///         .with_relay_threshold(500.0);
    ///
    ///     // Test email trigger (should fire)
    ///     let trigger = ActionTrigger::ConcentrationThreshold {
    ///         value: 1500.0,
    ///         threshold: 1000.0,
    ///         source_node_id: "sensor_1".to_string(),
    ///     };
    ///     assert!(node.trigger_action(trigger)?);
    ///     assert_eq!(node.emails_sent, 1);
    ///
    ///     // Test relay trigger (should fire)
    ///     let trigger = ActionTrigger::ConcentrationThreshold {
    ///         value: 600.0,
    ///         threshold: 500.0,
    ///         source_node_id: "sensor_1".to_string(),
    ///     };
    ///     assert!(node.trigger_action(trigger)?);
    ///     assert!(node.relay_activated);
    ///
    ///     // Test below threshold (should not fire)
    ///     let trigger = ActionTrigger::ConcentrationThreshold {
    ///         value: 200.0,
    ///         threshold: 500.0,
    ///         source_node_id: "sensor_1".to_string(),
    ///     };
    ///     assert!(!node.trigger_action(trigger)?);
    ///     
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Important Test Cases to Include
    ///
    /// - **Edge cases**: Test exactly at threshold values
    /// - **Multiple triggers**: Test rapid successive triggers
    /// - **Different trigger types**: Test all ActionTrigger variants you support
    /// - **Rate limiting**: Test that repeated triggers don't overwhelm actions
    /// - **Error conditions**: Test trigger_action error handling
    #[tokio::test]
    async fn test_action_node_triggers() -> Result<()> {
        let mut action_node = UniversalActionNode::new("test_display".to_string())
            .with_history_buffer_capacity(100) // REQUIRED: explicit buffer capacity
            .with_concentration_threshold(1000.0)
            .with_amplitude_threshold(0.8);

        // Test concentration threshold trigger
        let trigger = ActionTrigger::ConcentrationThreshold {
            value: 1500.0,
            threshold: 1000.0,
            source_node_id: "test_node".to_string(),
        };

        assert!(action_node.trigger_action(trigger)?);
        assert_eq!(action_node.actions_triggered, 1);

        // Test amplitude threshold trigger (below threshold)
        let trigger = ActionTrigger::AmplitudeThreshold {
            value: 0.5,
            threshold: 0.8,
            source_node_id: "test_node".to_string(),
        };

        assert!(!action_node.trigger_action(trigger)?);
        assert_eq!(action_node.actions_triggered, 1); // No change

        Ok(())
    }

    #[tokio::test]
    async fn test_action_node_buffer_management() -> Result<()> {
        let mut action_node =
            UniversalActionNode::new("test_display".to_string()).with_history_buffer_capacity(25); // REQUIRED: explicit buffer capacity

        // Verify initial buffer size is as configured
        assert_eq!(action_node.buffer_size(), 25);

        // Test buffer resize
        action_node.set_buffer_size(50)?;
        assert_eq!(action_node.buffer_size(), 50);

        // Test invalid buffer size
        assert!(action_node.set_buffer_size(0).is_err());

        Ok(())
    }
}
