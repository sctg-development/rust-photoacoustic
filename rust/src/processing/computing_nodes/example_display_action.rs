// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Example ActionNode implementation for demonstration purposes
//!
//! This module provides a comprehensive reference implementation of the ActionNode trait
//! that demonstrates all the key concepts and patterns needed to create specialized
//! ActionNode instances. Use this as a template when developing your own ActionNode.
//!
//! # Architecture Overview
//!
//! ActionNode extends ProcessingNode with reactive capabilities:
//! - **Pass-through Processing**: Signal data flows unchanged through the node
//! - **Reactive Monitoring**: Monitors ComputingSharedData for analytical results
//! - **Trigger System**: Configurable conditions that trigger physical actions
//! - **Historical Buffer**: Circular buffer maintains history for analysis
//!
//! # Key Implementation Patterns
//!
//! 1. **Builder Pattern**: Fluent API for configuration (see `with_*` methods)
//! 2. **Borrow-Safe Design**: Cloning shared state to avoid borrowing conflicts
//! 3. **Graceful Error Handling**: Actions log errors but don't fail the pipeline
//! 4. **Hot-Reload Support**: Dynamic configuration updates without restart
//!
//! # Creating Your Own ActionNode
//!
//! To create a custom ActionNode (e.g., RelayActionNode, EmailActionNode):
//!
//! 1. **Copy the structure** of ExampleDisplayActionNode
//! 2. **Replace display methods** with your hardware/service interface
//! 3. **Customize trigger logic** in `trigger_action()` for your use case
//! 4. **Add specific configuration** fields and validation
//! 5. **Implement your action methods** (send_email, activate_relay, etc.)
//!
//! # Thread Safety Notes
//!
//! ActionNode implementations must be thread-safe as they're accessed concurrently
//! by the processing pipeline and external monitoring systems. This example uses:
//! - Arc<RwLock<>> for shared state access
//! - Clone patterns to avoid borrow conflicts
//! - try_read() for non-blocking access

use crate::processing::computing_nodes::{
    ActionHistoryEntry, ActionNode, ActionNodeHelper, ActionTrigger, CircularBuffer,
    ComputingSharedData, SharedComputingState,
};
use crate::processing::nodes::{ProcessingData, ProcessingNode};
use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use serde_json::json;
use std::collections::HashMap;
use std::time::SystemTime;

/// Example DisplayActionNode implementation - REFERENCE TEMPLATE
///
/// This is a comprehensive reference implementation of the ActionNode trait that serves
/// as a template for creating your own specialized ActionNode instances. Every pattern
/// and technique demonstrated here is designed to be reusable.
///
/// # Core Responsibilities
/// - **Signal Pass-through**: Maintains pipeline integrity by passing data unchanged
/// - **Computing Data Monitoring**: Watches for updates from PeakFinder/Concentration nodes
/// - **Threshold Monitoring**: Configurable triggers for automated responses
/// - **Historical Tracking**: Maintains circular buffer of computing data history
/// - **Hardware Simulation**: Shows how to interface with physical devices
///
/// # Implementation Pattern for Custom ActionNodes
///
/// ```rust,ignore
/// // 1. Define your struct with ActionNode-required fields
/// pub struct MyCustomActionNode {
///     // Required ActionNode fields (copy from this example)
///     id: String,
///     history_buffer: CircularBuffer<ActionHistoryEntry>,
///     monitored_nodes: Vec<String>,
///     shared_computing_state: Option<SharedComputingState>,
///     
///     // Your custom configuration fields
///     my_custom_threshold: f64,
///     my_hardware_config: MyHardwareConfig,
///     // ... other custom fields
/// }
///
/// // 2. Implement the same pattern as ExampleDisplayActionNode
/// impl ProcessingNode for MyCustomActionNode { /* copy pattern */ }
/// impl ActionNode for MyCustomActionNode { /* copy pattern, customize trigger_action */ }
/// ```
///
/// # Key Design Decisions Explained
///
/// - **Optional thresholds**: Allows disabling specific trigger types
/// - **Cloned monitoring list**: Prevents borrow conflicts during iteration
/// - **try_read() pattern**: Non-blocking access to shared state
/// - **Separate helper methods**: Clean separation of concerns
/// - **Builder pattern**: Fluent configuration API
///
/// # Real-World Usage Examples
///
/// This template can be adapted for:
/// - **RelayActionNode**: GPIO control for industrial automation
/// - **EmailActionNode**: SMTP notifications for alerts
/// - **DatabaseActionNode**: Data logging to SQL/NoSQL databases
/// - **WebhookActionNode**: HTTP callbacks to external systems
/// - **DisplayActionNode**: LCD/LED/OLED display management
///
/// In a real implementation, replace the `update_display()` and `flash_display()`
/// methods with actual hardware interface code (GPIO, SPI, I2C, HTTP, SMTP, etc.).
#[derive(Debug)]
pub struct ExampleDisplayActionNode {
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
    display_update_interval_ms: u64, // How often to update display (throttling)

    /// Performance statistics - MONITORING PATTERN
    /// These fields demonstrate how to track ActionNode performance
    /// Useful for debugging and system monitoring
    processing_count: u64, // Total number of process() calls
    actions_triggered: u64,               // Total number of actions executed
    last_update_time: Option<SystemTime>, // When computing data was last processed
    last_display_update: Option<SystemTime>, // When display was last updated (hardware-specific)
}

impl ExampleDisplayActionNode {
    /// Create a new ExampleDisplayActionNode with required configuration
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
    /// let node = ExampleDisplayActionNode::new("display".to_string())
    ///     .with_history_buffer_capacity(200)  // REQUIRED: Set buffer size
    ///     .with_concentration_threshold(1000.0)
    ///     .with_monitored_node("concentration_calc".to_string());
    /// ```
    pub fn new(id: String) -> Self {
        Self {
            id,
            history_buffer: CircularBuffer::new(1), // Minimal buffer - MUST configure with with_history_buffer_capacity()
            monitored_nodes: Vec::new(),            // Empty: add nodes via with_monitored_node()
            shared_computing_state: None,           // Set later by ProcessingGraph
            concentration_threshold: Some(1000.0),  // Default: 1000 ppm CO2 alarm
            amplitude_threshold: Some(0.8),         // Default: 80% amplitude alarm
            display_update_interval_ms: 1000,       // Default: update every second
            processing_count: 0,                    // Performance counter
            actions_triggered: 0,                   // Action counter
            last_update_time: None,                 // No updates yet
            last_display_update: None,              // No display updates yet
        }
    }

    /// Create a new ExampleDisplayActionNode with shared computing state
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
    /// let node = ExampleDisplayActionNode::new_with_shared_state(
    ///     "display".to_string(),
    ///     shared_state
    /// ).with_history_buffer_capacity(100);
    /// ```
    pub fn new_with_shared_state(id: String, shared_state: Option<SharedComputingState>) -> Self {
        Self {
            id,
            history_buffer: CircularBuffer::new(1), // Minimal buffer - MUST configure with with_history_buffer_capacity()
            monitored_nodes: Vec::new(),            // Empty: add nodes via with_monitored_node()
            shared_computing_state: shared_state,   // Use provided shared state
            concentration_threshold: Some(1000.0),  // Default: 1000 ppm CO2 alarm
            amplitude_threshold: Some(0.8),         // Default: 80% amplitude alarm
            display_update_interval_ms: 1000,       // Default: update every second
            processing_count: 0,                    // Performance counter
            actions_triggered: 0,                   // Action counter
            last_update_time: None,                 // No updates yet
            last_display_update: None,              // No display updates yet
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
    /// // For a real-time display updating every second
    /// let display_node = ExampleDisplayActionNode::new("display".to_string())
    ///     .with_history_buffer_capacity(60);  // 1 minute of history
    ///
    /// // For trend analysis over several hours
    /// let logger_node = ExampleDisplayActionNode::new("logger".to_string())
    ///     .with_history_buffer_capacity(3600); // 1 hour at 1Hz
    ///
    /// // For memory-constrained embedded systems
    /// let led_node = ExampleDisplayActionNode::new("led".to_string())
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
    /// let node = ExampleDisplayActionNode::new("display".to_string())
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
    /// let node = ExampleDisplayActionNode::new("display".to_string())
    ///     .with_monitored_node("concentration_co2".to_string())
    ///     .with_monitored_node("co2_concentration".to_string());
    /// ```
    pub fn with_monitored_node(mut self, node_id: String) -> Self {
        if !self.monitored_nodes.contains(&node_id) {
            self.monitored_nodes.push(node_id);
        }
        self
    }

    /// Set display update interval for throttling
    ///
    /// # PATTERN: Builder method for hardware-specific timing configuration
    /// Replace this with your own hardware/service timing parameters.
    /// Examples: email send intervals, relay activation delays, etc.
    ///
    /// # Arguments
    /// * `interval_ms` - Minimum milliseconds between display updates
    pub fn with_update_interval(mut self, interval_ms: u64) -> Self {
        self.display_update_interval_ms = interval_ms;
        self
    }

    // ========================================================================
    // HARDWARE INTERFACE METHODS - CUSTOMIZE FOR YOUR ACTIONNODE
    // ========================================================================
    // These methods simulate hardware interaction. Replace them with actual
    // hardware drivers, API calls, or service interfaces for your ActionNode.

    /// Simulate updating a physical display with concentration data
    ///
    /// # PATTERN: Hardware interaction method
    /// This method demonstrates how to interface with physical hardware or services.
    /// In your ActionNode, replace this with:
    /// - GPIO operations for LED/relay control
    /// - SPI/I2C for display communication  
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
    /// * `concentration` - Current concentration value to display
    /// * `source_node` - Node ID that provided this data (for debugging)
    ///
    /// # Returns
    /// * `Ok(())` - Display updated successfully
    /// * `Err(anyhow::Error)` - Hardware communication failed
    fn update_display(&mut self, concentration: f64, source_node: &str) -> Result<()> {
        // In a real implementation, this would interface with actual display hardware:
        // - LCD via I2C: lcd.write_string(&format!("CO2: {:.2} ppm", concentration))?;
        // - LED matrix: led_matrix.display_number(concentration as u32)?;
        // - Web interface: http_client.post("/api/display", json!({...}))?;
        // - OLED display: oled.clear(); oled.write_text(&format!("{:.2}", concentration))?;

        info!(
            "Display Update [{}]: {:.2} ppm from node '{}'",
            self.id, concentration, source_node
        );

        // IMPORTANT: Always update timing fields for throttling and monitoring
        self.last_display_update = Some(SystemTime::now());

        Ok(())
    }

    /// Simulate flashing the display for alarm conditions
    ///
    /// # PATTERN: Alarm/alert action method  
    /// This method demonstrates how to implement alarm responses.
    /// In your ActionNode, replace this with appropriate alarm mechanisms:
    /// - GPIO pin toggling for buzzers/lights
    /// - Email/SMS sending for notifications  
    /// - Relay activation for safety systems
    /// - Webhook calls for external system integration
    ///
    /// # Key Design Points
    /// - Log the alarm with full context for debugging
    /// - Increment action counter for monitoring
    /// - Include reason string for human-readable logs
    /// - Keep alarm actions fast and reliable
    ///
    /// # Arguments
    /// * `reason` - Human-readable description of why alarm was triggered
    ///
    /// # Returns  
    /// * `Ok(())` - Alarm action completed successfully
    /// * `Err(anyhow::Error)` - Alarm action failed
    fn flash_display(&mut self, reason: &str) -> Result<()> {
        // In a real implementation, this would trigger actual alarm mechanisms:
        // - GPIO: gpio_pin.set_high(); thread::sleep(100ms); gpio_pin.set_low();
        // - Email: smtp_client.send_email("ALARM", reason)?;
        // - Webhook: http_client.post("/api/alarm", json!({"reason": reason}))?;
        // - Display: lcd.blink(); or oled.invert_display();

        warn!("Display Alarm [{}]: Flashing display - {}", self.id, reason);

        // IMPORTANT: Track action execution for monitoring and rate limiting
        self.actions_triggered += 1;

        Ok(())
    }

    // ========================================================================
    // UTILITY METHODS - REUSABLE PATTERNS
    // ========================================================================

    /// Check if enough time has passed since last display update (throttling)
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
    fn should_update_display(&self) -> bool {
        if let Some(last_update) = self.last_display_update {
            if let Ok(elapsed) = last_update.elapsed() {
                elapsed.as_millis() >= self.display_update_interval_ms as u128
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
}

// ============================================================================
// PROCESSINGNODE IMPLEMENTATION - REQUIRED BOILERPLATE
// ============================================================================
// This implementation is mostly boilerplate that you can copy to your own
// ActionNode. The key method is process() which implements pass-through behavior.

impl ProcessingNode for ExampleDisplayActionNode {
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
    /// - "action_display" for display management
    /// - "action_relay" for GPIO/relay control  
    /// - "action_email" for email notifications
    /// - "action_webhook" for HTTP callbacks
    /// - "action_database" for data logging
    fn node_type(&self) -> &str {
        "action_display_example"
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
        // Pass-through: output type matches input type
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
        let mut cloned = ExampleDisplayActionNode::new(self.id.clone())
            .with_history_buffer_capacity(self.history_buffer.capacity()) // IMPORTANT: Preserve buffer capacity
            .with_update_interval(self.display_update_interval_ms);

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
            self.display_update_interval_ms = interval;
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
}

impl ActionNode for ExampleDisplayActionNode {
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
    ///     type: "action_display_example"
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
        for node_id in &self.monitored_nodes {
            let has_recent_data = computing_data.has_recent_concentration_data(node_id);

            if !has_recent_data {
                // Calculate elapsed time since last concentration update
                let elapsed =
                    if let Some(conc_result) = computing_data.get_concentration_result(node_id) {
                        conc_result.timestamp.elapsed().map_or(0, |d| d.as_secs())
                    } else {
                        timeout_seconds + 1 // Force timeout if no data ever
                    };

                if elapsed > timeout_seconds {
                    triggers.push(ActionTrigger::DataTimeout {
                        elapsed_seconds: elapsed,
                        timeout_seconds,
                        source_node_id: node_id.clone(),
                    });
                }
            }
        }

        // Process triggers
        for trigger in triggers {
            let _ = self.trigger_action(trigger);
        }

        // Update display if enough time has passed
        if self.should_update_display() {
            if let Some(latest_concentration) = computing_data.get_latest_concentration_result() {
                self.update_display(
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
                    self.flash_display(&format!(
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
                    self.flash_display(&format!(
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
                    self.flash_display(&format!(
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
                    self.flash_display(&format!(
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
                "last_display_update": self.last_display_update.map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
            },
            "configuration": {
                "display_update_interval_ms": self.display_update_interval_ms
            }
        }))
    }

    fn reset_action_state(&mut self) {
        self.history_buffer.clear();
        self.processing_count = 0;
        self.actions_triggered = 0;
        self.last_update_time = None;
        self.last_display_update = None;

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
    async fn test_example_display_action_node_creation() {
        let display_node = ExampleDisplayActionNode::new("test_display".to_string())
            .with_history_buffer_capacity(150) // REQUIRED: explicit buffer capacity
            .with_concentration_threshold(500.0)
            .with_amplitude_threshold(0.6)
            .with_monitored_node("concentration_co2".to_string())
            .with_update_interval(2000);

        assert_eq!(display_node.node_id(), "test_display");
        assert_eq!(display_node.node_type(), "action_display_example");
        assert_eq!(display_node.buffer_size(), 150); // Test the configured capacity
        assert_eq!(
            display_node.get_monitored_node_ids(),
            vec!["concentration_co2"]
        );
        assert_eq!(display_node.concentration_threshold, Some(500.0));
        assert_eq!(display_node.amplitude_threshold, Some(0.6));
    }

    #[tokio::test]
    async fn test_action_node_monitoring() -> Result<()> {
        let mut display_node = ExampleDisplayActionNode::new("test_display".to_string())
            .with_history_buffer_capacity(50); // REQUIRED: explicit buffer capacity

        // Test adding monitored nodes
        display_node.add_monitored_node("node1".to_string())?;
        display_node.add_monitored_node("node2".to_string())?;

        assert_eq!(display_node.get_monitored_node_ids().len(), 2);
        assert!(display_node.is_monitoring_node("node1"));
        assert!(display_node.is_monitoring_node("node2"));

        // Test removing monitored node
        assert!(display_node.remove_monitored_node("node1")?);
        assert!(!display_node.remove_monitored_node("nonexistent")?);

        assert_eq!(display_node.get_monitored_node_ids().len(), 1);
        assert!(!display_node.is_monitoring_node("node1"));
        assert!(display_node.is_monitoring_node("node2"));

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
        let mut display_node = ExampleDisplayActionNode::new("test_display".to_string())
            .with_history_buffer_capacity(100) // REQUIRED: explicit buffer capacity
            .with_concentration_threshold(1000.0)
            .with_amplitude_threshold(0.8);

        // Test concentration threshold trigger
        let trigger = ActionTrigger::ConcentrationThreshold {
            value: 1500.0,
            threshold: 1000.0,
            source_node_id: "test_node".to_string(),
        };

        assert!(display_node.trigger_action(trigger)?);
        assert_eq!(display_node.actions_triggered, 1);

        // Test amplitude threshold trigger (below threshold)
        let trigger = ActionTrigger::AmplitudeThreshold {
            value: 0.5,
            threshold: 0.8,
            source_node_id: "test_node".to_string(),
        };

        assert!(!display_node.trigger_action(trigger)?);
        assert_eq!(display_node.actions_triggered, 1); // No change

        Ok(())
    }

    #[tokio::test]
    async fn test_action_node_buffer_management() -> Result<()> {
        let mut display_node = ExampleDisplayActionNode::new("test_display".to_string())
            .with_history_buffer_capacity(25); // REQUIRED: explicit buffer capacity

        // Verify initial buffer size is as configured
        assert_eq!(display_node.buffer_size(), 25);

        // Test buffer resize
        display_node.set_buffer_size(50)?;
        assert_eq!(display_node.buffer_size(), 50);

        // Test invalid buffer size
        assert!(display_node.set_buffer_size(0).is_err());

        Ok(())
    }
}

// ============================================================================
// ACTIONNODE DEVELOPMENT FINAL CHECKLIST
// ============================================================================
//
// When you've finished implementing your custom ActionNode using this template,
// verify you have implemented all required components:
//
// ## 1. REQUIRED FIELDS ✓
// □ id: String                                    // Unique identifier
// □ history_buffer: CircularBuffer<...>           // Historical data storage
// □ monitored_nodes: Vec<String>                  // Nodes to watch
// □ shared_computing_state: Option<...>           // Access to computing data
// □ Your custom configuration fields              // Thresholds, hardware config, etc.
//
// ## 2. REQUIRED TRAIT IMPLEMENTATIONS ✓
// □ ProcessingNode trait fully implemented        // Core node interface
//   - process() for pass-through behavior
//   - node_id() and node_type()
//   - accepts_input() and output_type()
//   - reset(), clone_node(), etc.
// □ ActionNode trait fully implemented            // Action-specific interface
//   - trigger_action() with your custom logic
//   - Buffer management methods
//   - Monitoring node management
//   - Status reporting
//
// ## 3. BUILDER PATTERN METHODS ✓
// □ new() constructor with minimal default configuration
// □ with_history_buffer_capacity() method - REQUIRED for proper configuration
// □ with_*() methods for all configurable fields
// □ Fluent API that chains method calls
//
// ## 4. HARDWARE/SERVICE INTERFACE ✓
// □ Replace simulation methods with real interfaces
// □ Error handling for hardware/network failures
// □ Rate limiting for hardware protection
// □ Proper logging for debugging
//
// ## 5. COMPREHENSIVE TESTS ✓
// □ Construction and configuration tests
// □ Trigger logic tests (all scenarios)
// □ Buffer management tests
// □ Error handling tests
// □ Performance/monitoring tests
//
// ## 6. DOCUMENTATION ✓
// □ Module-level documentation explaining purpose
// □ Struct documentation with field explanations
// □ Method documentation with examples
// □ Integration examples in comments
// □ Configuration examples (YAML/JSON)
//
// ## 7. INTEGRATION CHECKLIST
// □ Export your ActionNode in mod.rs
// □ Add to ProcessingGraph node creation logic
// □ Update configuration schemas if needed
// □ Test in real processing pipeline
// □ Document in feasibility analysis or user guide
//
// ## COMMON PITFALLS TO AVOID
//
// ❌ Don't modify signal data in process() - ActionNodes are pass-through only
// ❌ Don't block in trigger_action() - keep actions fast and non-blocking
// ❌ Don't panic on hardware errors - log errors and continue gracefully
// ❌ Don't forget to update timing fields for throttling
// ❌ Don't skip error handling in hardware interface methods
// ❌ Don't hardcode configuration - use builder pattern for flexibility
// ❌ Don't forget with_history_buffer_capacity() - it's required for proper buffer sizing
//
// ## PERFORMANCE CONSIDERATIONS
//
// 💡 Use try_read() for non-blocking shared state access
// 💡 Clone shared state references to avoid borrow conflicts
// 💡 Implement rate limiting for hardware protection
// 💡 Keep trigger_action() execution time under 1ms when possible
// 💡 Use appropriate log levels (trace/debug for verbose, warn/error for issues)
// 💡 Consider async hardware operations for non-blocking behavior
//
// ## EXAMPLE ACTIONNODE TYPES TO IMPLEMENT
//
// 🎯 **Safety Critical**
//    - EmergencyShutdownActionNode: Triggers safety systems
//    - AlarmActionNode: Sound/visual alarms for threshold violations
//    - VentilationControlActionNode: Automatic air quality management
//
// 🎯 **Monitoring & Alerting**
//    - EmailAlertActionNode: Send email notifications
//    - SMSAlertActionNode: Send SMS alerts via API
//    - SlackWebhookActionNode: Post to Slack channels
//    - DatabaseLoggerActionNode: Log events to database
//
// 🎯 **Hardware Control**
//    - RelayControlActionNode: GPIO relay activation
//    - ServoControlActionNode: Servo motor positioning
//    - LEDIndicatorActionNode: LED arrays and indicators
//    - LCDDisplayActionNode: LCD/OLED display updates
//
// 🎯 **Data & Analytics**
//    - DataExportActionNode: Export data to CSV/JSON
//    - CloudUploadActionNode: Upload to AWS/Azure/GCP
//    - WebhookActionNode: HTTP callbacks to external systems
//    - MQTTPublisherActionNode: IoT data publishing
//
// Your ActionNode implementation is complete when it passes all tests,
// integrates cleanly with the processing graph, and reliably performs
// your intended actions without disrupting the signal processing pipeline.
