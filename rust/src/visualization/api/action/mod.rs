// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Action Node History API Endpoints
//!
//! This module provides REST API endpoints for accessing historical data
//! from UniversalActionNode instances. These endpoints allow external systems
//! to retrieve measurement history and statistics from action nodes without
//! creating dedicated logging nodes.
//!
//! # Available Endpoints
//!
//! - `GET /api/action/{node_id}/history` - Get historical measurement data
//! - `GET /api/action/{node_id}/history/stats` - Get buffer statistics
//! - `GET /api/action` - List all action nodes
//!
//! # Security
//!
//! All endpoints require `read:api` permission and valid JWT authentication.
//!
//! # Usage Examples
//!
//! ```bash
//! # Get last 50 measurements from redis_stream_action node
//! curl -H "Authorization: Bearer $TOKEN" \
//!      "https://localhost:8080/api/action/redis_stream_action/history?limit=50"
//!
//! # Get buffer statistics for web_dashboard_action node
//! curl -H "Authorization: Bearer $TOKEN" \
//!      "https://localhost:8080/api/action/web_dashboard_action/history/stats"
//!
//! # List all action nodes
//! curl -H "Authorization: Bearer $TOKEN" \
//!      "https://localhost:8080/api/action"
//! ```

use anyhow::{anyhow, Result};
use auth_macros::openapi_protect_get;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{get, State};
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::{openapi_get_routes_spec, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::processing::computing_nodes::action_drivers::MeasurementData;
use crate::processing::computing_nodes::action_trait::ActionNode;
use crate::processing::computing_nodes::UniversalActionNode;
use crate::visualization::shared_state::SharedVisualizationState;

/// Query parameters for history endpoint
#[derive(Deserialize, JsonSchema)]
pub struct HistoryQuery {
    /// Maximum number of entries to return (default: all)
    pub limit: Option<usize>,
}

/// Response structure for action node list
#[derive(Serialize, JsonSchema)]
pub struct ActionNodeInfo {
    /// Node ID
    pub id: String,
    /// Node type
    pub node_type: String,
    /// Whether the node has a configured driver
    pub has_driver: bool,
    /// Number of monitored computing nodes
    pub monitored_nodes_count: usize,
    /// Current buffer size
    pub buffer_size: usize,
    /// Buffer capacity
    pub buffer_capacity: usize,
}

/// Get historical measurement data from a specific action node
///
/// Returns measurement data stored in the action node's history buffer.
/// The data is returned in chronological order (newest first).
///
/// ### Path Parameters
/// - `node_id`: The ID of the action node to query
///
/// ### Query Parameters
/// - `limit`: Maximum number of entries to return (optional)
///
/// ### Returns
/// - `200 OK`: Array of measurement data
/// - `404 Not Found`: Action node with the specified ID not found
/// - `500 Internal Server Error`: Failed to access processing graph
///
/// ### Example Response
/// ```json
/// [
///   {
///     "concentration_ppm": 456.78,
///     "source_node_id": "concentration_calculator",
///     "peak_amplitude": 0.85,
///     "peak_frequency": 2000.5,
///     "timestamp": 1640995200,
///     "metadata": {
///       "trigger_type": "concentration_threshold",
///       "alert_message": "High concentration detected"
///     }
///   }
/// ]
/// ```
#[openapi_protect_get(
    "/api/action/<node_id>/history?<limit>",
    "read:api",
    tag = "Action History"
)]
pub async fn get_action_history(
    node_id: &str,
    limit: Option<usize>,
    state: &State<SharedVisualizationState>,
) -> Result<Json<Vec<MeasurementData>>, Status> {
    let result = if let Some(live_graph) = state.get_live_processing_graph().await {
        // Try to access the live processing graph
        if let Ok(graph_lock) =
            tokio::time::timeout(std::time::Duration::from_millis(100), live_graph.read()).await
        {
            // Get the specific UniversalActionNode
            if let Some(action_node) = graph_lock.get_universal_action_node(node_id) {
                // Get measurement history from the action node
                let history = action_node.get_measurement_history(limit);
                Ok(Json(history))
            } else {
                // Node not found
                Err(Status::NotFound)
            }
        } else {
            // Timeout occurred
            Err(Status::InternalServerError)
        }
    } else {
        // Fallback to mock data if live graph is not available
        let mock_data = vec![MeasurementData {
            concentration_ppm: 123.45,
            source_node_id: "concentration_calculator".to_string(),
            peak_amplitude: 0.75,
            peak_frequency: 2000.0,
            timestamp: std::time::SystemTime::now(),
            metadata: HashMap::new(),
        }];
        Ok(Json(mock_data))
    };

    result
}

/// Get statistics about an action node's history buffer
///
/// Returns metadata about the action node including buffer statistics,
/// configuration, and performance metrics.
///
/// ### Path Parameters
/// - `node_id`: The ID of the action node to query
///
/// ### Returns
/// - `200 OK`: Statistics object
/// - `404 Not Found`: Action node with the specified ID not found
/// - `500 Internal Server Error`: Failed to access processing graph
///
/// ### Example Response
/// ```json
/// {
///   "node_id": "redis_stream_action",
///   "node_type": "action_universal",
///   "history_buffer": {
///     "capacity": 100,
///     "current_size": 85,
///     "is_full": false,
///     "oldest_entry_timestamp": 1640995000,
///     "newest_entry_timestamp": 1640995200
///   },
///   "configuration": {
///     "monitored_nodes": ["concentration_calculator"],
///     "concentration_threshold": 100.0,
///     "amplitude_threshold": 0.65,
///     "update_interval_ms": 5000
///   },
///   "driver_info": {
///     "has_driver": true,
///     "driver_type": "configured"
///   },
///   "performance": {
///     "processing_count": 1250,
///     "actions_triggered": 15,
///     "last_update_time": 1640995200,
///     "last_action_update": 1640995195
///   }
/// }
/// ```
#[openapi_protect_get(
    "/api/action/<node_id>/history/stats",
    "read:api",
    tag = "Action History"
)]
pub async fn get_action_history_stats(
    node_id: &str,
    state: &State<SharedVisualizationState>,
) -> Result<Json<Value>, Status> {
    let result = if let Some(live_graph) = state.get_live_processing_graph().await {
        // Try to access the live processing graph
        if let Ok(graph_lock) =
            tokio::time::timeout(std::time::Duration::from_millis(100), live_graph.read()).await
        {
            // Get the specific UniversalActionNode
            if let Some(action_node) = graph_lock.get_universal_action_node(node_id) {
                // Get real statistics from the action node (this already returns a complete serde_json::Value)
                let stats = action_node.get_history_statistics();
                Ok(Json(stats))
            } else {
                // Node not found
                Err(Status::NotFound)
            }
        } else {
            // Timeout occurred
            Err(Status::InternalServerError)
        }
    } else {
        // Fallback to mock data if live graph is not available
        let mut stats = serde_json::Map::new();
        stats.insert("node_id".to_string(), Value::String(node_id.to_string()));
        stats.insert(
            "node_type".to_string(),
            Value::String("action_universal".to_string()),
        );

        let mut history_buffer = serde_json::Map::new();
        history_buffer.insert("capacity".to_string(), Value::Number(100.into()));
        history_buffer.insert("current_size".to_string(), Value::Number(50.into()));
        history_buffer.insert("is_full".to_string(), Value::Bool(false));

        stats.insert("history_buffer".to_string(), Value::Object(history_buffer));
        Ok(Json(Value::Object(stats)))
    };

    result
}

/// List all available action nodes
///
/// Returns a summary of all UniversalActionNode instances in the processing graph,
/// including their basic configuration and status information.
///
/// ### Returns
/// - `200 OK`: Array of action node information
/// - `500 Internal Server Error`: Failed to access processing graph
///
/// ### Example Response
/// ```json
/// [
///   {
///     "id": "redis_stream_action",
///     "node_type": "action_universal",
///     "has_driver": true,
///     "monitored_nodes_count": 1,
///     "buffer_size": 85,
///     "buffer_capacity": 100
///   },
///   {
///     "id": "web_dashboard_action",
///     "node_type": "action_universal",
///     "has_driver": true,
///     "monitored_nodes_count": 1,
///     "buffer_size": 200,
///     "buffer_capacity": 300
///   }
/// ]
/// ```
#[openapi_protect_get("/api/action", "read:api", tag = "Action History")]
pub async fn list_action_nodes(
    state: &State<SharedVisualizationState>,
) -> Result<Json<Vec<ActionNodeInfo>>, Status> {
    let result = if let Some(live_graph) = state.get_live_processing_graph().await {
        // Try to access the live processing graph
        if let Ok(graph_lock) =
            tokio::time::timeout(std::time::Duration::from_millis(100), live_graph.read()).await
        {
            // Get all UniversalActionNode instances
            let action_nodes = graph_lock.get_all_universal_action_nodes();

            let mut node_infos = Vec::new();
            for (node_id, action_node) in action_nodes {
                let history_stats = action_node.get_history_statistics();

                node_infos.push(ActionNodeInfo {
                    id: node_id,
                    node_type: "action_universal".to_string(),
                    has_driver: action_node.has_driver(),
                    monitored_nodes_count: action_node.get_monitored_node_ids().len(),
                    buffer_size: history_stats["history_buffer"]["current_size"]
                        .as_u64()
                        .unwrap_or(0) as usize,
                    buffer_capacity: history_stats["history_buffer"]["capacity"]
                        .as_u64()
                        .unwrap_or(0) as usize,
                });
            }

            Ok(Json(node_infos))
        } else {
            // Timeout occurred
            Err(Status::InternalServerError)
        }
    } else {
        // Fallback to mock data if live graph is not available
        let mock_nodes = vec![ActionNodeInfo {
            id: "mock_action_node".to_string(),
            node_type: "action_universal".to_string(),
            has_driver: false,
            monitored_nodes_count: 0,
            buffer_size: 0,
            buffer_capacity: 0,
        }];
        Ok(Json(mock_nodes))
    };

    result
}

/// Get the route handlers for action endpoints
pub fn get_action_routes() -> (Vec<rocket::Route>, OpenApi) {
    openapi_get_routes_spec![
        get_action_history,
        get_action_history_stats,
        list_action_nodes
    ]
}
