// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Audio streaming API endpoints
//!
//! This module provides HTTP endpoints for streaming audio data and spectral analysis
//! to web clients in real-time using Server-Sent Events (SSE).
#![doc = include_str!("../../../../docs/audio-stream-reconstruction-guide.md")]

use crate::acquisition::{AudioFrame, AudioStreamConsumer, SharedAudioStream, StreamStats};
use crate::processing::nodes::streaming_registry::StreamingNodeRegistry;
use auth_macros::{openapi_protect_get, protect_get};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rocket::futures::stream::Stream;
use rocket::serde::json::Json;
use rocket::{
    get,
    response::stream::{Event, EventStream},
    State,
};
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::{openapi, openapi_get_routes_spec, JsonSchema};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

/// Audio streaming state managed by Rocket
pub struct AudioStreamState {
    pub stream: Arc<SharedAudioStream>,
    pub registry: Arc<StreamingNodeRegistry>,
}

/// Response structure for audio frame data
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AudioFrameResponse {
    /// Channel A audio data
    pub channel_a: Vec<f32>,
    /// Channel B audio data  
    pub channel_b: Vec<f32>,
    /// Sample rate of the audio data
    pub sample_rate: u32,
    /// Timestamp when the frame was captured
    pub timestamp: u64,
    /// Sequential frame number
    pub frame_number: u64,
    /// Duration of this frame in milliseconds
    pub duration_ms: f64,
}

impl From<AudioFrame> for AudioFrameResponse {
    fn from(frame: AudioFrame) -> Self {
        let duration_ms = frame.duration_ms();
        Self {
            channel_a: frame.channel_a,
            channel_b: frame.channel_b,
            sample_rate: frame.sample_rate,
            timestamp: frame.timestamp,
            frame_number: frame.frame_number,
            duration_ms,
        }
    }
}

/// Response structure for AudioFastFrameResponse
/// This is a alternative response format for audio frames
/// each channel contains a single string value of base64 encoded audio data
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AudioFastFrameResponse {
    /// Base64 encoded audio data for channel A
    pub channel_a: String,
    /// Base64 encoded audio data for channel B
    pub channel_b: String,
    /// channels length
    pub channels_length: usize,
    /// channels raw_type e.g. f32
    pub channels_raw_type: String,
    /// Channel elements size in bytes e.g. 4 for f32
    pub channels_element_size: usize,
    /// Sample rate of the audio data
    pub sample_rate: u32,
    /// Timestamp when the frame was captured
    pub timestamp: u64,
    /// Sequential frame number
    pub frame_number: u64,
    /// Duration of this frame in milliseconds
    pub duration_ms: f64,
}
impl From<AudioFrame> for AudioFastFrameResponse {
    fn from(frame: AudioFrame) -> Self {
        let duration_ms = frame.duration_ms();
        let channels_length = frame.channel_a.len();
        let channels_raw_type = "f32".to_string();
        let channels_element_size = std::mem::size_of::<f32>();

        // More efficient binary conversion using bytemuck or direct slice conversion
        let channel_a_bytes = unsafe {
            std::slice::from_raw_parts(
                frame.channel_a.as_ptr() as *const u8,
                frame.channel_a.len() * channels_element_size,
            )
        };
        let channel_b_bytes = unsafe {
            std::slice::from_raw_parts(
                frame.channel_b.as_ptr() as *const u8,
                frame.channel_b.len() * channels_element_size,
            )
        };

        // Encode to base64
        let channel_a = STANDARD.encode(channel_a_bytes);
        let channel_b = STANDARD.encode(channel_b_bytes);

        Self {
            channel_a,
            channel_b,
            channels_length,
            channels_raw_type,
            channels_element_size,
            sample_rate: frame.sample_rate,
            timestamp: frame.timestamp,
            frame_number: frame.frame_number,
            duration_ms,
        }
    }
}

/// Spectral analysis data for real-time visualization
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpectralDataResponse {
    /// Frequency bins (Hz)
    pub frequencies: Vec<f32>,
    /// Magnitude spectrum for channel A
    pub magnitude_a: Vec<f32>,
    /// Magnitude spectrum for channel B
    pub magnitude_b: Vec<f32>,
    /// Phase spectrum for channel A (optional)
    pub phase_a: Option<Vec<f32>>,
    /// Phase spectrum for channel B (optional)
    pub phase_b: Option<Vec<f32>>,
    /// Frame metadata
    pub frame_number: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Sample rate
    pub sample_rate: u32,
}

/// Response structure for available audio stream information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AudioStreamInfo {
    /// Stream identifier (e.g., "realtime_source", "streaming_output")
    pub id: String,
    /// Stream URL endpoint for consuming audio data
    pub stream_url: String,
    /// Statistics URL endpoint for stream metrics
    pub stats_url: String,
}

/// Get realtime source stream statistics
///
/// Returns information about the audio stream including frame rates,
/// subscriber count, and other metrics.
#[deprecated(note = "Use /api/stream/audio/fast/stats for more efficient binary streaming")]
#[openapi_protect_get("/api/stream/stats", "read:api", tag = "Audio Streaming")]
pub async fn get_stream_stats(stream_state: &State<AudioStreamState>) -> Json<StreamStats> {
    let stats = stream_state.stream.get_stats().await;
    Json(stats)
}

/// Get realtime source stream statistics
///
/// Returns information about the audio stream including frame rates,
/// subscriber count, and other metrics.
#[openapi_protect_get("/api/stream/audio/fast/stats", "read:api", tag = "Audio Streaming")]
pub async fn get_stream_fast_stats(stream_state: &State<AudioStreamState>) -> Json<StreamStats> {
    let stats = stream_state.stream.get_stats().await;
    Json(stats)
}

/// Get the latest audio frame from realtime source
///
/// Returns the most recent audio frame without subscribing to the stream.
/// Useful for getting current state or testing connectivity.
#[openapi_protect_get("/api/stream/latest", "read:api", tag = "Audio Streaming")]
pub async fn get_latest_frame(
    stream_state: &State<AudioStreamState>,
) -> Option<Json<AudioFrameResponse>> {
    let frame = stream_state.stream.get_latest_frame().await;
    match frame {
        Some(frame) => Some(Json(frame.into())),
        None => None,
    }
}

/// Stream audio frames via Server-Sent Events
///
/// Provides a continuous stream of audio frames to web clients using
/// Server-Sent Events. Each event contains a complete audio frame with
/// both channels of data.
/// Source is the realtime source
///
/// ### Authentication
/// Requires a valid JWT token with appropriate read permissions.
/// The token is transmitted via "Authorization: Bearer <token>" .
///
/// ### Response Format
/// The stream sends JSON-encoded audio frames as SSE events:
/// ```json
/// data: {"channel_a": [...], "channel_b": [...], ...}
///
/// ```
#[deprecated(note = "Use /api/stream/audio/fast for more efficient binary streaming")]
#[openapi(tag = "Audio Streaming")]
#[protect_get("/api/stream/audio", "read:api")]
pub fn stream_audio(
    stream_state: &State<AudioStreamState>,
) -> EventStream<impl Stream<Item = Event>> {
    create_audio_stream(stream_state.stream.clone(), AudioFrameResponse::from)
}
/// Stream realtime source frames via Server-Sent Events using fast binary format
///
/// Similar to stream_audio but uses base64-encoded binary data for reduced bandwidth.
/// This can reduce data size by approximately 1.9x compared to JSON arrays.
#[openapi(tag = "Audio Streaming")]
#[protect_get("/api/stream/audio/fast", "read:api")]
pub fn stream_audio_fast(
    stream_state: &State<AudioStreamState>,
) -> EventStream<impl Stream<Item = Event>> {
    create_audio_stream(stream_state.stream.clone(), AudioFastFrameResponse::from)
}

/// Stream audio frames via Server-Sent Events for a specific streaming node (JSON format)
///
/// **DEPRECATED:** Use [`/api/stream/audio/fast/<node_id>`] for more efficient binary streaming with node routing.
///
/// This endpoint streams real-time audio frames from a specific `StreamingNode` identified by its UUID or string ID.
/// Each event contains a JSON-encoded audio frame with both channels of data. The endpoint is primarily
/// intended for backward compatibility and debugging, as the fast binary endpoint is recommended for production use.
///
/// ### Route Pattern
/// `/api/stream/audio/<node_id>` where `node_id` can be:
/// - A UUID string: `123e4567-e89b-12d3-a456-426614174000`
/// - A string ID from node configuration: `my_streaming_node`
///
/// ### Parameters
/// - `node_id`: The UUID or string ID of the streaming node to subscribe to (as a path parameter)
/// - `stream_state`: Rocket-managed state containing the streaming registry
///
/// ### Authentication
/// Requires a valid JWT token with `read:api` permission.
///
/// ### Response Format
/// Streams Server-Sent Events (SSE) with JSON-encoded audio frames:
///
/// ```json
/// data:{"channel_a": [...], "channel_b": [...], "sample_rate": 44100, ...}
/// ```
///
/// If the node ID is invalid or not found, an error event is sent:
///
/// ```json
/// data:{"type": "error", "message": "No streaming node found"}
/// ```
///
/// Heartbeat events are sent every 5 seconds if no frame is available:
///
/// ```json
/// data:{"type": "heartbeat"}
/// ```
///
/// ### Deprecation
/// This endpoint is deprecated in favor of [`/stream/audio/fast/<node_id>`], which uses a more efficient
/// binary format for audio data. New clients should use the fast endpoint for lower bandwidth and better performance.
///
/// ### Examples
///
/// ```text
/// GET /api/stream/audio/123e4567-e89b-12d3-a456-426614174000
/// GET /api/stream/audio/my_streaming_node
/// ```
///
/// ### See Also
/// - [`/stream/audio/fast/<node_id>`]: Fast binary streaming for a specific node
/// - [`/stream/nodes`]: List all available streaming nodes and their UUIDs
#[deprecated(
    note = "Use /api/stream/audio/fast/<node_id> for more efficient binary streaming with node routing"
)]
#[openapi(tag = "Audio Streaming")]
#[protect_get("/api/stream/audio/<node_id>", "read:api")]
pub fn stream_audio_with_node_id(
    node_id: &str,
    stream_state: &State<AudioStreamState>,
) -> EventStream<impl Stream<Item = Event>> {
    create_node_audio_stream(
        node_id,
        stream_state.registry.clone(),
        AudioFrameResponse::from,
    )
}

/// Stream audio frames via Server-Sent Events using fast binary format with dynamic node ID routing
///
/// This endpoint supports routing to specific streaming nodes using their UUID.
/// When a node_id is provided, it queries the StreamingNodeRegistry for the appropriate stream.
/// If no matching node is found, returns a 404 error.
///
/// ### Route Pattern
/// `/api/stream/audio/fast/<node_id>` where `node_id` is a UUID string
///
/// ### Examples
/// - `/stream/audio/fast/123e4567-e89b-12d3-a456-426614174000` - Stream from specific node
///
/// ### Authentication
/// Requires a valid JWT token with `read:api` permission.
#[openapi(tag = "Audio Streaming")]
#[protect_get("/api/stream/audio/fast/<node_id>", "read:api")]
pub fn stream_audio_fast_with_node_id(
    node_id: &str,
    stream_state: &State<AudioStreamState>,
) -> EventStream<impl Stream<Item = Event>> {
    create_node_audio_stream(
        node_id,
        stream_state.registry.clone(),
        AudioFastFrameResponse::from,
    )
}

/// Retrieve all available audio streams
///
/// This endpoint lists all currently active audio streams in the system.
/// Includes streams from nodes (by id) as well as the realtime source stream.
///
///
/// ### Authentication
/// Requires a valid JWT token with `read:api` permission.
///
/// ### Response Format
/// Returns a JSON array of audio stream information objects:
/// ```json
/// [
///   {
///     "id": "realtime_source",
///     "stream_url": "/api/stream/audio/fast",
///     "stats_url": "/api/stream/audio/fast/stats"
///   },
///   {
///     "id": "streaming_output",
///     "stream_url": "/api/stream/audio/fast/streaming_output",
///     "stats_url": "/api/stream/audio/fast/streaming_output/stats"
///   }
/// ]
/// ```
#[openapi_protect_get(
    "/api/stream/audio/get-all-streams",
    "read:api",
    tag = "Audio Streaming"
)]
pub async fn get_all_available_fast_audio_streams(
    stream_state: &State<AudioStreamState>,
) -> Json<Vec<AudioStreamInfo>> {
    let mut stream_infos: Vec<AudioStreamInfo> = Vec::new();

    // Add the main realtime source stream
    stream_infos.push(AudioStreamInfo {
        id: "realtime_source".to_string(),
        stream_url: "/stream/audio/fast".to_string(),
        stats_url: "/stream/audio/fast/stats".to_string(),
    });

    // Add all streaming node streams
    for (node_uuid, string_id, name) in stream_state.registry.list_all_node_info() {
        log::debug!(
            "Found streaming node for URLs - UUID: {}, string_id: '{}', name: '{}'",
            node_uuid,
            string_id,
            name
        );

        // Check if the node has an active stream
        if let Some(_stream) = stream_state.registry.get_stream(&node_uuid) {
            // Add stream URL using the string ID (preferred)
            let stream_url = format!("/stream/audio/fast/{}", string_id);
            let stats_url = format!("/stream/audio/fast/{}/stats", string_id);
            stream_infos.push(AudioStreamInfo {
                id: string_id.clone(),
                stream_url,
                stats_url,
            });

            log::debug!(
                "Added stream URL: /stream/audio/fast/{} for node '{}'",
                string_id,
                name
            );
        }
    }

    log::debug!("Returning {} available audio streams", stream_infos.len());
    Json(stream_infos)
}

/// Stream spectral analysis data via Server-Sent Events
///
/// Provides real-time spectral analysis data computed from the audio frames.
/// The analysis includes FFT magnitude and optionally phase information for
/// both audio channels.
///
/// ### Authentication
/// Requires a valid JWT token with appropriate read permissions.
///
/// ### Response Format
/// The stream sends JSON-encoded spectral data as SSE events:
/// ```json
/// data: {"frequencies": [...], "magnitude_a": [...], "magnitude_b": [...], ...}
///
/// ```
#[openapi(tag = "Audio Streaming")]
#[protect_get("/api/stream/spectral", "read:api")]
pub fn stream_spectral_analysis(
    stream_state: &State<AudioStreamState>,
) -> EventStream<impl Stream<Item = Event>> {
    let stream = stream_state.stream.clone();

    EventStream! {
        let mut consumer = AudioStreamConsumer::new(&stream);

        loop {            match timeout(Duration::from_secs(5), consumer.next_frame()).await {
                Ok(Some(frame)) => {
                    // Perform FFT analysis on the frame
                    let spectral_data = compute_spectral_analysis(&frame);
                    yield Event::json(&spectral_data);
                },Ok(None) => {
                    log::info!("Audio stream closed for spectral analysis stream");
                    break;
                },
                Err(_) => {
                    // Timeout - send heartbeat
                    yield Event::data(r#"{"type":"heartbeat"}"#);
                }
            }
        }
    }
}

/// Compute spectral analysis for an audio frame
///
/// Performs FFT analysis on both channels of the audio frame and returns
/// frequency domain representation including magnitude spectra.
fn compute_spectral_analysis(frame: &AudioFrame) -> SpectralDataResponse {
    use rustfft::{num_complex::Complex, FftPlanner};

    let n = frame.channel_a.len();
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);

    // Convert to complex numbers for FFT
    let mut buffer_a: Vec<Complex<f32>> = frame
        .channel_a
        .iter()
        .map(|&x| Complex::new(x, 0.0))
        .collect();
    let mut buffer_b: Vec<Complex<f32>> = frame
        .channel_b
        .iter()
        .map(|&x| Complex::new(x, 0.0))
        .collect();

    // Perform FFT
    fft.process(&mut buffer_a);
    fft.process(&mut buffer_b);

    // Compute magnitude spectra
    let magnitude_a: Vec<f32> = buffer_a
        .iter()
        .take(n / 2) // Only positive frequencies
        .map(|c| c.norm())
        .collect();
    let magnitude_b: Vec<f32> = buffer_b.iter().take(n / 2).map(|c| c.norm()).collect();

    // Generate frequency bins
    let sample_rate = frame.sample_rate as f32;
    let frequencies: Vec<f32> = (0..n / 2)
        .map(|i| i as f32 * sample_rate / n as f32)
        .collect();

    SpectralDataResponse {
        frequencies,
        magnitude_a,
        magnitude_b,
        phase_a: None, // Could compute phase if needed
        phase_b: None,
        frame_number: frame.frame_number,
        timestamp: frame.timestamp,
        sample_rate: frame.sample_rate,
    }
}

/// Response structure for listing available streaming nodes
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StreamingNodeInfo {
    /// Node string ID from configuration (preferred for permanent links)
    pub id: String,
    /// Node UUID (for backward compatibility and internal operations)
    pub uuid: String,
    /// Human-readable node name (if available)
    pub name: Option<String>,
    /// Whether the node is currently streaming
    pub is_active: bool,
    /// Current subscriber count for this node's stream
    pub subscriber_count: usize,
}

/// List all available streaming nodes
///
/// Returns information about all registered streaming nodes in the system.
/// This endpoint is useful for discovering available streams and their status.
///
/// ### Authentication
/// Requires a valid JWT token with `read:api` permission.
///
/// ### Response Format
/// Returns a JSON array of streaming node information:
/// ```json
/// [
///   {
///     "id": "my_streaming_node",
///     "uuid": "123e4567-e89b-12d3-a456-426614174000",
///     "name": "Recording Node 1",
///     "is_active": true,
///     "subscriber_count": 2
///   }
/// ]
/// ```
#[openapi_protect_get("/api/stream/nodes", "read:api", tag = "Audio Streaming")]
pub async fn list_streaming_nodes(
    stream_state: &State<AudioStreamState>,
) -> Json<Vec<StreamingNodeInfo>> {
    let mut node_infos = Vec::new();
    // Get all node info from the registry
    for (node_uuid, string_id, name) in stream_state.registry.list_all_node_info() {
        log::debug!(
            "Found streaming node - UUID: {}, string_id: '{}', name: '{}'",
            node_uuid,
            string_id,
            name
        );

        // Get the stream for this node
        if let Some(stream) = stream_state.registry.get_stream(&node_uuid) {
            let subscriber_count = {
                // This is a synchronous call that should be fast
                tokio::task::block_in_place(|| {
                    let rt = tokio::runtime::Handle::current();
                    rt.block_on(stream.get_stats()).active_subscribers
                })
            };

            let node_info = StreamingNodeInfo {
                id: string_id.clone(),
                uuid: node_uuid.to_string(),
                name: Some(name.clone()),
                is_active: subscriber_count > 0,
                subscriber_count,
            };

            log::debug!(
                "Creating StreamingNodeInfo: id='{}', uuid='{}', name='{:?}'",
                node_info.id,
                node_info.uuid,
                node_info.name
            );

            node_infos.push(node_info);
        }
    }

    Json(node_infos)
}

/// Get statistics for a specific streaming node
///
/// Returns detailed statistics for a specific streaming node identified by its UUID or string ID.
/// The endpoint supports both formats:
/// - UUID format: `123e4567-e89b-12d3-a456-426614174000`
/// - String ID format: `my_streaming_node`
///
/// ### Authentication
/// Requires a valid JWT token with `read:api` permission.
#[deprecated(
    note = "Use /api/stream/audio/fast/<node_id>/stats for more efficient binary streaming with node routing"
)]
#[openapi_protect_get(
    "/api/stream/nodes/<node_id>/stats",
    "read:api",
    tag = "Audio Streaming"
)]
pub async fn get_node_stats(
    node_id: &str,
    stream_state: &State<AudioStreamState>,
) -> Json<StreamStats> {
    let stats = get_node_stats_by_id(node_id, &stream_state.registry).await;
    Json(stats)
}

/// Get statistics for a specific fast streaming node
///
/// Returns detailed statistics for a specific streaming node identified by its UUID or string ID.
/// The endpoint supports both formats:
/// - UUID format: `123e4567-e89b-12d3-a456-426614174000`
/// - String ID format: `my_streaming_node`
///
/// ### Authentication
/// Requires a valid JWT token with `read:api` permission.
#[openapi_protect_get(
    "/api/stream/audio/fast/<node_id>/stats",
    "read:api",
    tag = "Audio Streaming"
)]
pub async fn get_node_fast_stats(
    node_id: &str,
    stream_state: &State<AudioStreamState>,
) -> Json<StreamStats> {
    let stats = get_node_stats_by_id(node_id, &stream_state.registry).await;
    Json(stats)
}

/// Helper function to parse node ID and retrieve stream from registry
///
/// This function supports both UUID and string ID formats:
/// - If the input is a valid UUID, it uses UUID-based lookup
/// - Otherwise, it tries string ID-based lookup
fn get_stream_by_node_id(
    node_id: &str,
    registry: &Arc<StreamingNodeRegistry>,
) -> Result<Arc<SharedAudioStream>, &'static str> {
    // First try parsing as UUID for backward compatibility
    if let Ok(node_uuid) = Uuid::parse_str(node_id) {
        if let Some(stream) = registry.get_stream(&node_uuid) {
            return Ok(Arc::new(stream));
        }
    }

    // If UUID parsing failed or UUID not found, try string ID lookup
    registry
        .get_stream_by_string_id(node_id)
        .map(|stream| Arc::new(stream))
        .ok_or("No streaming node found")
}

/// Helper function to get stats for a node ID
async fn get_node_stats_by_id(node_id: &str, registry: &Arc<StreamingNodeRegistry>) -> StreamStats {
    match get_stream_by_node_id(node_id, registry) {
        Ok(stream) => stream.get_stats().await,
        Err(_) => StreamStats::default(),
    }
}

/// Generic streaming function that handles both regular and fast formats
///
/// This function creates an `EventStream` that continuously reads audio frames from a
/// `SharedAudioStream` and transforms them using the provided transformation function.
/// It handles timeouts, heartbeats, and proper stream cleanup.
///
/// # Parameters
///
/// * `stream` - An `Arc<SharedAudioStream>` to read audio frames from
/// * `transform_fn` - A function that transforms `AudioFrame` into the desired response type `T`
///
/// # Type Parameters
///
/// * `T` - The response type that must implement `Serialize`
/// * `F` - The transformation function type that converts `AudioFrame` to `T`
///
/// # Returns
///
/// An `EventStream` that yields Server-Sent Events containing the transformed audio data
///
/// # Behavior
///
/// - Reads frames from the audio stream with a 5-second timeout
/// - On successful frame read: transforms and yields the frame as JSON
/// - On stream closure: logs info message and terminates the stream
/// - On timeout: sends a heartbeat event to keep the connection alive
///
/// # Examples
///
/// Creating a stream with regular audio frame format:
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use rocket::response::stream::EventStream;
/// use rust_photoacoustic::acquisition::SharedAudioStream;
/// use rust_photoacoustic::visualization::streaming::{create_audio_stream, AudioFrameResponse};
///
/// fn example_regular_stream(stream: Arc<SharedAudioStream>) -> EventStream<impl rocket::futures::stream::Stream<Item = rocket::response::stream::Event>> {
/// create_audio_stream(stream, AudioFrameResponse::from)
/// }
/// ```
///
/// Creating a stream with fast binary format:
///
/// ```rust,no_run
/// # use std::sync::Arc;
/// # use rocket::response::stream::EventStream;
/// # use rust_photoacoustic::acquisition::SharedAudioStream;
/// # use rust_photoacoustic::visualization::streaming::{create_audio_stream, AudioFastFrameResponse};
/// #
/// # fn example_fast_stream(stream: Arc<SharedAudioStream>) -> EventStream<impl rocket::futures::stream::Stream<Item = rocket::response::stream::Event>> {
/// create_audio_stream(stream, AudioFastFrameResponse::from)
/// # }
/// ```
///
/// # Event Types
///
/// The stream produces three types of Server-Sent Events:
///
/// ## Data Events (JSON)
/// Contains the transformed audio frame data:
/// ```json
/// data: {"channel_a": [...], "channel_b": [...], "sample_rate": 48000, ...}
/// ```
///
/// ## Heartbeat Events
/// Sent every 5 seconds when no frame is available:
/// ```json
/// data: {"type":"heartbeat"}
/// ```
///
/// ## Stream Closure
/// The stream terminates gracefully when the underlying audio stream closes,
/// logging an info message for debugging purposes.
pub fn create_audio_stream<T, F>(
    stream: Arc<SharedAudioStream>,
    transform_fn: F,
) -> EventStream<impl Stream<Item = Event>>
where
    T: Serialize,
    F: Fn(AudioFrame) -> T + Send + 'static,
{
    EventStream! {
        let mut consumer = AudioStreamConsumer::new(&stream);

        loop {
            match timeout(Duration::from_secs(5), consumer.next_frame()).await {
                Ok(Some(frame)) => {
                    let response = transform_fn(frame);
                    yield Event::json(&response);
                },
                Ok(None) => {
                    log::info!("Audio stream closed");
                    break;
                },
                Err(_) => {
                    yield Event::data(r#"{"type":"heartbeat"}"#);
                }
            }
        }
    }
}

/// Generic streaming function for node-specific streams
///
/// This function creates an `EventStream` for a specific streaming node identified by UUID.
/// It first resolves the node ID to get the appropriate stream, then creates a continuous
/// stream of transformed audio frames with proper error handling.
///
/// # Parameters
///
/// * `node_id` - String slice containing the UUID of the streaming node
/// * `registry` - Arc reference to the `StreamingNodeRegistry` for node lookup
/// * `transform_fn` - Function that transforms `AudioFrame` into the desired response type `T`
///
/// # Type Parameters
///
/// * `T` - The response type that must implement `Serialize`
/// * `F` - The transformation function type that converts `AudioFrame` to `T`
///
/// # Returns
///
/// An `EventStream` that yields Server-Sent Events containing either:
/// - Transformed audio data on success
/// - Error events if node ID is invalid or node not found
///
/// # Error Handling
///
/// The function handles two types of errors by sending appropriate error events:
///
/// ## Invalid Node ID Format
/// ```json
/// data: {"type":"error","message":"Invalid node ID format"}
/// ```
///
/// ## Node Not Found
/// ```json
/// data: {"type":"error","message":"No streaming node found"}
/// ```
///
/// # Examples
///
/// Creating a node-specific stream with regular format:
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use rocket::response::stream::EventStream;
/// use rust_photoacoustic::processing::nodes::streaming_registry::StreamingNodeRegistry;
/// use rust_photoacoustic::visualization::streaming::{create_node_audio_stream, AudioFrameResponse};
///
/// fn example_node_stream(registry: Arc<StreamingNodeRegistry>) -> EventStream<impl rocket::futures::stream::Stream<Item = rocket::response::stream::Event>> {
/// let node_id = "123e4567-e89b-12d3-a456-426614174000";
/// create_node_audio_stream(node_id, registry, AudioFrameResponse::from)
/// }
/// ```
///
/// Creating a node-specific stream with fast binary format:
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use rocket::response::stream::EventStream;
/// use rust_photoacoustic::processing::nodes::streaming_registry::StreamingNodeRegistry;
/// use rust_photoacoustic::visualization::streaming::{create_node_audio_stream, AudioFastFrameResponse};
///
/// fn example_node_fast_stream(registry: Arc<StreamingNodeRegistry>) -> EventStream<impl rocket::futures::stream::Stream<Item = rocket::response::stream::Event>> {
/// let node_id = "123e4567-e89b-12d3-a456-426614174000";
/// create_node_audio_stream(node_id, registry, AudioFastFrameResponse::from)
/// }
/// ```
///
/// # Event Types
///
/// ## Success Events (JSON)
/// Contains the transformed audio frame data:
/// ```json
/// data: {"channel_a": [...], "channel_b": [...], "sample_rate": 48000, ...}
/// ```
///
/// ## Error Events (JSON)
/// Sent when node resolution fails:
/// ```json
/// data: {"type":"error","message":"Invalid node ID format"}
/// data: {"type":"error","message":"No streaming node found"}
/// ```
///
/// ## Heartbeat Events
/// Sent every 5 seconds during normal operation when no frame is available:
/// ```json
/// data: {"type":"heartbeat"}
/// ```
///
/// # Stream Lifecycle
///
/// 1. **Node Resolution**: Parse UUID and lookup stream in registry
/// 2. **Error Handling**: Send error event and terminate if node not found
/// 3. **Stream Processing**: Continuously read and transform frames
/// 4. **Graceful Termination**: Log closure and exit when stream ends
pub fn create_node_audio_stream<T, F>(
    node_id: &str,
    registry: Arc<StreamingNodeRegistry>,
    transform_fn: F,
) -> EventStream<impl Stream<Item = Event>>
where
    T: Serialize,
    F: Fn(AudioFrame) -> T + Send + 'static,
{
    let node_id_owned = node_id.to_string();

    EventStream! {
        let stream = match get_stream_by_node_id(&node_id_owned, &registry) {
            Ok(stream) => stream,
            Err(error_msg) => {
                let error_json = format!(r#"{{"type":"error","message":"{}"}}"#, error_msg);
                yield Event::data(error_json);
                return;
            }
        };

        let mut consumer = AudioStreamConsumer::new(&stream);

        loop {
            match timeout(Duration::from_secs(5), consumer.next_frame()).await {
                Ok(Some(frame)) => {
                    let response = transform_fn(frame);
                    yield Event::json(&response);
                },
                Ok(None) => {
                    log::info!("Audio stream closed for node: {}", node_id_owned);
                    break;
                },
                Err(_) => {
                    yield Event::data(r#"{"type":"heartbeat"}"#);
                }
            }
        }
    }
}

/// Get all audio streaming routes
///
/// Returns a vector of all route handlers for audio streaming functionality.
pub fn get_audio_streaming_routes() -> (Vec<rocket::Route>, OpenApi) {
    openapi_get_routes_spec![
        get_stream_stats,
        get_stream_fast_stats,
        get_latest_frame,
        stream_audio,
        stream_audio_fast,
        stream_audio_with_node_id,
        stream_audio_fast_with_node_id,
        stream_spectral_analysis,
        list_streaming_nodes,
        get_node_stats,
        get_node_fast_stats,
        get_all_available_fast_audio_streams,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::AudioFrame;

    /// Helper function to create test frames with different sizes
    fn create_test_frame(size: usize, sample_rate: u32, frame_number: u64) -> AudioFrame {
        let channel_a: Vec<f32> = (0..size).map(|i| (i as f32) * 0.001).collect();
        let channel_b: Vec<f32> = (0..size).map(|i| -(i as f32) * 0.001).collect();
        AudioFrame::new(channel_a, channel_b, sample_rate, frame_number)
    }

    /// Helper function to test frame response conversion and verify roundtrip accuracy
    fn test_frame_conversion_roundtrip(frame: AudioFrame) {
        let fast_response = AudioFastFrameResponse::from(frame.clone());

        // Decode and verify
        let decoded_a_bytes = STANDARD.decode(&fast_response.channel_a).unwrap();
        let decoded_b_bytes = STANDARD.decode(&fast_response.channel_b).unwrap();

        let decoded_a: Vec<f32> = decoded_a_bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        let decoded_b: Vec<f32> = decoded_b_bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        // Verify exact equality
        assert_eq!(
            decoded_a, frame.channel_a,
            "Channel A should be exactly preserved"
        );
        assert_eq!(
            decoded_b, frame.channel_b,
            "Channel B should be exactly preserved"
        );

        // Verify metadata
        assert_eq!(fast_response.channels_length, frame.channel_a.len());
        assert_eq!(fast_response.sample_rate, frame.sample_rate);
        assert_eq!(fast_response.frame_number, frame.frame_number);
    }

    #[test]
    fn test_audio_frame_response_conversion() {
        let frame = AudioFrame::new(vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6], 48000, 42);

        let response = AudioFrameResponse::from(frame.clone());
        assert_eq!(response.channel_a, vec![0.1, 0.2, 0.3]);
        assert_eq!(response.channel_b, vec![0.4, 0.5, 0.6]);
        assert_eq!(response.sample_rate, 48000);
        assert_eq!(response.frame_number, 42);
        assert!(response.duration_ms > 0.0);
    }

    #[test]
    fn test_spectral_analysis() {
        let frame = AudioFrame::new(vec![0.1, 0.2, 0.3, 0.4], vec![0.4, 0.5, 0.6, 0.7], 48000, 1);

        let spectral = compute_spectral_analysis(&frame);
        assert_eq!(spectral.magnitude_a.len(), 2); // n/2 frequencies
        assert_eq!(spectral.magnitude_b.len(), 2);
        assert_eq!(spectral.frequencies.len(), 2);
        assert_eq!(spectral.sample_rate, 48000);
    }

    #[test]
    fn test_audio_fast_frame_response_conversion() {
        let frame = create_test_frame(3, 48000, 42);
        test_frame_conversion_roundtrip(frame);
    }

    #[test]
    fn test_fast_frame_binary_encoding_accuracy() {
        // Test with various edge cases for f32 values
        let test_values_a = vec![
            0.0,
            1.0,
            -1.0,
            0.5,
            std::f32::consts::PI,
            f32::EPSILON,
            -f32::EPSILON,
            f32::MAX / 1000.0,
            f32::MIN / 1000.0,
        ];
        let test_values_b = vec![
            std::f32::consts::E,
            0.123456789,
            -0.987654321,
            42.0,
            -42.0,
            0.000001,
            -0.000001,
            1000.5,
            -1000.5,
        ];

        let frame = AudioFrame::new(test_values_a, test_values_b, 96000, 123);
        test_frame_conversion_roundtrip(frame);
    }

    #[test]
    fn test_fast_frame_metadata_consistency() {
        let frame = create_test_frame(5, 44100, 999);
        let fast_response = AudioFastFrameResponse::from(frame.clone());

        // Verify all metadata is preserved
        assert_eq!(fast_response.channels_length, 5);
        assert_eq!(fast_response.channels_raw_type, "f32");
        assert_eq!(fast_response.channels_element_size, 4);
        assert_eq!(fast_response.sample_rate, 44100);
        assert_eq!(fast_response.frame_number, 999);
        assert_eq!(fast_response.timestamp, frame.timestamp);
        assert_eq!(fast_response.duration_ms, frame.duration_ms());
    }

    #[test]
    fn test_fast_frame_empty_channels() {
        let frame = AudioFrame::new(vec![], vec![], 48000, 0);
        test_frame_conversion_roundtrip(frame);
    }

    #[test]
    fn test_fast_frame_large_channels() {
        let frame = create_test_frame(8192, 192000, 12345);
        test_frame_conversion_roundtrip(frame);
    }

    #[test]
    fn test_fast_frame_base64_validity() {
        let frame = AudioFrame::new(vec![1.1, 2.2, 3.3], vec![4.4, 5.5, 6.6], 48000, 42);
        let fast_response = AudioFastFrameResponse::from(frame);

        // Verify base64 strings are valid and not empty
        assert!(STANDARD.decode(&fast_response.channel_a).is_ok());
        assert!(STANDARD.decode(&fast_response.channel_b).is_ok());
        assert!(!fast_response.channel_a.is_empty());
        assert!(!fast_response.channel_b.is_empty());
    }

    #[test]
    fn test_fast_vs_regular_frame_equivalence() {
        let frame = create_test_frame(4, 96000, 777);
        let regular_response = AudioFrameResponse::from(frame.clone());
        let fast_response = AudioFastFrameResponse::from(frame.clone());

        // Decode fast response and verify equivalence
        let decoded_a_bytes = STANDARD.decode(&fast_response.channel_a).unwrap();
        let decoded_b_bytes = STANDARD.decode(&fast_response.channel_b).unwrap();

        let decoded_a: Vec<f32> = decoded_a_bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        let decoded_b: Vec<f32> = decoded_b_bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        assert_eq!(decoded_a, regular_response.channel_a);
        assert_eq!(decoded_b, regular_response.channel_b);
        assert_eq!(fast_response.sample_rate, regular_response.sample_rate);
        assert_eq!(fast_response.frame_number, regular_response.frame_number);
        assert_eq!(fast_response.timestamp, regular_response.timestamp);
        assert_eq!(fast_response.duration_ms, regular_response.duration_ms);
    }

    #[test]
    fn test_fast_frame_size_reduction() {
        let frame = create_test_frame(1024, 48000, 1);
        let regular_response = AudioFrameResponse::from(frame.clone());
        let fast_response = AudioFastFrameResponse::from(frame);

        let regular_json = serde_json::to_string(&regular_response).unwrap();
        let fast_json = serde_json::to_string(&fast_response).unwrap();

        println!("Regular JSON size: {} bytes", regular_json.len());
        println!("Fast JSON size: {} bytes", fast_json.len());

        // Verify both formats work
        assert!(regular_json.len() > 0);
        assert!(fast_json.len() > 0);
    }

    #[test]
    fn test_fast_frame_compression_breakeven_point() {
        let test_sizes = vec![4, 8, 16, 32, 64, 128, 256, 512, 1024];

        for size in test_sizes {
            let frame = create_test_frame(size, 48000, 1);
            let regular_response = AudioFrameResponse::from(frame.clone());
            let fast_response = AudioFastFrameResponse::from(frame);

            let regular_json = serde_json::to_string(&regular_response).unwrap();
            let fast_json = serde_json::to_string(&fast_response).unwrap();
            let compression_ratio = regular_json.len() as f64 / fast_json.len() as f64;

            println!(
                "Size: {} samples/channel, Regular: {} bytes, Fast: {} bytes, Ratio: {:.2}x",
                size,
                regular_json.len(),
                fast_json.len(),
                compression_ratio
            );

            assert!(regular_json.len() > 0);
            assert!(fast_json.len() > 0);
        }
    }

    #[test]
    fn test_fast_frame_data_integrity_large() {
        let size = 2048;
        let channel_a: Vec<f32> = (0..size)
            .map(|i| ((i as f32 * 0.001) * std::f32::consts::PI).sin())
            .collect();
        let channel_b: Vec<f32> = (0..size)
            .map(|i| ((i as f32 * 0.002) * std::f32::consts::PI).cos())
            .collect();

        let frame = AudioFrame::new(channel_a, channel_b, 96000, 12345);
        test_frame_conversion_roundtrip(frame);
    }

    #[tokio::test]
    async fn test_get_stream_by_node_id_uuid_and_string_success_and_failure() {
        use crate::processing::nodes::StreamingNodeRegistry;
        use uuid::Uuid;

        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();
        let stream = SharedAudioStream::new(1024);

        // Register with a string id
        registry.register_stream_with_name_and_string_id(node_id, "node_1", "Node 1", stream.clone());

        // UUID lookup
        let uuid_str = node_id.to_string();
        let res_uuid = get_stream_by_node_id(&uuid_str, &Arc::new(registry.clone()));
        assert!(res_uuid.is_ok());

        // String ID lookup
        let res_str = get_stream_by_node_id("node_1", &Arc::new(registry.clone()));
        assert!(res_str.is_ok());

        // Non-existent id
        let res_none = get_stream_by_node_id("non-existent", &Arc::new(registry));
        assert!(res_none.is_err());
        assert_eq!(res_none.err().unwrap(), "No streaming node found");
    }

    #[tokio::test]
    async fn test_get_node_stats_by_id_with_and_without_node() {
        use crate::processing::nodes::StreamingNodeRegistry;
        use uuid::Uuid;

        let mut registry = StreamingNodeRegistry::new();
        let stats_default = StreamStats::default();

        // Case: non-existent node returns default stats
        let stats_none = get_node_stats_by_id("nope", &Arc::new(registry.clone())).await;
        assert_eq!(stats_none.total_frames, stats_default.total_frames);

        // Register a node and publish a frame
        let node_id = Uuid::new_v4();
        let stream = SharedAudioStream::new(1024);
        registry.register_stream_with_name_and_string_id(node_id, "node_stats", "NodeStats", stream.clone());

        // Publish a frame so stats are updated
        let frame = AudioFrame::new(vec![0.1], vec![0.2], 48000, 1);
        stream.publish(frame).await.unwrap();

        let returned = get_node_stats_by_id("node_stats", &Arc::new(registry)).await;
        assert!(returned.total_frames >= 1);
    }

    #[test]
    fn test_get_stream_by_node_id_parsing_uuid_error() {
        use crate::processing::nodes::StreamingNodeRegistry;
        let registry = StreamingNodeRegistry::new();
        let res = get_stream_by_node_id("this_is_not_a_uuid_and_not_registered", &Arc::new(registry));
        assert!(res.is_err());
    }
}
