//! Shared state management for the visualization server
//!
//! This module provides a global state system for sharing data between
//! the daemon components and the web API endpoints. It ensures thread-safe
//! access to runtime information like processing statistics.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::processing::graph::{ProcessingGraph, ProcessingGraphStatistics};
use crate::processing::SerializableProcessingGraph;

/// Global shared state for the visualization server
///
/// This structure contains runtime data that needs to be accessed by both
/// the daemon components (like ProcessingConsumer) and the web API endpoints.
/// All data is protected by async RwLock for safe concurrent access.
#[derive(Clone)]
pub struct SharedVisualizationState {
    /// Current processing graph statistics
    ///
    /// Updated by the ProcessingConsumer as it processes frames.
    /// Can be None if no processing is currently active.
    processing_statistics: Arc<RwLock<Option<ProcessingGraphStatistics>>>,

    /// Current processing graph structure
    ///
    /// Contains the serializable representation of the processing graph
    /// including nodes, connections, and topology information.
    /// Updated when the processing graph is initialized or modified.
    processing_graph: Arc<RwLock<Option<SerializableProcessingGraph>>>,

    /// Live processing graph reference
    ///
    /// Direct access to the live ProcessingGraph instance from ProcessingConsumer.
    /// This allows API endpoints to access real-time data from UniversalActionNode
    /// instances without copying data. The graph is wrapped in Arc<RwLock<>> to
    /// allow safe concurrent access between ProcessingConsumer and API endpoints.
    live_processing_graph: Arc<RwLock<Option<Arc<RwLock<ProcessingGraph>>>>>,
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
            processing_graph: Arc::new(RwLock::new(None)),
            live_processing_graph: Arc::new(RwLock::new(None)),
        }
    }

    /// Update the processing graph statistics
    ///
    /// This should be called by the ProcessingConsumer when it has
    /// updated statistics to share.
    ///
    /// ### Parameters
    ///
    /// * `stats` - The latest processing graph statistics
    pub async fn update_processing_statistics(&self, stats: ProcessingGraphStatistics) {
        let mut processing_stats = self.processing_statistics.write().await;
        *processing_stats = Some(stats);
    }

    /// Update the processing graph structure
    ///
    /// This should be called when the processing graph is initialized
    /// or modified to update the API-accessible representation.
    ///
    /// ### Parameters
    ///
    /// * `graph` - The serializable processing graph structure
    pub async fn update_processing_graph(&self, graph: SerializableProcessingGraph) {
        let mut processing_graph = self.processing_graph.write().await;
        *processing_graph = Some(graph);
    }

    /// Get the current processing graph statistics
    ///
    /// Returns None if no processing is currently active or if
    /// no statistics have been recorded yet.
    ///
    /// ### Returns
    ///
    /// The current processing statistics, or None if unavailable
    pub async fn get_processing_statistics(&self) -> Option<ProcessingGraphStatistics> {
        let processing_stats = self.processing_statistics.read().await;
        processing_stats.clone()
    }

    /// Get the current processing graph structure
    ///
    /// Returns the serializable representation of the processing graph
    /// including nodes, connections, and topology information.
    ///
    /// ### Returns
    ///
    /// The current processing graph structure, or None if unavailable
    pub async fn get_processing_graph(&self) -> Option<SerializableProcessingGraph> {
        let processing_graph = self.processing_graph.read().await;
        processing_graph.clone()
    }

    /// Clear the processing statistics
    ///
    /// This should be called when processing stops or is reset.
    pub async fn clear_processing_statistics(&self) {
        let mut processing_stats = self.processing_statistics.write().await;
        *processing_stats = None;
    }

    /// Clear the processing graph
    ///
    /// This should be called when processing stops or is reset.
    pub async fn clear_processing_graph(&self) {
        let mut processing_graph = self.processing_graph.write().await;
        *processing_graph = None;
    }
    /// Clear all processing data
    ///
    /// This should be called when processing stops or is reset.
    pub async fn clear_all_processing_data(&self) {
        self.clear_processing_statistics().await;
        self.clear_processing_graph().await;
        self.clear_live_processing_graph().await;
    }

    /// Check if processing statistics are available
    ///
    /// ### Returns
    ///
    /// True if statistics are available, false otherwise
    pub async fn has_processing_statistics(&self) -> bool {
        let processing_stats = self.processing_statistics.read().await;
        processing_stats.is_some()
    }
    /// Check if processing graph is available
    ///
    /// ### Returns
    ///
    /// True if processing graph is available, false otherwise
    pub async fn has_processing_graph(&self) -> bool {
        let processing_graph = self.processing_graph.read().await;
        processing_graph.is_some()
    }

    /// Set the live processing graph reference
    ///
    /// This should be called by ProcessingConsumer when it initializes
    /// to share its ProcessingGraph with the API endpoints.
    ///
    /// ### Parameters
    ///
    /// * `graph` - Shared reference to the live ProcessingGraph
    pub async fn set_live_processing_graph(&self, graph: Arc<RwLock<ProcessingGraph>>) {
        let mut live_graph = self.live_processing_graph.write().await;
        *live_graph = Some(graph);
    }

    /// Get the live processing graph reference
    ///
    /// Returns the shared reference to the live ProcessingGraph for
    /// direct access to UniversalActionNode instances and their data.
    ///
    /// ### Returns
    ///
    /// The live processing graph reference, or None if unavailable
    pub async fn get_live_processing_graph(&self) -> Option<Arc<RwLock<ProcessingGraph>>> {
        let live_graph = self.live_processing_graph.read().await;
        live_graph.clone()
    }

    /// Clear the live processing graph
    ///
    /// This should be called when processing stops or is reset.
    pub async fn clear_live_processing_graph(&self) {
        let mut live_graph = self.live_processing_graph.write().await;
        *live_graph = None;
    }

    /// Check if live processing graph is available
    ///
    /// ### Returns
    ///
    /// True if live processing graph is available, false otherwise
    pub async fn has_live_processing_graph(&self) -> bool {
        let live_graph = self.live_processing_graph.read().await;
        live_graph.is_some()
    }
}

impl std::fmt::Debug for SharedVisualizationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedVisualizationState")
            .field(
                "processing_statistics",
                &"Arc<RwLock<Option<ProcessingGraphStatistics>>>",
            )
            .field(
                "processing_graph",
                &"Arc<RwLock<Option<SerializableProcessingGraph>>>",
            )
            .field(
                "live_processing_graph",
                &"Arc<RwLock<Option<Arc<RwLock<ProcessingGraph>>>>>",
            )
            .finish()
    }
}

/// Rocket request guard for accessing the shared visualization state
///
/// This allows endpoints to easily access the shared state by including
/// `SharedVisualizationState` as a parameter.
///
/// ### Example
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
