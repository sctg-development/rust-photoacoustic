// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! ActionNode trait for specialized processing nodes that react to computing data
//!
//! ActionNode extends ProcessingNode to provide specialized functionality for nodes that:
//! - Monitor computing results from PeakFinderNode and ConcentrationNode
//! - Maintain circular buffers of historical data
//! - Trigger physical actions (display updates, relay control, notifications)
//! - Implement pass-through behavior for signal data
//!
//! # Architecture
//!
//! ActionNode is designed to be the "output" layer of the computing architecture:
//! ```text
//! Signal Processing Pipeline:
//! Input → Filter → PeakFinder (Computing) → Concentration (Computing) → ActionNode → Output
//!         ↓           ↓                       ↓                         ↑
//!      Signal     Analytics              Calculations              Physical Actions
//!      Data      (pass-through)        (pass-through)            (displays, relays, etc.)
//! ```
//!
//! # Features
//!
//! - **Circular Buffer Management**: Configurable buffer size for historical data storage
//! - **Computing State Monitoring**: Direct access to PeakResult and ConcentrationResult data
//! - **Action Triggering**: Configurable thresholds and conditions for triggering actions
//! - **Pass-through Processing**: Signal data flows unchanged to maintain pipeline integrity
//! - **Multi-source Support**: Can monitor multiple computing nodes simultaneously
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use rust_photoacoustic::processing::computing_nodes::{ActionNode, ProcessingNode};
//!
//! struct DisplayActionNode {
//!     // ActionNode implementation that updates an LCD display
//! }
//!
//! impl ActionNode for DisplayActionNode {
//!     fn buffer_size(&self) -> usize { 100 }
//!     
//!     fn update_from_computing_data(&mut self, computing_data: &ComputingSharedData) -> anyhow::Result<()> {
//!         // Update display with latest concentration values
//!         if let Some(latest_result) = computing_data.get_latest_concentration_result() {
//!             self.update_action(latest_result.concentration_ppm)?;
//!         }
//!         Ok(())
//!     }
//!     
//!     fn trigger_action(&mut self, trigger: ActionTrigger) -> anyhow::Result<bool> {
//!         // Flash display on high concentration alarm
//!         match trigger {
//!             ActionTrigger::ConcentrationThreshold { value, threshold } if value > threshold => {
//!                 self.flash_display()?;
//!                 Ok(true)
//!             }
//!             _ => Ok(false)
//!         }
//!     }
//! }
//! ```

use crate::processing::computing_nodes::{ComputingSharedData, ConcentrationResult, PeakResult};
use crate::processing::nodes::ProcessingNode;
use anyhow::Result;
use std::collections::VecDeque;
use std::time::SystemTime;

/// Circular buffer for storing historical data with automatic size management
#[derive(Debug, Clone)]
pub struct CircularBuffer<T> {
    /// Internal storage using VecDeque for efficient push/pop operations
    buffer: VecDeque<T>,
    /// Maximum capacity of the buffer
    capacity: usize,
}

impl<T> CircularBuffer<T> {
    /// Create a new circular buffer with the specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Add an item to the buffer, removing the oldest if at capacity
    pub fn push(&mut self, item: T) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(item);
    }

    /// Get the current number of items in the buffer
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get the buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get an iterator over the buffer contents (oldest to newest)
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }

    /// Get the most recent item
    pub fn latest(&self) -> Option<&T> {
        self.buffer.back()
    }

    /// Get the oldest item
    pub fn oldest(&self) -> Option<&T> {
        self.buffer.front()
    }

    /// Clear all items from the buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Resize the buffer capacity
    /// If new capacity is smaller, oldest items are removed
    pub fn resize(&mut self, new_capacity: usize) {
        self.capacity = new_capacity;
        while self.buffer.len() > new_capacity {
            self.buffer.pop_front();
        }
    }

    /// Get a vector of all items (oldest to newest)
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.buffer.iter().cloned().collect()
    }
}

/// Action trigger types for automated responses to computing data
#[derive(Debug, Clone)]
pub enum ActionTrigger {
    /// Triggered when concentration exceeds a threshold
    ConcentrationThreshold {
        /// Current concentration value in ppm
        value: f64,
        /// Threshold value in ppm
        threshold: f64,
        /// Source node ID that provided this data
        source_node_id: String,
    },
    /// Triggered when peak amplitude exceeds a threshold
    AmplitudeThreshold {
        /// Current amplitude (normalized 0.0-1.0)
        value: f32,
        /// Threshold value (normalized 0.0-1.0)
        threshold: f32,
        /// Source node ID that provided this data
        source_node_id: String,
    },
    /// Triggered when frequency deviates from expected range
    FrequencyDeviation {
        /// Current frequency in Hz
        value: f32,
        /// Expected frequency in Hz
        expected: f32,
        /// Maximum allowed deviation in Hz
        tolerance: f32,
        /// Source node ID that provided this data
        source_node_id: String,
    },
    /// Triggered when data becomes stale (no recent updates)
    DataTimeout {
        /// How long since last update
        elapsed_seconds: u64,
        /// Timeout threshold in seconds
        timeout_seconds: u64,
        /// Source node ID that timed out
        source_node_id: String,
    },
    /// Custom trigger with arbitrary data
    Custom {
        /// Trigger identifier
        trigger_id: String,
        /// Custom data as JSON value
        data: serde_json::Value,
    },
}

/// Historical data entry for ActionNode circular buffers
#[derive(Debug, Clone)]
pub struct ActionHistoryEntry {
    /// Timestamp when this entry was recorded
    pub timestamp: SystemTime,
    /// Peak detection data if available
    pub peak_data: Option<PeakResult>,
    /// Concentration calculation data if available
    pub concentration_data: Option<ConcentrationResult>,
    /// Source node ID that provided this data
    pub source_node_id: String,
    /// Additional metadata for this entry
    pub metadata: std::collections::HashMap<String, String>,
}

/// Trait for action nodes that extend ProcessingNode with reactive capabilities
///
/// ActionNode provides a standardized interface for nodes that monitor computing data
/// and trigger physical actions based on configurable conditions. All ActionNode
/// implementations must also implement ProcessingNode for integration into the
/// processing pipeline.
///
/// # Key Features
///
/// - **Buffer Management**: Maintains circular buffers of historical computing data
/// - **Threshold Monitoring**: Configurable thresholds for automated action triggering
/// - **Multi-source Support**: Can monitor data from multiple computing nodes
/// - **Pass-through Processing**: Signal data flows unchanged through action nodes
/// - **State Persistence**: Maintains action state and history for analysis
///
/// # Thread Safety
///
/// ActionNode implementations should be thread-safe as they may be accessed
/// concurrently by the processing pipeline and external monitoring systems.
pub trait ActionNode: ProcessingNode {
    /// Get the configured buffer size for historical data storage
    ///
    /// Returns the maximum number of historical entries this node will store.
    /// When the buffer is full, the oldest entries are automatically removed.
    ///
    /// # Returns
    ///
    /// The buffer capacity as a number of entries
    fn buffer_size(&self) -> usize;

    /// Update the buffer size configuration
    ///
    /// Changes the buffer capacity and adjusts existing buffer contents if necessary.
    /// If the new size is smaller than the current buffer contents, the oldest
    /// entries will be removed.
    ///
    /// # Arguments
    ///
    /// * `new_size` - The new buffer capacity
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Buffer size updated successfully
    /// * `Err(anyhow::Error)` - Update failed (e.g., invalid size)
    fn set_buffer_size(&mut self, new_size: usize) -> Result<()>;

    /// Update internal state based on current computing data
    ///
    /// This method is called by the processing pipeline to provide the latest
    /// computing data to the action node. The node should update its internal
    /// buffers and check for trigger conditions.
    ///
    /// # Arguments
    ///
    /// * `computing_data` - Current shared computing data from all computing nodes
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Update completed successfully
    /// * `Err(anyhow::Error)` - Update failed with error details
    fn update_from_computing_data(&mut self, computing_data: &ComputingSharedData) -> Result<()>;

    /// Trigger an action based on the specified trigger condition
    ///
    /// This method is called when a trigger condition is detected. The implementation
    /// should perform the appropriate action (update display, activate relay, send
    /// notification, etc.) and return whether the action was executed.
    ///
    /// # Arguments
    ///
    /// * `trigger` - The trigger condition that was detected
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Action was triggered and executed successfully
    /// * `Ok(false)` - Trigger condition was not handled by this node
    /// * `Err(anyhow::Error)` - Action execution failed
    fn trigger_action(&mut self, trigger: ActionTrigger) -> Result<bool>;

    /// Get the current historical data buffer
    ///
    /// Returns a reference to the circular buffer containing historical computing
    /// data. This can be used for analysis, graphing, or status reporting.
    ///
    /// # Returns
    ///
    /// A reference to the historical data buffer
    fn get_history_buffer(&self) -> &CircularBuffer<ActionHistoryEntry>;

    /// Get the IDs of computing nodes this action node is monitoring
    ///
    /// Returns a list of computing node IDs that this action node is configured
    /// to monitor. This is used for dependency tracking and validation.
    ///
    /// # Returns
    ///
    /// Vector of computing node IDs being monitored
    fn get_monitored_node_ids(&self) -> Vec<String>;

    /// Check if this action node is monitoring a specific computing node
    ///
    /// # Arguments
    ///
    /// * `node_id` - The computing node ID to check
    ///
    /// # Returns
    ///
    /// * `true` - This action node monitors the specified computing node
    /// * `false` - This action node does not monitor the specified computing node
    fn is_monitoring_node(&self, node_id: &str) -> bool {
        self.get_monitored_node_ids().contains(&node_id.to_string())
    }

    /// Add a computing node to the monitoring list
    ///
    /// Configures this action node to monitor an additional computing node.
    /// This enables dynamic reconfiguration of monitoring relationships.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The computing node ID to start monitoring
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Node added to monitoring list successfully
    /// * `Err(anyhow::Error)` - Failed to add node (e.g., already monitored, invalid ID)
    fn add_monitored_node(&mut self, node_id: String) -> Result<()>;

    /// Remove a computing node from the monitoring list
    ///
    /// Stops monitoring the specified computing node. Historical data from this
    /// node remains in the buffer until it naturally cycles out.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The computing node ID to stop monitoring
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Node was monitored and successfully removed
    /// * `Ok(false)` - Node was not in the monitoring list
    /// * `Err(anyhow::Error)` - Failed to remove node
    fn remove_monitored_node(&mut self, node_id: &str) -> Result<bool>;

    /// Get current action node status and statistics
    ///
    /// Returns a JSON object containing current status information including:
    /// - Buffer utilization
    /// - Last update timestamp
    /// - Number of actions triggered
    /// - Monitoring node status
    ///
    /// # Returns
    ///
    /// * `Ok(serde_json::Value)` - Status information as JSON
    /// * `Err(anyhow::Error)` - Failed to collect status
    fn get_status(&self) -> Result<serde_json::Value>;

    /// Reset action node state and clear buffers
    ///
    /// Clears all historical data, resets counters, and returns the node to
    /// its initial state. This is useful for testing or starting fresh.
    fn reset_action_state(&mut self);
}

/// Helper trait for ActionNode implementations that provides common functionality
///
/// This trait provides default implementations for common ActionNode operations
/// to reduce boilerplate code in specific ActionNode implementations.
pub trait ActionNodeHelper: ActionNode {
    /// Check all configured thresholds against current data
    ///
    /// This helper method checks common threshold conditions and generates
    /// appropriate ActionTrigger events. Can be called from update_from_computing_data().
    ///
    /// # Arguments
    ///
    /// * `computing_data` - Current computing data to check
    /// * `concentration_threshold` - Optional concentration threshold in ppm
    /// * `amplitude_threshold` - Optional amplitude threshold (0.0-1.0)
    ///
    /// # Returns
    ///
    /// Vector of triggered conditions
    fn check_common_thresholds(
        &self,
        computing_data: &ComputingSharedData,
        concentration_threshold: Option<f64>,
        amplitude_threshold: Option<f32>,
    ) -> Vec<ActionTrigger> {
        let mut triggers = Vec::new();

        // Check concentration thresholds
        if let Some(threshold) = concentration_threshold {
            for (node_id, result) in &computing_data.concentration_results {
                if result.concentration_ppm > threshold {
                    triggers.push(ActionTrigger::ConcentrationThreshold {
                        value: result.concentration_ppm,
                        threshold,
                        source_node_id: node_id.clone(),
                    });
                }
            }
        }

        // Check amplitude thresholds
        if let Some(threshold) = amplitude_threshold {
            for (node_id, result) in &computing_data.peak_results {
                if result.amplitude > threshold {
                    triggers.push(ActionTrigger::AmplitudeThreshold {
                        value: result.amplitude,
                        threshold,
                        source_node_id: node_id.clone(),
                    });
                }
            }
        }

        // Check for data timeouts (30 seconds default)
        let timeout_seconds = 30;
        for node_id in self.get_monitored_node_ids() {
            let has_recent_data = computing_data.has_recent_peak_data(&node_id)
                || computing_data.has_recent_concentration_data(&node_id);

            if !has_recent_data {
                // Calculate elapsed time since last update
                let elapsed = if let Some(peak_result) = computing_data.get_peak_result(&node_id) {
                    peak_result.timestamp.elapsed().map_or(0, |d| d.as_secs())
                } else if let Some(conc_result) = computing_data.get_concentration_result(&node_id)
                {
                    conc_result.timestamp.elapsed().map_or(0, |d| d.as_secs())
                } else {
                    timeout_seconds + 1 // Force timeout if no data ever
                };

                if elapsed > timeout_seconds {
                    triggers.push(ActionTrigger::DataTimeout {
                        elapsed_seconds: elapsed,
                        timeout_seconds,
                        source_node_id: node_id,
                    });
                }
            }
        }

        triggers
    }

    /// Create a history entry from current computing data
    ///
    /// Helper method to create ActionHistoryEntry from computing data for a specific node.
    ///
    /// # Arguments
    ///
    /// * `computing_data` - Current computing data
    /// * `node_id` - Node ID to create entry for
    ///
    /// # Returns
    ///
    /// History entry for the specified node, or None if no data available
    fn create_history_entry(
        &self,
        computing_data: &ComputingSharedData,
        node_id: &str,
    ) -> Option<ActionHistoryEntry> {
        let peak_data = computing_data.get_peak_result(node_id).cloned();
        let concentration_data = computing_data.get_concentration_result(node_id).cloned();

        if peak_data.is_some() || concentration_data.is_some() {
            Some(ActionHistoryEntry {
                timestamp: SystemTime::now(),
                peak_data,
                concentration_data,
                source_node_id: node_id.to_string(),
                metadata: std::collections::HashMap::new(),
            })
        } else {
            None
        }
    }
}

// Automatic implementation of ActionNodeHelper for all ActionNode implementations
impl<T: ActionNode> ActionNodeHelper for T {}
