//! Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
//! This file is part of the rust-photoacoustic project and is licensed under the
//! SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Python action driver implementation
//!
//! This module implements a driver for executing custom Python scripts as actions.
//! It uses PyO3 to bridge between Rust and Python, allowing users to write
//! custom action logic in Python while maintaining performance and safety.
//!
//! # Overview
//!
//! The Python action driver enables you to implement custom measurement processing
//! and alert handling logic in Python scripts, which are then called from Rust.
//! This provides the flexibility of Python scripting while maintaining the
//! performance and safety of Rust.
//!
//! # Features
//!
//! - **Script Hot-reloading**: Automatically reload Python scripts when they change
//! - **Timeout Protection**: Configurable timeouts prevent hanging Python scripts
//! - **Error Handling**: Robust error handling with detailed error messages
//! - **Measurement History**: Automatic tracking of measurement data
//! - **Async/Await Support**: Full async support for non-blocking operation
//!
//! # Example Python Script
//!
//! Your Python script should implement the following functions:
//!
//! ```python
//! import json
//! import time
//!
//! # Global state
//! measurement_count = 0
//! last_concentration = None
//!
//! def initialize():
//!     """Called when the driver is initialized"""
//!     global measurement_count
//!     measurement_count = 0
//!     print("Python driver initialized")
//!     return {"status": "initialized"}
//!
//! def on_measurement(data):
//!     """Called for each measurement"""
//!     global measurement_count, last_concentration
//!     measurement_count += 1
//!     last_concentration = data.get("concentration_ppm")
//!     
//!     print(f"Processing measurement #{measurement_count}: {last_concentration} ppm")
//!     return {"processed": True, "count": measurement_count}
//!
//! def on_alert(alert):
//!     """Called when an alert is triggered"""
//!     severity = alert.get("severity", "unknown")
//!     message = alert.get("message", "No message")
//!     print(f"ALERT [{severity}]: {message}")
//!     return {"alert_handled": True}
//!
//! def get_status():
//!     """Return current status"""
//!     return {
//!         "measurement_count": measurement_count,
//!         "last_concentration": last_concentration,
//!         "uptime": time.time(),
//!         "status": "active"
//!     }
//!
//! def shutdown():
//!     """Called when the driver is shutting down"""
//!     print("Python driver shutting down")
//!     return {"status": "shutdown"}
//! ```
//!
//!```yaml
//! # Python Action Driver - For custom Python processing
//! # This driver allows executing custom Python code for advanced processing
//! - id: "python_action_node"
//!   node_type: "action_universal"
//!   parameters:
//!     buffer_capacity: 500                    # Moderate buffer for Python processing
//!     monitored_nodes:
//!       - "concentration_calculator"
//!     concentration_threshold: 500.0          # Higher threshold for Python processing
//!     amplitude_threshold: 70                 # Alert at 70dB amplitude
//!     update_interval_ms: 15000               # 15 seconds updates
//!     driver:
//!       type: "python"
//!       config:
//!         script_path: "./action.py"  # Path to custom Python script
//!         auto_reload: true  # Automatically reload script on changes
//!         timeout_seconds: 10  # Timeout for script execution
//!         init_function: initialize  # Function to call on initialization
//!         update_function: on_measurement  # Function to call on each measurement
//!         alert_function: on_alert  # Function to call on alerts
//!         status_function: get_status  # Function to call for status updates
//!         shutdown_function: shutdown  # Function to call on shutdown
//!```
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use rust_photoacoustic::processing::computing_nodes::action_drivers::{
//!     PythonActionDriver, PythonDriverConfig, ActionDriver
//! };
//! use std::path::PathBuf;
//! use std::collections::HashMap;
//! use serde_json::{json, Value};
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Create configuration
//! let config = PythonDriverConfig {
//!     script_path: PathBuf::from("my_action.py"),
//!     auto_reload: true,
//!     timeout_seconds: 10,
//!     ..Default::default()
//! };
//!
//! // Create driver
//! let mut driver = PythonActionDriver::new(config);
//!
//! // Initialize
//! driver.initialize().await?;
//!
//! // Process measurement
//! let measurement = rust_photoacoustic::processing::computing_nodes::action_drivers::MeasurementData {
//!     concentration_ppm: 42.5,
//!     source_node_id: "output_node".to_string(),
//!     peak_amplitude: 0.8,
//!     peak_frequency: 1000.0,
//!     timestamp: std::time::SystemTime::now(),
//!     metadata: HashMap::new(),
//! };
//!
//! driver.update_action(&measurement).await?;
//!
//! // Get status
//! let status = driver.get_status().await?;
//! println!("Driver status: {}", status);
//!
//! // Shutdown
//! driver.shutdown().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Configuration from JSON
//!
//! ```rust
//! use rust_photoacoustic::processing::computing_nodes::action_drivers::PythonActionDriver;
//! use serde_json::json;
//! use std::collections::HashMap;
//!
//! # fn example() -> anyhow::Result<()> {
//! let config_json = json!({
//!     "script_path": "/path/to/script.py",
//!     "auto_reload": true,
//!     "timeout_seconds": 30,
//!     "update_function": "process_measurement",
//!     "alert_function": "handle_alert"
//! });
//!
//! let config: HashMap<String, serde_json::Value> =
//!     serde_json::from_value(config_json)?;
//!
//! let driver = PythonActionDriver::from_config(config)?;
//! # Ok(())
//! # }
//! ```
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use super::{ActionDriver, AlertData, MeasurementData};

/// Python action driver configuration
///
/// This structure defines all the configuration options for the Python action driver.
/// It allows you to customize how Python scripts are executed and which functions
/// are called for different events.
///
/// # Example
///
/// ```rust
/// use rust_photoacoustic::processing::computing_nodes::action_drivers::PythonDriverConfig;
/// use std::path::PathBuf;
///
/// let config = PythonDriverConfig {
///     script_path: PathBuf::from("action.py"), // Use default path
///     auto_reload: true,
///     timeout_seconds: 15,
///     update_function: "handle_measurement".to_string(),
///     alert_function: "handle_alert".to_string(),
///     ..Default::default()
/// };
///
/// assert_eq!(config.script_path, PathBuf::from("action.py"));
/// assert_eq!(config.auto_reload, true);
/// assert_eq!(config.timeout_seconds, 15);
/// ```
///
/// # Default Configuration
///
/// ```rust
/// use rust_photoacoustic::processing::computing_nodes::action_drivers::PythonDriverConfig;
/// use std::path::PathBuf;
///
/// let default_config = PythonDriverConfig::default();
///
/// assert_eq!(default_config.script_path, PathBuf::from("action.py"));
/// assert_eq!(default_config.update_function, "on_measurement");
/// assert_eq!(default_config.alert_function, "on_alert");
/// assert_eq!(default_config.timeout_seconds, 30);
/// assert_eq!(default_config.auto_reload, false);
/// ```
#[derive(Debug, Clone)]
pub struct PythonDriverConfig {
    /// Path to the Python script file
    pub script_path: PathBuf,
    /// Python virtual environment path (optional)
    pub venv_path: Option<PathBuf>,
    /// Function name to call for updates (default: "on_measurement")
    pub update_function: String,
    /// Function name to call for alerts (default: "on_alert")
    pub alert_function: String,
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
}

impl Default for PythonDriverConfig {
    fn default() -> Self {
        Self {
            script_path: PathBuf::from("action.py"),
            venv_path: None,
            update_function: "on_measurement".to_string(),
            alert_function: "on_alert".to_string(),
            init_function: "initialize".to_string(),
            shutdown_function: "shutdown".to_string(),
            status_function: "get_status".to_string(),
            timeout_seconds: 30,
            auto_reload: false,
            python_paths: Vec::new(),
        }
    }
}

/// Python action driver that executes Python scripts for custom actions
///
/// This driver does NOT store a Python interpreter instance to avoid Send/Sync issues.
/// Instead, it acquires the GIL for each Python operation, which is safer in async contexts.
///
/// # Thread Safety
///
/// The driver is designed to be thread-safe and can be used in async contexts.
/// All shared state is protected by mutexes, and Python operations are executed
/// in blocking tasks to avoid blocking the async runtime.
///
/// # Performance Considerations
///
/// - Python scripts are reloaded from disk when `auto_reload` is enabled
/// - Function calls have configurable timeouts to prevent hanging
/// - Measurement history is limited to prevent memory growth
/// - Python GIL acquisition has minimal overhead in typical usage
///
/// # Example
///
/// ```rust,no_run
/// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
///     PythonActionDriver, PythonDriverConfig, ActionDriver, MeasurementData
/// };
/// use std::path::PathBuf;
/// use std::collections::HashMap;
/// use std::time::SystemTime;
///
/// # async fn example() -> anyhow::Result<()> {
/// // Create configuration
/// let config = PythonDriverConfig {
///     script_path: PathBuf::from("test_script.py"),
///     auto_reload: false,
///     timeout_seconds: 5,
///     ..Default::default()
/// };
///
/// // Create and initialize driver
/// let mut driver = PythonActionDriver::new(config);
/// // driver.initialize().await?;  // Requires python-driver feature
///
/// // Check driver type
/// assert_eq!(driver.driver_type(), "python");
///
/// // Create test measurement
/// let measurement = MeasurementData {
///     concentration_ppm: 123.45,
///     source_node_id: "test_node".to_string(),
///     peak_amplitude: 0.7,
///     peak_frequency: 1500.0,
///     timestamp: SystemTime::now(),
///     metadata: HashMap::new(),
/// };
///
/// // Process measurement (requires python-driver feature)
/// // driver.update_action(&measurement).await?;
///
/// // Get status
/// let status = driver.get_status().await?;
/// println!("Status: {}", status);
/// # Ok(())
/// # }
/// ```
pub struct PythonActionDriver {
    config: PythonDriverConfig,
    last_modified: Arc<Mutex<Option<SystemTime>>>,
    history: Arc<Mutex<Vec<MeasurementData>>>,
    status: Arc<Mutex<String>>,
    max_history: usize,
}

impl std::fmt::Debug for PythonActionDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PythonActionDriver")
            .field("config", &self.config)
            .field("max_history", &self.max_history)
            .finish()
    }
}

impl PythonActionDriver {
    /// Create a new Python action driver
    ///
    /// Creates a new driver instance with the specified configuration.
    /// The driver is not initialized until [`initialize`](Self::initialize) is called.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the Python driver
    ///
    /// # Example
    ///
    /// ```rust
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig, ActionDriver
    /// };
    /// use std::path::PathBuf;
    ///
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("action.py"), // Use default name
    ///     timeout_seconds: 10,
    ///     ..Default::default()
    /// };
    ///
    /// let driver = PythonActionDriver::new(config);
    /// assert_eq!(driver.driver_type(), "python");
    /// ```
    pub fn new(config: PythonDriverConfig) -> Self {
        Self {
            config,
            last_modified: Arc::new(Mutex::new(None)),
            history: Arc::new(Mutex::new(Vec::new())),
            status: Arc::new(Mutex::new("Not initialized".to_string())),
            max_history: 1000,
        }
    }

    /// Create a Python action driver from configuration
    ///
    /// Creates a driver from a configuration hash map, typically loaded from JSON
    /// or another configuration format. This is useful for dynamic configuration
    /// loading from config files.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration hash map with string keys and JSON values
    ///
    /// # Required Configuration Keys
    ///
    /// - `script_path`: Path to the Python script file
    ///
    /// # Optional Configuration Keys
    ///
    /// - `venv_path`: Path to Python virtual environment
    /// - `update_function`: Name of measurement update function (default: "on_measurement")
    /// - `alert_function`: Name of alert function (default: "on_alert")
    /// - `init_function`: Name of initialization function (default: "initialize")
    /// - `shutdown_function`: Name of shutdown function (default: "shutdown")
    /// - `status_function`: Name of status function (default: "get_status")
    /// - `timeout_seconds`: Timeout for Python calls in seconds (default: 30)
    /// - `auto_reload`: Whether to reload script on changes (default: false)
    /// - `python_paths`: Array of additional Python path directories
    ///
    /// # Errors
    ///
    /// Returns an error if the `script_path` is missing or if any configuration
    /// values have incorrect types.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, ActionDriver
    /// };
    /// use serde_json::json;
    /// use std::collections::HashMap;
    ///
    /// # fn example() -> anyhow::Result<()> {
    /// let config_json = json!({
    ///     "script_path": "test_script.py",
    ///     "auto_reload": true,
    ///     "timeout_seconds": 15,
    ///     "update_function": "process_data",
    ///     "python_paths": ["/additional/path"]
    /// });
    ///
    /// let config: HashMap<String, serde_json::Value> =
    ///     serde_json::from_value(config_json)?;
    ///
    /// let driver = PythonActionDriver::from_config(config)?;
    /// assert_eq!(driver.driver_type(), "python");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Example with minimal configuration
    ///
    /// ```rust
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, ActionDriver
    /// };
    /// use serde_json::json;
    /// use std::collections::HashMap;
    ///
    /// # fn example() -> anyhow::Result<()> {
    /// let config_json = json!({
    ///     "script_path": "action.py"
    /// });
    ///
    /// let config: HashMap<String, serde_json::Value> =
    ///     serde_json::from_value(config_json)?;
    ///
    /// let driver = PythonActionDriver::from_config(config)?;
    /// assert_eq!(driver.driver_type(), "python");
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_config(config: HashMap<String, Value>) -> Result<Self> {
        let script_path = config
            .get("script_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("Missing script_path in Python driver config"))?;

        let venv_path = config
            .get("venv_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);

        let update_function = config
            .get("update_function")
            .and_then(|v| v.as_str())
            .unwrap_or("on_measurement")
            .to_string();

        let alert_function = config
            .get("alert_function")
            .and_then(|v| v.as_str())
            .unwrap_or("on_alert")
            .to_string();

        let init_function = config
            .get("init_function")
            .and_then(|v| v.as_str())
            .unwrap_or("initialize")
            .to_string();

        let shutdown_function = config
            .get("shutdown_function")
            .and_then(|v| v.as_str())
            .unwrap_or("shutdown")
            .to_string();

        let status_function = config
            .get("status_function")
            .and_then(|v| v.as_str())
            .unwrap_or("get_status")
            .to_string();

        let timeout_seconds = config
            .get("timeout_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(30);

        let auto_reload = config
            .get("auto_reload")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let python_paths = config
            .get("python_paths")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(PathBuf::from)
                    .collect()
            })
            .unwrap_or_default();

        let config = PythonDriverConfig {
            script_path,
            venv_path,
            update_function,
            alert_function,
            init_function,
            shutdown_function,
            status_function,
            timeout_seconds,
            auto_reload,
            python_paths,
        };

        Ok(Self::new(config))
    }

    /// Get the script modification time
    ///
    /// Returns the last modification time of the Python script file.
    /// Used internally for auto-reload functionality.
    ///
    /// # Returns
    ///
    /// - `Some(SystemTime)` if the file exists and metadata is accessible
    /// - `None` if the file doesn't exist or metadata cannot be read
    ///
    /// # Example
    ///
    /// ```rust
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig
    /// };
    /// use std::path::PathBuf;
    ///
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("nonexistent.py"),
    ///     ..Default::default()
    /// };
    ///
    /// let driver = PythonActionDriver::new(config);
    /// // For a non-existent file, this would return None
    /// // let mtime = driver.get_script_mtime();
    /// ```
    fn get_script_mtime(&self) -> Option<SystemTime> {
        self.config
            .script_path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
    }

    /// Check if the script needs to be reloaded
    ///
    /// Determines whether the Python script should be reloaded based on:
    /// - The `auto_reload` configuration setting
    /// - Whether the script file has been modified since last load
    ///
    /// # Returns
    ///
    /// - `true` if auto-reload is enabled and the script has been modified
    /// - `false` if auto-reload is disabled or the script hasn't changed
    ///
    /// # Example
    ///
    /// ```rust
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig
    /// };
    /// use std::path::PathBuf;
    ///
    /// // With auto-reload disabled
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("action.py"),
    ///     auto_reload: false,
    ///     ..Default::default()
    /// };
    /// let driver = PythonActionDriver::new(config);
    /// // Will always return false when auto_reload is disabled
    ///
    /// // With auto-reload enabled
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("action.py"),
    ///     auto_reload: true,
    ///     ..Default::default()
    /// };
    /// let driver = PythonActionDriver::new(config);
    /// // Will check file modification time
    /// ```
    fn needs_reload(&self) -> bool {
        if !self.config.auto_reload {
            return false;
        }

        let current_mtime = self.get_script_mtime();
        let last_mtime = self.last_modified.lock().unwrap().clone();

        current_mtime != last_mtime
    }

    /// Update the last modification time
    fn update_mtime(&self) {
        let mut last_modified = self.last_modified.lock().unwrap();
        *last_modified = self.get_script_mtime();
    }

    /// Call a Python function with error handling and timeout
    ///
    /// This is the core method that executes Python functions. It handles:
    /// - Loading and parsing the Python script
    /// - Converting Rust data to Python objects
    /// - Calling the specified function with timeout protection
    /// - Converting Python results back to Rust data
    ///
    /// # Arguments
    ///
    /// * `func_name` - Name of the Python function to call
    /// * `args` - Array of JSON values to pass as function arguments
    ///
    /// # Returns
    ///
    /// - `Ok(Value)` containing the function's return value as JSON
    /// - `Err` if the function doesn't exist, times out, or raises an exception
    ///
    /// # Timeout Behavior
    ///
    /// Function calls are executed in a separate thread with a configurable timeout.
    /// If the timeout is exceeded, the call is cancelled and an error is returned.
    ///
    /// # Type Conversion
    ///
    /// The method automatically converts between Rust and Python types:
    /// - `Value::Null` ↔ `None`
    /// - `Value::Bool` ↔ `bool`
    /// - `Value::Number` ↔ `int`/`float`
    /// - `Value::String` ↔ `str`
    /// - `Value::Array` ↔ `list`
    /// - `Value::Object` ↔ `dict`
    ///
    /// # Example Python Function
    ///
    /// ```python
    /// def process_measurement(data):
    ///     concentration = data.get("concentration_ppm", 0)
    ///     if concentration > 100:
    ///         return {"status": "high", "alert": True}
    ///     else:
    ///         return {"status": "normal", "alert": False}
    /// ```
    ///
    /// # Feature Gate
    ///
    /// This method requires the `python-driver` feature to be enabled.
    /// Without it, calls will return an error indicating the feature is missing.
    #[cfg(feature = "python-driver")]
    async fn call_python_function(&self, func_name: &str, args: &[Value]) -> Result<Value> {
        use pyo3::prelude::*;
        use pyo3::types::{PyDict, PyList, PyModule, PyTuple};
        use std::ffi::CString;

        let script_path = self.config.script_path.clone();
        let timeout = Duration::from_secs(self.config.timeout_seconds);
        let func_name = func_name.to_string();
        let args = args.to_vec();

        // Execute Python code in a blocking task with timeout
        let result = tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || {
                Python::with_gil(|py| -> Result<Value> {
                    // Capture Python stdout/stderr
                    let sys = py.import("sys")?;
                    let io = py.import("io")?;

                    // Create StringIO objects to capture output
                    let stdout_capture = io.call_method0("StringIO")?;
                    let stderr_capture = io.call_method0("StringIO")?;

                    // Save original stdout/stderr
                    let original_stdout = sys.getattr("stdout")?;
                    let original_stderr = sys.getattr("stderr")?;

                    // Redirect stdout/stderr to our capture objects
                    sys.setattr("stdout", &stdout_capture)?;
                    sys.setattr("stderr", &stderr_capture)?;

                    // Load and execute the script
                    let code = std::fs::read_to_string(&script_path)
                        .map_err(|e| anyhow!("Failed to read Python script: {}", e))?;

                    let code_cstr = CString::new(code).map_err(|e| {
                        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                            "Invalid code string: {}",
                            e
                        ))
                    })?;

                    let result = Python::with_gil(|py| -> PyResult<PyObject> {
                        // Convert filename and module name to CString for PyO3 0.25+
                        let filename = CString::new("action_script.py").map_err(|e| {
                            pyo3::PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                                "Invalid filename: {}",
                                e
                            ))
                        })?;
                        let module_name = CString::new("action_script").map_err(|e| {
                            pyo3::PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                                "Invalid module name: {}",
                                e
                            ))
                        })?;

                        let module = PyModule::from_code(
                            py,
                            code_cstr.as_c_str(),
                            filename.as_c_str(),
                            module_name.as_c_str(),
                        )?;

                        // Check if the function exists
                        if !module.hasattr(func_name.as_str())? {
                            debug!("Python function '{}' not found, skipping", func_name);
                            return Ok(py.None());
                        }

                        // Get the function
                        let func = module.getattr(func_name.as_str())?;

                        // Call the function
                        if args.is_empty() {
                            func.call0().map(|v| v.into())
                        } else {
                            // Convert each argument to Python objects individually using pythonize
                            let py_args: Result<Vec<PyObject>, _> = args
                                .iter()
                                .map(|arg| {
                                    pythonize::pythonize(py, arg)
                                        .map_err(|e| {
                                            PyErr::new::<pyo3::exceptions::PyValueError, _>(
                                                e.to_string(),
                                            )
                                        })
                                        .map(|bound| bound.into())
                                })
                                .collect();
                            let py_args = py_args?;
                            let args_tuple = PyTuple::new(py, py_args)?;
                            func.call1(&args_tuple).map(|v| v.into())
                        }
                    });

                    // Restore original stdout/stderr
                    sys.setattr("stdout", original_stdout)?;
                    sys.setattr("stderr", original_stderr)?;

                    // Get captured output
                    let stdout_output = stdout_capture
                        .call_method0("getvalue")?
                        .extract::<String>()
                        .unwrap_or_default();
                    let stderr_output = stderr_capture
                        .call_method0("getvalue")?
                        .extract::<String>()
                        .unwrap_or_default();

                    // Log captured output using Rust logging
                    if !stdout_output.trim().is_empty() {
                        info!("[Python:{}] {}", func_name, stdout_output.trim());
                    }
                    if !stderr_output.trim().is_empty() {
                        warn!("[Python:{}] stderr: {}", func_name, stderr_output.trim());
                    }

                    // Handle the result
                    let result = result.map_err(|e| anyhow!("Python execution error: {}", e))?;

                    // Convert result back to JSON
                    if result.is_none(py) {
                        Ok(Value::Null)
                    } else if let Ok(b) = result.extract::<bool>(py) {
                        Ok(Value::Bool(b))
                    } else if let Ok(i) = result.extract::<i64>(py) {
                        Ok(Value::Number(i.into()))
                    } else if let Ok(f) = result.extract::<f64>(py) {
                        Ok(Value::Number(
                            serde_json::Number::from_f64(f).unwrap_or(0.into()),
                        ))
                    } else if let Ok(s) = result.extract::<String>(py) {
                        Ok(Value::String(s))
                    } else {
                        // Try to convert to string representation
                        let str_repr = result.bind(py).str()?.to_string();
                        Ok(Value::String(str_repr))
                    }
                })
            }),
        )
        .await;

        match result {
            Ok(task_result) => task_result.map_err(|e| anyhow!("Task error: {}", e))?,
            Err(_) => Err(anyhow!(
                "Python function call timed out after {} seconds",
                self.config.timeout_seconds
            )),
        }
    }

    /// Call a Python function without the python-driver feature
    ///
    /// This is a fallback implementation that returns an error when the
    /// `python-driver` feature is not enabled during compilation.
    ///
    /// # Returns
    ///
    /// Always returns an error indicating that the Python driver was not compiled.
    ///
    /// # Example
    ///
    /// ```rust
    /// // When compiled without python-driver feature:
    /// // let result = driver.call_python_function("test", &[]).await;
    /// // assert!(result.is_err());
    /// ```
    #[cfg(not(feature = "python-driver"))]
    async fn call_python_function(&self, _func_name: &str, _args: &[Value]) -> Result<Value> {
        Err(anyhow!(
            "Python driver not compiled - missing python-driver feature"
        ))
    }

    /// Add measurement to history
    ///
    /// Stores a measurement in the driver's internal history buffer.
    /// The history is automatically limited to prevent unbounded memory growth.
    ///
    /// # Arguments
    ///
    /// * `data` - The measurement data to add to history
    ///
    /// # History Management
    ///
    /// - History is limited to `max_history` entries (default: 1000)
    /// - When the limit is exceeded, oldest entries are removed
    /// - History is stored in chronological order
    ///
    /// # Example
    ///
    /// ```rust
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig, MeasurementData
    /// };
    /// use std::path::PathBuf;
    /// use std::collections::HashMap;
    /// use std::time::SystemTime;
    ///
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("action.py"),
    ///     ..Default::default()
    /// };
    /// let driver = PythonActionDriver::new(config);
    ///
    /// let measurement = MeasurementData {
    ///     concentration_ppm: 50.0,
    ///     source_node_id: "output".to_string(),
    ///     peak_amplitude: 0.6,
    ///     peak_frequency: 800.0,
    ///     timestamp: SystemTime::now(),
    ///     metadata: HashMap::new(),
    /// };
    ///
    /// // driver.add_to_history(&measurement); // Private method
    /// ```
    fn add_to_history(&self, data: &MeasurementData) {
        let mut history = self.history.lock().unwrap();
        history.push(data.clone());

        // Keep only the last max_history entries
        let len = history.len();
        if len > self.max_history {
            history.drain(0..len - self.max_history);
        }
    }

    /// Update driver status
    ///
    /// Updates the internal status string that tracks the driver's current state.
    /// This status is returned by the [`get_status`](Self::get_status) method.
    ///
    /// # Arguments
    ///
    /// * `status` - New status string to set
    ///
    /// # Common Status Values
    ///
    /// - "Not initialized" - Driver created but not yet initialized
    /// - "Initialized" - Driver successfully initialized
    /// - "Active" - Driver actively processing measurements
    /// - "Error: <message>" - Driver encountered an error
    /// - "Cleared" - Driver action cleared
    /// - "Shutdown" - Driver has been shut down
    ///
    /// # Example
    ///
    /// ```rust
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig
    /// };
    /// use std::path::PathBuf;
    ///
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("action.py"),
    ///     ..Default::default()
    /// };
    /// let driver = PythonActionDriver::new(config);
    ///
    /// // driver.update_status("Custom status".to_string()); // Private method
    /// ```
    fn update_status(&self, status: String) {
        let mut current_status = self.status.lock().unwrap();
        *current_status = status;
    }
}

#[async_trait]
impl ActionDriver for PythonActionDriver {
    /// Returns the driver type identifier
    ///
    /// # Returns
    ///
    /// Always returns `"python"` to identify this as a Python action driver.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig, ActionDriver
    /// };
    /// use std::path::PathBuf;
    ///
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("action.py"),
    ///     ..Default::default()
    /// };
    /// let driver = PythonActionDriver::new(config);
    ///
    /// assert_eq!(driver.driver_type(), "python");
    /// ```
    fn driver_type(&self) -> &str {
        "python"
    }

    /// Initialize the Python action driver
    ///
    /// Performs the following initialization steps:
    /// 1. Verifies that the Python script file exists
    /// 2. Records the script's modification time for auto-reload
    /// 3. Sets the driver status to "Initialized"
    /// 4. Calls the Python script's initialization function (if it exists)
    ///
    /// # Errors
    ///
    /// - Returns an error if the script file doesn't exist
    /// - Returns an error if the `python-driver` feature is not enabled
    /// - Logs a warning (but doesn't fail) if the Python init function fails
    ///
    /// # Python Function
    ///
    /// Calls the function specified by `init_function` in the configuration
    /// (default: "initialize"). The function should:
    /// - Take no arguments
    /// - Return any value (ignored) or None
    /// - Perform any necessary setup for the script
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig, ActionDriver
    /// };
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("existing_script.py"),
    ///     ..Default::default()
    /// };
    /// let mut driver = PythonActionDriver::new(config);
    ///
    /// // This will verify the script exists and call initialize()
    /// driver.initialize().await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn initialize(&mut self) -> Result<()> {
        info!(
            "Initializing Python action driver with script: {}",
            self.config.script_path.display()
        );

        #[cfg(feature = "python-driver")]
        {
            // Check if script exists
            if !self.config.script_path.exists() {
                return Err(anyhow!(
                    "Python script not found: {}",
                    self.config.script_path.display()
                ));
            }

            self.update_mtime();
            self.update_status("Initialized".to_string());

            // Call initialization function if it exists
            match self
                .call_python_function(&self.config.init_function, &[])
                .await
            {
                Ok(_) => debug!("Python initialization function completed successfully"),
                Err(e) => warn!("Python initialization function failed: {}", e),
            }
        }

        #[cfg(not(feature = "python-driver"))]
        {
            return Err(anyhow!(
                "Python driver not compiled - missing python-driver feature"
            ));
        }

        Ok(())
    }

    /// Process a new measurement through the Python script
    ///
    /// This method is called for each new measurement and performs the following:
    /// 1. Checks if the script needs reloading (if auto-reload is enabled)
    /// 2. Adds the measurement to the internal history
    /// 3. Converts the measurement data to JSON
    /// 4. Calls the Python update function with the measurement data
    /// 5. Updates the driver status based on the result
    ///
    /// # Arguments
    ///
    /// * `data` - The measurement data to process
    ///
    /// # Python Function
    ///
    /// Calls the function specified by `update_function` in the configuration
    /// (default: "on_measurement"). The function receives:
    /// - A dictionary with measurement data (concentration_ppm, temperature, etc.)
    /// - Should return any value (logged but otherwise ignored)
    ///
    /// # Errors
    ///
    /// - Returns an error if the Python function raises an exception
    /// - Returns an error if the function call times out
    /// - Returns an error if the `python-driver` feature is not enabled
    ///
    /// # Example Python Function
    ///
    /// ```python
    /// def on_measurement(data):
    ///     concentration = data["concentration_ppm"]
    ///     temperature = data["temperature"]
    ///     
    ///     print(f"Processing: {concentration} ppm at {temperature}°C")
    ///     
    ///     # Perform custom logic here
    ///     if concentration > 100:
    ///         # Trigger some action
    ///         return {"action": "alert", "level": "high"}
    ///     
    ///     return {"action": "none"}
    /// ```
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig, ActionDriver, MeasurementData
    /// };
    /// use std::path::PathBuf;
    /// use std::collections::HashMap;
    /// use std::time::SystemTime;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("process_script.py"),
    ///     ..Default::default()
    /// };
    /// let mut driver = PythonActionDriver::new(config);
    ///
    /// let measurement = MeasurementData {
    ///     concentration_ppm: 75.5,
    ///     source_node_id: "analyzer".to_string(),
    ///     peak_amplitude: 0.9,
    ///     peak_frequency: 1200.0,
    ///     timestamp: SystemTime::now(),
    ///     metadata: HashMap::new(),
    /// };
    ///
    /// // Process the measurement through Python
    /// driver.update_action(&measurement).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn update_action(&mut self, data: &MeasurementData) -> Result<()> {
        #[cfg(feature = "python-driver")]
        {
            // Check if we need to reload the script
            if self.needs_reload() {
                debug!("Reloading Python script due to changes");
                self.update_mtime();
            }

            // Add to history
            self.add_to_history(data);

            // Convert measurement data to JSON for Python
            let json_data = serde_json::to_value(data)?;

            // Call the update function
            match self
                .call_python_function(&self.config.update_function, &[json_data])
                .await
            {
                Ok(result) => {
                    debug!("Python update function completed: {:?}", result);
                    self.update_status("Active".to_string());
                }
                Err(e) => {
                    error!("Python update function failed: {}", e);
                    self.update_status(format!("Error: {}", e));
                    return Err(e);
                }
            }
        }

        #[cfg(not(feature = "python-driver"))]
        {
            return Err(anyhow!("Python driver not compiled"));
        }

        Ok(())
    }

    /// Send an alert to the Python script for handling
    ///
    /// This method is called when an alert needs to be processed and:
    /// 1. Converts the alert data to JSON format
    /// 2. Calls the Python alert function with the alert data
    /// 3. Logs the result or any errors
    ///
    /// # Arguments
    ///
    /// * `alert` - The alert data to process
    ///
    /// # Python Function
    ///
    /// Calls the function specified by `alert_function` in the configuration
    /// (default: "on_alert"). The function receives:
    /// - A dictionary with alert data (severity, message, timestamp, etc.)
    /// - Should return any value (logged but otherwise ignored)
    ///
    /// # Errors
    ///
    /// - Returns an error if the Python function raises an exception
    /// - Returns an error if the function call times out
    /// - Returns an error if the `python-driver` feature is not enabled
    ///
    /// # Example Python Function
    ///
    /// ```python
    /// def on_alert(alert):
    ///     severity = alert["severity"]
    ///     message = alert["message"]
    ///     timestamp = alert["timestamp"]
    ///     
    ///     print(f"ALERT [{severity}]: {message}")
    ///     
    ///     # Handle different alert levels
    ///     if severity == "critical":
    ///         # Send emergency notification
    ///         send_emergency_email(message)
    ///     elif severity == "warning":
    ///         # Log to monitoring system
    ///         log_warning(message)
    ///     
    ///     return {"handled": True, "action_taken": f"processed_{severity}"}
    /// ```
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig, ActionDriver, AlertData
    /// };
    /// use std::path::PathBuf;
    /// use std::time::SystemTime;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("alert_handler.py"),
    ///     ..Default::default()
    /// };
    /// let mut driver = PythonActionDriver::new(config);
    ///
    /// let alert = AlertData {
    ///     alert_type: "concentration_threshold".to_string(),
    ///     severity: "warning".to_string(),
    ///     message: "Concentration above threshold".to_string(),
    ///     data: {
    ///         let mut data = std::collections::HashMap::new();
    ///         data.insert("concentration_ppm".to_string(), serde_json::Value::Number(serde_json::Number::from(150)));
    ///         data
    ///     },
    ///     timestamp: SystemTime::now(),
    /// };
    ///
    /// // Send alert to Python for processing
    /// driver.show_alert(&alert).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn show_alert(&mut self, alert: &AlertData) -> Result<()> {
        #[cfg(feature = "python-driver")]
        {
            // Convert alert data to JSON for Python
            let json_alert = serde_json::to_value(alert)?;

            match self
                .call_python_function(&self.config.alert_function, &[json_alert])
                .await
            {
                Ok(_) => debug!("Python alert function completed successfully"),
                Err(e) => {
                    error!("Python alert function failed: {}", e);
                    return Err(e);
                }
            }
        }

        #[cfg(not(feature = "python-driver"))]
        {
            return Err(anyhow!("Python driver not compiled"));
        }

        Ok(())
    }

    /// Clear any active actions in the Python script
    ///
    /// This method calls an optional "clear_action" function in the Python script
    /// to reset or clear any ongoing actions. This is useful for:
    /// - Resetting alert states
    /// - Clearing temporary data
    /// - Stopping ongoing processes
    ///
    /// # Python Function
    ///
    /// Calls the "clear_action" function if it exists. The function should:
    /// - Take no arguments  
    /// - Return any value (ignored)
    /// - Perform cleanup or reset operations
    ///
    /// # Error Handling
    ///
    /// This method is designed to be forgiving:
    /// - If the function doesn't exist, no error is returned
    /// - If the function fails, only a warning is logged
    /// - Returns an error only if the `python-driver` feature is not enabled
    ///
    /// # Example Python Function
    ///
    /// ```python
    /// # Global state
    /// active_alerts = []
    /// processing_queue = []
    ///
    /// def clear_action():
    ///     global active_alerts, processing_queue
    ///     
    ///     print("Clearing all active actions")
    ///     active_alerts.clear()
    ///     processing_queue.clear()
    ///     
    ///     # Reset any hardware or external systems
    ///     reset_led_indicators()
    ///     
    ///     return {"cleared": True, "timestamp": time.time()}
    /// ```
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig, ActionDriver
    /// };
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("stateful_script.py"),
    ///     ..Default::default()
    /// };
    /// let mut driver = PythonActionDriver::new(config);
    ///
    /// // Clear any active actions (safe to call even if function doesn't exist)
    /// driver.clear_action().await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn clear_action(&mut self) -> Result<()> {
        #[cfg(feature = "python-driver")]
        {
            match self.call_python_function("clear_action", &[]).await {
                Ok(_) => {
                    debug!("Python clear function completed successfully");
                    self.update_status("Cleared".to_string());
                }
                Err(e) => {
                    warn!("Python clear function failed: {}", e);
                    // Don't return error as clear is often optional
                }
            }
        }

        #[cfg(not(feature = "python-driver"))]
        {
            return Err(anyhow!("Python driver not compiled"));
        }

        Ok(())
    }

    /// Get the current status of the Python driver and script
    ///
    /// Returns a comprehensive status object that includes both driver-level
    /// information and status from the Python script itself.
    ///
    /// # Status Information
    ///
    /// The returned JSON object contains:
    /// - `type`: Always "python"
    /// - `script_path`: Path to the Python script file
    /// - `driver_status`: Current driver state ("Initialized", "Active", etc.)
    /// - `python_status`: Status returned by the Python script's status function
    /// - `auto_reload`: Whether auto-reload is enabled
    /// - `history_size`: Number of measurements in history
    ///
    /// # Python Function
    ///
    /// Attempts to call the function specified by `status_function` in the configuration
    /// (default: "get_status"). The function should:
    /// - Take no arguments
    /// - Return any serializable value (dict, string, number, etc.)
    /// - Provide information about the script's internal state
    ///
    /// # Fallback Behavior
    ///
    /// If the Python status function doesn't exist or fails:
    /// - The `python_status` field will be "function not available"
    /// - Other status information is still returned
    /// - No error is raised
    ///
    /// # Example Python Function
    ///
    /// ```python
    /// import time
    ///
    /// start_time = time.time()
    /// measurement_count = 0
    /// last_error = None
    ///
    /// def get_status():
    ///     global start_time, measurement_count, last_error
    ///     
    ///     uptime = time.time() - start_time
    ///     
    ///     return {
    ///         "uptime_seconds": uptime,
    ///         "measurements_processed": measurement_count,
    ///         "last_error": last_error,
    ///         "memory_usage": get_memory_info(),
    ///         "custom_metrics": get_custom_metrics()
    ///     }
    /// ```
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig, ActionDriver
    /// };
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("monitored_script.py"),
    ///     auto_reload: true,
    ///     ..Default::default()
    /// };
    /// let driver = PythonActionDriver::new(config);
    ///
    /// let status = driver.get_status().await?;
    /// println!("Driver status: {}", status);
    ///
    /// // Status will include information like:
    /// // {
    /// //   "type": "python",
    /// //   "script_path": "monitored_script.py",
    /// //   "driver_status": "Initialized",
    /// //   "python_status": {...},
    /// //   "auto_reload": true,
    /// //   "history_size": 42
    /// // }
    /// # Ok(())
    /// # }
    /// ```
    async fn get_status(&self) -> Result<Value> {
        #[cfg(feature = "python-driver")]
        {
            let status = self.status.lock().unwrap().clone();

            // Try to get status from Python function
            match self
                .call_python_function(&self.config.status_function, &[])
                .await
            {
                Ok(py_status) => Ok(json!({
                    "type": "python",
                    "script_path": self.config.script_path,
                    "driver_status": status,
                    "python_status": py_status,
                    "auto_reload": self.config.auto_reload,
                    "history_size": self.history.lock().unwrap().len()
                })),
                Err(_) => Ok(json!({
                    "type": "python",
                    "script_path": self.config.script_path,
                    "driver_status": status,
                    "python_status": "function not available",
                    "auto_reload": self.config.auto_reload,
                    "history_size": self.history.lock().unwrap().len()
                })),
            }
        }

        #[cfg(not(feature = "python-driver"))]
        Ok(json!({
            "type": "python",
            "status": "not_compiled",
            "error": "Python driver not compiled"
        }))
    }

    /// Shut down the Python driver gracefully
    ///
    /// Performs cleanup operations and calls the Python script's shutdown function:
    /// 1. Calls the Python shutdown function (if it exists)
    /// 2. Updates the driver status to "Shutdown"
    /// 3. Logs any errors but doesn't fail if the Python function fails
    ///
    /// # Python Function
    ///
    /// Calls the function specified by `shutdown_function` in the configuration
    /// (default: "shutdown"). The function should:
    /// - Take no arguments
    /// - Return any value (ignored)
    /// - Perform cleanup operations (close files, connections, etc.)
    ///
    /// # Error Handling
    ///
    /// The method is designed to always succeed for the driver itself:
    /// - Python function errors are logged as warnings but don't cause failure
    /// - Only returns an error if the `python-driver` feature is not enabled
    /// - Driver state is always updated to "Shutdown"
    ///
    /// # Example Python Function
    ///
    /// ```python
    /// import atexit
    ///
    /// # Resources to clean up
    /// open_files = []
    /// network_connections = []
    ///
    /// def shutdown():
    ///     print("Shutting down Python driver...")
    ///     
    ///     # Close any open files
    ///     for file_handle in open_files:
    ///         file_handle.close()
    ///     
    ///     # Close network connections
    ///     for conn in network_connections:
    ///         conn.close()
    ///     
    ///     # Save state if needed
    ///     save_persistent_state()
    ///     
    ///     print("Shutdown complete")
    ///     return {"shutdown_time": time.time()}
    /// ```
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig, ActionDriver
    /// };
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("cleanup_script.py"),
    ///     ..Default::default()
    /// };
    /// let mut driver = PythonActionDriver::new(config);
    ///
    /// // Always safe to call, even if Python function doesn't exist
    /// driver.shutdown().await?;
    ///
    /// // Driver is now in "Shutdown" state
    /// let status = driver.get_status().await?;
    /// // status["driver_status"] == "Shutdown"
    /// # Ok(())
    /// # }
    /// ```
    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down Python action driver");

        #[cfg(feature = "python-driver")]
        {
            // Call shutdown function if it exists
            match self
                .call_python_function(&self.config.shutdown_function, &[])
                .await
            {
                Ok(_) => debug!("Python shutdown function completed successfully"),
                Err(e) => warn!("Python shutdown function failed: {}", e),
            }

            self.update_status("Shutdown".to_string());
        }

        #[cfg(not(feature = "python-driver"))]
        {
            return Err(anyhow!("Python driver not compiled"));
        }

        Ok(())
    }

    /// Get measurement history with optional limit
    ///
    /// Returns the stored measurement history, optionally limited to the most recent entries.
    /// The history is automatically maintained as measurements are processed.
    ///
    /// # Arguments
    ///
    /// * `limit` - Optional maximum number of recent measurements to return
    ///
    /// # Returns
    ///
    /// A vector of measurement data in chronological order:
    /// - If `limit` is `None`: returns all stored measurements
    /// - If `limit` is `Some(n)`: returns the most recent `n` measurements
    /// - Measurements are ordered from oldest to newest
    ///
    /// # Memory Management
    ///
    /// The internal history buffer is automatically limited to prevent memory growth:
    /// - Maximum history size is configurable (default: 1000 entries)
    /// - When the limit is exceeded, oldest entries are automatically removed
    /// - This method returns a cloned copy, so it's safe for concurrent access
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig, ActionDriver, MeasurementData
    /// };
    /// use std::path::PathBuf;
    /// use std::collections::HashMap;
    /// use std::time::SystemTime;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("data_logger.py"),
    ///     ..Default::default()
    /// };
    /// let mut driver = PythonActionDriver::new(config);
    ///
    /// // Process some measurements (they get added to history automatically)
    /// let measurement1 = MeasurementData {
    ///     concentration_ppm: 100.0,
    ///     source_node_id: "history_test".to_string(),
    ///     peak_amplitude: 0.5,
    ///     peak_frequency: 1000.0,
    ///     timestamp: SystemTime::now(),
    ///     metadata: HashMap::new(),
    /// };
    ///
    /// // driver.update_action(&measurement1).await?;
    ///
    /// // Get all history
    /// let all_history = driver.get_history(None).await?;
    /// println!("Total measurements: {}", all_history.len());
    ///
    /// // Get only the last 10 measurements
    /// let recent_history = driver.get_history(Some(10)).await?;
    /// println!("Recent measurements: {}", recent_history.len());
    /// # Ok(())
    /// # }
    /// ```
    async fn get_history(&self, limit: Option<usize>) -> Result<Vec<MeasurementData>> {
        let history = self.history.lock().unwrap();
        let data = history.clone();
        drop(history);

        if let Some(limit) = limit {
            Ok(data.into_iter().rev().take(limit).rev().collect())
        } else {
            Ok(data)
        }
    }

    /// Get statistics about the measurement history
    ///
    /// Returns statistical information about the stored measurement history,
    /// useful for monitoring and debugging purposes.
    ///
    /// # Returns
    ///
    /// A JSON object containing:
    /// - `size`: Current number of measurements in history
    /// - `max_size`: Maximum allowed history size (buffer limit)
    /// - `oldest`: Timestamp of the oldest measurement (or null if empty)
    /// - `newest`: Timestamp of the newest measurement (or null if empty)
    ///
    /// # Example Response
    ///
    /// ```json
    /// {
    ///   "size": 150,
    ///   "max_size": 1000,
    ///   "oldest": 1640995200,
    ///   "newest": 1640998800
    /// }
    /// ```
    ///
    /// # Use Cases
    ///
    /// - **Monitoring**: Check if history buffer is approaching capacity
    /// - **Debugging**: Verify that measurements are being stored correctly
    /// - **Analytics**: Understand the time span of available data
    /// - **Health Checks**: Ensure the driver is receiving regular updates
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rust_photoacoustic::processing::computing_nodes::action_drivers::{
    ///     PythonActionDriver, PythonDriverConfig, ActionDriver
    /// };
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = PythonDriverConfig {
    ///     script_path: PathBuf::from("analytics_script.py"),
    ///     ..Default::default()
    /// };
    /// let driver = PythonActionDriver::new(config);
    ///
    /// let stats = driver.get_history_stats().await?;
    ///
    /// println!("History statistics: {}", stats);
    ///
    /// // Check if buffer is getting full
    /// let size = stats["size"].as_u64().unwrap_or(0);
    /// let max_size = stats["max_size"].as_u64().unwrap_or(0);
    ///
    /// if size > max_size * 80 / 100 {
    ///     println!("Warning: History buffer is {}% full", size * 100 / max_size);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn get_history_stats(&self) -> Result<Value> {
        let history = self.history.lock().unwrap();
        let size = history.len();
        let oldest = history.first().map(|d| d.timestamp);
        let newest = history.last().map(|d| d.timestamp);
        drop(history);

        Ok(json!({
            "size": size,
            "max_size": self.max_history,
            "oldest": oldest,
            "newest": newest
        }))
    }
}
