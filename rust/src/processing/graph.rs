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
use crate::processing::nodes::{
    ChannelMixerNode, ChannelSelectorNode, ChannelTarget, DifferentialNode, FilterNode, InputNode,
    MixStrategy, NodeId, PhotoacousticOutputNode, ProcessingData, ProcessingNode, RecordNode,
    StreamingNode, StreamingNodeRegistry,
};
use anyhow::Result;
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::time::{Duration, Instant};
use thiserror::Error;
use uuid::Uuid;

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
        }
    }

    /// Add a processing node to the graph
    pub fn add_node(&mut self, node: Box<dyn ProcessingNode>) -> Result<()> {
        let node_id = node.node_id().to_string();
        let node_type = node.node_type().to_string();

        if self.nodes.contains_key(&node_id) {
            anyhow::bail!("Node '{}' already exists", node_id);
        }

        // If this is an input node, set it as the input
        if node.node_type() == "input" {
            self.input_node = Some(node_id.clone());
        }

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

    /// Create a new processing graph from configuration with optional streaming registry
    pub fn from_config_with_registry(
        config: &ProcessingGraphConfig,
        streaming_registry: Option<StreamingNodeRegistry>,
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
            let node = Self::create_node_from_config(node_config, &streaming_registry)?;
            graph.add_node(node)?;
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
    ) -> Result<Box<dyn ProcessingNode>> {
        match config.node_type.as_str() {
            "input" => Ok(Box::new(InputNode::new(config.id.clone()))),
            "channel_selector" => {
                // Extract target_channel parameter
                let target_channel = if let Some(params) = config.parameters.as_mapping() {
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
                let mix_strategy = if let Some(params) = config.parameters.as_mapping() {
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
                    .as_mapping()
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

                        let filter = BandpassFilter::new(center_freq, bandwidth);
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

                        let filter = LowpassFilter::new(cutoff_freq);
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

                        let filter = HighpassFilter::new(cutoff_freq);
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

                if let Some(params) = config.parameters.as_mapping() {
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
                    .as_mapping()
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
                let name = if let Some(params) = config.parameters.as_mapping() {
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
        PerformanceSummary {
            total_nodes: self.node_count(),
            total_connections: self.connection_count(),
            total_executions: self.statistics.total_executions,
            average_execution_time_ms: self.statistics.average_graph_processing_time.as_secs_f64()
                * 1000.0,
            throughput_fps: self.statistics.get_throughput_fps(),
            efficiency_percentage: self.statistics.get_efficiency_percentage(),
            slowest_node: self
                .statistics
                .get_slowest_node()
                .map(|stats| stats.node_id.clone()),
            fastest_node: self
                .statistics
                .get_fastest_node()
                .map(|stats| stats.node_id.clone()),
        }
    }

    /// Update the graph structure information in statistics
    fn update_statistics_structure(&mut self) {
        self.statistics
            .update_graph_structure(self.node_count(), self.connection_count());
    }
}

/// Summary of performance metrics for easy access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub total_nodes: usize,
    pub total_connections: usize,
    pub total_executions: u64,
    pub average_execution_time_ms: f64,
    pub throughput_fps: f64,
    pub efficiency_percentage: f64,
    pub slowest_node: Option<String>,
    pub fastest_node: Option<String>,
}

impl fmt::Display for PerformanceSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Graph: {} nodes, {} connections | Perf: {:.2}ms avg, {:.1} FPS, {:.1}% efficiency",
            self.total_nodes,
            self.total_connections,
            self.average_execution_time_ms,
            self.throughput_fps,
            self.efficiency_percentage
        )
    }
}
