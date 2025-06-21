// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! routes for computing nodes
use crate::processing::computing_nodes::{PeakResult, SharedComputingState};
use auth_macros::openapi_protect_get;
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket::{get, response::status, State};
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use rocket_okapi::JsonSchema;
use std::collections::HashMap;
use std::time::SystemTime;

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct PeakResultResponse {
    pub frequency: f32,
    pub amplitude: f32,
    pub concentration_ppm: Option<f32>,
    pub timestamp: SystemTime,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct ComputingResponse {
    /// Peak results from multiple nodes, keyed by node ID
    pub peak_results: HashMap<String, PeakResultResponse>,

    /// Legacy fields for backward compatibility
    pub peak_frequency: Option<f32>,
    pub peak_amplitude: Option<f32>,
    pub concentration_ppm: Option<f32>,
    pub polynomial_coefficients: [f64; 5],

    /// Node IDs that have recent data (within last 30 seconds)
    pub active_node_ids: Vec<String>,

    /// Most recent result across all nodes
    pub latest_result: Option<PeakResultResponse>,
}

/// Computing API endpoint that returns live data from SharedComputingState
#[openapi_protect_get("/api/computing", "read:api", tag = "Computing")]
pub async fn computing_api(
    computing_state: &State<SharedComputingState>,
) -> Json<ComputingResponse> {
    // Read from the shared computing state
    let shared_data = computing_state.read().await;

    // Convert peak results to response format
    let peak_results: HashMap<String, PeakResultResponse> = shared_data
        .peak_results
        .iter()
        .map(|(node_id, result)| {
            (
                node_id.clone(),
                PeakResultResponse {
                    frequency: result.frequency,
                    amplitude: result.amplitude,
                    concentration_ppm: result.concentration_ppm,
                    timestamp: result.timestamp,
                },
            )
        })
        .collect();

    // Find the most recent result
    let latest_result = shared_data
        .get_latest_peak_result()
        .map(|result| PeakResultResponse {
            frequency: result.frequency,
            amplitude: result.amplitude,
            concentration_ppm: result.concentration_ppm,
            timestamp: result.timestamp,
        });

    // Get active node IDs (nodes with recent data)
    let active_node_ids: Vec<String> = shared_data
        .peak_results
        .keys()
        .filter(|node_id| shared_data.has_recent_peak_data(node_id))
        .cloned()
        .collect();

    let response = ComputingResponse {
        peak_results,
        // Legacy fields for backward compatibility
        peak_frequency: shared_data.peak_frequency,
        peak_amplitude: shared_data.peak_amplitude,
        concentration_ppm: shared_data.concentration_ppm,
        polynomial_coefficients: shared_data.polynomial_coefficients,
        active_node_ids,
        latest_result,
    };

    Json(response)
}

/// Centralized function to get all computing routes with OpenAPI documentation
pub fn get_computing_routes() -> (Vec<rocket::Route>, OpenApi) {
    openapi_get_routes_spec![computing_api]
}
