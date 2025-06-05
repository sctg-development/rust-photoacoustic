//! Centralized registry for managing streaming nodes and their audio streams.
//! 
//! This module provides the `StreamingNodeRegistry` which acts as a central
//! coordinator for managing `SharedAudioStream` instances created by `StreamingNode`
//! instances throughout the processing graph.

use crate::acquisition::stream::SharedAudioStream;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

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
/// registry.register_stream(node_id, stream.clone());
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
    /// Internal storage mapping node IDs to their audio streams
    streams: Arc<RwLock<HashMap<Uuid, SharedAudioStream>>>,
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
            streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a new audio stream for the specified node ID.
    /// 
    /// If a stream was already registered for this node ID, it will be replaced
    /// with the new stream.
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
        let mut streams = self.streams.write().unwrap();
        streams.insert(node_id, stream);
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
        let streams = self.streams.read().unwrap();
        streams.get(node_id).cloned()
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
        let mut streams = self.streams.write().unwrap();
        streams.remove(node_id).is_some()
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
        let streams = self.streams.read().unwrap();
        streams.len()
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
        let streams = self.streams.read().unwrap();
        streams.is_empty()
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
        let streams = self.streams.read().unwrap();
        streams.keys().cloned().collect()
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
        let mut streams = self.streams.write().unwrap();
        streams.clear();
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
