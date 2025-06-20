// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Thermal data retrieval API for photoacoustic applications
//! This module provides an API for retrieving thermal data from the SharedThermalRegulationState

use crate::thermal_regulation::shared_state::{SharedThermalRegulationState, SharedThermalState};
use auth_macros::openapi_protect_get;
use rocket::get;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

#[openapi_protect_get("/api/thermal", "read:api", tag = "Thermal Regulation")]
pub async fn get_thermal_data(
    state: &rocket::State<SharedThermalState>,
) -> Result<rocket::serde::json::Json<SharedThermalRegulationState>, rocket::http::Status> {
    // Retrieve the current thermal regulation state
    let thermal_state = state.read().await.clone();

    // Return the thermal state as JSON
    Ok(rocket::serde::json::Json(thermal_state))
}

/// Centralized function to get all thermal routes with OpenAPI documentation
pub fn get_thermal_routes() -> (Vec<rocket::Route>, OpenApi) {
    openapi_get_routes_spec![get_thermal_data]
}
