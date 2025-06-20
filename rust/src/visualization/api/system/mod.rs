// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project
// and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! System health and statistics API endpoint
//!
//! This module provides protected endpoints for system monitoring including
//! CPU usage, memory consumption, thread count, and combined system health metrics.

use log::info;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{get, response::status, State};
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use rocket_okapi::JsonSchema;

use crate::processing::SerializableProcessingGraph;
use crate::utility::system_stats::SystemStats;
use crate::visualization::shared_state::SharedVisualizationState;
use auth_macros::openapi_protect_get;
use serde::{Deserialize, Serialize};

/// Combined system and processing health report
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SystemHealthReport {
    /// Current system resource statistics
    pub system_stats: SystemStats,
    /// Processing pipeline performance summary
    pub processing_summary: Option<ProcessingPerformanceSummary>,
    /// Overall system health assessment
    pub health_status: HealthStatus,
    /// Recommendations for system optimization
    pub recommendations: Vec<String>,
}

/// Processing performance summary for health monitoring
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProcessingPerformanceSummary {
    /// Average processing time per frame in milliseconds
    pub avg_execution_time_ms: f64,
    /// Processing efficiency percentage (0-100)
    pub efficiency_percentage: f64,
    /// Number of active processing nodes
    pub active_nodes: usize,
    /// Total number of completed executions
    pub total_executions: u64,
    /// ID of the slowest node (bottleneck)
    pub slowest_node: Option<String>,
}

/// System health status assessment
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum HealthStatus {
    /// All systems operating normally
    Healthy,
    /// Minor performance issues detected
    Warning { issues: Vec<String> },
    /// Significant performance degradation
    Critical { issues: Vec<String> },
}

/// Get current system statistics
///
/// **Endpoint:** `GET /api/system/stats`
///
/// Returns current system resource usage including:
/// - CPU usage percentage
/// - Memory consumption (physical and virtual)
/// - Thread count
/// - System uptime information
///
/// ### Authentication
///
/// This endpoint requires a valid JWT bearer token in the Authorization header.
///
/// ### Returns
///
/// Returns JSON response containing `SystemStats` with current system metrics.
///
/// ### Example Response
///
/// ```json
/// {
///   "cpu_usage_percent": 25.4,
///   "memory_usage_mb": 512,
///   "virtual_memory_mb": 1024,
///   "thread_count": 8,
///   "total_cpu_cores": 4,
///   "available_memory_mb": 3584,
///   "uptime_seconds": 86400,
///   "process_uptime_seconds": 3600,
///   "timestamp": 1640995200
/// }
/// ```
#[openapi_protect_get("/api/system/stats", "read:api", tag = "System")]
pub async fn get_system_stats() -> Result<Json<SystemStats>, Status> {
    info!("Fetching current system statistics");

    match SystemStats::current() {
        Ok(stats) => {
            info!(
                "System stats collected successfully: {}",
                stats.format_for_logging()
            );
            Ok(Json(stats))
        }
        Err(e) => {
            let error_msg = format!("Failed to collect system statistics: {}", e);
            log::error!("{}", error_msg);
            Err(Status::InternalServerError)
        }
    }
}

/// Get comprehensive system health report
///
/// **Endpoint:** `GET /api/system/health`
///
/// Returns a comprehensive health assessment combining:
/// - System resource statistics
/// - Processing pipeline performance
/// - Health status evaluation
/// - Optimization recommendations
///
/// ### Authentication
///
/// This endpoint requires a valid JWT bearer token in the Authorization header.
///
/// ### Returns
///
/// Returns JSON response containing `SystemHealthReport` with complete system assessment.
///
/// ### Health Status Levels
///
/// - **Healthy**: All systems operating within normal parameters
/// - **Warning**: Minor performance issues detected but system functional
/// - **Critical**: Significant performance degradation requiring attention
///
/// ### Example Response
///
/// ```json
/// {
///   "system_stats": { /* SystemStats object */ },
///   "processing_summary": {
///     "avg_execution_time_ms": 5.2,
///     "efficiency_percentage": 98.5,
///     "active_nodes": 4,
///     "total_executions": 15420,
///     "slowest_node": "bandpass_filter"
///   },
///   "health_status": {
///     "Healthy": null
///   },
///   "recommendations": [
///     "System operating optimally"
///   ]
/// }
/// ```
#[openapi_protect_get("/api/system/health", "read:api", tag = "System")]
pub async fn get_system_health(
    shared_state: &State<SharedVisualizationState>,
) -> Result<Json<SystemHealthReport>, Status> {
    info!("Generating comprehensive system health report");

    // Collect system statistics and handle potential errors
    let result = SystemStats::current()
        .map_err(|e| {
            let error_msg = format!("Failed to collect system statistics: {}", e);
            log::error!("{}", error_msg);
            Status::InternalServerError
        })
        .and_then(|system_stats| {
            // Get processing statistics if available
            let processing_future = async {
                if shared_state.has_processing_statistics().await {
                    match shared_state.get_processing_graph().await {
                        Some(graph) => Some(create_processing_summary(&graph)),
                        None => None,
                    }
                } else {
                    None
                }
            };

            // Since we can't use async blocks in sync context easily,
            // we'll handle this synchronously for now
            Ok(system_stats)
        });

    match result {
        Ok(system_stats) => {
            // Get processing statistics if available (simplified approach)
            let processing_summary = if shared_state.has_processing_statistics().await {
                match shared_state.get_processing_graph().await {
                    Some(graph) => Some(create_processing_summary(&graph)),
                    None => None,
                }
            } else {
                None
            };

            // Assess health status and generate recommendations
            let (health_status, recommendations) =
                assess_system_health(&system_stats, &processing_summary);

            let health_report = SystemHealthReport {
                system_stats,
                processing_summary,
                health_status,
                recommendations,
            };

            info!("System health report generated successfully");
            Ok(Json(health_report))
        }
        Err(status) => Err(status),
    }
}

/// Create processing performance summary from graph statistics
fn create_processing_summary(graph: &SerializableProcessingGraph) -> ProcessingPerformanceSummary {
    let performance_summary = &graph.performance_summary;

    ProcessingPerformanceSummary {
        avg_execution_time_ms: performance_summary.average_execution_time_ms,
        efficiency_percentage: performance_summary.efficiency_percentage,
        active_nodes: performance_summary.active_nodes,
        total_executions: performance_summary.total_executions,
        slowest_node: performance_summary.slowest_node.clone(),
    }
}

/// Assess overall system health and generate recommendations
fn assess_system_health(
    system_stats: &SystemStats,
    processing_summary: &Option<ProcessingPerformanceSummary>,
) -> (HealthStatus, Vec<String>) {
    let mut issues = Vec::new();
    let mut recommendations = Vec::new();

    // CPU usage assessment
    if system_stats.cpu_usage_percent > 90.0 {
        issues.push("Extremely high CPU usage detected".to_string());
        recommendations
            .push("Consider reducing processing load or optimizing algorithms".to_string());
    } else if system_stats.cpu_usage_percent > 70.0 {
        issues.push("High CPU usage detected".to_string());
        recommendations
            .push("Monitor CPU usage and consider optimization if sustained".to_string());
    }

    // Memory usage assessment
    let memory_usage_percent = system_stats.memory_usage_percent();
    if memory_usage_percent > 85.0 {
        issues.push(format!("High memory usage: {:.1}%", memory_usage_percent));
        recommendations
            .push("Consider increasing available memory or optimizing memory usage".to_string());
    } else if memory_usage_percent > 70.0 {
        issues.push(format!(
            "Elevated memory usage: {:.1}%",
            memory_usage_percent
        ));
        recommendations.push("Monitor memory usage trends".to_string());
    }

    // Thread count assessment
    if system_stats.thread_count > system_stats.total_cpu_cores * 4 {
        issues.push("High thread count relative to CPU cores".to_string());
        recommendations
            .push("Review threading strategy to avoid context switching overhead".to_string());
    }

    // Processing pipeline assessment
    if let Some(processing) = processing_summary {
        if processing.efficiency_percentage < 80.0 {
            issues.push(format!(
                "Low processing efficiency: {:.1}%",
                processing.efficiency_percentage
            ));
            recommendations
                .push("Investigate processing bottlenecks and optimize pipeline".to_string());
        }

        if processing.avg_execution_time_ms > 50.0 {
            issues.push("High average processing time detected".to_string());
            recommendations
                .push("Profile processing nodes to identify performance bottlenecks".to_string());
        }

        if let Some(ref slowest_node) = processing.slowest_node {
            recommendations.push(format!(
                "Consider optimizing node '{}' which shows the highest processing time",
                slowest_node
            ));
        }
    }

    // Determine overall health status
    let health_status = if issues
        .iter()
        .any(|issue| issue.contains("Extremely high") || issue.contains("critical"))
    {
        HealthStatus::Critical {
            issues: issues.clone(),
        }
    } else if !issues.is_empty() {
        HealthStatus::Warning {
            issues: issues.clone(),
        }
    } else {
        recommendations.push("System operating optimally".to_string());
        HealthStatus::Healthy
    };

    (health_status, recommendations)
}

/// Get system API routes and OpenAPI specification
///
/// This function returns the Rocket routes and OpenAPI specification for
/// system health and statistics endpoints, ready for mounting in the server.
///
/// ### Returns
///
/// A tuple containing:
/// * Vector of Rocket routes for system endpoints
/// * OpenAPI specification for documentation
pub fn get_system_routes() -> (Vec<rocket::Route>, OpenApi) {
    openapi_get_routes_spec![get_system_stats, get_system_health]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_assessment_healthy() {
        let stats = SystemStats {
            cpu_usage_percent: 25.0,
            memory_usage_mb: 512,
            virtual_memory_mb: 1024,
            thread_count: 4,
            total_cpu_cores: 4,
            available_memory_mb: 3584,
            uptime_seconds: 86400,
            process_uptime_seconds: 3600,
            timestamp: 1640995200,
        };

        let processing = Some(ProcessingPerformanceSummary {
            avg_execution_time_ms: 5.0,
            efficiency_percentage: 95.0,
            active_nodes: 4,
            total_executions: 1000,
            slowest_node: Some("filter".to_string()),
        });

        let (health_status, recommendations) = assess_system_health(&stats, &processing);

        assert!(matches!(health_status, HealthStatus::Healthy));
        assert!(recommendations.iter().any(|r| r.contains("optimally")));
    }

    #[test]
    fn test_health_assessment_warning() {
        let stats = SystemStats {
            cpu_usage_percent: 75.0, // High CPU
            memory_usage_mb: 2048,
            virtual_memory_mb: 4096,
            thread_count: 8,
            total_cpu_cores: 4,
            available_memory_mb: 2048,
            uptime_seconds: 86400,
            process_uptime_seconds: 3600,
            timestamp: 1640995200,
        };

        let (health_status, _) = assess_system_health(&stats, &None);

        assert!(matches!(health_status, HealthStatus::Warning { .. }));
    }

    #[test]
    fn test_health_assessment_critical() {
        let stats = SystemStats {
            cpu_usage_percent: 95.0, // Extremely high CPU
            memory_usage_mb: 4096,
            virtual_memory_mb: 8192,
            thread_count: 20,
            total_cpu_cores: 4,
            available_memory_mb: 512,
            uptime_seconds: 86400,
            process_uptime_seconds: 3600,
            timestamp: 1640995200,
        };

        let (health_status, _) = assess_system_health(&stats, &None);

        assert!(matches!(health_status, HealthStatus::Critical { .. }));
    }
}
