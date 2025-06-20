// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Thermal data retrieval API for photoacoustic applications
//! This module provides an API for retrieving thermal data from the SharedThermalRegulationState

use crate::thermal_regulation::shared_state::{
    RegulatorStatus, SharedThermalRegulationState, SharedThermalState, ThermalDataPoint,
    ThermalRegulatorHistory,
};
use auth_macros::openapi_protect_get;
use rocket::get;
use rocket::response::status;
use rocket::serde::{Deserialize, Serialize};
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use schemars::JsonSchema;
use std::collections::HashMap;

/// Current temperature information for a thermal regulator
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CurrentTemperatureInfo {
    /// Current temperature reading in degrees Celsius
    pub temperature_celsius: f64,
    /// Timestamp of the temperature reading (Unix seconds)
    pub timestamp: u64,
    /// Current setpoint temperature in degrees Celsius
    pub setpoint_celsius: f64,
    /// Current output power of the regulator
    pub control_output_percent: f64,
    /// Current status of the regulator
    pub status: String,
}

/// Paginated thermal data response
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaginatedThermalResponse {
    /// Filtered thermal regulation data
    pub data: HashMap<String, Vec<ThermalDataPoint>>,
    /// Pagination metadata
    pub pagination: PaginationInfo,
    /// Applied filters summary
    pub filters: FilterSummary,
}

/// Pagination information for thermal data responses
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaginationInfo {
    /// Current page number (1-indexed)
    pub page: u32,
    /// Number of items per page
    pub limit: u32,
    /// Total number of items across all pages
    pub total_items: u32,
    /// Total number of pages available
    pub total_pages: u32,
    /// Whether there is a next page available
    pub has_next: bool,
    /// Whether there is a previous page available
    pub has_previous: bool,
}

/// Summary of applied filters
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FilterSummary {
    /// Step size in seconds between data points
    pub step_seconds: u32,
    /// List of regulator IDs included in the response
    pub included_regulators: Vec<String>,
    /// Start timestamp (Unix seconds) for data range
    pub from_timestamp: Option<u64>,
    /// End timestamp (Unix seconds) for data range
    pub to_timestamp: Option<u64>,
}

/// Get the list of available thermal regulators
///
/// **Endpoint:** `GET /api/thermal/regulators`
///
/// Returns a list of all thermal regulator identifiers that are currently
/// configured in the system. This list can be used to filter thermal data
/// requests to specific regulators.
///
/// ### Authentication
///
/// This endpoint requires a valid JWT bearer token in the Authorization header
/// with read access privileges. The token must have the `read:api` scope.
///
/// ### Returns
///
/// Returns a JSON array containing the string identifiers of all available
/// thermal regulators in the system.
///
/// ### Response Structure
///
/// ```json
/// [
///   "regulator_1",
///   "regulator_2",
///   "main_chamber_heater",
///   "coolant_system"
/// ]
/// ```
///
/// ### Error Responses
///
/// - `401 Unauthorized`: Missing or invalid JWT token
/// - `403 Forbidden`: Token lacks required `read:api` scope
/// - `500 Internal Server Error`: Server error accessing thermal regulation state
#[openapi_protect_get("/api/thermal/regulators", "read:api", tag = "Thermal Regulation")]
pub async fn get_thermal_regulators(
    state: &rocket::State<SharedThermalState>,
) -> Result<rocket::serde::json::Json<Vec<String>>, status::NotFound<String>> {
    // Retrieve the current thermal state
    let thermal_state = state.read().await;

    // Extract the list of regulator names
    let regulators = thermal_state.get_regulator_ids();

    // Return the list of regulators as JSON
    Ok(rocket::serde::json::Json(regulators))
}

/// Get the last recorded temperatures for all thermal regulators
///
/// **Endpoint:** `GET /api/thermal/temperatures`
///
/// Returns the most recent temperature reading for each thermal regulator in the system.
/// This provides a quick snapshot of the current thermal state without historical data.
///
/// ### Authentication
///
/// This endpoint requires a valid JWT bearer token in the Authorization header
/// with read access privileges. The token must have the `read:api` scope.
///
/// ### Returns
///
/// Returns a JSON object containing the regulator ID as the key and current temperature
/// information as the value. Each regulator entry includes:
/// - Current temperature in degrees Celsius
/// - Timestamp of the last reading (Unix seconds)
/// - Current setpoint temperature
/// - Regulator status
///
/// ### Response Structure
///
/// ```json
/// {
///   "regulator_1": {
///     "temperature_celsius": 23.5,
///     "timestamp": 1672531260,
///     "setpoint_celsius": 25.0,
///     "status": "Running"
///   },
///   "main_chamber_heater": {
///     "temperature_celsius": 45.2,
///     "timestamp": 1672531265,
///     "setpoint_celsius": 50.0,
///     "status": "Running"
///   }
/// }
/// ```
///
/// ### Error Responses
///
/// - `401 Unauthorized`: Missing or invalid JWT token
/// - `403 Forbidden`: Token lacks required `read:api` scope
/// - `500 Internal Server Error`: Server error accessing thermal regulation state
///
/// ### Notes
///
/// - If a regulator has no temperature readings, it will not appear in the response
/// - The timestamp indicates when the temperature reading was taken
/// - Status values include: "Uninitialized", "Initializing", "Running", "Error", "Stopped"
#[openapi_protect_get("/api/thermal/temperatures", "read:api", tag = "Thermal Regulation")]
pub async fn get_last_temperatures(
    state: &rocket::State<SharedThermalState>,
) -> rocket::serde::json::Json<HashMap<String, CurrentTemperatureInfo>> {
    // Retrieve the current thermal state
    let thermal_state = state.read().await;

    let mut temperature_data = HashMap::new();

    // Iterate through all regulators and get their latest temperature readings
    for (regulator_id, regulator_history) in &thermal_state.regulators {
        // Get the most recent temperature reading if available
        if let Some(latest_data_point) = regulator_history.history.back() {
            let temp_info = CurrentTemperatureInfo {
                temperature_celsius: latest_data_point.temperature_celsius,
                timestamp: latest_data_point.timestamp,
                setpoint_celsius: latest_data_point.setpoint_celsius,
                control_output_percent: latest_data_point.control_output_percent,
                status: regulator_status_to_string(&regulator_history.status),
            };
            temperature_data.insert(regulator_id.clone(), temp_info);
        }
    }

    rocket::serde::json::Json(temperature_data)
}
/// Get thermal regulation data with filtering and pagination
///
/// **Endpoint:** `GET /api/thermal`
///
/// Returns historical thermal regulation data with optional filtering by time range,
/// regulators, and data sampling. The response includes pagination support for
/// efficient handling of large datasets.
///
/// This endpoint provides access to:
/// - Historical temperature measurements from all thermal regulators
/// - PID controller output values and setpoints
/// - Individual PID component values (P, I, D terms) for control analysis
/// - Configurable data sampling intervals to reduce response size
/// - Time-range filtering for specific analysis periods
/// - Regulator-specific data filtering
///
/// ### Query Parameters
///
/// All query parameters are optional and can be combined to customize the response:
///
/// - `steps` - Time interval in seconds between returned data points
///   - Default: 60 seconds (1 minute intervals)
///   - Set to 0 to return all available data points
///   - Higher values reduce response size by sampling data
///
/// - `regulators` - Array of regulator IDs to include in response
///   - Default: All regulators included
///   - Use `/api/thermal/regulators` to get available regulator IDs
///   - Example: `?regulators=main_heater&regulators=coolant_pump`
///
/// - `from` - Start timestamp for data range (Unix seconds or ISO 8601)
///   - Default: Beginning of available data
///   - Example: `?from=1672531200` or `?from=2023-01-01T00:00:00Z`
///
/// - `to` - End timestamp for data range (Unix seconds or ISO 8601)
///   - Default: Most recent data
///   - Example: `?to=1672617600` or `?to=2023-01-02T00:00:00Z`
///
/// - `page` - Page number for pagination (1-indexed)
///   - Default: 1
///   - Used with `limit` parameter for result pagination
///
/// - `limit` - Maximum number of data points per regulator per page
///   - Default: 1000
///   - Maximum: 10000 to prevent excessive response sizes
///
/// ### Authentication
///
/// This endpoint requires a valid JWT bearer token in the Authorization header
/// with read access privileges. The token must have the `read:api` scope.
///
/// ### Response Structure
///
/// Returns a JSON response with the following structure:
///
/// ```json
/// {
///   "data": {
///     "regulator_1": [
///       {
///         "timestamp": 1672531260,
///         "temperature_celsius": 23.5,
///         "control_output_percent": 15.2,
///         "setpoint_celsius": 25.0,
///         "pid_components": {
///           "proportional": 1.5,
///           "integral": 0.3,
///           "derivative": -0.1,
///           "error": 1.5
///         }
///       }
///     ]
///   },
///   "pagination": {
///     "page": 1,
///     "limit": 1000,
///     "total_items": 2500,
///     "total_pages": 3,
///     "has_next": true,
///     "has_previous": false
///   },
///   "filters": {
///     "step_seconds": 60,
///     "included_regulators": ["regulator_1", "regulator_2"],
///     "from_timestamp": 1672531200,
///     "to_timestamp": 1672617600
///   }
/// }
/// ```
///
/// ### Data Sampling and Performance
///
/// For optimal performance with large datasets:
/// - Use the `steps` parameter to reduce data density
/// - Apply time range filtering with `from` and `to` parameters
/// - Use pagination for datasets exceeding 1000 points per regulator
/// - Consider filtering to specific regulators when only subset analysis is needed
///
/// ### Error Responses
///
/// - `400 Bad Request`: Invalid query parameters (e.g., invalid timestamp format)
/// - `401 Unauthorized`: Missing or invalid JWT token
/// - `403 Forbidden`: Token lacks required `read:api` scope
/// - `422 Unprocessable Entity`: Invalid parameter values (e.g., page < 1, limit > 10000)
/// - `500 Internal Server Error`: Server error accessing thermal regulation data
///
/// ### Examples
///
/// Get last hour of data with 5-minute intervals:
/// ```
/// GET /api/thermal?steps=300&from=1672531200&to=1672534800
/// ```
///
/// Get specific regulator data with pagination:
/// ```
/// GET /api/thermal?regulators=main_heater&page=2&limit=500
/// ```
///
/// Get all available data for analysis:
/// ```
/// GET /api/thermal?steps=0&limit=10000
/// ```
#[openapi_protect_get(
    "/api/thermal?<steps>&<regulators>&<from>&<to>&<page>&<limit>",
    "read:api",
    tag = "Thermal Regulation"
)]
pub async fn get_thermal_data(
    steps: Option<u32>,
    regulators: Option<Vec<String>>,
    from: Option<String>,
    to: Option<String>,
    page: Option<u32>,
    limit: Option<u32>,
    state: &rocket::State<SharedThermalState>,
) -> rocket::serde::json::Json<PaginatedThermalResponse> {
    // Validate and set default parameters
    let step_seconds = steps.unwrap_or(60); // Default to 1-minute intervals
    let page_num = page.unwrap_or(1).max(1); // Ensure page >= 1
    let page_limit = limit.unwrap_or(1000).min(10000); // Cap at 10000 items per page

    // Parse timestamp parameters with error handling that doesn't use early returns
    let (from_timestamp, to_timestamp, parse_errors) = {
        let mut errors = Vec::new();

        let from_ts = match from {
            Some(ts_str) => match ts_str.parse::<u64>() {
                Ok(ts) => Some(ts),
                Err(_) => {
                    errors.push("Invalid 'from' timestamp format".to_string());
                    None
                }
            },
            None => None,
        };

        let to_ts = match to {
            Some(ts_str) => match ts_str.parse::<u64>() {
                Ok(ts) => Some(ts),
                Err(_) => {
                    errors.push("Invalid 'to' timestamp format".to_string());
                    None
                }
            },
            None => None,
        };

        // Validate time range
        if let (Some(start), Some(end)) = (from_ts, to_ts) {
            if start > end {
                errors.push("'from' timestamp must be before 'to' timestamp".to_string());
            }
        }

        (from_ts, to_ts, errors)
    };

    // If there are parsing errors, return error response
    let response = if !parse_errors.is_empty() {
        PaginatedThermalResponse {
            data: HashMap::new(),
            pagination: PaginationInfo {
                page: page_num,
                limit: page_limit,
                total_items: 0,
                total_pages: 0,
                has_next: false,
                has_previous: false,
            },
            filters: FilterSummary {
                step_seconds: step_seconds,
                included_regulators: regulators.unwrap_or_default(),
                from_timestamp,
                to_timestamp,
            },
        }
    } else {
        // Access thermal state
        let thermal_state = state.read().await;

        // Get available regulator IDs
        let available_regulator_ids = thermal_state.get_regulator_ids();

        // Determine which regulators to include
        match regulators {
            Some(requested) => {
                // Validate requested regulators exist
                let missing: Vec<String> = requested
                    .iter()
                    .filter(|id| !available_regulator_ids.contains(id))
                    .cloned()
                    .collect();

                if !missing.is_empty() {
                    PaginatedThermalResponse {
                        data: HashMap::new(),
                        pagination: PaginationInfo {
                            page: page_num,
                            limit: page_limit,
                            total_items: 0,
                            total_pages: 0,
                            has_next: false,
                            has_previous: false,
                        },
                        filters: FilterSummary {
                            step_seconds: step_seconds,
                            included_regulators: requested,
                            from_timestamp,
                            to_timestamp,
                        },
                    }
                } else {
                    build_thermal_response(
                        thermal_state,
                        requested,
                        from_timestamp,
                        to_timestamp,
                        step_seconds,
                        page_num,
                        page_limit,
                    )
                }
            }
            None => build_thermal_response(
                thermal_state,
                available_regulator_ids,
                from_timestamp,
                to_timestamp,
                step_seconds,
                page_num,
                page_limit,
            ),
        }
    };

    rocket::serde::json::Json(response)
}

/// Helper function to build thermal response without early returns
fn build_thermal_response(
    thermal_state: tokio::sync::RwLockReadGuard<
        '_,
        crate::thermal_regulation::shared_state::SharedThermalRegulationState,
    >,
    target_regulators: Vec<String>,
    from_timestamp: Option<u64>,
    to_timestamp: Option<u64>,
    step_seconds: u32,
    page_num: u32,
    page_limit: u32,
) -> PaginatedThermalResponse {
    // Collect and filter data points
    let mut all_data_points: Vec<(String, ThermalDataPoint)> = Vec::new();

    for regulator_id in &target_regulators {
        if let Some(history) = thermal_state.get_regulator_history(regulator_id) {
            // First apply time filtering
            let time_filtered_points: Vec<ThermalDataPoint> = history
                .history
                .iter()
                .filter(|point| match (from_timestamp, to_timestamp) {
                    (Some(start), Some(end)) => point.timestamp >= start && point.timestamp <= end,
                    (Some(start), None) => point.timestamp >= start,
                    (None, Some(end)) => point.timestamp <= end,
                    (None, None) => true,
                })
                .cloned()
                .collect();

            // Then apply step filtering based on timestamps
            let step_filtered_points = if step_seconds == 0 {
                // Include all time-filtered points
                time_filtered_points
            } else {
                // Apply timestamp-based step filtering
                let mut filtered = Vec::new();
                let mut last_included_timestamp: Option<u64> = None;

                for point in time_filtered_points {
                    let should_include = match last_included_timestamp {
                        None => true, // Always include the first point
                        Some(last_ts) => {
                            // Include point if it's at least step_seconds after the last included point
                            point.timestamp >= last_ts + (step_seconds as u64)
                        }
                    };

                    if should_include {
                        last_included_timestamp = Some(point.timestamp);
                        filtered.push(point);
                    }
                }

                filtered
            };

            // Add regulator ID to each data point for global sorting
            for point in step_filtered_points {
                all_data_points.push((regulator_id.clone(), point));
            }
        }
    }

    // Sort all data points by timestamp (most recent first)
    all_data_points.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));

    // Apply pagination
    let total_items = all_data_points.len() as u32;
    let total_pages = if page_limit > 0 {
        ((total_items as f64) / (page_limit as f64)).ceil() as u32
    } else {
        0
    };

    let start_index = ((page_num - 1) * page_limit) as usize;
    let end_index = (start_index + page_limit as usize).min(all_data_points.len());

    let paginated_points = if start_index < all_data_points.len() {
        &all_data_points[start_index..end_index]
    } else {
        &[]
    };

    // Group paginated data back by regulator
    let mut response_data: HashMap<String, Vec<ThermalDataPoint>> = HashMap::new();
    for (regulator_id, data_point) in paginated_points {
        response_data
            .entry(regulator_id.clone())
            .or_insert_with(Vec::new)
            .push(data_point.clone());
    }

    // Build filter summary
    let filter_summary = FilterSummary {
        step_seconds,
        included_regulators: target_regulators,
        from_timestamp,
        to_timestamp,
    };

    // Build pagination info
    let pagination = PaginationInfo {
        page: page_num,
        limit: page_limit,
        total_items,
        total_pages,
        has_next: page_num < total_pages,
        has_previous: page_num > 1,
    };

    PaginatedThermalResponse {
        data: response_data,
        pagination,
        filters: filter_summary,
    }
}

/// Centralized function to get all thermal routes with OpenAPI documentation
pub fn get_thermal_routes() -> (Vec<rocket::Route>, OpenApi) {
    openapi_get_routes_spec![
        get_thermal_regulators,
        get_thermal_data,
        get_last_temperatures
    ]
}

/// Format the regulator status for display
///
/// ### Parameters
///
/// - `status`: The raw status string from the regulator history
///
/// ### Returns
///
/// Returns a formatted status string. If the status is "Stopped", "Error", or
/// "Uninitialized", it returns the status as is. For "Initializing", it returns
/// "Starting up...". For "Running", it returns "Active".
///
/// ### Notes
///
/// This function is used to convert internal status representations to a more
/// user-friendly format for API responses.
fn format_regulator_status(status: &str) -> String {
    match status {
        "Stopped" | "Error" | "Uninitialized" => status.to_string(),
        "Initializing" => "Starting up...".to_string(),
        "Running" => "Active".to_string(),
        _ => "Unknown status".to_string(),
    }
}

/// Helper function to convert regulator status enum to string
fn regulator_status_to_string(status: &RegulatorStatus) -> String {
    match status {
        RegulatorStatus::Uninitialized => "Uninitialized".to_string(),
        RegulatorStatus::Initializing => "Initializing".to_string(),
        RegulatorStatus::Running => "Running".to_string(),
        RegulatorStatus::Error { message } => format!("Error: {}", message),
        RegulatorStatus::Stopped => "Stopped".to_string(),
    }
}
