//! Shared state management for the visualization server
//!
//! This module provides a global state system for sharing data between
//! the daemon components and the web API endpoints. It ensures thread-safe
//! access to runtime information like processing statistics.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::processing::graph::ProcessingGraphStatistics;

/// Global shared state for the visualization server
///
/// This structure contains runtime data that needs to be accessed by both
/// the daemon components (like ProcessingConsumer) and the web API endpoints.
/// All data is protected by async RwLock for safe concurrent access.
#[derive(Debug, Clone)]
pub struct SharedVisualizationState {
    /// Current processing graph statistics
    ///
    /// Updated by the ProcessingConsumer as it processes frames.
    /// Can be None if no processing is currently active.
    processing_statistics: Arc<RwLock<Option<ProcessingGraphStatistics>>>,
}

impl Default for SharedVisualizationState {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedVisualizationState {
    /// Create a new shared state instance
    pub fn new() -> Self {
        Self {
            processing_statistics: Arc::new(RwLock::new(None)),
        }
    }

    /// Update the processing graph statistics
    ///
    /// This should be called by the ProcessingConsumer when it has
    /// updated statistics to share.
    ///
    /// # Parameters
    ///
    /// * `stats` - The latest processing graph statistics
    pub async fn update_processing_statistics(&self, stats: ProcessingGraphStatistics) {
        let mut processing_stats = self.processing_statistics.write().await;
        *processing_stats = Some(stats);
    }

    /// Get the current processing graph statistics
    ///
    /// Returns None if no processing is currently active or if
    /// no statistics have been recorded yet.
    ///
    /// # Returns
    ///
    /// The current processing statistics, or None if unavailable
    pub async fn get_processing_statistics(&self) -> Option<ProcessingGraphStatistics> {
        let processing_stats = self.processing_statistics.read().await;
        processing_stats.clone()
    }

    /// Clear the processing statistics
    ///
    /// This should be called when processing stops or is reset.
    pub async fn clear_processing_statistics(&self) {
        let mut processing_stats = self.processing_statistics.write().await;
        *processing_stats = None;
    }

    /// Check if processing statistics are available
    ///
    /// # Returns
    ///
    /// True if statistics are available, false otherwise
    pub async fn has_processing_statistics(&self) -> bool {
        let processing_stats = self.processing_statistics.read().await;
        processing_stats.is_some()
    }
}

/// Rocket request guard for accessing the shared visualization state
///
/// This allows endpoints to easily access the shared state by including
/// `SharedVisualizationState` as a parameter.
///
/// # Example
///
/// ```rust,no_run
/// use rust_photoacoustic::visualization::shared_state::SharedVisualizationState;
/// use rocket::get;
/// use serde::Serialize;
/// use rocket::State;
/// use rocket::serde::json::Json;
/// #[derive(Serialize)]
/// struct StatusResponse {
///     processing_active: bool,
/// }
///
/// #[get("/api/status")]
/// async fn get_status(state: &State<SharedVisualizationState>) -> Json<StatusResponse> {
///     let has_stats = state.has_processing_statistics().await;
///     Json(StatusResponse { processing_active: has_stats })
/// }
/// ```
#[rocket::async_trait]
impl<'r> rocket::request::FromRequest<'r> for &'r SharedVisualizationState {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        request
            .rocket()
            .state::<SharedVisualizationState>()
            .map(|state| rocket::request::Outcome::Success(state))
            .unwrap_or_else(|| {
                rocket::request::Outcome::Error((rocket::http::Status::InternalServerError, ()))
            })
    }
}
