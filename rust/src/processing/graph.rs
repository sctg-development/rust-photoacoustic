// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Processing graph management
//!
//! This module manages the processing graph structure, connections between nodes,
//! and graph execution logic.

use crate::config::processing::{NodeConfig, ProcessingGraphConfig};
use crate::preprocessing::differential::SimpleDifferential;
use crate::preprocessing::filters::{BandpassFilter, HighpassFilter, LowpassFilter};
use crate::processing::computing_nodes::{
    ConcentrationNode, PeakFinderNode, SharedComputingState, UniversalDisplayActionNode,
};
use crate::processing::nodes::{
    ChannelMixerNode, ChannelSelectorNode, ChannelTarget, DifferentialNode, FilterNode, GainNode,
    InputNode, MixStrategy, NodeId, PhotoacousticOutputNode, ProcessingData, ProcessingNode,
    RecordNode, StreamingNode, StreamingNodeRegistry,
};
use anyhow::Result;
use log::debug;
use rocket_okapi::JsonSchema;
use schemars::{gen::SchemaGenerator, schema::Schema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Module for serializing/deserializing Duration
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_nanos().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let nanos = u128::deserialize(deserializer)?;
        Ok(Duration::from_nanos(nanos as u64))
    }
}

/// Errors that can occur during graph operations
#[derive(Error, Debug)]
pub enum ProcessingGraphError {
    #[error("Node '{0}' not found")]
    NodeNotFound(String),
    #[error("Connection would create a cycle")]
    CyclicConnection,
    #[error("Invalid connection: {0}")]
    InvalidConnection(String),
    #[error("No input node defined")]
    NoInputNode,
    #[error("Graph execution failed: {0}")]
    ExecutionFailed(String),
}

/// Represents a connection between two nodes
#[derive(Debug, Clone)]
pub struct Connection {
    pub from: NodeId,
    pub to: NodeId,
}

/// Statistics for individual node performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatistics {
    /// Node ID
    pub node_id: String,
    /// Node type
    pub node_type: String,
    /// Number of frames processed
    pub frames_processed: u64,
    /// Total processing time across all frames
    #[serde(with = "duration_serde")]
    pub total_processing_time: Duration,
    /// Average processing time per frame
    #[serde(with = "duration_serde")]
    pub average_processing_time: Duration,
    /// Minimum processing time observed
    #[serde(with = "duration_serde")]
    pub fastest_processing_time: Duration,
    /// Maximum processing time observed
    #[serde(with = "duration_serde")]
    pub worst_processing_time: Duration,
    /// Last update timestamp (not serialized)
    #[serde(skip)]
    pub last_update: Option<Instant>,
}

impl JsonSchema for NodeStatistics {
    fn schema_name() -> String {
        "NodeStatistics".to_string()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        use schemars::schema::{InstanceType, Metadata, ObjectValidation, SchemaObject};
        use schemars::Map;

        let mut properties = Map::new();

        // Add properties for the struct fields
        properties.insert("node_id".to_string(), gen.subschema_for::<String>());
        properties.insert("node_type".to_string(), gen.subschema_for::<String>());
        properties.insert("frames_processed".to_string(), gen.subschema_for::<u64>());

        // For Duration fields that are serialized as nanoseconds (u64)
        let duration_schema = Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::Integer.into()),
            format: Some("int64".to_string()),
            metadata: Some(Box::new(Metadata {
                title: Some("Duration in nanoseconds".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        });

        properties.insert("total_processing_time".to_string(), duration_schema.clone());
        properties.insert(
            "average_processing_time".to_string(),
            duration_schema.clone(),
        );
        properties.insert(
            "fastest_processing_time".to_string(),
            duration_schema.clone(),
        );
        properties.insert("worst_processing_time".to_string(), duration_schema);

        Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            object: Some(Box::new(ObjectValidation {
                properties,
                required: vec![
                    "node_id".to_string(),
                    "node_type".to_string(),
                    "frames_processed".to_string(),
                    "total_processing_time".to_string(),
                    "average_processing_time".to_string(),
                    "fastest_processing_time".to_string(),
                    "worst_processing_time".to_string(),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}

impl NodeStatistics {
    pub fn new(node_id: String, node_type: String) -> Self {
        Self {
            node_id,
            node_type,
            frames_processed: 0,
            total_processing_time: Duration::ZERO,
            average_processing_time: Duration::ZERO,
            fastest_processing_time: Duration::MAX,
            worst_processing_time: Duration::ZERO,
            last_update: None,
        }
    }

    pub fn record_processing_time(&mut self, duration: Duration) {
        self.frames_processed += 1;
        self.total_processing_time += duration;
        self.average_processing_time = self.total_processing_time / self.frames_processed as u32;

        if duration < self.fastest_processing_time {
            self.fastest_processing_time = duration;
        }

        if duration > self.worst_processing_time {
            self.worst_processing_time = duration;
        }

        self.last_update = Some(Instant::now());
    }

    pub fn reset(&mut self) {
        self.frames_processed = 0;
        self.total_processing_time = Duration::ZERO;
        self.average_processing_time = Duration::ZERO;
        self.fastest_processing_time = Duration::MAX;
        self.worst_processing_time = Duration::ZERO;
        self.last_update = None;
    }
}

impl fmt::Display for NodeStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Node '{}' [{}]: {} frames, avg: {:.2}ms, min: {:.2}ms, max: {:.2}ms",
            self.node_id,
            self.node_type,
            self.frames_processed,
            self.average_processing_time.as_secs_f64() * 1000.0,
            self.fastest_processing_time.as_secs_f64() * 1000.0,
            self.worst_processing_time.as_secs_f64() * 1000.0
        )
    }
}

/// Overall processing graph statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingGraphStatistics {
    /// Statistics for each node
    pub node_statistics: HashMap<String, NodeStatistics>,
    /// Total number of graph executions
    pub total_executions: u64,
    /// Total processing time for the entire graph
    #[serde(with = "duration_serde")]
    pub total_graph_processing_time: Duration,
    /// Average time per graph execution
    #[serde(with = "duration_serde")]
    pub average_graph_processing_time: Duration,
    /// Fastest graph execution time
    #[serde(with = "duration_serde")]
    pub fastest_graph_execution: Duration,
    /// Slowest graph execution time
    #[serde(with = "duration_serde")]
    pub worst_graph_execution: Duration,
    /// Number of active nodes
    pub active_nodes: usize,
    /// Number of connections
    pub connections_count: usize,
    /// Graph creation timestamp (not serialized)
    #[serde(skip)]
    pub graph_created_at: Option<Instant>,
    /// Last execution timestamp (not serialized)
    #[serde(skip)]
    pub last_execution: Option<Instant>,
}

impl JsonSchema for ProcessingGraphStatistics {
    fn schema_name() -> String {
        "ProcessingGraphStatistics".to_string()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        use schemars::schema::{InstanceType, Metadata, ObjectValidation, SchemaObject};
        use schemars::Map;

        let mut properties = Map::new();

        // Add properties for the struct fields
        properties.insert("node_statistics".to_string(), {
            let mut map_schema = SchemaObject {
                instance_type: Some(InstanceType::Object.into()),
                metadata: Some(Box::new(Metadata {
                    title: Some("Node Statistics Map".to_string()),
                    description: Some("Map of node ID to statistics".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            };
            map_schema.object = Some(Box::new(ObjectValidation {
                additional_properties: Some(Box::new(gen.subschema_for::<NodeStatistics>())),
                ..Default::default()
            }));
            Schema::Object(map_schema)
        });

        properties.insert("total_executions".to_string(), gen.subschema_for::<u64>());

        // For Duration fields that are serialized as nanoseconds (u64)
        let duration_schema = Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::Integer.into()),
            format: Some("int64".to_string()),
            metadata: Some(Box::new(Metadata {
                title: Some("Duration in nanoseconds".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        });

        properties.insert(
            "total_graph_processing_time".to_string(),
            duration_schema.clone(),
        );
        properties.insert(
            "average_graph_processing_time".to_string(),
            duration_schema.clone(),
        );
        properties.insert(
            "fastest_graph_execution".to_string(),
            duration_schema.clone(),
        );
        properties.insert("worst_graph_execution".to_string(), duration_schema);
        properties.insert("active_nodes".to_string(), gen.subschema_for::<usize>());
        properties.insert(
            "connections_count".to_string(),
            gen.subschema_for::<usize>(),
        );

        Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            object: Some(Box::new(ObjectValidation {
                properties,
                required: vec![
                    "node_statistics".to_string(),
                    "total_executions".to_string(),
                    "total_graph_processing_time".to_string(),
                    "average_graph_processing_time".to_string(),
                    "fastest_graph_execution".to_string(),
                    "worst_graph_execution".to_string(),
                    "active_nodes".to_string(),
                    "connections_count".to_string(),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            })),
            metadata: Some(Box::new(Metadata {
                title: Some("Processing Graph Statistics".to_string()),
                description: Some(
                    "Overall performance statistics for the entire processing graph".to_string(),
                ),
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}

impl ProcessingGraphStatistics {
    pub fn new() -> Self {
        Self {
            node_statistics: HashMap::new(),
            total_executions: 0,
            total_graph_processing_time: Duration::ZERO,
            average_graph_processing_time: Duration::ZERO,
            fastest_graph_execution: Duration::MAX,
            worst_graph_execution: Duration::ZERO,
            active_nodes: 0,
            connections_count: 0,
            graph_created_at: Some(Instant::now()),
            last_execution: None,
        }
    }

    pub fn record_graph_execution(&mut self, duration: Duration) {
        self.total_executions += 1;
        self.total_graph_processing_time += duration;
        self.average_graph_processing_time =
            self.total_graph_processing_time / self.total_executions as u32;

        if duration < self.fastest_graph_execution {
            self.fastest_graph_execution = duration;
        }

        if duration > self.worst_graph_execution {
            self.worst_graph_execution = duration;
        }

        self.last_execution = Some(Instant::now());
    }

    pub fn update_graph_structure(&mut self, nodes_count: usize, connections_count: usize) {
        self.active_nodes = nodes_count;
        self.connections_count = connections_count;
    }

    pub fn add_node_statistics(&mut self, node_id: String, node_type: String) {
        self.node_statistics
            .insert(node_id.clone(), NodeStatistics::new(node_id, node_type));
    }

    pub fn remove_node_statistics(&mut self, node_id: &str) {
        self.node_statistics.remove(node_id);
    }

    pub fn record_node_processing(&mut self, node_id: &str, duration: Duration) {
        if let Some(stats) = self.node_statistics.get_mut(node_id) {
            stats.record_processing_time(duration);
        }
    }

    pub fn reset_all_statistics(&mut self) {
        for stats in self.node_statistics.values_mut() {
            stats.reset();
        }

        self.total_executions = 0;
        self.total_graph_processing_time = Duration::ZERO;
        self.average_graph_processing_time = Duration::ZERO;
        self.fastest_graph_execution = Duration::MAX;
        self.worst_graph_execution = Duration::ZERO;
        self.last_execution = None;
    }

    /// Get the slowest node by average processing time
    pub fn get_slowest_node(&self) -> Option<&NodeStatistics> {
        self.node_statistics
            .values()
            .max_by_key(|stats| stats.average_processing_time)
    }

    /// Get the fastest node by average processing time
    pub fn get_fastest_node(&self) -> Option<&NodeStatistics> {
        self.node_statistics
            .values()
            .filter(|stats| stats.frames_processed > 0)
            .min_by_key(|stats| stats.average_processing_time)
    }

    /// Get nodes sorted by processing time (slowest first)
    pub fn get_nodes_by_performance(&self) -> Vec<&NodeStatistics> {
        let mut nodes: Vec<_> = self.node_statistics.values().collect();
        nodes.sort_by(|a, b| b.average_processing_time.cmp(&a.average_processing_time));
        nodes
    }

    /// Calculate throughput in frames per second
    pub fn get_throughput_fps(&self) -> f64 {
        if self.total_graph_processing_time.is_zero() {
            return 0.0;
        }

        self.total_executions as f64 / self.total_graph_processing_time.as_secs_f64()
    }

    /// Get efficiency percentage (0-100)
    pub fn get_efficiency_percentage(&self) -> f64 {
        if self.worst_graph_execution.is_zero() {
            return 100.0;
        }

        (self.fastest_graph_execution.as_secs_f64() / self.worst_graph_execution.as_secs_f64())
            * 100.0
    }
}

impl fmt::Display for ProcessingGraphStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Processing Graph Statistics ===")?;
        writeln!(f, "Graph Structure:")?;
        writeln!(f, "  • Active nodes: {}", self.active_nodes)?;
        writeln!(f, "  • Connections: {}", self.connections_count)?;

        if self.total_executions > 0 {
            writeln!(f, "\nExecution Performance:")?;
            writeln!(f, "  • Total executions: {}", self.total_executions)?;
            writeln!(
                f,
                "  • Average execution time: {:.2}ms",
                self.average_graph_processing_time.as_secs_f64() * 1000.0
            )?;
            writeln!(
                f,
                "  • Fastest execution: {:.2}ms",
                self.fastest_graph_execution.as_secs_f64() * 1000.0
            )?;
            writeln!(
                f,
                "  • Slowest execution: {:.2}ms",
                self.worst_graph_execution.as_secs_f64() * 1000.0
            )?;
            writeln!(f, "  • Throughput: {:.1} FPS", self.get_throughput_fps())?;
            writeln!(
                f,
                "  • Efficiency: {:.1}%",
                self.get_efficiency_percentage()
            )?;

            if !self.node_statistics.is_empty() {
                writeln!(f, "\nNode Performance (by average processing time):")?;
                for stats in self.get_nodes_by_performance() {
                    if stats.frames_processed > 0 {
                        writeln!(f, "  • {}", stats)?;
                    }
                }

                if let Some(slowest) = self.get_slowest_node() {
                    writeln!(
                        f,
                        "\n⚠️  Bottleneck: {} ({:.2}ms avg)",
                        slowest.node_id,
                        slowest.average_processing_time.as_secs_f64() * 1000.0
                    )?;
                }
            }
        } else {
            writeln!(f, "\nNo executions recorded yet.")?;
        }

        Ok(())
    }
}

/// Processing graph that manages nodes and their connections
pub struct ProcessingGraph {
    /// Map of node ID to processing node
    nodes: HashMap<NodeId, Box<dyn ProcessingNode>>,
    /// List of connections between nodes
    connections: Vec<Connection>,
    /// Cached execution order (topologically sorted)
    execution_order: Option<Vec<NodeId>>,
    /// Input node ID
    input_node: Option<NodeId>,
    /// Output node ID(s)
    output_nodes: Vec<NodeId>,
    /// Performance statistics
    statistics: ProcessingGraphStatistics,
    /// Original node configuration parameters (for serialization)
    node_parameters: HashMap<NodeId, HashMap<String, serde_json::Value>>,
    /// Shared computing state for all nodes
    shared_computing_state: Option<SharedComputingState>,
}

impl ProcessingGraph {
    /// Create a new empty processing graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            connections: Vec::new(),
            execution_order: None,
            input_node: None,
            output_nodes: Vec::new(),
            statistics: ProcessingGraphStatistics::new(),
            node_parameters: HashMap::new(),
            shared_computing_state: None,
        }
    }

    /// Set the shared computing state for the graph
    ///
    /// This method sets the shared computing state that will be propagated to all nodes
    /// in the graph. Computing nodes like PeakFinderNode will use this state to share
    /// their analytical results.
    pub fn set_shared_computing_state(&mut self, shared_state: Option<SharedComputingState>) {
        self.shared_computing_state = shared_state.clone();

        // Propagate the shared state to all existing nodes
        for node in self.nodes.values_mut() {
            node.set_shared_computing_state(shared_state.clone());
        }
    }

    /// Get the shared computing state for the graph
    ///
    /// Returns the current shared computing state that contains analytical results
    /// from computing nodes in the graph.
    pub fn get_shared_computing_state(&self) -> Option<SharedComputingState> {
        self.shared_computing_state.clone()
    }

    /// Add a processing node to the graph
    pub fn add_node(&mut self, node: Box<dyn ProcessingNode>) -> Result<()> {
        self.add_node_with_params(node, HashMap::new())
    }

    /// Add a processing node to the graph with configuration parameters
    pub fn add_node_with_params(
        &mut self,
        mut node: Box<dyn ProcessingNode>,
        parameters: HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let node_id = node.node_id().to_string();
        let node_type = node.node_type().to_string();

        if self.nodes.contains_key(&node_id) {
            anyhow::bail!("Node '{}' already exists", node_id);
        }

        // Set the shared computing state on the node if available
        if let Some(shared_state) = &self.shared_computing_state {
            node.set_shared_computing_state(Some(shared_state.clone()));
        }

        // If this is an input node, set it as the input
        if node.node_type() == "input" {
            self.input_node = Some(node_id.clone());
        }

        // Store node parameters for serialization
        self.node_parameters.insert(node_id.clone(), parameters);

        // Add node statistics tracking
        self.statistics
            .add_node_statistics(node_id.clone(), node_type);

        self.nodes.insert(node_id, node);
        self.update_statistics_structure();
        self.invalidate_execution_order();
        Ok(())
    }

    /// Remove a node from the graph
    pub fn remove_node(&mut self, node_id: &str) -> Result<()> {
        if !self.nodes.contains_key(node_id) {
            return Err(ProcessingGraphError::NodeNotFound(node_id.to_string()).into());
        }

        // Remove all connections involving this node
        self.connections
            .retain(|conn| conn.from != node_id && conn.to != node_id);

        // Remove the node
        self.nodes.remove(node_id);

        // Remove node parameters
        self.node_parameters.remove(node_id);

        // Remove node statistics
        self.statistics.remove_node_statistics(node_id);

        // Clear input node if it was removed
        if self.input_node.as_ref() == Some(&node_id.to_string()) {
            self.input_node = None;
        }

        // Remove from output nodes
        self.output_nodes.retain(|id| id != node_id);

        self.update_statistics_structure();
        self.invalidate_execution_order();
        Ok(())
    }

    /// Connect two nodes in the graph
    pub fn connect(&mut self, from_id: &str, to_id: &str) -> Result<()> {
        // Validate that both nodes exist
        if !self.nodes.contains_key(from_id) {
            return Err(ProcessingGraphError::NodeNotFound(from_id.to_string()).into());
        }
        if !self.nodes.contains_key(to_id) {
            return Err(ProcessingGraphError::NodeNotFound(to_id.to_string()).into());
        }

        // Check if connection already exists
        if self
            .connections
            .iter()
            .any(|conn| conn.from == from_id && conn.to == to_id)
        {
            anyhow::bail!(
                "Connection already exists from '{}' to '{}'",
                from_id,
                to_id
            );
        }

        let connection = Connection {
            from: from_id.to_string(),
            to: to_id.to_string(),
        };

        // Add the connection
        self.connections.push(connection);

        // Validate that this doesn't create a cycle
        if self.has_cycle() {
            // Remove the connection we just added
            self.connections.pop();
            return Err(ProcessingGraphError::CyclicConnection.into());
        }

        self.invalidate_execution_order();
        Ok(())
    }

    /// Disconnect two nodes
    pub fn disconnect(&mut self, from_id: &str, to_id: &str) -> Result<()> {
        let initial_len = self.connections.len();
        self.connections
            .retain(|conn| !(conn.from == from_id && conn.to == to_id));

        if self.connections.len() == initial_len {
            anyhow::bail!("No connection found from '{}' to '{}'", from_id, to_id);
        }

        self.invalidate_execution_order();
        Ok(())
    }

    /// Set a node as an output node
    pub fn set_output_node(&mut self, node_id: &str) -> Result<()> {
        if !self.nodes.contains_key(node_id) {
            return Err(ProcessingGraphError::NodeNotFound(node_id.to_string()).into());
        }

        if !self.output_nodes.contains(&node_id.to_string()) {
            self.output_nodes.push(node_id.to_string());
        }

        Ok(())
    }

    /// Execute the processing graph with the given input data
    pub fn execute(&mut self, input_data: ProcessingData) -> Result<Vec<ProcessingData>> {
        let graph_start_time = Instant::now();

        // Ensure we have an input node
        let input_node_id = self
            .input_node
            .as_ref()
            .ok_or(ProcessingGraphError::NoInputNode)?
            .clone();

        // Get execution order
        let execution_order = self.get_execution_order()?.clone();

        // Store intermediate results
        let mut node_outputs: HashMap<NodeId, ProcessingData> = HashMap::new();

        // Execute nodes in topological order
        for node_id in &execution_order {
            let node_start_time = Instant::now();

            let node = self.nodes.get_mut(node_id).unwrap();

            let input_for_node = if node_id == &input_node_id {
                // Input node gets the original input data
                input_data.clone()
            } else {
                // Find the input for this node from connected predecessors
                let predecessors: Vec<&str> = self
                    .connections
                    .iter()
                    .filter(|conn| &conn.to == node_id)
                    .map(|conn| conn.from.as_str())
                    .collect();

                if predecessors.is_empty() {
                    // This shouldn't happen in a well-formed graph
                    return Err(ProcessingGraphError::ExecutionFailed(format!(
                        "Node '{}' has no input connections",
                        node_id
                    ))
                    .into());
                }

                // For now, we assume single input per node
                // In a more complex system, we'd need to handle multiple inputs
                let predecessor_id = predecessors[0];
                node_outputs
                    .get(predecessor_id)
                    .ok_or_else(|| {
                        ProcessingGraphError::ExecutionFailed(format!(
                            "No output from predecessor '{}'",
                            predecessor_id
                        ))
                    })?
                    .clone()
            };

            // Process the data through this node
            let output = node.process(input_for_node).map_err(|e| {
                ProcessingGraphError::ExecutionFailed(format!("Node '{}' failed: {}", node_id, e))
            })?;

            // Record node processing time
            let node_duration = node_start_time.elapsed();
            self.statistics
                .record_node_processing(node_id, node_duration);

            node_outputs.insert(node_id.clone(), output);
        }

        // Record total graph execution time
        let graph_duration = graph_start_time.elapsed();
        self.statistics.record_graph_execution(graph_duration);

        // Collect outputs from designated output nodes
        let mut results = Vec::new();
        if self.output_nodes.is_empty() {
            // If no specific output nodes, return the last node's output
            if let Some(last_node_id) = execution_order.last() {
                if let Some(output) = node_outputs.get(last_node_id) {
                    results.push(output.clone());
                }
            }
        } else {
            // Return outputs from all designated output nodes
            for output_node_id in &self.output_nodes {
                if let Some(output) = node_outputs.get(output_node_id) {
                    results.push(output.clone());
                }
            }
        }

        Ok(results)
    }

    /// Create a new processing graph from configuration
    pub fn from_config(config: &ProcessingGraphConfig) -> Result<Self> {
        Self::from_config_with_registry(config, None)
    }

    /// Create a new processing graph from configuration with photoacoustic parameters
    pub fn from_config_with_photoacoustic(
        config: &ProcessingGraphConfig,
        photoacoustic_config: &crate::config::PhotoacousticConfig,
    ) -> Result<Self> {
        Self::from_config_with_registry_and_photoacoustic(config, None, photoacoustic_config)
    }

    /// Create a new processing graph from configuration with shared computing state
    pub fn from_config_with_computing_state(
        config: &ProcessingGraphConfig,
        computing_state: Option<SharedComputingState>,
    ) -> Result<Self> {
        Self::from_config_with_all_params(
            config,
            None,
            &crate::config::PhotoacousticConfig::default(),
            computing_state,
        )
    }

    /// Create a new processing graph from configuration with optional streaming registry
    pub fn from_config_with_registry(
        config: &ProcessingGraphConfig,
        streaming_registry: Option<StreamingNodeRegistry>,
    ) -> Result<Self> {
        Self::from_config_with_registry_and_photoacoustic(
            config,
            streaming_registry,
            &crate::config::PhotoacousticConfig::default(),
        )
    }

    /// Create a new processing graph from configuration with optional streaming registry and photoacoustic parameters
    pub fn from_config_with_registry_and_photoacoustic(
        config: &ProcessingGraphConfig,
        streaming_registry: Option<StreamingNodeRegistry>,
        photoacoustic_config: &crate::config::PhotoacousticConfig,
    ) -> Result<Self> {
        Self::from_config_with_all_params(config, streaming_registry, photoacoustic_config, None)
    }

    /// Create a new processing graph from configuration with all optional parameters
    pub fn from_config_with_all_params(
        config: &ProcessingGraphConfig,
        streaming_registry: Option<StreamingNodeRegistry>,
        photoacoustic_config: &crate::config::PhotoacousticConfig,
        computing_state: Option<SharedComputingState>,
    ) -> Result<Self> {
        let mut graph = Self::new();

        debug!("Creating processing graph from config: {}", config.id);
        debug!("Number of nodes to create: {}", config.nodes.len());
        debug!(
            "Number of connections to create: {}",
            config.connections.len()
        );
        debug!(
            "Streaming registry provided: {}",
            streaming_registry.is_some()
        );

        // First, create all nodes
        for node_config in &config.nodes {
            debug!(
                "Creating node: {} of type: {}",
                node_config.id, node_config.node_type
            );
            let node = Self::create_node_from_config(
                node_config,
                &streaming_registry,
                photoacoustic_config,
                &computing_state,
            )?;

            // Convert node_config.parameters to HashMap<String, serde_json::Value>
            let parameters = if let Some(params_object) = node_config.parameters.as_object() {
                params_object
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect()
            } else {
                HashMap::new()
            };

            graph.add_node_with_params(node, parameters)?;
            debug!("Successfully created node: {}", node_config.id);
        }

        debug!("Total nodes created: {}", graph.nodes.len());
        debug!("Node IDs: {:?}", graph.nodes.keys().collect::<Vec<_>>());

        // Then, create all connections
        for connection_config in &config.connections {
            debug!(
                "Creating connection from '{}' to '{}'",
                connection_config.from, connection_config.to
            );
            graph.connect(&connection_config.from, &connection_config.to)?;
            debug!(
                "Successfully created connection from '{}' to '{}'",
                connection_config.from, connection_config.to
            );
        }

        // Set output node if specified
        if let Some(ref output_id) = config.output_node {
            debug!("Setting output node: {}", output_id);
            let _ = graph.set_output_node(output_id);
        }

        debug!("Processing graph created successfully");
        Ok(graph)
    }

    /// Create a processing node from configuration
    fn create_node_from_config(
        config: &NodeConfig,
        streaming_registry: &Option<StreamingNodeRegistry>,
        photoacoustic_config: &crate::config::PhotoacousticConfig,
        computing_state: &Option<SharedComputingState>,
    ) -> Result<Box<dyn ProcessingNode>> {
        match config.node_type.as_str() {
            "input" => Ok(Box::new(InputNode::new(config.id.clone()))),
            "channel_selector" => {
                // Extract target_channel parameter
                let target_channel = if let Some(params) = config.parameters.as_object() {
                    if let Some(channel_value) = params.get("target_channel") {
                        if let Some(channel_str) = channel_value.as_str() {
                            match channel_str {
                                "ChannelA" => ChannelTarget::ChannelA,
                                "ChannelB" => ChannelTarget::ChannelB,
                                _ => {
                                    return Err(anyhow::anyhow!("Invalid channel: {}", channel_str))
                                }
                            }
                        } else {
                            return Err(anyhow::anyhow!("target_channel must be a string"));
                        }
                    } else {
                        ChannelTarget::ChannelA // Default
                    }
                } else {
                    ChannelTarget::ChannelA // Default
                };

                Ok(Box::new(ChannelSelectorNode::new(
                    config.id.clone(),
                    target_channel,
                )))
            }
            "channel_mixer" => {
                // Extract mix strategy parameters
                let mix_strategy = if let Some(params) = config.parameters.as_object() {
                    if let Some(strategy_value) = params.get("strategy") {
                        match strategy_value.as_str() {
                            Some("add") => MixStrategy::Add,
                            Some("subtract") => MixStrategy::Subtract,
                            Some("average") => MixStrategy::Average,
                            Some("weighted") => {
                                let a_weight = params
                                    .get("a_weight")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.5)
                                    as f32;
                                let b_weight = params
                                    .get("b_weight")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.5)
                                    as f32;
                                MixStrategy::Weighted { a_weight, b_weight }
                            }
                            _ => MixStrategy::Average, // Default
                        }
                    } else {
                        MixStrategy::Average // Default
                    }
                } else {
                    MixStrategy::Average // Default
                };

                Ok(Box::new(ChannelMixerNode::new(
                    config.id.clone(),
                    mix_strategy,
                )))
            }
            "filter" => {
                // Extract filter parameters
                let params = config
                    .parameters
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("Filter node requires parameters"))?;

                let filter_type = params
                    .get("type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Filter requires 'type' parameter"))?;

                let target_channel = if let Some(channel_value) = params.get("target_channel") {
                    if let Some(channel_str) = channel_value.as_str() {
                        match channel_str {
                            "ChannelA" => ChannelTarget::ChannelA,
                            "ChannelB" => ChannelTarget::ChannelB,
                            "Both" => ChannelTarget::Both,
                            _ => ChannelTarget::Both, // Default
                        }
                    } else {
                        ChannelTarget::Both // Default
                    }
                } else {
                    ChannelTarget::Both // Default
                };

                match filter_type {
                    "bandpass" => {
                        let center_freq = params
                            .get("center_frequency")
                            .and_then(|v| v.as_f64())
                            .ok_or_else(|| {
                                anyhow::anyhow!("Bandpass filter requires 'center_frequency'")
                            })? as f32;

                        let bandwidth = params
                            .get("bandwidth")
                            .and_then(|v| v.as_f64())
                            .ok_or_else(|| {
                                anyhow::anyhow!("Bandpass filter requires 'bandwidth'")
                            })? as f32;

                        let order =
                            params.get("order").and_then(|v| v.as_u64()).unwrap_or(4) as usize; // Default to 4th order for bandpass

                        let filter = BandpassFilter::new(center_freq, bandwidth).with_order(order);
                        Ok(Box::new(FilterNode::new(
                            config.id.clone(),
                            Box::new(filter),
                            target_channel,
                        )))
                    }
                    "lowpass" => {
                        let cutoff_freq = params
                            .get("cutoff_frequency")
                            .and_then(|v| v.as_f64())
                            .ok_or_else(|| {
                                anyhow::anyhow!("Lowpass filter requires 'cutoff_frequency'")
                            })? as f32;

                        let order =
                            params.get("order").and_then(|v| v.as_u64()).unwrap_or(1) as usize; // Default to 1st order for lowpass

                        let filter = LowpassFilter::new(cutoff_freq).with_order(order);
                        Ok(Box::new(FilterNode::new(
                            config.id.clone(),
                            Box::new(filter),
                            target_channel,
                        )))
                    }
                    "highpass" => {
                        let cutoff_freq = params
                            .get("cutoff_frequency")
                            .and_then(|v| v.as_f64())
                            .ok_or_else(|| {
                                anyhow::anyhow!("Highpass filter requires 'cutoff_frequency'")
                            })? as f32;

                        let order =
                            params.get("order").and_then(|v| v.as_u64()).unwrap_or(1) as usize; // Default to 1st order for highpass

                        let filter = HighpassFilter::new(cutoff_freq).with_order(order);
                        Ok(Box::new(FilterNode::new(
                            config.id.clone(),
                            Box::new(filter),
                            target_channel,
                        )))
                    }
                    _ => Err(anyhow::anyhow!("Unknown filter type: {}", filter_type)),
                }
            }
            "differential" => {
                // Extract differential parameters (if any)
                let differential = SimpleDifferential::new();
                Ok(Box::new(DifferentialNode::new(
                    config.id.clone(),
                    Box::new(differential),
                )))
            }
            "photoacoustic_output" => {
                // Extract photoacoustic output parameters
                let mut node = PhotoacousticOutputNode::new(config.id.clone());

                if let Some(params) = config.parameters.as_object() {
                    if let Some(threshold_value) = params.get("detection_threshold") {
                        if let Some(threshold) = threshold_value.as_f64() {
                            node = node.with_detection_threshold(threshold as f32);
                        }
                    }

                    if let Some(window_size_value) = params.get("analysis_window_size") {
                        if let Some(window_size) = window_size_value.as_u64() {
                            node = node.with_analysis_window_size(window_size as usize);
                        }
                    }
                }

                Ok(Box::new(node))
            }
            "record" => {
                // Extract record parameters
                let params = config
                    .parameters
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("Record node requires parameters"))?;

                let record_file = params
                    .get("record_file")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Record node requires 'record_file' parameter")
                    })?;

                let max_size = params
                    .get("max_size")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1024) as usize; // Default 1MB

                let auto_delete = params
                    .get("auto_delete")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false); // Default false

                let total_limit = params
                    .get("total_limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize); // Optional total limit

                Ok(Box::new(RecordNode::new(
                    config.id.clone(),
                    std::path::PathBuf::from(record_file),
                    max_size,
                    auto_delete,
                    total_limit,
                )))
            }
            "streaming" => {
                debug!("Creating streaming node: {}", config.id);
                // Streaming node requires a registry
                let registry = streaming_registry
                    .as_ref()
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Streaming node '{}' requires a StreamingNodeRegistry, but none was provided",
                            config.id
                        )
                    })?
                    .clone();

                // Extract streaming parameters
                let name = if let Some(params) = config.parameters.as_object() {
                    params
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&config.id)
                        .to_string()
                } else {
                    config.id.clone()
                };

                // Use the configured string ID for the node
                Ok(Box::new(StreamingNode::new_with_string_id(
                    &config.id, &name, registry,
                )))
            }
            "computing_peak_finder" => {
                // Extract peak finder parameters
                let mut peak_finder = PeakFinderNode::new_with_shared_state(
                    config.id.clone(),
                    computing_state.clone(),
                );

                // Use global photoacoustic parameters for sample_rate and fft_size (frame_size)
                peak_finder = peak_finder.with_sample_rate(photoacoustic_config.sample_rate as u32);
                peak_finder = peak_finder.with_fft_size(photoacoustic_config.frame_size as usize);

                if let Some(params) = config.parameters.as_object() {
                    if let Some(threshold_value) = params.get("detection_threshold") {
                        if let Some(threshold) = threshold_value.as_f64() {
                            peak_finder = peak_finder.with_detection_threshold(threshold as f32);
                        }
                    }

                    if let Some(freq_min_value) = params.get("frequency_min") {
                        if let Some(freq_min) = freq_min_value.as_f64() {
                            let freq_max = params
                                .get("frequency_max")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(20000.0)
                                as f32;
                            peak_finder =
                                peak_finder.with_frequency_range(freq_min as f32, freq_max);
                        }
                    }

                    if let Some(freq_max_value) = params.get("frequency_max") {
                        if let Some(freq_max) = freq_max_value.as_f64() {
                            let freq_min = params
                                .get("frequency_min")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(20.0) as f32;
                            peak_finder =
                                peak_finder.with_frequency_range(freq_min, freq_max as f32);
                        }
                    }

                    if let Some(smoothing_value) = params.get("smoothing_factor") {
                        if let Some(smoothing) = smoothing_value.as_f64() {
                            peak_finder = peak_finder.with_smoothing_factor(smoothing as f32);
                        }
                    }
                }

                Ok(Box::new(peak_finder))
            }
            "computing_concentration" => {
                // Extract concentration calculator parameters
                let params = config
                    .parameters
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("Concentration node requires parameters"))?;

                // Create concentration node with shared state
                let mut concentration_node = ConcentrationNode::new_with_shared_state(
                    config.id.clone(),
                    computing_state.clone(),
                );

                // Extract computing_peak_finder_id (required)
                let peak_finder_id = params
                    .get("computing_peak_finder_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Concentration node requires 'computing_peak_finder_id' parameter"
                        )
                    })?;
                concentration_node =
                    concentration_node.with_peak_finder_source(peak_finder_id.to_string());

                // Extract polynomial coefficients (required array of 5 values)
                if let Some(coeffs_value) = params.get("polynomial_coefficients") {
                    if let Some(coeffs_array) = coeffs_value.as_array() {
                        if coeffs_array.len() == 5 {
                            let mut coefficients = [0.0; 5];
                            for (i, coeff) in coeffs_array.iter().enumerate() {
                                if let Some(val) = coeff.as_f64() {
                                    coefficients[i] = val;
                                } else {
                                    return Err(anyhow::anyhow!(
                                        "Polynomial coefficient {} must be a number",
                                        i
                                    ));
                                }
                            }
                            concentration_node =
                                concentration_node.with_polynomial_coefficients(coefficients);
                        } else {
                            return Err(anyhow::anyhow!(
                                "Polynomial coefficients must be an array of exactly 5 values, got {}",
                                coeffs_array.len()
                            ));
                        }
                    } else {
                        return Err(anyhow::anyhow!("Polynomial coefficients must be an array"));
                    }
                }

                // Extract optional parameters
                if let Some(temp_comp) = params.get("temperature_compensation") {
                    if let Some(enable_temp_comp) = temp_comp.as_bool() {
                        concentration_node =
                            concentration_node.with_temperature_compensation(enable_temp_comp);
                    }
                }

                if let Some(spectral_line) = params.get("spectral_line_id") {
                    if let Some(line_id) = spectral_line.as_str() {
                        concentration_node =
                            concentration_node.with_spectral_line_id(line_id.to_string());
                    }
                }

                if let Some(min_threshold) = params.get("min_amplitude_threshold") {
                    if let Some(threshold) = min_threshold.as_f64() {
                        concentration_node =
                            concentration_node.with_min_amplitude_threshold(threshold as f32);
                    }
                }

                if let Some(max_conc) = params.get("max_concentration_ppm") {
                    if let Some(max_ppm) = max_conc.as_f64() {
                        concentration_node =
                            concentration_node.with_max_concentration(max_ppm as f32);
                    }
                }

                Ok(Box::new(concentration_node))
            }
            "gain" => {
                // Extract gain parameters
                let params = config
                    .parameters
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("Gain node requires parameters"))?;

                let gain_db = params
                    .get("gain_db")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Gain node requires 'gain_db' parameter in dB")
                    })? as f32;

                Ok(Box::new(GainNode::new(config.id.clone(), gain_db)))
            }
            "action_universal_display" => {
                // Extract example display action parameters
                let mut action_node = UniversalDisplayActionNode::new_with_shared_state(
                    config.id.clone(),
                    computing_state.clone(),
                );

                if let Some(params) = config.parameters.as_object() {
                    // Extract buffer_capacity parameter (optional, maps to history_buffer_capacity)
                    if let Some(buffer_capacity_value) = params.get("buffer_capacity") {
                        if let Some(buffer_capacity) = buffer_capacity_value.as_u64() {
                            action_node =
                                action_node.with_history_buffer_capacity(buffer_capacity as usize);
                        }
                    }

                    // Extract monitored_nodes parameter (optional array of node IDs)
                    if let Some(monitored_value) = params.get("monitored_nodes") {
                        if let Some(monitored_array) = monitored_value.as_array() {
                            for node_id_value in monitored_array {
                                if let Some(node_id) = node_id_value.as_str() {
                                    action_node =
                                        action_node.with_monitored_node(node_id.to_string());
                                }
                            }
                        }
                    }

                    // Extract concentration_threshold parameter (optional)
                    if let Some(threshold_value) = params.get("concentration_threshold") {
                        if let Some(threshold) = threshold_value.as_f64() {
                            action_node = action_node.with_concentration_threshold(threshold);
                        }
                    }

                    // Extract amplitude_threshold parameter (optional)
                    if let Some(threshold_value) = params.get("amplitude_threshold") {
                        if let Some(threshold) = threshold_value.as_f64() {
                            action_node = action_node.with_amplitude_threshold(threshold as f32);
                        }
                    }

                    // Extract update_interval_ms parameter (optional)
                    if let Some(interval_value) = params.get("update_interval_ms") {
                        if let Some(interval) = interval_value.as_u64() {
                            action_node = action_node.with_update_interval(interval);
                        }
                    }
                }

                Ok(Box::new(action_node))
            }
            _ => Err(anyhow::anyhow!("Unknown node type: {}", config.node_type)),
        }
    }

    /// Get the execution order (topologically sorted)
    fn get_execution_order(&mut self) -> Result<Vec<NodeId>> {
        if let Some(ref order) = self.execution_order {
            return Ok(order.clone());
        }

        let order = self.topological_sort()?;
        self.execution_order = Some(order.clone());
        Ok(order)
    }

    /// Get the execution order (topologically sorted) without caching
    ///
    /// This method computes the execution order without modifying the graph state,
    /// making it suitable for use in immutable contexts like serialization.
    ///
    /// ### Returns
    ///
    /// * `Ok(Vec<NodeId>)` - Topologically sorted execution order
    /// * `Err(ProcessingGraphError)` - If the graph contains cycles
    pub fn get_execution_order_immutable(&self) -> Result<Vec<NodeId>> {
        self.topological_sort()
    }

    /// Perform topological sort to determine execution order
    fn topological_sort(&self) -> Result<Vec<NodeId>> {
        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
        let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

        // Initialize in-degree and adjacency list
        for node_id in self.nodes.keys() {
            in_degree.insert(node_id.clone(), 0);
            adjacency.insert(node_id.clone(), Vec::new());
        }

        // Build adjacency list and calculate in-degrees
        for connection in &self.connections {
            adjacency
                .get_mut(&connection.from)
                .unwrap()
                .push(connection.to.clone());
            *in_degree.get_mut(&connection.to).unwrap() += 1;
        }

        // Queue for nodes with no incoming edges
        let mut queue: VecDeque<NodeId> = VecDeque::new();
        for (node_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node_id.clone());
            }
        }

        let mut sorted_order = Vec::new();

        while let Some(node_id) = queue.pop_front() {
            sorted_order.push(node_id.clone());

            // For each neighbor of the current node
            if let Some(neighbors) = adjacency.get(&node_id) {
                for neighbor in neighbors {
                    let neighbor_degree = in_degree.get_mut(neighbor).unwrap();
                    *neighbor_degree -= 1;
                    if *neighbor_degree == 0 {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }

        // Check if all nodes were processed (no cycles)
        if sorted_order.len() != self.nodes.len() {
            return Err(ProcessingGraphError::CyclicConnection.into());
        }

        Ok(sorted_order)
    }

    /// Check if the graph has cycles using DFS
    fn has_cycle(&self) -> bool {
        let mut visited = HashMap::new();
        let mut rec_stack = HashMap::new();

        for node_id in self.nodes.keys() {
            visited.insert(node_id.clone(), false);
            rec_stack.insert(node_id.clone(), false);
        }

        for node_id in self.nodes.keys() {
            if !visited[node_id] {
                if self.has_cycle_util(node_id, &mut visited, &mut rec_stack) {
                    return true;
                }
            }
        }

        false
    }

    /// Utility function for cycle detection
    fn has_cycle_util(
        &self,
        node_id: &str,
        visited: &mut HashMap<NodeId, bool>,
        rec_stack: &mut HashMap<NodeId, bool>,
    ) -> bool {
        visited.insert(node_id.to_string(), true);
        rec_stack.insert(node_id.to_string(), true);

        // Get all neighbors of this node
        for connection in &self.connections {
            if connection.from == node_id {
                let neighbor = &connection.to;

                if !visited[neighbor] {
                    if self.has_cycle_util(neighbor, visited, rec_stack) {
                        return true;
                    }
                } else if rec_stack[neighbor] {
                    return true;
                }
            }
        }

        rec_stack.insert(node_id.to_string(), false);
        false
    }

    /// Invalidate cached execution order
    fn invalidate_execution_order(&mut self) {
        self.execution_order = None;
    }

    /// Get a list of all node IDs
    pub fn node_ids(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Get all connections
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    /// Reset all nodes in the graph
    pub fn reset(&mut self) {
        for node in self.nodes.values_mut() {
            node.reset();
        }
        self.invalidate_execution_order();
    }

    /// Validate the graph structure
    pub fn validate(&self) -> Result<()> {
        // Check if we have an input node
        if self.input_node.is_none() {
            return Err(ProcessingGraphError::NoInputNode.into());
        }

        // Check for cycles
        if self.has_cycle() {
            return Err(ProcessingGraphError::CyclicConnection.into());
        }

        // Validate that all connections are between existing nodes
        for connection in &self.connections {
            if !self.nodes.contains_key(&connection.from) {
                return Err(ProcessingGraphError::NodeNotFound(connection.from.clone()).into());
            }
            if !self.nodes.contains_key(&connection.to) {
                return Err(ProcessingGraphError::NodeNotFound(connection.to.clone()).into());
            }
        }

        Ok(())
    }

    /// Get the current processing statistics
    pub fn get_statistics(&self) -> &ProcessingGraphStatistics {
        &self.statistics
    }

    /// Get a mutable reference to the statistics (for advanced operations)
    pub fn get_statistics_mut(&mut self) -> &mut ProcessingGraphStatistics {
        &mut self.statistics
    }

    /// Reset all performance statistics
    pub fn reset_statistics(&mut self) {
        self.statistics.reset_all_statistics();
    }

    /// Get statistics for a specific node
    pub fn get_node_statistics(&self, node_id: &str) -> Option<&NodeStatistics> {
        self.statistics.node_statistics.get(node_id)
    }

    /// Get a summary of performance metrics
    pub fn get_performance_summary(&self) -> PerformanceSummary {
        let nodes_by_performance = self
            .statistics
            .get_nodes_by_performance()
            .into_iter()
            .cloned()
            .collect();

        PerformanceSummary {
            // Original fields required by client
            throughput_fps: self.statistics.get_throughput_fps(),
            efficiency_percentage: self.statistics.get_efficiency_percentage(),
            slowest_node: self
                .statistics
                .get_slowest_node()
                .map(|n| n.node_id.clone()),
            fastest_node: self
                .statistics
                .get_fastest_node()
                .map(|n| n.node_id.clone()),

            // Additional fields for enhanced functionality
            total_nodes: self.node_count(),
            active_nodes: self.statistics.active_nodes,
            total_connections: self.connection_count(),
            average_execution_time_ms: self.statistics.average_graph_processing_time.as_secs_f64()
                * 1000.0,
            fastest_execution_time_ms: self.statistics.fastest_graph_execution.as_secs_f64()
                * 1000.0,
            slowest_execution_time_ms: self.statistics.worst_graph_execution.as_secs_f64() * 1000.0,
            total_executions: self.statistics.total_executions,
            nodes_by_performance,
        }
    }

    /// Create a serializable representation of this graph
    ///
    /// This method creates a SerializableProcessingGraph from a reference to this graph,
    /// allowing the original graph to remain in place. This is useful when you need
    /// to serialize the graph state without moving ownership.
    ///
    /// ### Returns
    ///
    /// A SerializableProcessingGraph containing the current state and statistics
    pub fn to_serializable(&self) -> SerializableProcessingGraph {
        let mut serializable_nodes = Vec::new();
        let mut validation_errors = Vec::new();

        // Convert nodes to serializable format
        for (node_id, node) in &self.nodes {
            let stored_parameters = self
                .node_parameters
                .get(node_id)
                .cloned()
                .unwrap_or_default();
            let node_statistics = self.statistics.node_statistics.get(node_id).cloned();
            let mut serializable_node = SerializableProcessingGraph::create_serializable_node(
                node.as_ref(),
                &stored_parameters,
            );
            serializable_node.statistics = node_statistics;
            serializable_nodes.push(serializable_node);
        }

        // Convert connections to serializable format
        let serializable_connections: Vec<SerializableConnection> = self
            .connections
            .iter()
            .map(|conn| SerializableConnection {
                from: conn.from.clone(),
                to: conn.to.clone(),
            })
            .collect();

        // Get execution order (this might fail if graph has cycles)
        let execution_order = match self.get_execution_order_immutable() {
            Ok(order) => order,
            Err(e) => {
                validation_errors.push(format!("Failed to determine execution order: {}", e));
                Vec::new()
            }
        };

        // Validate the graph and collect errors
        let is_valid = match self.validate() {
            Ok(()) => validation_errors.is_empty(),
            Err(e) => {
                validation_errors.push(format!("Graph validation failed: {}", e));
                false
            }
        };

        // Get performance summary
        let performance_summary = self.get_performance_summary();

        SerializableProcessingGraph {
            nodes: serializable_nodes,
            connections: serializable_connections,
            execution_order,
            input_node: self.input_node.clone(),
            output_nodes: self.output_nodes.clone(),
            statistics: self.statistics.clone(),
            performance_summary,
            is_valid,
            validation_errors,
        }
    }

    /// Update the graph structure information in statistics
    fn update_statistics_structure(&mut self) {
        self.statistics
            .update_graph_structure(self.node_count(), self.connection_count());
    }

    /// Update configuration for a specific node
    ///
    /// Attempts to update the configuration of a specific node in the graph.
    /// This method supports hot-reload for compatible parameters and indicates
    /// whether the node needs to be reconstructed for incompatible changes.
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
    /// * `Err(anyhow::Error)` - Node not found or configuration update failed
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::ProcessingGraph;
    /// use rust_photoacoustic::processing::nodes::{GainNode, ProcessingNode};
    /// use serde_json::json;
    ///
    /// let mut graph = ProcessingGraph::new();
    /// let gain_node = Box::new(GainNode::new("amp".to_string(), 0.0));
    /// graph.add_node(gain_node).unwrap();
    ///
    /// // Update gain parameter
    /// let result = graph.update_node_config("amp", &json!({"gain_db": 6.0}));
    /// assert!(result.unwrap()); // true = hot-reload successful
    /// ```
    pub fn update_node_config(
        &mut self,
        node_id: &str,
        parameters: &serde_json::Value,
    ) -> Result<bool> {
        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| ProcessingGraphError::NodeNotFound(node_id.to_string()))?;

        debug!(
            "Processing graph: Updating configuration for node '{}'",
            node_id
        );

        // Attempt to update the node's configuration
        let result = node.update_config(parameters)?;

        // Update stored parameters if the update was successful
        if result {
            // Update stored parameters for serialization
            if let serde_json::Value::Object(new_params) = parameters {
                let stored_params = self.node_parameters.entry(node_id.to_string()).or_default();
                for (key, value) in new_params {
                    stored_params.insert(key.clone(), value.clone());
                }
            }
            debug!(
                "Processing graph: Node '{}' configuration updated successfully",
                node_id
            );
        } else {
            debug!(
                "Processing graph: Node '{}' requires reconstruction for configuration change",
                node_id
            );
        }

        Ok(result)
    }

    /// Update configuration for multiple nodes
    ///
    /// Attempts to update the configuration for multiple nodes in the graph.
    /// Returns a map indicating
    /// and which require reconstruction.
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
    /// use rust_photoacoustic::processing::ProcessingGraph;
    /// use serde_json::json;
    /// use std::collections::HashMap;
    ///
    /// let mut graph = ProcessingGraph::new();
    /// // ... add nodes to graph ...
    ///
    /// let mut updates = HashMap::new();
    /// updates.insert("gain1".to_string(), json!({"gain_db": 6.0}));
    /// updates.insert("gain2".to_string(), json!({"gain_db": -3.0}));
    ///
    /// let results = graph.update_multiple_node_configs(&updates);
    /// ```
    pub fn update_multiple_node_configs(
        &mut self,
        node_configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, Result<bool>> {
        let mut results = std::collections::HashMap::new();

        for (node_id, parameters) in node_configs {
            let result = self.update_node_config(node_id, parameters);
            results.insert(node_id.clone(), result);
        }

        results
    }
}

/// Represents a connection between two nodes in serializable format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableConnection {
    pub from: NodeId,
    pub to: NodeId,
}

/// Represents a processing node in serializable format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableNode {
    pub id: NodeId,
    pub node_type: String,
    pub accepts_input_types: Vec<String>,
    pub output_type: String,
    pub parameters: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<NodeStatistics>,
    pub supports_hot_reload: bool,
}

/// Performance summary for the entire processing graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    // Original fields required by client
    pub throughput_fps: f64,
    pub efficiency_percentage: f64,
    pub slowest_node: Option<String>,
    pub fastest_node: Option<String>,

    // Additional fields for enhanced functionality
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub total_connections: usize,
    pub average_execution_time_ms: f64,
    pub fastest_execution_time_ms: f64,
    pub slowest_execution_time_ms: f64,
    pub total_executions: u64,
    pub nodes_by_performance: Vec<NodeStatistics>,
}

/// Serializable representation of the entire processing graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableProcessingGraph {
    pub nodes: Vec<SerializableNode>,
    pub connections: Vec<SerializableConnection>,
    pub execution_order: Vec<NodeId>,
    pub input_node: Option<NodeId>,
    pub output_nodes: Vec<NodeId>,
    pub statistics: ProcessingGraphStatistics,
    pub performance_summary: PerformanceSummary,
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
}

impl SerializableProcessingGraph {
    /// Create a serializable node from a processing node and its stored parameters
    pub fn create_serializable_node(
        node: &dyn ProcessingNode,
        stored_parameters: &HashMap<String, Value>,
    ) -> SerializableNode {
        SerializableNode {
            id: node.node_id().to_string(),
            node_type: node.node_type().to_string(),
            accepts_input_types: Self::get_accepts_input_types(node),
            output_type: Self::get_output_type(node),
            parameters: Value::Object(
                stored_parameters
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            ),
            config: None,     // Legacy field for backward compatibility
            statistics: None, // Will be populated from graph statistics if available
            supports_hot_reload: node.supports_hot_reload(),
        }
    }

    /// Get the accepted input types for a node
    fn get_accepts_input_types(node: &dyn ProcessingNode) -> Vec<String> {
        let mut input_types = Vec::new();

        // Create test data for each type to check acceptance
        let test_audio_frame = ProcessingData::AudioFrame(crate::acquisition::AudioFrame {
            channel_a: vec![0.0],
            channel_b: vec![0.0],
            sample_rate: 44100,
            timestamp: 0,
            frame_number: 0,
        });

        let test_single_channel = ProcessingData::SingleChannel {
            samples: vec![0.0],
            sample_rate: 44100,
            timestamp: 0,
            frame_number: 0,
        };

        let test_dual_channel = ProcessingData::DualChannel {
            channel_a: vec![0.0],
            channel_b: vec![0.0],
            sample_rate: 44100,
            timestamp: 0,
            frame_number: 0,
        };

        let test_photoacoustic_result = ProcessingData::PhotoacousticResult {
            signal: vec![0.0],
            metadata: crate::processing::nodes::data::ProcessingMetadata {
                original_frame_number: 0,
                original_timestamp: 0,
                sample_rate: 44100,
                processing_steps: vec![],
                processing_latency_us: 0,
            },
        };

        // Check which input types this node accepts
        if node.accepts_input(&test_audio_frame) {
            input_types.push("AudioFrame".to_string());
        }
        if node.accepts_input(&test_dual_channel) {
            input_types.push("DualChannel".to_string());
        }
        if node.accepts_input(&test_single_channel) {
            input_types.push("SingleChannel".to_string());
        }
        if node.accepts_input(&test_photoacoustic_result) {
            input_types.push("PhotoacousticResult".to_string());
        }

        input_types
    }

    /// Get the output type for a node
    fn get_output_type(node: &dyn ProcessingNode) -> String {
        // Try different input types to determine the most common output type
        let test_dual_channel = ProcessingData::DualChannel {
            channel_a: vec![0.0],
            channel_b: vec![0.0],
            sample_rate: 44100,
            timestamp: 0,
            frame_number: 0,
        };

        if let Some(output_type) = node.output_type(&test_dual_channel) {
            return output_type;
        }

        let test_audio_frame = ProcessingData::AudioFrame(crate::acquisition::AudioFrame {
            channel_a: vec![0.0],
            channel_b: vec![0.0],
            sample_rate: 44100,
            timestamp: 0,
            frame_number: 0,
        });

        if let Some(output_type) = node.output_type(&test_audio_frame) {
            return output_type;
        }

        let test_single_channel = ProcessingData::SingleChannel {
            samples: vec![0.0],
            sample_rate: 44100,
            timestamp: 0,
            frame_number: 0,
        };

        if let Some(output_type) = node.output_type(&test_single_channel) {
            return output_type;
        }

        // Default fallback
        "Unknown".to_string()
    }
}

impl JsonSchema for SerializableConnection {
    fn schema_name() -> String {
        "SerializableConnection".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        use schemars::schema::{InstanceType, Metadata, ObjectValidation, SchemaObject};
        use schemars::Map;

        let mut properties = Map::new();
        properties.insert(
            "from".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::String.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Source node ID".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "to".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::String.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Target node ID".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );

        Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            metadata: Some(Box::new(Metadata {
                title: Some("Connection between processing nodes".to_string()),
                description: Some(
                    "Represents a connection from one processing node to another".to_string(),
                ),
                ..Default::default()
            })),
            object: Some(Box::new(ObjectValidation {
                properties,
                required: ["from".to_string(), "to".to_string()].into_iter().collect(),
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}

impl JsonSchema for SerializableNode {
    fn schema_name() -> String {
        "SerializableNode".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        use schemars::schema::{InstanceType, Metadata, ObjectValidation, SchemaObject};
        use schemars::Map;

        let mut properties = Map::new();
        properties.insert(
            "id".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::String.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Unique node identifier".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "node_type".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::String.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Type of processing node".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "accepts_input_types".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Array.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("List of accepted input data types".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "output_type".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::String.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Expected output data type".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "parameters".to_string(),
            Schema::Object(SchemaObject {
                metadata: Some(Box::new(Metadata {
                    description: Some("Node configuration parameters".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "config".to_string(),
            Schema::Object(SchemaObject {
                metadata: Some(Box::new(Metadata {
                    description: Some("Node configuration (legacy)".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "statistics".to_string(),
            Schema::Object(SchemaObject {
                metadata: Some(Box::new(Metadata {
                    description: Some("Node performance statistics".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "supports_hot_reload".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Boolean.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some(
                        "Whether the node supports hot-reload configuration updates".to_string(),
                    ),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );

        Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            metadata: Some(Box::new(Metadata {
                title: Some("Processing node".to_string()),
                description: Some(
                    "Represents a processing node with its configuration and statistics"
                        .to_string(),
                ),
                ..Default::default()
            })),
            object: Some(Box::new(ObjectValidation {
                properties,
                required: [
                    "id".to_string(),
                    "node_type".to_string(),
                    "accepts_input_types".to_string(),
                    "output_type".to_string(),
                    "parameters".to_string(),
                    "supports_hot_reload".to_string(),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}

impl JsonSchema for PerformanceSummary {
    fn schema_name() -> String {
        "PerformanceSummary".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        use schemars::schema::{InstanceType, Metadata, ObjectValidation, SchemaObject};
        use schemars::Map;

        let mut properties = Map::new();

        // Original fields required by client
        properties.insert(
            "throughput_fps".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Number.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Processing throughput in frames per second".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "efficiency_percentage".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Number.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some(
                        "Efficiency percentage (0-100) based on fastest vs slowest execution"
                            .to_string(),
                    ),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "slowest_node".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::String.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some(
                        "ID of the node with the highest average processing time".to_string(),
                    ),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "fastest_node".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::String.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some(
                        "ID of the node with the lowest average processing time".to_string(),
                    ),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );

        // Additional fields for enhanced functionality
        properties.insert(
            "total_nodes".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Integer.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Total number of nodes in the graph".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "active_nodes".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Integer.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Number of currently active nodes".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "total_connections".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Integer.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Total number of connections between nodes".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "average_execution_time_ms".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Number.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Average execution time in milliseconds".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "fastest_execution_time_ms".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Number.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Fastest execution time in milliseconds".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "slowest_execution_time_ms".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Number.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Slowest execution time in milliseconds".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "total_executions".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Integer.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Total number of graph executions".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "nodes_by_performance".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Array.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("List of nodes sorted by performance".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );

        Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            metadata: Some(Box::new(Metadata {
                title: Some("Performance Summary".to_string()),
                description: Some("Summary of processing graph performance metrics".to_string()),
                ..Default::default()
            })),
            object: Some(Box::new(ObjectValidation {
                properties,
                required: [
                    "throughput_fps".to_string(),
                    "efficiency_percentage".to_string(),
                    "total_nodes".to_string(),
                    "active_nodes".to_string(),
                    "total_connections".to_string(),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}

impl JsonSchema for SerializableProcessingGraph {
    fn schema_name() -> String {
        "SerializableProcessingGraph".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        use schemars::schema::{InstanceType, Metadata, ObjectValidation, SchemaObject};
        use schemars::Map;

        let mut properties = Map::new();
        properties.insert(
            "nodes".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Array.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("List of processing nodes in the graph".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "connections".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Array.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("List of connections between nodes".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "execution_order".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Array.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Topological execution order of nodes".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "input_node".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::String.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("ID of the input node".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "output_nodes".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Array.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("List of output node IDs".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "is_valid".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Boolean.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("Whether the graph is valid".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );
        properties.insert(
            "validation_errors".to_string(),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::Array.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some("List of validation errors if any".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        );

        Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            metadata: Some(Box::new(Metadata {
                title: Some("Serializable Processing Graph".to_string()),
                description: Some("Complete serializable representation of a processing graph with nodes, connections and statistics".to_string()),
                ..Default::default()
            })),
            object: Some(Box::new(ObjectValidation {
                properties,
                required: ["nodes".to_string(), "connections".to_string(), "execution_order".to_string()].into_iter().collect(),
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}

impl fmt::Display for PerformanceSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Performance Summary:")?;

        // Original fields
        writeln!(f, "  Throughput: {:.1} FPS", self.throughput_fps)?;
        writeln!(f, "  Efficiency: {:.1}%", self.efficiency_percentage)?;

        if let Some(ref slowest) = self.slowest_node {
            writeln!(f, "  Slowest Node: {}", slowest)?;
        }

        if let Some(ref fastest) = self.fastest_node {
            writeln!(f, "  Fastest Node: {}", fastest)?;
        }

        // Additional fields
        writeln!(f, "  Total Nodes: {}", self.total_nodes)?;
        writeln!(f, "  Active Nodes: {}", self.active_nodes)?;
        writeln!(f, "  Total Connections: {}", self.total_connections)?;
        writeln!(f, "  Total Executions: {}", self.total_executions)?;
        writeln!(
            f,
            "  Average Execution Time: {:.2}ms",
            self.average_execution_time_ms
        )?;
        writeln!(
            f,
            "  Fastest Execution: {:.2}ms",
            self.fastest_execution_time_ms
        )?;
        writeln!(
            f,
            "  Slowest Execution: {:.2}ms",
            self.slowest_execution_time_ms
        )?;

        if !self.nodes_by_performance.is_empty() {
            writeln!(f, "  Node Performance (sorted by average time):")?;
            for (i, stats) in self.nodes_by_performance.iter().enumerate() {
                writeln!(
                    f,
                    "    {}. {}: {:.2}ms avg",
                    i + 1,
                    stats.node_id,
                    stats.average_processing_time.as_secs_f64() * 1000.0
                )?;
            }
        }

        Ok(())
    }
}
