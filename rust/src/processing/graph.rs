// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Processing graph management
//!
//! This module manages the processing graph structure, connections between nodes,
//! and graph execution logic.

use crate::config::processing::{NodeConfig, ProcessingGraphConfig};
use crate::preprocessing::differential::SimpleDifferential;
use crate::preprocessing::filters::{BandpassFilter, LowpassFilter};
use crate::processing::nodes::{
    ChannelMixerNode, ChannelSelectorNode, ChannelTarget, DifferentialNode, FilterNode, InputNode,
    MixStrategy, NodeId, PhotoacousticOutputNode, ProcessingData, ProcessingNode,
};
use anyhow::Result;
use std::collections::{HashMap, VecDeque};
use thiserror::Error;

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
        }
    }

    /// Add a processing node to the graph
    pub fn add_node(&mut self, node: Box<dyn ProcessingNode>) -> Result<()> {
        let node_id = node.node_id().to_string();

        if self.nodes.contains_key(&node_id) {
            anyhow::bail!("Node '{}' already exists", node_id);
        }

        // If this is an input node, set it as the input
        if node.node_type() == "input" {
            self.input_node = Some(node_id.clone());
        }

        self.nodes.insert(node_id, node);
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

        // Clear input node if it was removed
        if self.input_node.as_ref() == Some(&node_id.to_string()) {
            self.input_node = None;
        }

        // Remove from output nodes
        self.output_nodes.retain(|id| id != node_id);

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

            node_outputs.insert(node_id.clone(), output);
        }

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
        let mut graph = Self::new();

        // First, create all nodes
        for node_config in &config.nodes {
            let node = Self::create_node_from_config(node_config)?;
            graph.add_node(node)?;
        }

        // Then, create all connections
        for connection_config in &config.connections {
            graph.connect(&connection_config.from, &connection_config.to)?;
        }

        // Set output node if specified
        if let Some(ref output_id) = config.output_node {
            graph.set_output_node(output_id);
        }

        Ok(graph)
    }

    /// Create a processing node from configuration
    fn create_node_from_config(config: &NodeConfig) -> Result<Box<dyn ProcessingNode>> {
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
}

impl Default for ProcessingGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processing::nodes::{ChannelSelectorNode, ChannelTarget, InputNode};

    #[test]
    fn test_graph_creation() {
        let graph = ProcessingGraph::new();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.connection_count(), 0);
    }

    #[test]
    fn test_add_node() {
        let mut graph = ProcessingGraph::new();
        let input_node = Box::new(InputNode::new("input".to_string()));

        graph.add_node(input_node).unwrap();
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.input_node, Some("input".to_string()));
    }

    #[test]
    fn test_connect_nodes() {
        let mut graph = ProcessingGraph::new();

        let input_node = Box::new(InputNode::new("input".to_string()));
        let selector_node = Box::new(ChannelSelectorNode::new(
            "selector".to_string(),
            ChannelTarget::ChannelA,
        ));

        graph.add_node(input_node).unwrap();
        graph.add_node(selector_node).unwrap();

        graph.connect("input", "selector").unwrap();
        assert_eq!(graph.connection_count(), 1);
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = ProcessingGraph::new();

        let input_node = Box::new(InputNode::new("input".to_string()));
        let selector_node = Box::new(ChannelSelectorNode::new(
            "selector".to_string(),
            ChannelTarget::ChannelA,
        ));

        graph.add_node(input_node).unwrap();
        graph.add_node(selector_node).unwrap();

        graph.connect("input", "selector").unwrap();

        // This should fail due to cycle
        assert!(graph.connect("selector", "input").is_err());
    }

    #[test]
    fn test_topological_sort() {
        let mut graph = ProcessingGraph::new();

        let input_node = Box::new(InputNode::new("input".to_string()));
        let selector_node = Box::new(ChannelSelectorNode::new(
            "selector".to_string(),
            ChannelTarget::ChannelA,
        ));

        graph.add_node(input_node).unwrap();
        graph.add_node(selector_node).unwrap();
        graph.connect("input", "selector").unwrap();

        let order = graph.get_execution_order().unwrap();
        assert_eq!(order, vec!["input".to_string(), "selector".to_string()]);
    }
}
