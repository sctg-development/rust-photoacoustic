// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Centralized registry for managing streaming nodes and their audio streams.
//!
//! This module provides the `StreamingNodeRegistry` which acts as a central
//! coordinator for managing `SharedAudioStream` instances created by `StreamingNode`
//! instances throughout the processing graph.

use crate::acquisition::stream::SharedAudioStream;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// Information about a streaming node
#[derive(Debug, Clone)]
pub struct StreamingNodeMetadata {
    /// Human-readable name for the node
    pub name: String,
    /// The shared audio stream for this node
    pub stream: SharedAudioStream,
}

/// Thread-safe registry for managing streaming nodes and their associated audio streams.
///
/// The registry maps node IDs to their corresponding `SharedAudioStream` instances,
/// allowing multiple consumers to access the same stream and enabling dynamic
/// routing based on node identifiers.
///
/// # Examples
///
/// ```no_run
/// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
/// use rust_photoacoustic::acquisition::stream::SharedAudioStream;
/// use uuid::Uuid;
///
/// let registry = StreamingNodeRegistry::new();
/// let node_id = Uuid::new_v4();
/// let stream = SharedAudioStream::new(1024); // Buffer size parameter required
///
/// // Register a stream for a node
/// registry.register_stream_with_name(node_id, "My Stream", stream.clone());
///
/// // Retrieve the stream later
/// if let Some(retrieved_stream) = registry.get_stream(&node_id) {
///     // Use the stream for audio processing
/// }
///
/// // Clean up when done
/// registry.unregister_stream(&node_id);
/// ```
#[derive(Debug, Clone)]
pub struct StreamingNodeRegistry {
    /// Internal storage mapping node IDs to their metadata and streams
    nodes: Arc<RwLock<HashMap<Uuid, StreamingNodeMetadata>>>,
}

impl StreamingNodeRegistry {
    /// Creates a new empty streaming node registry.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// ```
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a new audio stream for the specified node ID with a name.
    ///
    /// If a stream was already registered for this node ID, it will be replaced
    /// with the new stream and name.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Unique identifier for the streaming node
    /// * `name` - Human-readable name for the node
    /// * `stream` - The shared audio stream to register
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
    /// use rust_photoacoustic::acquisition::stream::SharedAudioStream;
    /// use uuid::Uuid;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// let node_id = Uuid::new_v4();
    /// let stream = SharedAudioStream::new(1024); // Buffer size required
    ///
    /// registry.register_stream_with_name(node_id, "My Audio Stream", stream);
    /// ```
    pub fn register_stream_with_name(&self, node_id: Uuid, name: &str, stream: SharedAudioStream) {
        let mut nodes = self.nodes.write().unwrap();
        nodes.insert(
            node_id,
            StreamingNodeMetadata {
                name: name.to_string(),
                stream,
            },
        );
    }

    /// Registers a new audio stream for the specified node ID (legacy method).
    ///
    /// This method is kept for backward compatibility. It registers the stream
    /// with a default name based on the node ID.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Unique identifier for the streaming node
    /// * `stream` - The shared audio stream to register
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
    /// use rust_photoacoustic::acquisition::stream::SharedAudioStream;
    /// use uuid::Uuid;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// let node_id = Uuid::new_v4();
    /// let stream = SharedAudioStream::new(1024); // Buffer size required
    ///
    /// registry.register_stream(node_id, stream);
    /// ```
    pub fn register_stream(&self, node_id: Uuid, stream: SharedAudioStream) {
        let default_name = format!("Stream {}", &node_id.to_string()[..8]);
        self.register_stream_with_name(node_id, &default_name, stream);
    }

    /// Retrieves the audio stream associated with the specified node ID.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Unique identifier for the streaming node
    ///
    /// # Returns
    ///
    /// Returns `Some(SharedAudioStream)` if a stream is registered for the node,
    /// or `None` if no stream is found.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
    /// use uuid::Uuid;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// let node_id = Uuid::new_v4();
    ///
    /// match registry.get_stream(&node_id) {
    ///     Some(stream) => {
    ///         // Use the stream for audio processing
    ///         println!("Found stream for node {}", node_id);
    ///     },
    ///     None => {
    ///         println!("No stream registered for node {}", node_id);
    ///     }
    /// }
    /// ```
    pub fn get_stream(&self, node_id: &Uuid) -> Option<SharedAudioStream> {
        let nodes = self.nodes.read().unwrap();
        nodes.get(node_id).map(|metadata| metadata.stream.clone())
    }

    /// Retrieves the name associated with the specified node ID.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Unique identifier for the streaming node
    ///
    /// # Returns
    ///
    /// Returns `Some(String)` if a node is registered for the ID,
    /// or `None` if no node is found.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
    /// use uuid::Uuid;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// let node_id = Uuid::new_v4();
    ///
    /// match registry.get_node_name(&node_id) {
    ///     Some(name) => {
    ///         println!("Node {} is named '{}'", node_id, name);
    ///     },
    ///     None => {
    ///         println!("No node registered for ID {}", node_id);
    ///     }
    /// }
    /// ```
    pub fn get_node_name(&self, node_id: &Uuid) -> Option<String> {
        let nodes = self.nodes.read().unwrap();
        nodes.get(node_id).map(|metadata| metadata.name.clone())
    }

    /// Removes the audio stream registration for the specified node ID.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Unique identifier for the streaming node
    ///
    /// # Returns
    ///
    /// Returns `true` if a stream was removed, `false` if no stream was registered
    /// for the specified node ID.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
    /// use rust_photoacoustic::acquisition::stream::SharedAudioStream;
    /// use uuid::Uuid;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// let node_id = Uuid::new_v4();
    /// let stream = SharedAudioStream::new(1024);
    ///
    /// registry.register_stream(node_id, stream);
    ///
    /// // Later, when cleaning up
    /// let was_removed = registry.unregister_stream(&node_id);
    /// assert!(was_removed);
    /// ```
    pub fn unregister_stream(&self, node_id: &Uuid) -> bool {
        let mut nodes = self.nodes.write().unwrap();
        nodes.remove(node_id).is_some()
    }

    /// Returns the number of currently registered streams.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// assert_eq!(registry.len(), 0);
    /// ```
    pub fn len(&self) -> usize {
        let nodes = self.nodes.read().unwrap();
        nodes.len()
    }

    /// Returns `true` if no streams are currently registered.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// assert!(registry.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        let nodes = self.nodes.read().unwrap();
        nodes.is_empty()
    }

    /// Returns a vector of all currently registered node IDs.
    ///
    /// This method provides a snapshot of all active streaming nodes that can be
    /// used for administrative purposes or debugging.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
    /// use rust_photoacoustic::acquisition::stream::SharedAudioStream;
    /// use uuid::Uuid;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// let node_id = Uuid::new_v4();
    /// let stream = SharedAudioStream::new(1024);
    ///
    /// registry.register_stream(node_id, stream);
    /// let nodes = registry.list_all_nodes();
    /// assert_eq!(nodes.len(), 1);
    /// assert!(nodes.contains(&node_id));
    /// ```
    pub fn list_all_nodes(&self) -> Vec<Uuid> {
        let nodes = self.nodes.read().unwrap();
        nodes.keys().cloned().collect()
    }

    /// Clears all registered streams from the registry.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// // ... register some streams ...
    ///
    /// registry.clear();
    /// assert!(registry.is_empty());
    /// ```
    pub fn clear(&self) {
        let mut nodes = self.nodes.write().unwrap();
        nodes.clear();
    }
}

impl Default for StreamingNodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_registry_is_empty() {
        let registry = StreamingNodeRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_register_and_get_stream() {
        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();
        let stream = SharedAudioStream::new(1024);

        registry.register_stream(node_id, stream.clone());

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        let retrieved = registry.get_stream(&node_id);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_unregister_stream() {
        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();
        let stream = SharedAudioStream::new(1024);

        registry.register_stream(node_id, stream);
        assert!(!registry.is_empty());

        let was_removed = registry.unregister_stream(&node_id);
        assert!(was_removed);
        assert!(registry.is_empty());

        // Trying to remove again should return false
        let was_removed_again = registry.unregister_stream(&node_id);
        assert!(!was_removed_again);
    }

    #[test]
    fn test_get_nonexistent_stream() {
        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();

        let result = registry.get_stream(&node_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_list_all_nodes() {
        let registry = StreamingNodeRegistry::new();
        let node_id1 = Uuid::new_v4();
        let node_id2 = Uuid::new_v4();
        let stream1 = SharedAudioStream::new(1024);
        let stream2 = SharedAudioStream::new(1024);

        registry.register_stream(node_id1, stream1);
        registry.register_stream(node_id2, stream2);

        let registered_ids = registry.list_all_nodes();
        assert_eq!(registered_ids.len(), 2);
        assert!(registered_ids.contains(&node_id1));
        assert!(registered_ids.contains(&node_id2));
    }

    #[test]
    fn test_clear() {
        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();
        let stream = SharedAudioStream::new(1024);

        registry.register_stream(node_id, stream);
        assert!(!registry.is_empty());

        registry.clear();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_replace_existing_stream() {
        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();
        let stream1 = SharedAudioStream::new(1024);
        let stream2 = SharedAudioStream::new(1024);

        registry.register_stream(node_id, stream1);
        assert_eq!(registry.len(), 1);

        // Registering again should replace the existing stream
        registry.register_stream(node_id, stream2);
        assert_eq!(registry.len(), 1);

        let retrieved = registry.get_stream(&node_id);
        assert!(retrieved.is_some());
    }
}
