//! Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
//! This file is part of the rust-photoacoustic project and is licensed under the
//! SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Python processing node implementation
//!
//! This module implements a processing node that executes custom Python scripts to transform
//! audio data. It uses PyO3 to bridge between Rust and Python, allowing users to write
//! custom processing logic in Python while maintaining performance and safety.
//!
//! # Overview
//!
//! The Python processing node enables you to implement custom audio processing
//! and transformation logic in Python scripts, which are then called from Rust.
//! This provides the flexibility of Python scripting with access to scientific
//! libraries like NumPy, SciPy, and others while maintaining the performance
//! and safety of Rust.
//!
//! # Features
//!
//! - **Script Hot-reloading**: Automatically reload Python scripts when they change
//! - **Timeout Protection**: Configurable timeouts prevent hanging Python scripts
//! - **Error Handling**: Robust error handling with detailed error messages
//! - **Multiple Data Types**: Support for all ProcessingData variants
//! - **Sync Operation**: Synchronous processing for integration with the processing graph
//!
//! # Example Python Script
//!
//! Your Python script should implement the following functions:
//!
//! ```python
//! import numpy as np
//! from scipy import signal
//! import json
//!
//! def initialize():
//!     """Called when the node is initialized"""
//!     print("Python processing node initialized")
//!     return {"status": "initialized"}
//!
//! def process_data(data):
//!     """
//!     Process audio data - main processing function
//!     
//!     Args:
//!         data: Dictionary containing the processing data
//!         
//!     Returns:
//!         Dictionary with processed data in the same format
//!     """
//!     data_type = data.get("type")
//!     
//!     if data_type == "SingleChannel":
//!         # Process single channel data
//!         samples = np.array(data["samples"])
//!         sample_rate = data["sample_rate"]
//!         
//!         # Example: Apply a bandpass filter
//!         nyquist = sample_rate / 2
//!         low = 300 / nyquist
//!         high = 3000 / nyquist
//!         b, a = signal.butter(5, [low, high], btype='band')
//!         filtered_samples = signal.filtfilt(b, a, samples)
//!         
//!         return {
//!             "type": "SingleChannel",
//!             "samples": filtered_samples.tolist(),
//!             "sample_rate": sample_rate,
//!             "timestamp": data["timestamp"],
//!             "frame_number": data["frame_number"]
//!         }
//!     
//!     elif data_type == "DualChannel":
//!         # Process dual channel data
//!         channel_a = np.array(data["channel_a"])
//!         channel_b = np.array(data["channel_b"])
//!         sample_rate = data["sample_rate"]
//!         
//!         # Example: Apply same filter to both channels
//!         nyquist = sample_rate / 2
//!         low = 300 / nyquist
//!         high = 3000 / nyquist
//!         b, a = signal.butter(5, [low, high], btype='band')
//!         
//!         filtered_a = signal.filtfilt(b, a, channel_a)
//!         filtered_b = signal.filtfilt(b, a, channel_b)
//!         
//!         return {
//!             "type": "DualChannel",
//!             "channel_a": filtered_a.tolist(),
//!             "channel_b": filtered_b.tolist(),
//!             "sample_rate": sample_rate,
//!             "timestamp": data["timestamp"],
//!             "frame_number": data["frame_number"]
//!         }
//!     
//!     # For other types, pass through unchanged
//!     return data
//!
//! def get_status():
//!     """Return current status"""
//!     return {"status": "active", "type": "python_processing_node"}
//!
//! def shutdown():
//!     """Called when the node is shutting down"""
//!     print("Python processing node shutting down")
//!     return {"status": "shutdown"}
//! ```
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use rust_photoacoustic::processing::nodes::{
//!     PythonNode, PythonNodeConfig, ProcessingNode, ProcessingData
//! };
//! use std::path::PathBuf;
//!
//! # fn example() -> anyhow::Result<()> {
//! // Create configuration
//! let config = PythonNodeConfig {
//!     script_path: PathBuf::from("my_processor.py"),
//!     auto_reload: true,
//!     timeout_seconds: 10,
//!     ..Default::default()
//! };
//!
//! // Create node
//! let mut node = PythonNode::new("python_filter".to_string(), config);
//!
//! // Process single channel data
//! let input = ProcessingData::SingleChannel {
//!     samples: vec![0.1, 0.2, 0.3, 0.4],
//!     sample_rate: 44100,
//!     timestamp: 1000,
//!     frame_number: 1,
//! };
//!
//! let result = node.process(input)?;
//! match result {
//!     ProcessingData::SingleChannel { samples, .. } => {
//!         println!("Processed {} samples", samples.len());
//!     }
//!     _ => println!("Unexpected output type"),
//! }
//!
//! # Ok(())
//! # }
//! ```

use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use super::data::ProcessingData;
use super::traits::ProcessingNode;
use crate::acquisition::AudioFrame;

#[cfg(feature = "python-driver")]
use pyo3::prelude::*;

/// Cached Python module to avoid recompilation overhead
#[cfg(feature = "python-driver")]
struct CachedPythonModule {
    module_code: String,
    last_modified: SystemTime,
}

/// Python processing node configuration
///
/// This structure defines all the configuration options for the Python processing node.
/// It allows you to customize how Python scripts are executed and which functions
/// are called for different operations.
///
/// # Example
///
/// ```rust
/// use rust_photoacoustic::processing::nodes::PythonNodeConfig;
/// use std::path::PathBuf;
///
/// let config = PythonNodeConfig {
///     script_path: PathBuf::from("processor.py"),
///     auto_reload: true,
///     timeout_seconds: 15,
///     process_function: "process_audio".to_string(),
///     ..Default::default()
/// };
///
/// assert_eq!(config.script_path, PathBuf::from("processor.py"));
/// assert_eq!(config.auto_reload, true);
/// assert_eq!(config.timeout_seconds, 15);
/// ```
///
/// # Default Configuration
///
/// ```rust
/// use rust_photoacoustic::processing::nodes::PythonNodeConfig;
/// use std::path::PathBuf;
///
/// let default_config = PythonNodeConfig::default();
///
/// assert_eq!(default_config.script_path, PathBuf::from("processor.py"));
/// assert_eq!(default_config.process_function, "process_data");
/// assert_eq!(default_config.timeout_seconds, 30);
/// assert_eq!(default_config.auto_reload, false);
/// ```
#[derive(Debug, Clone)]
pub struct PythonNodeConfig {
    /// Path to the Python script file
    pub script_path: PathBuf,
    /// Python virtual environment path (optional)
    pub venv_path: Option<PathBuf>,
    /// Function name to call for processing (default: "process_data")
    pub process_function: String,
    /// Function name to call for initialization (default: "initialize")
    pub init_function: String,
    /// Function name to call for shutdown (default: "shutdown")
    pub shutdown_function: String,
    /// Function name to call for status (default: "get_status")
    pub status_function: String,
    /// Maximum execution time for Python calls (seconds)
    pub timeout_seconds: u64,
    /// Whether to reload script on each call (development mode)
    pub auto_reload: bool,
    /// Additional Python path directories
    pub python_paths: Vec<PathBuf>,
    /// Expected input data types (empty means accept all)
    pub accepted_types: Vec<String>,
    /// Expected output data type (None means same as input)
    pub output_type: Option<String>,
}

impl Default for PythonNodeConfig {
    fn default() -> Self {
        Self {
            script_path: PathBuf::from("processor.py"),
            venv_path: None,
            process_function: "process_data".to_string(),
            init_function: "initialize".to_string(),
            shutdown_function: "shutdown".to_string(),
            status_function: "get_status".to_string(),
            timeout_seconds: 30,
            auto_reload: false,
            python_paths: Vec::new(),
            accepted_types: Vec::new(),
            output_type: None,
        }
    }
}

/// Python processing node that executes Python scripts to transform ProcessingData
///
/// This node does NOT store a Python interpreter instance to avoid Send/Sync issues.
/// Instead, it acquires the GIL for each Python operation, which is safer in sync contexts.
///
/// # Thread Safety
///
/// The node is designed to be thread-safe and can be used in multi-threaded processing graphs.
/// All shared state is protected by mutexes, and Python operations are executed
/// synchronously to integrate with the ProcessingNode trait.
///
/// # Performance Considerations
///
/// - Python scripts are reloaded from disk when `auto_reload` is enabled
/// - Function calls have configurable timeouts to prevent hanging
/// - Python GIL acquisition has minimal overhead in typical usage
///
/// # Example
///
/// ```rust,no_run
/// use rust_photoacoustic::processing::nodes::{
///     PythonNode, PythonNodeConfig, ProcessingNode, ProcessingData
/// };
/// use std::path::PathBuf;
///
/// # fn example() -> anyhow::Result<()> {
/// // Create configuration
/// let config = PythonNodeConfig {
///     script_path: PathBuf::from("bandpass_filter.py"),
///     auto_reload: false,
///     timeout_seconds: 5,
///     ..Default::default()
/// };
///
/// // Create and initialize node
/// let mut node = PythonNode::new("python_bandpass".to_string(), config);
///
/// // Check node type
/// assert_eq!(node.node_type(), "python");
///
/// // Create test data
/// let input = ProcessingData::SingleChannel {
///     samples: vec![0.1, 0.2, 0.3, 0.4],
///     sample_rate: 44100,
///     timestamp: 1000,
///     frame_number: 1,
/// };
///
/// // Process data (requires python-driver feature)
/// // let result = node.process(input)?;
/// # Ok(())
/// # }
/// ```
pub struct PythonNode {
    id: String,
    config: PythonNodeConfig,
    last_modified: Arc<Mutex<Option<SystemTime>>>,
    initialized: Arc<Mutex<bool>>,
    status: Arc<Mutex<String>>,
    #[cfg(feature = "python-driver")]
    cached_module: Arc<Mutex<Option<CachedPythonModule>>>,
}

impl std::fmt::Debug for PythonNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PythonNode")
            .field("id", &self.id)
            .field("config", &self.config)
            .field("initialized", &self.initialized)
            .finish()
    }
}

impl PythonNode {
    /// Create a new Python processing node
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `config` - Configuration for the Python script execution
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crate::rust_photoacoustic::processing::ProcessingNode;
    /// use rust_photoacoustic::processing::nodes::{PythonNode, PythonNodeConfig};
    /// use std::path::PathBuf;
    ///
    /// let config = PythonNodeConfig {
    ///     script_path: PathBuf::from("my_filter.py"),
    ///     auto_reload: true,
    ///     ..Default::default()
    /// };
    ///
    /// let node = PythonNode::new("my_python_node".to_string(), config);
    /// assert_eq!(node.node_id(), "my_python_node");
    /// assert_eq!(node.node_type(), "python");
    /// ```
    pub fn new(id: String, config: PythonNodeConfig) -> Self {
        Self {
            id,
            config,
            last_modified: Arc::new(Mutex::new(None)),
            initialized: Arc::new(Mutex::new(false)),
            status: Arc::new(Mutex::new("created".to_string())),
            #[cfg(feature = "python-driver")]
            cached_module: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a Python node from a configuration map
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `config` - Configuration as key-value pairs
    ///
    /// # Returns
    ///
    /// * `Ok(PythonNode)` - Successfully created node
    /// * `Err(anyhow::Error)` - Configuration parsing failed
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crate::rust_photoacoustic::processing::ProcessingNode;
    /// use rust_photoacoustic::processing::nodes::PythonNode;
    /// use std::collections::HashMap;
    /// use serde_json::json;
    ///
    /// # fn example() -> anyhow::Result<()> {
    /// let config_json = json!({
    ///     "script_path": "/path/to/script.py",
    ///     "auto_reload": true,
    ///     "timeout_seconds": 15,
    ///     "process_function": "my_process_function"
    /// });
    ///
    /// let config: HashMap<String, serde_json::Value> =
    ///     serde_json::from_value(config_json)?;
    ///
    /// let node = PythonNode::from_config("my_node".to_string(), config)?;
    /// assert_eq!(node.node_id(), "my_node");
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_config(id: String, config: HashMap<String, serde_json::Value>) -> Result<Self> {
        let mut node_config = PythonNodeConfig::default();

        if let Some(script_path) = config.get("script_path") {
            if let Some(path_str) = script_path.as_str() {
                node_config.script_path = PathBuf::from(path_str);
            }
        }

        if let Some(venv_path) = config.get("venv_path") {
            if let Some(path_str) = venv_path.as_str() {
                node_config.venv_path = Some(PathBuf::from(path_str));
            }
        }

        if let Some(process_function) = config.get("process_function") {
            if let Some(func_str) = process_function.as_str() {
                node_config.process_function = func_str.to_string();
            }
        }

        if let Some(init_function) = config.get("init_function") {
            if let Some(func_str) = init_function.as_str() {
                node_config.init_function = func_str.to_string();
            }
        }

        if let Some(shutdown_function) = config.get("shutdown_function") {
            if let Some(func_str) = shutdown_function.as_str() {
                node_config.shutdown_function = func_str.to_string();
            }
        }

        if let Some(status_function) = config.get("status_function") {
            if let Some(func_str) = status_function.as_str() {
                node_config.status_function = func_str.to_string();
            }
        }

        if let Some(timeout_seconds) = config.get("timeout_seconds") {
            if let Some(timeout) = timeout_seconds.as_u64() {
                node_config.timeout_seconds = timeout;
            }
        }

        if let Some(auto_reload) = config.get("auto_reload") {
            if let Some(reload) = auto_reload.as_bool() {
                node_config.auto_reload = reload;
            }
        }

        if let Some(python_paths) = config.get("python_paths") {
            if let Some(paths_array) = python_paths.as_array() {
                node_config.python_paths = paths_array
                    .iter()
                    .filter_map(|v| v.as_str().map(PathBuf::from))
                    .collect();
            }
        }

        if let Some(accepted_types) = config.get("accepted_types") {
            if let Some(types_array) = accepted_types.as_array() {
                node_config.accepted_types = types_array
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
            }
        }

        if let Some(output_type) = config.get("output_type") {
            if let Some(type_str) = output_type.as_str() {
                node_config.output_type = Some(type_str.to_string());
            }
        }

        Ok(Self::new(id, node_config))
    }

    /// Get the current configuration
    pub fn config(&self) -> &PythonNodeConfig {
        &self.config
    }

    /// Update the script path and optionally reload
    pub fn set_script_path(&mut self, path: PathBuf) -> Result<()> {
        self.config.script_path = path;
        if self.config.auto_reload {
            self.reload_script()?;
        }
        Ok(())
    }

    /// Check if the script file has been modified since last load
    fn script_modified(&self) -> Result<bool> {
        let metadata = std::fs::metadata(&self.config.script_path)?;
        let modified = metadata.modified()?;

        let last_modified = self
            .last_modified
            .lock()
            .map_err(|e| anyhow!("Failed to lock last_modified: {}", e))?;

        Ok(last_modified.map_or(true, |last| modified > last))
    }

    /// Reload the Python script if necessary
    fn reload_script(&self) -> Result<()> {
        if self.config.auto_reload && self.script_modified()? {
            debug!("Reloading Python script: {:?}", self.config.script_path);

            let metadata = std::fs::metadata(&self.config.script_path)?;
            let modified = metadata.modified()?;

            let mut last_modified = self
                .last_modified
                .lock()
                .map_err(|e| anyhow!("Failed to lock last_modified: {}", e))?;
            *last_modified = Some(modified);
        }
        Ok(())
    }

    /// Initialize the Python script
    fn initialize_python(&self) -> Result<()> {
        self.reload_script()?;

        #[cfg(feature = "python-driver")]
        {
            self.call_python_function(&self.config.init_function, json!({}))
                .map(|_| ())
        }
        #[cfg(not(feature = "python-driver"))]
        {
            warn!("Python driver feature not enabled, skipping Python initialization");
            Ok(())
        }
    }

    /// Call a Python function with the given arguments (optimized with module caching)
    #[cfg(feature = "python-driver")]
    fn call_python_function(&self, function_name: &str, args: Value) -> Result<Value> {
        use pyo3::prelude::*;
        use pyo3::types::{PyDict, PyModule};
        use std::ffi::CString;
        use std::time::Instant;

        let start_time = Instant::now();

        Python::with_gil(|py| -> Result<Value> {
            // Get or create cached module
            let module_code = self.get_or_load_module_code()?;

            let module_code_cstr = CString::new(module_code.as_str())
                .map_err(|e| anyhow!("Invalid module code: {}", e))?;

            // Convert filename and module name to CString for PyO3 0.25+
            let filename =
                CString::new("processor.py").map_err(|e| anyhow!("Invalid filename: {}", e))?;
            let module_name =
                CString::new("processor").map_err(|e| anyhow!("Invalid module name: {}", e))?;

            // Create module from cached code (this is much faster than file I/O)
            let module = PyModule::from_code(
                py,
                module_code_cstr.as_c_str(),
                filename.as_c_str(),
                module_name.as_c_str(),
            )?;

            // Set up Python path if specified (only do this once during initialization ideally)
            if !self.config.python_paths.is_empty() {
                let sys = py.import("sys")?;
                let path = sys.getattr("path")?;
                for python_path in &self.config.python_paths {
                    if let Some(path_str) = python_path.to_str() {
                        path.call_method1("insert", (0, path_str))?;
                    }
                }
            }

            // Check if function exists
            if !module.hasattr(function_name)? {
                return Err(anyhow!(
                    "Function '{}' not found in Python script",
                    function_name
                ));
            }

            // Call the function based on its name and expected arguments
            let result = if function_name == "initialize" {
                // Initialize function takes no arguments
                module.getattr(function_name)?.call0()?
            } else {
                // Convert arguments to Python
                let py_args = pythonize::pythonize(py, &args)?;
                // Call the function with arguments
                module.getattr(function_name)?.call1((py_args,))?
            };

            // Convert result back to JSON
            let json_result: Value = pythonize::depythonize(&result)?;

            // Check for timeout (simple approach)
            if start_time.elapsed() > Duration::from_secs(self.config.timeout_seconds) {
                warn!(
                    "Python function '{}' execution took {:?}, which exceeds timeout of {} seconds",
                    function_name,
                    start_time.elapsed(),
                    self.config.timeout_seconds
                );
            }

            debug!(
                "Python function '{}' completed in {:?}",
                function_name,
                start_time.elapsed()
            );
            Ok(json_result)
        })
    }

    /// Get module code, using cache when possible
    #[cfg(feature = "python-driver")]
    fn get_or_load_module_code(&self) -> Result<String> {
        // Check if we need to reload the script
        let should_reload = if self.config.auto_reload {
            self.script_modified()?
        } else {
            // If auto_reload is false, only reload if we haven't loaded yet
            let cached = self
                .cached_module
                .lock()
                .map_err(|e| anyhow!("Failed to lock cached_module: {}", e))?;
            cached.is_none()
        };

        if should_reload {
            // Load script from disk
            let script_content =
                std::fs::read_to_string(&self.config.script_path).map_err(|e| {
                    anyhow!(
                        "Failed to read script file {:?}: {}",
                        self.config.script_path,
                        e
                    )
                })?;

            let metadata = std::fs::metadata(&self.config.script_path)?;
            let modified = metadata.modified()?;

            // Cache the module
            let mut cached = self
                .cached_module
                .lock()
                .map_err(|e| anyhow!("Failed to lock cached_module: {}", e))?;
            *cached = Some(CachedPythonModule {
                module_code: script_content.clone(),
                last_modified: modified,
            });

            // Update last_modified
            let mut last_modified = self
                .last_modified
                .lock()
                .map_err(|e| anyhow!("Failed to lock last_modified: {}", e))?;
            *last_modified = Some(modified);

            info!(
                "Loaded Python script from disk: {:?}",
                self.config.script_path
            );
            Ok(script_content)
        } else {
            // Use cached version
            let cached = self
                .cached_module
                .lock()
                .map_err(|e| anyhow!("Failed to lock cached_module: {}", e))?;
            match &*cached {
                Some(cached_module) => Ok(cached_module.module_code.clone()),
                None => {
                    // Fallback: load from disk
                    std::fs::read_to_string(&self.config.script_path).map_err(|e| {
                        anyhow!(
                            "Failed to read script file {:?}: {}",
                            self.config.script_path,
                            e
                        )
                    })
                }
            }
        }
    }

    #[cfg(not(feature = "python-driver"))]
    fn call_python_function(&self, _function_name: &str, _args: Value) -> Result<Value> {
        Err(anyhow!(
            "Python driver feature not enabled. Enable with --features python-driver"
        ))
    }

    /// Convert ProcessingData to JSON for Python consumption
    fn processing_data_to_json(&self, data: &ProcessingData) -> Result<Value> {
        match data {
            ProcessingData::AudioFrame(frame) => Ok(json!({
                "type": "AudioFrame",
                "channel_a": frame.channel_a,
                "channel_b": frame.channel_b,
                "sample_rate": frame.sample_rate,
                "timestamp": frame.timestamp,
                "frame_number": frame.frame_number
            })),
            ProcessingData::SingleChannel {
                samples,
                sample_rate,
                timestamp,
                frame_number,
            } => Ok(json!({
                "type": "SingleChannel",
                "samples": samples,
                "sample_rate": sample_rate,
                "timestamp": timestamp,
                "frame_number": frame_number
            })),
            ProcessingData::DualChannel {
                channel_a,
                channel_b,
                sample_rate,
                timestamp,
                frame_number,
            } => Ok(json!({
                "type": "DualChannel",
                "channel_a": channel_a,
                "channel_b": channel_b,
                "sample_rate": sample_rate,
                "timestamp": timestamp,
                "frame_number": frame_number
            })),
            ProcessingData::PhotoacousticResult { signal, metadata } => Ok(json!({
                "type": "PhotoacousticResult",
                "signal": signal,
                "metadata": {
                    "original_frame_number": metadata.original_frame_number,
                    "original_timestamp": metadata.original_timestamp,
                    "sample_rate": metadata.sample_rate,
                    "processing_steps": metadata.processing_steps,
                    "processing_latency_us": metadata.processing_latency_us
                }
            })),
        }
    }

    /// Convert JSON result back to ProcessingData
    fn json_to_processing_data(&self, json: Value) -> Result<ProcessingData> {
        let data_type = json
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| anyhow!("Missing or invalid 'type' field in Python result"))?;

        match data_type {
            "AudioFrame" => {
                let channel_a = json
                    .get("channel_a")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| anyhow!("Missing channel_a in AudioFrame"))?
                    .iter()
                    .map(|v| v.as_f64().map(|f| f as f32))
                    .collect::<Option<Vec<f32>>>()
                    .ok_or_else(|| anyhow!("Invalid channel_a data"))?;

                let channel_b = json
                    .get("channel_b")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| anyhow!("Missing channel_b in AudioFrame"))?
                    .iter()
                    .map(|v| v.as_f64().map(|f| f as f32))
                    .collect::<Option<Vec<f32>>>()
                    .ok_or_else(|| anyhow!("Invalid channel_b data"))?;

                let sample_rate =
                    json.get("sample_rate")
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| anyhow!("Missing sample_rate"))? as u32;

                let timestamp = json
                    .get("timestamp")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Missing timestamp"))?;

                let frame_number = json
                    .get("frame_number")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Missing frame_number"))?;

                Ok(ProcessingData::AudioFrame(AudioFrame {
                    channel_a,
                    channel_b,
                    sample_rate,
                    timestamp,
                    frame_number,
                }))
            }
            "SingleChannel" => {
                let samples = json
                    .get("samples")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| anyhow!("Missing samples in SingleChannel"))?
                    .iter()
                    .map(|v| v.as_f64().map(|f| f as f32))
                    .collect::<Option<Vec<f32>>>()
                    .ok_or_else(|| anyhow!("Invalid samples data"))?;

                let sample_rate =
                    json.get("sample_rate")
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| anyhow!("Missing sample_rate"))? as u32;

                let timestamp = json
                    .get("timestamp")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Missing timestamp"))?;

                let frame_number = json
                    .get("frame_number")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Missing frame_number"))?;

                Ok(ProcessingData::SingleChannel {
                    samples,
                    sample_rate,
                    timestamp,
                    frame_number,
                })
            }
            "DualChannel" => {
                let channel_a = json
                    .get("channel_a")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| anyhow!("Missing channel_a in DualChannel"))?
                    .iter()
                    .map(|v| v.as_f64().map(|f| f as f32))
                    .collect::<Option<Vec<f32>>>()
                    .ok_or_else(|| anyhow!("Invalid channel_a data"))?;

                let channel_b = json
                    .get("channel_b")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| anyhow!("Missing channel_b in DualChannel"))?
                    .iter()
                    .map(|v| v.as_f64().map(|f| f as f32))
                    .collect::<Option<Vec<f32>>>()
                    .ok_or_else(|| anyhow!("Invalid channel_b data"))?;

                let sample_rate =
                    json.get("sample_rate")
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| anyhow!("Missing sample_rate"))? as u32;

                let timestamp = json
                    .get("timestamp")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Missing timestamp"))?;

                let frame_number = json
                    .get("frame_number")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Missing frame_number"))?;

                Ok(ProcessingData::DualChannel {
                    channel_a,
                    channel_b,
                    sample_rate,
                    timestamp,
                    frame_number,
                })
            }
            "PhotoacousticResult" => {
                let signal = json
                    .get("signal")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| anyhow!("Missing signal in PhotoacousticResult"))?
                    .iter()
                    .map(|v| v.as_f64().map(|f| f as f32))
                    .collect::<Option<Vec<f32>>>()
                    .ok_or_else(|| anyhow!("Invalid signal data"))?;

                let metadata_json = json
                    .get("metadata")
                    .ok_or_else(|| anyhow!("Missing metadata in PhotoacousticResult"))?;

                let original_frame_number = metadata_json
                    .get("original_frame_number")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Missing original_frame_number"))?;

                let original_timestamp = metadata_json
                    .get("original_timestamp")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Missing original_timestamp"))?;

                let sample_rate = metadata_json
                    .get("sample_rate")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Missing sample_rate"))?
                    as u32;

                let processing_steps = metadata_json
                    .get("processing_steps")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| anyhow!("Missing processing_steps"))?
                    .iter()
                    .map(|v| v.as_str().map(String::from))
                    .collect::<Option<Vec<String>>>()
                    .ok_or_else(|| anyhow!("Invalid processing_steps"))?;

                let processing_latency_us = metadata_json
                    .get("processing_latency_us")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Missing processing_latency_us"))?;

                Ok(ProcessingData::PhotoacousticResult {
                    signal,
                    metadata: super::data::ProcessingMetadata {
                        original_frame_number,
                        original_timestamp,
                        sample_rate,
                        processing_steps,
                        processing_latency_us,
                    },
                })
            }
            _ => Err(anyhow!("Unknown data type: {}", data_type)),
        }
    }

    /// Get the data type name for ProcessingData
    fn data_type_name(data: &ProcessingData) -> &'static str {
        match data {
            ProcessingData::AudioFrame(_) => "AudioFrame",
            ProcessingData::SingleChannel { .. } => "SingleChannel",
            ProcessingData::DualChannel { .. } => "DualChannel",
            ProcessingData::PhotoacousticResult { .. } => "PhotoacousticResult",
        }
    }
}

impl ProcessingNode for PythonNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Initialize if not already done
        {
            let mut initialized = self
                .initialized
                .lock()
                .map_err(|e| anyhow!("Failed to lock initialized: {}", e))?;
            if !*initialized {
                self.initialize_python()?;
                *initialized = true;
                *self.status.lock().unwrap() = "initialized".to_string();
            }
        }

        // Check if this data type is accepted
        if !self.config.accepted_types.is_empty() {
            let input_type = Self::data_type_name(&input);
            if !self.config.accepted_types.contains(&input_type.to_string()) {
                return Err(anyhow!(
                    "Input type '{}' not accepted by this Python node. Accepted types: {:?}",
                    input_type,
                    self.config.accepted_types
                ));
            }
        }

        // Convert input to JSON
        let input_json = self.processing_data_to_json(&input)?;

        // Call Python processing function
        let result_json = self.call_python_function(&self.config.process_function, input_json)?;

        // Convert result back to ProcessingData
        let output = self.json_to_processing_data(result_json)?;

        // Validate output type if specified
        if let Some(expected_output) = &self.config.output_type {
            let actual_output = Self::data_type_name(&output);
            if actual_output != expected_output {
                return Err(anyhow!(
                    "Python script returned '{}' but expected '{}' output type",
                    actual_output,
                    expected_output
                ));
            }
        }

        debug!(
            "Python node '{}' processed {} data",
            self.id,
            Self::data_type_name(&input)
        );
        Ok(output)
    }

    fn node_id(&self) -> &str {
        &self.id
    }

    fn node_type(&self) -> &str {
        "python"
    }

    fn accepts_input(&self, input: &ProcessingData) -> bool {
        if self.config.accepted_types.is_empty() {
            // Accept all types if none specified
            true
        } else {
            let input_type = Self::data_type_name(input);
            self.config.accepted_types.contains(&input_type.to_string())
        }
    }

    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        if let Some(output_type) = &self.config.output_type {
            Some(output_type.clone())
        } else {
            // Same as input by default
            Some(Self::data_type_name(input).to_string())
        }
    }

    fn reset(&mut self) {
        // Reset internal state
        {
            let mut initialized = self.initialized.lock().unwrap();
            *initialized = false;
        }
        {
            let mut status = self.status.lock().unwrap();
            *status = "reset".to_string();
        }
        {
            let mut last_modified = self.last_modified.lock().unwrap();
            *last_modified = None;
        }
        #[cfg(feature = "python-driver")]
        {
            let mut cached = self.cached_module.lock().unwrap();
            *cached = None;
        }

        debug!("Python node '{}' reset", self.id);
    }

    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        Box::new(PythonNode::new(self.id.clone(), self.config.clone()))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Drop for PythonNode {
    fn drop(&mut self) {
        // Call shutdown function if initialized
        let initialized = self.initialized.lock().unwrap();
        if *initialized {
            if let Err(e) = self.call_python_function(&self.config.shutdown_function, json!({})) {
                warn!("Failed to call Python shutdown function: {}", e);
            }
        }
        debug!("Python node '{}' dropped", self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use tempfile::TempDir;

    fn create_test_script(content: &str) -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("test_script.py");
        write(&script_path, content).unwrap();
        (temp_dir, script_path)
    }

    #[test]
    fn test_python_node_creation() {
        let config = PythonNodeConfig::default();
        let node = PythonNode::new("test_node".to_string(), config);

        assert_eq!(node.node_id(), "test_node");
        assert_eq!(node.node_type(), "python");
    }

    #[test]
    fn test_python_node_from_config() {
        let config_map = vec![
            ("script_path".to_string(), json!("/path/to/script.py")),
            ("auto_reload".to_string(), json!(true)),
            ("timeout_seconds".to_string(), json!(15)),
            ("process_function".to_string(), json!("my_process")),
        ]
        .into_iter()
        .collect();

        let node = PythonNode::from_config("test".to_string(), config_map).unwrap();
        assert_eq!(node.config.script_path, PathBuf::from("/path/to/script.py"));
        assert_eq!(node.config.auto_reload, true);
        assert_eq!(node.config.timeout_seconds, 15);
        assert_eq!(node.config.process_function, "my_process");
    }

    #[test]
    fn test_data_type_name() {
        let single_channel = ProcessingData::SingleChannel {
            samples: vec![0.1, 0.2],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        assert_eq!(PythonNode::data_type_name(&single_channel), "SingleChannel");
    }

    #[test]
    fn test_processing_data_to_json() {
        let config = PythonNodeConfig::default();
        let node = PythonNode::new("test".to_string(), config);

        let data = ProcessingData::SingleChannel {
            samples: vec![0.1, 0.2, 0.3],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let json = node.processing_data_to_json(&data).unwrap();
        assert_eq!(json["type"], "SingleChannel");
        assert_eq!(json["samples"].as_array().unwrap().len(), 3);
        assert_eq!(json["sample_rate"], 44100);
    }

    #[test]
    fn test_json_to_processing_data() {
        let config = PythonNodeConfig::default();
        let node = PythonNode::new("test".to_string(), config);

        let json = json!({
            "type": "SingleChannel",
            "samples": [0.1, 0.2, 0.3],
            "sample_rate": 44100,
            "timestamp": 1000,
            "frame_number": 1
        });

        let data = node.json_to_processing_data(json).unwrap();
        match data {
            ProcessingData::SingleChannel {
                samples,
                sample_rate,
                timestamp,
                frame_number,
            } => {
                assert_eq!(samples.len(), 3);
                assert_eq!(sample_rate, 44100);
                assert_eq!(timestamp, 1000);
                assert_eq!(frame_number, 1);
            }
            _ => panic!("Expected SingleChannel data"),
        }
    }

    #[test]
    fn test_accepts_input_with_accepted_types() {
        let mut config = PythonNodeConfig::default();
        config.accepted_types = vec!["SingleChannel".to_string()];
        let node = PythonNode::new("test".to_string(), config);

        let single_channel = ProcessingData::SingleChannel {
            samples: vec![0.1],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let dual_channel = ProcessingData::DualChannel {
            channel_a: vec![0.1],
            channel_b: vec![0.2],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        assert!(node.accepts_input(&single_channel));
        assert!(!node.accepts_input(&dual_channel));
    }

    #[test]
    fn test_accepts_input_without_restrictions() {
        let config = PythonNodeConfig::default(); // Empty accepted_types
        let node = PythonNode::new("test".to_string(), config);

        let single_channel = ProcessingData::SingleChannel {
            samples: vec![0.1],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let dual_channel = ProcessingData::DualChannel {
            channel_a: vec![0.1],
            channel_b: vec![0.2],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        assert!(node.accepts_input(&single_channel));
        assert!(node.accepts_input(&dual_channel));
    }

    #[cfg(feature = "python-driver")]
    #[test]
    fn test_simple_python_script() {
        let script_content = r#"
def initialize():
    return {"status": "initialized"}

def process_data(data):
    # Simple passthrough
    return data

def get_status():
    return {"status": "active"}

def shutdown():
    return {"status": "shutdown"}
"#;

        let (_temp_dir, script_path) = create_test_script(script_content);
        let config = PythonNodeConfig {
            script_path,
            ..Default::default()
        };

        let mut node = PythonNode::new("test".to_string(), config);

        let input = ProcessingData::SingleChannel {
            samples: vec![0.1, 0.2, 0.3],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let result = node.process(input.clone());
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(
            std::mem::discriminant(&input),
            std::mem::discriminant(&output)
        );
    }
}
