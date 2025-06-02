// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Processing graph statistics API endpoint
//!
//! This module provides a protected endpoint for serving ProcessingGraphStatistics as JSON.
//! The endpoint uses JWT token protection via the protect_get macro.

use rocket::serde::json::Json;
use std::collections::HashMap;
use std::time::Duration;

use crate::processing::graph::{NodeStatistics, ProcessingGraphStatistics};
use auth_macros::protect_get;

/// Get processing graph statistics
///
/// **Endpoint:** `GET /api/graph-statistics`
///
/// Returns comprehensive statistics about the processing graph including:
/// - Overall graph performance metrics
/// - Individual node statistics
/// - Execution timing information
/// - Graph structure details
///
/// # Authentication
///
/// This endpoint requires a valid JWT bearer token in the Authorization header.
/// The token must have the appropriate scope for API access.
///
/// # Returns
///
/// Returns a JSON response containing `ProcessingGraphStatistics` with:
/// - `node_statistics`: HashMap of node ID to performance statistics
/// - `total_executions`: Total number of graph executions
/// - `total_graph_processing_time`: Cumulative processing time
/// - `average_graph_processing_time`: Average time per execution
/// - `fastest_graph_execution`: Fastest recorded execution
/// - `worst_graph_execution`: Slowest recorded execution
/// - `active_nodes`: Number of active nodes in the graph
/// - `connections_count`: Number of connections between nodes
///
/// # Example Response
///
/// ```json
/// {
///   "node_statistics": {
///     "input": {
///       "node_id": "input",
///       "node_type": "input",
///       "frames_processed": 1000,
///       "total_processing_time": "PT1.234S",
///       "average_processing_time": "PT0.001234S",
///       "fastest_processing_time": "PT0.0008S",
///       "worst_processing_time": "PT0.002S"
///     }
///   },
///   "total_executions": 1000,
///   "total_graph_processing_time": "PT5.678S",
///   "average_graph_processing_time": "PT0.005678S",
///   "fastest_graph_execution": "PT0.003S",
///   "worst_graph_execution": "PT0.012S",
///   "active_nodes": 4,
///   "connections_count": 3
/// }
/// ```
///
/// # Error Responses
///
/// - `401 Unauthorized`: Missing or invalid JWT token
/// - `403 Forbidden`: Token lacks required scope
/// - `500 Internal Server Error`: Server error accessing statistics
///
/// # Future Enhancement
///
/// Currently returns sample statistics. In a future version, this will access
/// a global ProcessingGraph instance managed through Rocket state or a dedicated
/// service layer.
#[protect_get("/graph-statistics", "read:api")]
pub async fn get_graph_statistics() -> Json<ProcessingGraphStatistics> {
    // TODO: Replace with actual ProcessingGraph access once global graph management is implemented
    // This could be done by:
    // 1. Adding ProcessingGraph to OxideState or separate state management
    // 2. Creating a ProcessingGraphService that manages graph instances
    // 3. Using a singleton pattern for graph access
    //
    // For now, return sample statistics that demonstrate the API structure

    Json(create_sample_statistics())
}

/// Create sample processing graph statistics for demonstration
///
/// This function generates realistic sample data that shows what the actual
/// statistics would look like. The data includes multiple nodes with varying
/// performance characteristics to demonstrate the full API capability.
fn create_sample_statistics() -> ProcessingGraphStatistics {
    let mut node_statistics = HashMap::new();

    // Sample input node statistics
    let input_stats = NodeStatistics {
        node_id: "input".to_string(),
        node_type: "input".to_string(),
        frames_processed: 1000,
        total_processing_time: Duration::from_millis(1234),
        average_processing_time: Duration::from_micros(1234),
        fastest_processing_time: Duration::from_micros(800),
        worst_processing_time: Duration::from_millis(2),
        last_update: None, // Not serialized
    };
    node_statistics.insert("input".to_string(), input_stats);

    // Sample channel selector node statistics
    let channel_selector_stats = NodeStatistics {
        node_id: "channel_selector".to_string(),
        node_type: "channel_selector".to_string(),
        frames_processed: 1000,
        total_processing_time: Duration::from_millis(890),
        average_processing_time: Duration::from_micros(890),
        fastest_processing_time: Duration::from_micros(650),
        worst_processing_time: Duration::from_micros(1200),
        last_update: None,
    };
    node_statistics.insert("channel_selector".to_string(), channel_selector_stats);

    // Sample filter node statistics (slightly slower, more variable)
    let filter_stats = NodeStatistics {
        node_id: "bandpass_filter".to_string(),
        node_type: "filter".to_string(),
        frames_processed: 1000,
        total_processing_time: Duration::from_millis(2567),
        average_processing_time: Duration::from_micros(2567),
        fastest_processing_time: Duration::from_millis(2),
        worst_processing_time: Duration::from_millis(4),
        last_update: None,
    };
    node_statistics.insert("bandpass_filter".to_string(), filter_stats);

    // Sample output node statistics
    let output_stats = NodeStatistics {
        node_id: "output".to_string(),
        node_type: "photoacoustic_output".to_string(),
        frames_processed: 1000,
        total_processing_time: Duration::from_millis(445),
        average_processing_time: Duration::from_micros(445),
        fastest_processing_time: Duration::from_micros(300),
        worst_processing_time: Duration::from_micros(600),
        last_update: None,
    };
    node_statistics.insert("output".to_string(), output_stats);

    ProcessingGraphStatistics {
        node_statistics,
        total_executions: 1000,
        total_graph_processing_time: Duration::from_millis(5678),
        average_graph_processing_time: Duration::from_micros(5678),
        fastest_graph_execution: Duration::from_millis(3),
        worst_graph_execution: Duration::from_millis(12),
        active_nodes: 4,
        connections_count: 3,
        graph_created_at: None, // Not serialized
        last_execution: None,   // Not serialized
    }
}
