// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Processing graph statistics API endpoint
//!
//! This module provides a protected endpoint for serving ProcessingGraphStatistics as JSON.
//! The endpoint uses JWT token protection via the protect_get macro and accesses real-time
//! statistics from the running ProcessingConsumer via SharedVisualizationState.

use rocket::serde::json::Json;
use rocket::{response::status, State};

use crate::processing::graph::ProcessingGraphStatistics;
use crate::visualization::shared_state::SharedVisualizationState;
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
/// - `404 Not Found`: No processing is currently active
/// - `500 Internal Server Error`: Server error accessing statistics
#[protect_get("/graph-statistics", "read:api")]
pub async fn get_graph_statistics(
    state: &State<SharedVisualizationState>,
) -> Result<Json<ProcessingGraphStatistics>, status::NotFound<String>> {
    // Get the current processing statistics from shared state
    match state.get_processing_statistics().await {
        Some(statistics) => Ok(Json(statistics)),
        None => Err(status::NotFound(
            "No processing is currently active or no statistics available".to_string(),
        )),
    }
}
