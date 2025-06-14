//! Processing configuration module
//!
//! This module defines the configuration structure for the processing system.
//! It allows configuration of processing graphs, nodes, and consumer behavior.

use rocket_okapi::JsonSchema;
use serde::{Deserialize, Serialize};
/// Configuration for the processing system
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProcessingConfig {
    /// Enable or disable the processing consumer
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Buffer size for processing results broadcasting
    #[serde(default = "default_result_buffer_size")]
    pub result_buffer_size: usize,

    /// Default processing graph configuration
    #[serde(default)]
    pub default_graph: ProcessingGraphConfig,

    /// Processing performance settings
    #[serde(default)]
    pub performance: ProcessingPerformanceConfig,
}

/// Configuration for a processing graph
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProcessingGraphConfig {
    /// Graph identifier
    #[serde(default = "default_graph_id")]
    pub id: String,

    /// List of nodes in the graph
    #[serde(default)]
    pub nodes: Vec<NodeConfig>,

    /// List of connections between nodes
    #[serde(default)]
    pub connections: Vec<ConnectionConfig>,

    /// Output node identifier
    #[serde(default)]
    pub output_node: Option<String>,
}

/// Configuration for a processing node
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeConfig {
    /// Node identifier
    pub id: String,

    /// Node type (filter, differential, channel_selector, etc.)
    pub node_type: String,

    /// Node-specific parameters
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// Configuration for a connection between nodes
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConnectionConfig {
    /// Source node identifier
    pub from: String,

    /// Target node identifier
    pub to: String,
}

/// Performance configuration for processing
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProcessingPerformanceConfig {
    /// Maximum processing time per frame (microseconds)
    #[serde(default = "default_max_processing_time_us")]
    pub max_processing_time_us: u64,

    /// Enable detailed processing statistics
    #[serde(default = "default_enable_stats")]
    pub enable_stats: bool,

    /// Statistics update interval (milliseconds)
    #[serde(default = "default_stats_interval_ms")]
    pub stats_interval_ms: u64,
}

// Default value functions
fn default_enabled() -> bool {
    true
}

fn default_result_buffer_size() -> usize {
    1000
}

fn default_graph_id() -> String {
    "default".to_string()
}

fn default_max_processing_time_us() -> u64 {
    10_000 // 10ms
}

fn default_enable_stats() -> bool {
    true
}

fn default_stats_interval_ms() -> u64 {
    1000 // 1 second
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            result_buffer_size: default_result_buffer_size(),
            default_graph: ProcessingGraphConfig::default(),
            performance: ProcessingPerformanceConfig::default(),
        }
    }
}

impl Default for ProcessingGraphConfig {
    fn default() -> Self {
        Self {
            id: default_graph_id(),
            nodes: vec![
                // Default graph: input -> channel_selector (A) -> output
                NodeConfig {
                    id: "input".to_string(),
                    node_type: "input".to_string(),
                    parameters: serde_json::Value::Null,
                },
                NodeConfig {
                    id: "channel_selector".to_string(),
                    node_type: "channel_selector".to_string(),
                    parameters: serde_json::json!({
                        "target_channel": "ChannelA"
                    }),
                },
            ],
            connections: vec![ConnectionConfig {
                from: "input".to_string(),
                to: "channel_selector".to_string(),
            }],
            output_node: Some("channel_selector".to_string()),
        }
    }
}

impl Default for ProcessingPerformanceConfig {
    fn default() -> Self {
        Self {
            max_processing_time_us: default_max_processing_time_us(),
            enable_stats: default_enable_stats(),
            stats_interval_ms: default_stats_interval_ms(),
        }
    }
}

impl ProcessingConfig {
    /// Validate the processing configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.result_buffer_size == 0 {
            return Err("result_buffer_size must be greater than 0".to_string());
        }

        if self.performance.max_processing_time_us == 0 {
            return Err("max_processing_time_us must be greater than 0".to_string());
        }

        if self.performance.stats_interval_ms == 0 {
            return Err("stats_interval_ms must be greater than 0".to_string());
        }

        // Validate default graph
        self.default_graph.validate()?;

        Ok(())
    }
}

impl ProcessingGraphConfig {
    /// Validate the processing graph configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.nodes.is_empty() {
            return Err("Graph must have at least one node".to_string());
        }

        // Check that all nodes have unique IDs
        let mut node_ids = std::collections::HashSet::new();
        for node in &self.nodes {
            if !node_ids.insert(&node.id) {
                return Err(format!("Duplicate node ID: {}", node.id));
            }
        }

        // Check that all connections reference valid nodes
        for connection in &self.connections {
            if !node_ids.contains(&connection.from) {
                return Err(format!(
                    "Connection references unknown source node: {}",
                    connection.from
                ));
            }
            if !node_ids.contains(&connection.to) {
                return Err(format!(
                    "Connection references unknown target node: {}",
                    connection.to
                ));
            }
        }

        // Check that output node exists if specified
        if let Some(ref output_id) = self.output_node {
            if !node_ids.contains(output_id) {
                return Err(format!(
                    "Output node references unknown node: {}",
                    output_id
                ));
            }
        }

        Ok(())
    }

    /// Check if the graph contains an input node
    pub fn has_input_node(&self) -> bool {
        self.nodes.iter().any(|node| node.node_type == "input")
    }

    /// Get the input node if it exists
    pub fn get_input_node(&self) -> Option<&NodeConfig> {
        self.nodes.iter().find(|node| node.node_type == "input")
    }
}
