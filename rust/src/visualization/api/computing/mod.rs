// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! routes for computing nodes
use crate::processing::computing_nodes::SharedComputingState;
use auth_macros::openapi_protect_get;
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket::{get, response::status, State};
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use rocket_okapi::JsonSchema;
use std::time::SystemTime;

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct ComputingResponse {
    pub peak_frequency: Option<f32>,
    pub peak_amplitude: Option<f32>,
    pub concentration_ppm: Option<f32>,
    pub polynomial_coefficients: [f64; 5],
}

/// Computing API endpoint that returns live data from SharedComputingState
#[openapi_protect_get("/api/computing", "read:api", tag = "Computing")]
pub async fn computing_api(
    computing_state: &State<SharedComputingState>,
) -> Json<ComputingResponse> {
    // Read from the shared computing state
    let shared_data = computing_state.read().await;

    let response = ComputingResponse {
        peak_frequency: shared_data.peak_frequency,
        peak_amplitude: shared_data.peak_amplitude,
        concentration_ppm: shared_data.concentration_ppm,
        polynomial_coefficients: shared_data.polynomial_coefficients,
    };
    Json(response)
}

/// Centralized function to get all computing routes with OpenAPI documentation
pub fn get_computing_routes() -> (Vec<rocket::Route>, OpenApi) {
    openapi_get_routes_spec![computing_api]
}
