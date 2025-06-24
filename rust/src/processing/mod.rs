// Copyright (c) 2025 Ronan LE MEILLART and SC
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! # Processing Module
//!
//! This module provides a modular audio processing pipeline architecture similar to an audio graph.
//! It allows for real-time processing of audio frames with configurable processing nodes that can
//! be rearranged at runtime. The system also supports action drivers for external integrations,
//! measurement data collection, and comprehensive REST API access to live processing data.
//!
//! ## Architecture Overview
//!
//! The processing system consists of:
//! - **ProcessingConsumer**: Main consumer that receives audio frames from SharedAudioStream
//! - **ProcessingGraph**: Container that manages processing nodes and their connections,
//!   with methods to access action nodes and extract live measurement data
//! - **ProcessingNode**: Individual processing units with specific roles:
//!   - `InputNode`: Entry point for audio data from acquisition
//!   - `FilterNode`: Applies filters (bandpass, lowpass, highpass) to audio channels
//!   - `DifferentialNode`: Calculates differential between channels
//!   - `ChannelSelectorNode`: Selects a specific channel (A or B)
//!   - `ChannelMixerNode`: Mixes channels using various strategies
//!   - `PhotoacousticOutputNode`: Final analysis node producing photoacoustic results
//!   - `RecordNode`: Records audio data with configurable parameters
//!   - `UniversalActionNode`: Executes actions via pluggable action drivers
//! - **ActionDriver**: Pluggable drivers for external integrations:
//!   - `RedisActionDriver`: Publishes measurement data to Redis with optional TLS
//!   - `HttpsCallbackActionDriver`: Sends data via HTTPS callbacks to external APIs
//!   - `KafkaActionDriver`: Publishes measurement data to Kafka topics
//! - **ProcessingResult**: Final photoacoustic analysis result with metadata
//! - **MeasurementData**: Structured measurement data for action driver consumption
//!
//! ## Design Principles
//!
//! - **Modular**: Each processing step is encapsulated in a separate node
//! - **Configurable**: Processing graph can be created from YAML configuration
//! - **Real-time**: Designed for low-latency streaming processing
//! - **Type-safe**: Uses Rust's type system to ensure correct data flow
//! - **Runtime Reconfigurable**: Graphs can be modified and validated at runtime
//! - **Extensible**: Action drivers provide pluggable external integrations
//! - **Observable**: Full REST API access to live processing data and statistics
//!
//! ## Node Types
//!
//! ### Input Nodes
//! - `input`: Entry point for audio frames from acquisition system
//!
//! ### Filter Nodes
//! - `filter` with `type: "bandpass"`: Bandpass filter with center frequency, bandwidth, and optional order (default: 4th order = 24dB/octave)
//! - `filter` with `type: "lowpass"`: Lowpass filter with cutoff frequency and optional order (default: 1st order = 6dB/octave)
//! - `filter` with `type: "highpass"`: Highpass filter with cutoff frequency and optional order (default: 1st order = 6dB/octave)
//!
//! All filters support an `order` parameter that controls the steepness of the roll-off:
//! - Order 1: 6dB/octave roll-off (gentle)
//! - Order 2: 12dB/octave roll-off (moderate)
//! - Order 3: 18dB/octave roll-off (steep)
//! - Order 4: 24dB/octave roll-off (very steep)
//!
//! ### Channel Operations
//! - `channel_selector`: Selects ChannelA, ChannelB, or Both channels
//! - `channel_mixer`: Mixes channels using Add, Subtract, Average, or Weighted strategies
//! - `differential`: Calculates differential between channels
//!
//! ### Output Nodes
//! - `photoacoustic_output`: Final analysis node with configurable detection threshold
//! - `record`: Records audio data with configurable duration, path, and format
//!
//! ### Action Nodes
//! - `universal_action`: Executes actions via pluggable action drivers with measurement data collection
//!   - Supports RedisActionDriver, HttpsCallbackActionDriver, and KafkaActionDriver
//!   - Maintains history buffer and statistics for real-time monitoring
//!   - Exposes data via REST API endpoints
//!
//! ## Action Driver System
//!
//! The action driver system provides extensible external integrations for measurement data:
//!
//! ### Available Drivers
//!
//! - **RedisActionDriver**: Publishes measurement data to Redis with optional TLS support
//!   - Supports both secure and insecure connections
//!   - Configurable channel/key patterns
//!   - Built-in connection pooling and error handling
//! - **HttpsCallbackActionDriver**: Sends measurement data via HTTPS callbacks to external APIs
//!   - Supports authentication headers and custom timeouts
//!   - Built-in retry logic and error handling
//!   - Configurable SSL verification
//! - **KafkaActionDriver**: Publishes measurement data to Kafka topics
//!   - High-throughput streaming for enterprise applications
//!   - Configurable partitioning and serialization
//!   - Built-in connection pooling and batching
//!
//! ### Configuration Examples
//!
//! ```yaml
//! # Redis Action Driver (with TLS)
//! - id: redis_action
//!   node_type: action_universal
//!   parameters:
//!     buffer_capacity: 500                    # Store 500 measurements
//!     monitored_nodes:
//!       - "concentration_calculator"
//!     concentration_threshold: 100.0
//!     update_interval_ms: 5000                # Update every 5 seconds
//!     driver:
//!       type: "redis"
//!       config:
//!         connection_string: "rediss://localhost:6379"
//!         mode: "pubsub"                      # Use Redis pub/sub mode
//!         channel_or_prefix: "measurements"
//!         max_retries: 3
//!
//! # Redis Action Driver (insecure with key-value mode)
//! - id: redis_action_dev
//!   node_type: action_universal
//!   parameters:
//!     buffer_capacity: 100
//!     monitored_nodes:
//!       - "concentration_calculator"
//!     driver:
//!       type: "redis"
//!       config:
//!         connection_string: "redis://localhost:6379"
//!         mode: "key_value"                   # Use key-value storage mode
//!         channel_or_prefix: "photoacoustic:dev:"
//!         expiry_seconds: 3600                # Data expires after 1 hour
//!
//! # HTTPS Callback Action Driver
//! - id: https_action
//!   node_type: action_universal
//!   parameters:
//!     buffer_capacity: 200
//!     monitored_nodes:
//!       - "concentration_calculator"
//!     amplitude_threshold: 0.8
//!     driver:
//!       type: "https_callback"
//!       config:
//!         callback_url: "https://api.company.com/sensors/data"
//!         auth_header: "Authorization"
//!         auth_token: "Bearer your_token_here"
//!         timeout_seconds: 5
//!         retry_count: 3
//!
//! # Kafka Action Driver
//! - id: kafka_action
//!   node_type: action_universal
//!   parameters:
//!     buffer_capacity: 1000
//!     monitored_nodes:
//!       - "concentration_calculator"
//!     driver:
//!       type: "kafka"
//!       config:
//!         brokers: "localhost:9092"
//!         topic: "photoacoustic-measurements"
//!         client_id: "photoacoustic-sensor"
//!         partition_strategy: "consistent"
//!         batch_size: 10
//! ```
//!
//! ### TLS Best Practices
//!
//! When using RedisActionDriver with TLS, ensure that the rustls CryptoProvider is initialized
//! once at application startup:
//!
//! ```rust
//! // In main() or application initialization
//! rustls::crypto::ring::default_provider().install_default().ok();
//! ```
//!
//! This should be done before creating any RedisActionDriver instances.
//!
//! ## Configuration-Based Setup
//!
//! ```rust
//! use rust_photoacoustic::config::processing::*;
//! use rust_photoacoustic::processing::ProcessingGraph;
//!
//! // Create graph configuration with action drivers
//! let config = ProcessingGraphConfig {
//!     id: "example_graph".to_string(),
//!     nodes: vec![
//!         NodeConfig {
//!             id: "input".to_string(),
//!             node_type: "input".to_string(),
//!             parameters: serde_json::Value::Null,
//!         },
//!         NodeConfig {
//!             id: "bandpass".to_string(),
//!             node_type: "filter".to_string(),
//!             parameters: serde_json::json!({
//!                 "type": "bandpass",
//!                 "center_frequency": 1000.0,
//!                 "bandwidth": 100.0,
//!                 "order": 2, // 2nd order = 12dB/octave
//!                 "target_channel": "Both"
//!             }),
//!         },
//!         NodeConfig {
//!             id: "action_node".to_string(),
//!             node_type: "action_universal".to_string(),
//!             parameters: serde_json::json!({
//!                 "buffer_capacity": 1000,
//!                 "driver": {
//!                     "type": "redis",
//!                     "config": {
//!                         "connection_string": "rediss://localhost:6379",
//!                         "channel": "measurements"
//!                     }
//!                 }
//!             }),
//!         },
//!         NodeConfig {
//!             id: "photoacoustic".to_string(),
//!             node_type: "photoacoustic_output".to_string(),
//!             parameters: serde_json::json!({
//!                 "detection_threshold": 0.1,
//!                 "analysis_window_size": 1024
//!             }),
//!         },
//!     ],
//!     connections: vec![
//!         ConnectionConfig {
//!             from: "input".to_string(),
//!             to: "bandpass".to_string(),
//!         },
//!         ConnectionConfig {
//!             from: "bandpass".to_string(),
//!             to: "action_node".to_string(),
//!         },
//!         ConnectionConfig {
//!             from: "action_node".to_string(),
//!             to: "photoacoustic".to_string(),
//!         },
//!     ],
//!     output_node: Some("photoacoustic".to_string()),
//! };
//!
//! // Initialize TLS support (once per application)
//! rustls::crypto::ring::default_provider().install_default().ok();
//!
//! // Create graph from configuration
//! let graph = ProcessingGraph::from_config(&config)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## REST API Access
//!
//! The processing system exposes comprehensive REST API endpoints for accessing live processing
//! data, action node statistics, and measurement history. All endpoints require JWT authentication
//! and provide OpenAPI documentation.
//!
//! ### Available Endpoints
//!
//! - **GET /api/action**: List all action nodes in the current processing graph
//! - **GET /api/action/{node_id}/history**: Retrieve measurement history for a specific action node
//! - **GET /api/action/{node_id}/history/stats**: Get statistics for a specific action node
//!
//! ### Example API Usage
//!
//! ```bash
//! # Get JWT token using the provided utility
//! TOKEN=$(create_token --config config.example.yaml --user administrator --client LaserSmartClient --quiet)
//!
//! # List all action nodes
//! curl -H "Authorization: Bearer $TOKEN" "https://localhost:8080/api/action"
//!
//! # Get measurement history for a specific node
//! curl -H "Authorization: Bearer $TOKEN" \
//!      "https://localhost:8080/api/action/redis_action/history?limit=100"
//!
//! # Get node statistics
//! curl -H "Authorization: Bearer $TOKEN" \
//!      "https://localhost:8080/api/action/redis_action/history/stats"
//! ```
//!
//! ### Response Formats
//!
//! #### List all action nodes response:
//!
//! ```json
//! [
//!   {
//!     "id": "redis_stream_action",
//!     "node_type": "action_universal",
//!     "has_driver": true,
//!     "monitored_nodes_count": 1,
//!     "buffer_size": 85,
//!     "buffer_capacity": 100
//!   }
//! ]
//! ```
//!
//! #### Get measurement history response:
//!
//! ```json
//! [
//!   {
//!     "concentration_ppm": 456.78,
//!     "source_node_id": "concentration_calculator",
//!     "peak_amplitude": 0.85,
//!     "peak_frequency": 2000.5,
//!     "timestamp": 1640995200,
//!     "metadata": {
//!       "trigger_type": "concentration_threshold",
//!       "alert_message": "High concentration detected"
//!     }
//!   }
//! ]
//! ```
//!
//! #### Get node statistics response:
//!
//! ```json
//! {
//!   "node_id": "redis_stream_action",
//!   "node_type": "action_universal",
//!   "history_buffer": {
//!     "capacity": 100,
//!     "current_size": 85,
//!     "is_full": false,
//!     "oldest_entry_timestamp": 1640995000,
//!     "newest_entry_timestamp": 1640995200
//!   },
//!   "configuration": {
//!     "monitored_nodes": ["concentration_calculator"],
//!     "concentration_threshold": 100.0,
//!     "amplitude_threshold": 0.65,
//!     "update_interval_ms": 5000
//!   },
//!   "driver_info": {
//!     "has_driver": true,
//!     "driver_type": "configured"
//!   },
//!   "performance": {
//!     "processing_count": 1250,
//!     "actions_triggered": 15,
//!     "last_update_time": 1640995200,
//!     "last_action_update": 1640995195
//!   }
//! }
//! ```
//!
//! #### Error responses:
//!
//! ```text
//! HTTP 404 Not Found        - Node not found
//! HTTP 500 Internal Server  - Failed to access processing graph
//! ```
//!
//! ## Example Usage
//!
//! ```rust
//! use rust_photoacoustic::processing::*;
//! use rust_photoacoustic::processing::nodes::*;
//! use rust_photoacoustic::processing::computing_nodes::action_drivers::*;
//! use rust_photoacoustic::preprocessing::filters::{BandpassFilter, HighpassFilter, LowpassFilter};
//! use rust_photoacoustic::preprocessing::differential::SimpleDifferential;
//! use rust_photoacoustic::acquisition::AudioFrame;
//!
//! // Initialize TLS support (once per application)
//! rustls::crypto::ring::default_provider().install_default().ok();
//!
//! // Create processing graph
//! let mut graph = ProcessingGraph::new();
//!
//! // Add processing nodes
//! let input_node = Box::new(InputNode::new("input".to_string()));
//! let filter_node = Box::new(FilterNode::new(
//!     "bandpass".to_string(),
//!     Box::new(BandpassFilter::new(1000.0, 100.0).with_order(4)), // 4th order = 24dB/octave
//!     ChannelTarget::ChannelA,
//! ));
//!
//! // Add action node with Redis driver
//! let redis_driver = Box::new(RedisActionDriver::new_pubsub(
//!     "rediss://localhost:6379".to_string(),
//!     "measurements".to_string(),
//! ));
//!
//! // Create action node
//! let action_node = UniversalActionNode::new("redis_action".to_string())
//!     .with_driver(redis_driver)
//!     .with_history_buffer_capacity(1000);
//!
//! // Box it for adding to the graph
//! let action_node = Box::new(action_node);
//!
//! let diff_node = Box::new(DifferentialNode::new(
//!     "diff".to_string(),
//!     Box::new(SimpleDifferential::new()),
//! ));
//! let output_node = Box::new(PhotoacousticOutputNode::new("photoacoustic".to_string()));
//!
//! graph.add_node(input_node)?;
//! graph.add_node(filter_node)?;
//! graph.add_node(action_node)?;
//! graph.add_node(diff_node)?;
//! graph.add_node(output_node)?;
//!
//! // Connect nodes in sequence
//! graph.connect("input", "bandpass")?;
//! graph.connect("bandpass", "redis_action")?;
//! graph.connect("redis_action", "diff")?;
//! graph.connect("diff", "photoacoustic")?;
//!
//! // Set output node
//! graph.set_output_node("photoacoustic")?;
//!
//! // Create some example audio data
//! let audio_frame = AudioFrame {
//!     channel_a: vec![0.1, 0.2, 0.3, 0.4],
//!     channel_b: vec![0.05, 0.15, 0.25, 0.35],
//!     sample_rate: 44100,
//!     timestamp: 1000,
//!     frame_number: 1,
//! };
//!
//! // Execute processing with input data
//! let input_data = ProcessingData::from_audio_frame(audio_frame);
//! let results = graph.execute(input_data)?;
//!
//! // Access action node data via ProcessingGraph methods
//! let action_nodes = graph.get_universal_action_node_ids();
//! if let Some(node_id) = action_nodes.first() {
//!     if let Some(action_node) = graph.get_universal_action_node(node_id) {
//!         // If we have an action node, we can access its history and statistics
//!         let history = action_node.get_measurement_history(Some(10));
//!         let stats = action_node.get_history_statistics();
//!         println!("Action node {} has {} measurements", node_id, history.len());
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Integration Testing
//!
//! The processing module includes comprehensive integration tests that validate the entire
//! pipeline with real ProcessingGraph instances and JWT authentication:
//!
//! ```bash
//! # Run integration tests
//! cargo test action_api_test -- --nocapture
//!
//! # Run all processing tests
//! cargo test processing:: -- --nocapture
//! ```
//!
//! These tests verify:
//! - Action driver functionality with real Redis connections
//! - REST API endpoints with proper authentication
//! - ProcessingGraph integration with action nodes
//! - Error handling and fallback mechanisms

pub mod computing_nodes;
pub mod consumer;
pub mod graph;
pub mod nodes;
pub mod result;

pub use consumer::ProcessingConsumer;
pub use graph::{
    PerformanceSummary, ProcessingGraph, ProcessingGraphError, SerializableConnection,
    SerializableNode, SerializableProcessingGraph,
};
pub use nodes::{
    ChannelMixerNode, ChannelSelectorNode, ChannelTarget, DifferentialNode, FilterNode, InputNode,
    MixStrategy, NodeId, PhotoacousticOutputNode, ProcessingData, ProcessingNode, RecordNode,
};
pub use result::{PhotoacousticAnalysis, ProcessingResult};

// Re-export action-related types from computing_nodes
pub use computing_nodes::{
    action_drivers::{
        ActionDriver, AlertData, HttpsCallbackActionDriver, KafkaActionDriver, MeasurementData,
        RedisActionDriver,
    },
    universal_action::UniversalActionNode,
};
