// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Processing graph statistics API endpoint
//!
//! This module provides a protected endpoint for serving ProcessingGraphStatistics as JSON.
//! The endpoint uses JWT token protection via the protect_get macro and accesses real-time
//! statistics from the running ProcessingConsumer via SharedVisualizationState.

use log::info;
use rocket::serde::json::Json;
use rocket::{get, post, response::status, State};
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

use crate::config::processing::NodeConfig;
use crate::processing::graph::ProcessingGraphStatistics;
use crate::processing::SerializableProcessingGraph;
use crate::visualization::api::ConfigState;
use crate::visualization::shared_state::SharedVisualizationState;
use auth_macros::{openapi_protect_get, openapi_protect_post};

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
/// ### Authentication
///
/// This endpoint requires a valid JWT bearer token in the Authorization header.
/// The token must have the appropriate scope for API access.
///
/// ### Returns
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
/// ### Example Response
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
/// ### Error Responses
///
/// - `401 Unauthorized`: Missing or invalid JWT token
/// - `403 Forbidden`: Token lacks required scope
/// - `404 Not Found`: No processing is currently active
/// - `500 Internal Server Error`: Server error accessing statistics
#[openapi_protect_get("/api/graph-statistics", "read:api", tag = "Processing")]
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

/// Get processing graph information
///
/// **Endpoint:** `GET /api/graph`
///
/// Returns a JSON object representing the current processing graph structure
/// including nodes, connections, execution order, and topology information.
///
/// ### Authentication
///
/// This endpoint requires a valid JWT bearer token in the Authorization header.
/// The token must have the appropriate scope for API access.
///
/// ### Returns
///
/// Returns a JSON response containing `SerializableProcessingGraph` with:
/// - `nodes`: Array of all processing nodes with their configurations
/// - `connections`: Array of connections between nodes
/// - `execution_order`: Topological order of node execution
/// - `input_node`: ID of the designated input node
/// - `output_node`: ID of the designated output node
/// - `statistics`: Current performance statistics for the graph
///
/// ### Error Responses
///
/// - `401 Unauthorized`: Missing or invalid JWT token
/// - `403 Forbidden`: Token lacks required scope
/// - `404 Not Found`: No processing graph is currently available
/// - `500 Internal Server Error`: Server error accessing graph data
#[openapi_protect_get("/api/graph", "read:api", tag = "Processing")]
pub async fn get_graph(
    state: &State<SharedVisualizationState>,
) -> Result<Json<SerializableProcessingGraph>, status::NotFound<String>> {
    // Get the current processing graph from shared state
    match state.get_processing_graph().await {
        Some(graph) => Ok(Json(graph)),
        None => Err(status::NotFound(
            "No processing graph is currently available".to_string(),
        )),
    }
}

/// Post new node configuration
///
/// **Endpoint:** `POST /api/graph/config`
///
/// This endpoint allows updating the configuration of processing nodes that support hot-reloading.
/// The configuration changes are applied to the shared configuration state and will be automatically
/// detected by the background monitoring thread for hot-reload without requiring a restart.
///
/// ### Request Body
///
/// The request body must be a valid JSON object conforming to the `NodeConfig` schema:
///
/// ```json
/// {
///   "id": "node_id",
///   "node_type": "filter",
///   "parameters": {
///     "cutoff_frequency": 1000.0,
///     "filter_type": "lowpass"
///   }
/// }
/// ```
///
/// ### Validation Process
///
/// The endpoint performs the following validation steps in order:
///
/// 1. **Processing graph availability**: Verifies that a processing graph is currently loaded
/// 2. **Node existence**: Confirms the specified `node_id` exists in the active processing graph
/// 3. **Hot-reload support**: Validates that the target node supports hot-reloading via `supports_hot_reload` flag
/// 4. **Configuration presence**: Ensures the node exists in the shared configuration state
/// 5. **Parameter merging**: Intelligently merges new parameters with existing configuration:
///    - If both existing and new parameters are JSON objects: performs key-by-key merging
///    - If existing parameters are not an object: replaces entirely with new parameters
///    - If new parameters are not an object: replaces existing parameters entirely
///
/// ### Configuration Update Behavior
///
/// - **Merge strategy**: New parameter values overwrite existing ones with the same key
/// - **Preservation**: Existing parameters not specified in the request are preserved
/// - **Atomic update**: Configuration changes are applied atomically within a write lock
/// - **Hot-reload trigger**: Changes are automatically detected by the monitoring thread
///
/// ### Authentication
///
/// This endpoint requires a valid JWT bearer token in the Authorization header.
/// The token must have the `admin:api` scope for API access.
///
/// ### Returns
///
/// Returns a JSON response containing the complete updated node parameters after merging:
///
/// ```json
/// {
///   "cutoff_frequency": 1000.0,
///   "filter_type": "lowpass",
///   "existing_param": "preserved_value"
/// }
/// ```
///
/// ### Error Responses
///
/// - `400 Bad Request`:
///   - No processing graph is currently available
///   - Node with the specified ID does not exist in the processing graph
///   - Node does not support hot reloading
///   - Node not found in configuration state
///   - Invalid JSON structure in request body
/// - `401 Unauthorized`: Missing or invalid JWT token
/// - `403 Forbidden`: Token lacks required `admin:api` scope
/// - `500 Internal Server Error`: Server error processing the request or configuration lock failure
#[openapi_protect_post(
    "/api/graph/config",
    "admin:api",
    tag = "Processing",
    data = "<new_config>"
)]
pub async fn post_node_config(
    config: &ConfigState,
    shared_state: &State<SharedVisualizationState>,
    new_config: Json<NodeConfig>,
) -> Result<Json<serde_json::Value>, status::BadRequest<String>> {
    let node_id = new_config.id.clone();
    // Chain all validations using match expressions to avoid early returns
    match shared_state.get_processing_graph().await {
        Some(graph) => {
            match graph.nodes.iter().find(|node| node.id == node_id) {
                Some(serializable_node) => {
                    if serializable_node.supports_hot_reload {                        // Update the configuration in the shared config
                        let updated_params = {
                            let mut config_write = config.inner().write().await;

                            // Find the node configuration in the config
                            match config_write
                                .processing
                                .default_graph
                                .nodes
                                .iter_mut()
                                .find(|n| n.id == node_id)
                            {
                                Some(node_config) => {
                                    // Extract the parameters from the new NodeConfig
                                    let new_node_config = new_config.into_inner(); // Validate that id and node_type match the existing node
                                    if new_node_config.id != node_config.id {
                                        let err_msg = format!(
                                            "ID mismatch: request ID '{}' does not match existing node ID '{}'",
                                            new_node_config.id, node_config.id
                                        );
                                        return rocket::Either::Right(Err(status::BadRequest(
                                            err_msg,
                                        )));
                                    }
                                    if new_node_config.node_type != node_config.node_type {
                                        let err_msg = format!(
                                            "Node type mismatch: request node_type '{}' does not match existing node_type '{}'",
                                            new_node_config.node_type, node_config.node_type
                                        );
                                        return rocket::Either::Right(Err(status::BadRequest(
                                            err_msg,
                                        )));
                                    }

                                    let new_params = &new_node_config.parameters; // Validate parameter compatibility before merging
                                    let validation_result = if let Some(new_params_obj) =
                                        new_params.as_object()
                                    {
                                        if let Some(existing_params_obj) =
                                            node_config.parameters.as_object()
                                        {
                                            // Validate each new parameter
                                            let mut validation_errors = Vec::new();
                                            for (key, new_value) in new_params_obj {
                                                match existing_params_obj.get(key) {
                                                    Some(existing_value) => {
                                                        // Check type compatibility
                                                        if !are_json_types_compatible(
                                                            existing_value,
                                                            new_value,
                                                        ) {
                                                            validation_errors.push(format!(
                                                                "Parameter '{}' type mismatch. Expected: {}, Got: {}",
                                                                key,
                                                                get_json_type_name(existing_value),
                                                                get_json_type_name(new_value)
                                                            ));
                                                        }
                                                    }
                                                    None => {
                                                        validation_errors.push(format!(
                                                            "Parameter '{}' does not exist in node '{}' configuration",
                                                            key, node_id
                                                        ));
                                                    }
                                                }
                                            }
                                            if validation_errors.is_empty() {
                                                Ok(())
                                            } else {
                                                Err(validation_errors.join("; "))
                                            }
                                        } else {
                                            Ok(()) // If existing params is not an object, allow replacement
                                        }
                                    } else {
                                        Ok(()) // If new params is not an object, allow replacement
                                    };

                                    match validation_result {
                                        Ok(()) => {
                                            // Validation passed, proceed with merging
                                            if let Some(new_params_obj) = new_params.as_object() {
                                                if let Some(existing_params_obj) =
                                                    node_config.parameters.as_object_mut()
                                                {
                                                    // Update existing parameters with new values (already validated)
                                                    for (key, value) in new_params_obj {
                                                        existing_params_obj
                                                            .insert(key.clone(), value.clone());
                                                    }
                                                } else {
                                                    // If existing parameters is not an object, replace entirely
                                                    node_config.parameters = new_params.clone();
                                                }
                                            } else {
                                                // If new params is not an object, replace entirely
                                                node_config.parameters = new_params.clone();
                                            }

                                            // Log the configuration update
                                            info!(
                                                "Updated configuration for node '{}' via API. Hot-reload will be detected by monitoring thread.",
                                                node_id
                                            );

                                            // Return the updated parameters
                                            Ok(Json(node_config.parameters.clone()))
                                        }
                                        Err(validation_error) => {
                                            Err(status::BadRequest(validation_error))
                                        }
                                    }
                                }
                                None => Err(status::BadRequest(format!(
                                    "Node '{}' not found in configuration",
                                    node_id
                                ))),
                            }
                        };
                        updated_params
                    } else {
                        Err(status::BadRequest(format!(
                            "Node '{}' does not support hot reloading",
                            node_id
                        )))
                    }
                }
                None => Err(status::BadRequest(format!(
                    "Node with ID '{}' not found in processing graph",
                    node_id
                ))),
            }
        }
        None => Err(status::BadRequest(
            "No processing graph is currently available".to_string(),
        )),
    }
}

/// Check if two JSON values have compatible types
///
/// Compatible types are:
/// - Same primitive types (bool, string, number)
/// - Both objects (for nested parameter structures)
/// - Both arrays (for array parameters)
fn are_json_types_compatible(existing: &serde_json::Value, new: &serde_json::Value) -> bool {
    use serde_json::Value::*;
    match (existing, new) {
        (Bool(_), Bool(_)) => true,
        (Number(_), Number(_)) => true,
        (String(_), String(_)) => true,
        (Array(_), Array(_)) => true,
        (Object(_), Object(_)) => true,
        (Null, Null) => true,
        _ => false,
    }
}

/// Get a human-readable name for a JSON value type
fn get_json_type_name(value: &serde_json::Value) -> &'static str {
    use serde_json::Value::*;
    match value {
        Bool(_) => "boolean",
        Number(_) => "number",
        String(_) => "string",
        Array(_) => "array",
        Object(_) => "object",
        Null => "null",
    }
}

/// Centralized function to get all graph routes with OpenAPI documentation
pub fn get_graph_routes() -> (Vec<rocket::Route>, OpenApi) {
    openapi_get_routes_spec![get_graph_statistics, get_graph, post_node_config]
}
