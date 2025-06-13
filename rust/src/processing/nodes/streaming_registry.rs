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
    /// String identifier from the node configuration
    pub string_id: String,
    /// The shared audio stream for this node
    pub stream: SharedAudioStream,
}

/// Thread-safe registry for managing streaming nodes and their associated audio streams.
///
/// The registry maps node IDs to their corresponding `SharedAudioStream` instances,
/// allowing multiple consumers to access the same stream and enabling dynamic
/// routing based on node identifiers. The registry supports both UUID-based
/// identification (for internal operations) and string-based identification
/// (from node configurations) for API endpoints.
///
/// ### Examples
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
/// registry.register_stream_with_name_and_string_id(node_id, "my_node", "My Stream", stream.clone());
///
/// // Retrieve the stream using UUID
/// if let Some(retrieved_stream) = registry.get_stream(&node_id) {
///     // Use the stream for audio processing
/// }
///
/// // Retrieve the stream using string ID
/// if let Some(retrieved_stream) = registry.get_stream_by_string_id("my_node") {
///     // Use the stream for audio processing
/// }
///
/// // Clean up when done
/// registry.unregister_stream(&node_id);
/// ```
#[derive(Debug, Clone)]
pub struct StreamingNodeRegistry {
    /// Internal storage mapping node UUIDs to their metadata and streams
    nodes: Arc<RwLock<HashMap<Uuid, StreamingNodeMetadata>>>,
    /// Secondary index mapping string IDs to UUIDs for efficient lookup
    string_id_to_uuid: Arc<RwLock<HashMap<String, Uuid>>>,
}

impl StreamingNodeRegistry {
    /// Creates a new empty streaming node registry.
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// ```
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            string_id_to_uuid: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a new audio stream for the specified node ID with a name and string ID.
    ///
    /// If a stream was already registered for this node ID, it will be replaced
    /// with the new stream, name, and string ID.
    ///
    /// ### Arguments
    ///
    /// * `node_id` - Unique UUID identifier for the streaming node
    /// * `string_id` - String identifier from the node configuration
    /// * `name` - Human-readable name for the node
    /// * `stream` - The shared audio stream to register
    ///
    /// ### Examples
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
    /// registry.register_stream_with_name_and_string_id(node_id, "my_node", "My Audio Stream", stream);
    /// ```
    pub fn register_stream_with_name_and_string_id(
        &self,
        node_id: Uuid,
        string_id: &str,
        name: &str,
        stream: SharedAudioStream,
    ) {
        let mut nodes = self.nodes.write().unwrap();
        let mut string_id_map = self.string_id_to_uuid.write().unwrap();

        // If this UUID was already registered, remove its old string ID mapping
        if let Some(existing_metadata) = nodes.get(&node_id) {
            string_id_map.remove(&existing_metadata.string_id);
        }

        // If this string_id was already mapped to a different UUID, remove the old mapping
        if let Some(&old_uuid) = string_id_map.get(string_id) {
            if old_uuid != node_id {
                nodes.remove(&old_uuid);
            }
        }

        // Update the mappings
        string_id_map.insert(string_id.to_string(), node_id);
        nodes.insert(
            node_id,
            StreamingNodeMetadata {
                name: name.to_string(),
                string_id: string_id.to_string(),
                stream,
            },
        );
    }

    /// Registers a new audio stream for the specified node ID with a name.
    ///
    /// This is a legacy method that generates a string ID from the UUID.
    /// For new code, prefer `register_stream_with_name_and_string_id`.
    ///
    /// If a stream was already registered for this node ID, it will be replaced
    /// with the new stream and name.
    ///
    /// ### Arguments
    ///
    /// * `node_id` - Unique identifier for the streaming node
    /// * `name` - Human-readable name for the node
    /// * `stream` - The shared audio stream to register
    ///
    /// ### Examples
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
        // Generate a string ID from the UUID for backward compatibility
        let string_id = node_id.to_string();
        self.register_stream_with_name_and_string_id(node_id, &string_id, name, stream);
    }

    /// Registers a new audio stream for the specified node ID (legacy method).
    ///
    /// This method is kept for backward compatibility. It registers the stream
    /// with a default name based on the node ID.
    ///
    /// ### Arguments
    ///
    /// * `node_id` - Unique identifier for the streaming node
    /// * `stream` - The shared audio stream to register
    ///
    /// ### Examples
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
    /// ### Arguments
    ///
    /// * `node_id` - Unique identifier for the streaming node
    ///
    /// ### Returns
    ///
    /// Returns `Some(SharedAudioStream)` if a stream is registered for the node,
    /// or `None` if no stream is found.
    ///
    /// ### Examples
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

    /// Retrieves the audio stream associated with the specified string ID.
    ///
    /// ### Arguments
    ///
    /// * `string_id` - String identifier for the streaming node
    ///
    /// ### Returns
    ///
    /// Returns `Some(SharedAudioStream)` if a stream is registered for the string ID,
    /// or `None` if no stream is found.
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::nodes::StreamingNodeRegistry;
    ///
    /// let registry = StreamingNodeRegistry::new();
    /// // Assume a stream was registered with string ID "my_node"
    ///
    /// match registry.get_stream_by_string_id("my_node") {
    ///     Some(stream) => {
    ///         // Use the stream for audio processing
    ///         println!("Found stream for node with string ID 'my_node'");
    ///     },
    ///     None => {
    ///         println!("No stream registered for string ID 'my_node'");
    ///     }
    /// }
    /// ```
    pub fn get_stream_by_string_id(&self, string_id: &str) -> Option<SharedAudioStream> {
        let string_id_map = self.string_id_to_uuid.read().unwrap();
        let uuid = string_id_map.get(string_id)?;
        self.get_stream(uuid)
    }

    /// Retrieves the UUID associated with the specified string ID.
    ///
    /// ### Arguments
    ///
    /// * `string_id` - String identifier for the streaming node
    ///
    /// ### Returns
    ///
    /// Returns `Some(Uuid)` if a node is registered for the string ID,
    /// or `None` if no node is found.
    pub fn get_uuid_by_string_id(&self, string_id: &str) -> Option<Uuid> {
        let string_id_map = self.string_id_to_uuid.read().unwrap();
        string_id_map.get(string_id).copied()
    }

    /// Retrieves the string ID associated with the specified UUID.
    ///
    /// ### Arguments
    ///
    /// * `node_id` - UUID identifier for the streaming node
    ///
    /// ### Returns
    ///
    /// Returns `Some(String)` if a node is registered for the UUID,
    /// or `None` if no node is found.
    pub fn get_string_id_by_uuid(&self, node_id: &Uuid) -> Option<String> {
        let nodes = self.nodes.read().unwrap();
        nodes
            .get(node_id)
            .map(|metadata| metadata.string_id.clone())
    }

    /// Retrieves the name associated with the specified string ID.
    ///
    /// ### Arguments
    ///
    /// * `string_id` - String identifier for the streaming node
    ///
    /// ### Returns
    ///
    /// Returns `Some(String)` if a node is registered for the string ID,
    /// or `None` if no node is found.
    pub fn get_node_name_by_string_id(&self, string_id: &str) -> Option<String> {
        let string_id_map = self.string_id_to_uuid.read().unwrap();
        let uuid = string_id_map.get(string_id)?;
        self.get_node_name(uuid)
    }

    /// Retrieves the name associated with the specified node ID.
    ///
    /// ### Arguments
    ///
    /// * `node_id` - Unique identifier for the streaming node
    ///
    /// ### Returns
    ///
    /// Returns `Some(String)` if a node is registered for the ID,
    /// or `None` if no node is found.
    ///
    /// ### Examples
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
    /// ### Arguments
    ///
    /// * `node_id` - Unique identifier for the streaming node
    ///
    /// ### Returns
    ///
    /// Returns `true` if a stream was removed, `false` if no stream was registered
    /// for the specified node ID.
    ///
    /// ### Examples
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
        let mut string_id_map = self.string_id_to_uuid.write().unwrap();

        if let Some(metadata) = nodes.remove(node_id) {
            // Also remove from the string ID index
            string_id_map.remove(&metadata.string_id);
            true
        } else {
            false
        }
    }

    /// Removes the audio stream registration for the specified string ID.
    ///
    /// ### Arguments
    ///
    /// * `string_id` - String identifier for the streaming node
    ///
    /// ### Returns
    ///
    /// Returns `true` if a stream was removed, `false` if no stream was registered
    /// for the specified string ID.
    pub fn unregister_stream_by_string_id(&self, string_id: &str) -> bool {
        let string_id_map = self.string_id_to_uuid.read().unwrap();
        if let Some(&uuid) = string_id_map.get(string_id) {
            drop(string_id_map); // Release read lock before calling unregister_stream
            self.unregister_stream(&uuid)
        } else {
            false
        }
    }

    /// Returns the number of currently registered streams.
    ///
    /// ### Examples
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
    /// ### Examples
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

    /// Returns a vector of all currently registered node IDs (UUIDs).
    ///
    /// This method provides a snapshot of all active streaming nodes that can be
    /// used for administrative purposes or debugging.
    ///
    /// ### Examples
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

    /// Returns a vector of all currently registered string IDs.
    ///
    /// This method provides a snapshot of all active streaming nodes' string identifiers
    /// that can be used for API endpoints or debugging.
    ///
    /// ### Examples
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
    /// registry.register_stream_with_name_and_string_id(node_id, "my_node", "My Stream", stream);
    /// let string_ids = registry.list_all_string_ids();
    /// assert_eq!(string_ids.len(), 1);
    /// assert!(string_ids.contains(&"my_node".to_string()));
    /// ```
    pub fn list_all_string_ids(&self) -> Vec<String> {
        let string_id_map = self.string_id_to_uuid.read().unwrap();
        string_id_map.keys().cloned().collect()
    }

    /// Returns a vector of all currently registered node metadata.
    ///
    /// This method provides detailed information about all registered nodes,
    /// including their UUIDs, string IDs, and names.
    pub fn list_all_node_info(&self) -> Vec<(Uuid, String, String)> {
        let nodes = self.nodes.read().unwrap();
        nodes
            .iter()
            .map(|(&uuid, metadata)| (uuid, metadata.string_id.clone(), metadata.name.clone()))
            .collect()
    }

    /// Clears all registered streams from the registry.
    ///
    /// ### Examples
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
        let mut string_id_map = self.string_id_to_uuid.write().unwrap();
        nodes.clear();
        string_id_map.clear();
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

    #[test]
    fn test_string_id_registration_and_lookup() {
        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();
        let stream = SharedAudioStream::new(1024);
        let string_id = "test_node";
        let name = "Test Node";

        registry.register_stream_with_name_and_string_id(node_id, string_id, name, stream);

        // Test lookup by string ID
        let retrieved_stream = registry.get_stream_by_string_id(string_id);
        assert!(retrieved_stream.is_some());

        // Test UUID <-> string ID mapping
        let retrieved_uuid = registry.get_uuid_by_string_id(string_id);
        assert_eq!(retrieved_uuid, Some(node_id));

        let retrieved_string_id = registry.get_string_id_by_uuid(&node_id);
        assert_eq!(retrieved_string_id, Some(string_id.to_string()));

        // Test name lookup by string ID
        let retrieved_name = registry.get_node_name_by_string_id(string_id);
        assert_eq!(retrieved_name, Some(name.to_string()));
    }

    #[test]
    fn test_string_id_unregistration() {
        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();
        let stream = SharedAudioStream::new(1024);
        let string_id = "test_node";

        registry.register_stream_with_name_and_string_id(node_id, string_id, "Test", stream);
        assert!(!registry.is_empty());

        // Test unregistration by string ID
        let was_removed = registry.unregister_stream_by_string_id(string_id);
        assert!(was_removed);
        assert!(registry.is_empty());

        // Verify both indexes are cleaned up
        assert!(registry.get_stream_by_string_id(string_id).is_none());
        assert!(registry.get_stream(&node_id).is_none());
    }

    #[test]
    fn test_string_id_replacement() {
        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();
        let old_string_id = "old_node";
        let new_string_id = "new_node";
        let stream1 = SharedAudioStream::new(1024);
        let stream2 = SharedAudioStream::new(1024);

        // Register with old string ID
        registry.register_stream_with_name_and_string_id(
            node_id,
            old_string_id,
            "Old Node",
            stream1,
        );
        assert!(registry.get_stream_by_string_id(old_string_id).is_some());

        // Re-register same UUID with new string ID
        registry.register_stream_with_name_and_string_id(
            node_id,
            new_string_id,
            "New Node",
            stream2,
        );

        // Old string ID should no longer work
        assert!(registry.get_stream_by_string_id(old_string_id).is_none());
        // New string ID should work
        assert!(registry.get_stream_by_string_id(new_string_id).is_some());
        // UUID should still work
        assert!(registry.get_stream(&node_id).is_some());
    }

    #[test]
    fn test_register_and_get_stream_by_string_id() {
        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();
        let stream = SharedAudioStream::new(1024);

        registry.register_stream_with_name_and_string_id(
            node_id,
            "test_node",
            "Test Node",
            stream.clone(),
        );

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        // Test retrieval by UUID
        let retrieved_by_uuid = registry.get_stream(&node_id);
        assert!(retrieved_by_uuid.is_some());

        // Test retrieval by string ID
        let retrieved_by_string = registry.get_stream_by_string_id("test_node");
        assert!(retrieved_by_string.is_some());

        // Test name retrieval
        let name = registry.get_node_name_by_string_id("test_node");
        assert_eq!(name, Some("Test Node".to_string()));

        // Test string ID to UUID mapping
        let uuid = registry.get_uuid_by_string_id("test_node");
        assert_eq!(uuid, Some(node_id));

        // Test UUID to string ID mapping
        let string_id = registry.get_string_id_by_uuid(&node_id);
        assert_eq!(string_id, Some("test_node".to_string()));
    }

    #[test]
    fn test_unregister_stream_by_string_id() {
        let registry = StreamingNodeRegistry::new();
        let node_id = Uuid::new_v4();
        let stream = SharedAudioStream::new(1024);

        registry.register_stream_with_name_and_string_id(node_id, "test_node", "Test Node", stream);
        assert!(!registry.is_empty());

        let was_removed = registry.unregister_stream_by_string_id("test_node");
        assert!(was_removed);
        assert!(registry.is_empty());

        // Verify both indexes are cleaned
        assert!(registry.get_stream(&node_id).is_none());
        assert!(registry.get_stream_by_string_id("test_node").is_none());
    }

    #[test]
    fn test_list_all_string_ids() {
        let registry = StreamingNodeRegistry::new();
        let node_id1 = Uuid::new_v4();
        let node_id2 = Uuid::new_v4();
        let stream1 = SharedAudioStream::new(1024);
        let stream2 = SharedAudioStream::new(1024);

        registry.register_stream_with_name_and_string_id(node_id1, "node_1", "Node 1", stream1);
        registry.register_stream_with_name_and_string_id(node_id2, "node_2", "Node 2", stream2);

        let string_ids = registry.list_all_string_ids();
        assert_eq!(string_ids.len(), 2);
        assert!(string_ids.contains(&"node_1".to_string()));
        assert!(string_ids.contains(&"node_2".to_string()));
    }

    #[test]
    fn test_replace_stream_with_same_string_id() {
        let registry = StreamingNodeRegistry::new();
        let node_id1 = Uuid::new_v4();
        let node_id2 = Uuid::new_v4();
        let stream1 = SharedAudioStream::new(1024);
        let stream2 = SharedAudioStream::new(1024);

        // Register first stream
        registry.register_stream_with_name_and_string_id(
            node_id1,
            "shared_id",
            "First Node",
            stream1,
        );
        assert_eq!(registry.len(), 1);

        // Register second stream with same string ID but different UUID
        registry.register_stream_with_name_and_string_id(
            node_id2,
            "shared_id",
            "Second Node",
            stream2,
        );
        assert_eq!(registry.len(), 1); // Should still have only 1 entry

        // The string ID should now map to the second UUID
        let mapped_uuid = registry.get_uuid_by_string_id("shared_id");
        assert_eq!(mapped_uuid, Some(node_id2));

        // The first UUID should no longer be in the registry
        assert!(registry.get_stream(&node_id1).is_none());

        // The second UUID should be accessible
        assert!(registry.get_stream(&node_id2).is_some());
    }

    #[test]
    fn test_list_all_node_info_returns_correct_string_ids() {
        let registry = StreamingNodeRegistry::new();
        let node_id1 = Uuid::new_v4();
        let node_id2 = Uuid::new_v4();
        let stream1 = SharedAudioStream::new(1024);
        let stream2 = SharedAudioStream::new(1024);

        // Register streams with different string IDs and names
        registry.register_stream_with_name_and_string_id(
            node_id1,
            "streaming_bandpass_filter",
            "Bandpass Filter Node",
            stream1,
        );
        registry.register_stream_with_name_and_string_id(
            node_id2,
            "streaming_post_differential",
            "Post Differential Node",
            stream2,
        );

        let all_info = registry.list_all_node_info();
        assert_eq!(all_info.len(), 2);

        // Convert to a map for easier lookup
        let info_map: std::collections::HashMap<Uuid, (String, String)> = all_info
            .into_iter()
            .map(|(uuid, string_id, name)| (uuid, (string_id, name)))
            .collect();

        // Verify node 1
        let (string_id1, name1) = info_map.get(&node_id1).unwrap();
        assert_eq!(string_id1, "streaming_bandpass_filter");
        assert_eq!(name1, "Bandpass Filter Node");

        // Verify node 2
        let (string_id2, name2) = info_map.get(&node_id2).unwrap();
        assert_eq!(string_id2, "streaming_post_differential");
        assert_eq!(name2, "Post Differential Node");

        // Verify string IDs are different from UUIDs
        assert_ne!(*string_id1, node_id1.to_string());
        assert_ne!(*string_id2, node_id2.to_string());
    }

    // ...existing tests...
}
