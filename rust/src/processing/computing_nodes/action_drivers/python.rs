//! Python action driver implementation
//!
//! This module implements a driver for executing custom Python scripts as actions.
//! It uses PyO3 to bridge between Rust and Python, allowing users to write
//! custom action logic in Python while maintaining performance and safety.
//!
//! Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
//! This file is part of the rust-photoacoustic project and is licensed under the
//! SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

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
    fn get_script_mtime(&self) -> Option<SystemTime> {
        self.config
            .script_path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
    }

    /// Check if the script needs to be reloaded
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
    #[cfg(feature = "python-driver")]
    async fn call_python_function(&self, func_name: &str, args: &[Value]) -> Result<Value> {
        use pyo3::prelude::*;
        use pyo3::types::{PyDict, PyList, PyModule, PyTuple};

        let script_path = self.config.script_path.clone();
        let timeout = Duration::from_secs(self.config.timeout_seconds);
        let func_name = func_name.to_string();
        let args = args.to_vec();

        // Execute Python code in a blocking task with timeout
        let result = tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || {
                Python::with_gil(|py| -> Result<Value> {
                    // Load the script as a module
                    let code = std::fs::read_to_string(&script_path)
                        .map_err(|e| anyhow!("Failed to read Python script: {}", e))?;

                    let module =
                        PyModule::from_code(py, &code, "action_script.py", "action_script")
                            .map_err(|e| anyhow!("Failed to load Python module: {}", e))?;

                    // Check if the function exists
                    if !module.hasattr(func_name.as_str())? {
                        debug!("Python function '{}' not found, skipping", func_name);
                        return Ok(Value::Null);
                    }

                    // Get the function
                    let func = module.getattr(func_name.as_str())?;

                    // Call the function
                    let result = if args.is_empty() {
                        func.call0()?
                    } else {
                        // Convert args to Python objects and call
                        let py_args: PyResult<Vec<PyObject>> = args
                            .iter()
                            .map(|v| {
                                // Simple conversion for basic types
                                match v {
                                    Value::Null => Ok(py.None()),
                                    Value::Bool(b) => Ok(b.to_object(py)),
                                    Value::Number(n) => {
                                        if let Some(i) = n.as_i64() {
                                            Ok(i.to_object(py))
                                        } else if let Some(f) = n.as_f64() {
                                            Ok(f.to_object(py))
                                        } else {
                                            Ok(py.None())
                                        }
                                    }
                                    Value::String(s) => Ok(s.to_object(py)),
                                    Value::Array(arr) => {
                                        let py_list = PyList::new(
                                            py,
                                            arr.iter().map(|item| {
                                                // Recursively convert array items
                                                match item {
                                                    Value::Null => py.None(),
                                                    Value::Bool(b) => b.to_object(py),
                                                    Value::Number(n) => {
                                                        if let Some(i) = n.as_i64() {
                                                            i.to_object(py)
                                                        } else if let Some(f) = n.as_f64() {
                                                            f.to_object(py)
                                                        } else {
                                                            py.None()
                                                        }
                                                    }
                                                    Value::String(s) => s.to_object(py),
                                                    _ => {
                                                        // For complex nested types, serialize to JSON string
                                                        serde_json::to_string(item)
                                                            .unwrap_or_default()
                                                            .to_object(py)
                                                    }
                                                }
                                            }),
                                        );
                                        Ok(py_list.to_object(py))
                                    }
                                    Value::Object(obj) => {
                                        let py_dict = PyDict::new(py);
                                        for (key, value) in obj {
                                            let py_value = match value {
                                                Value::Null => py.None(),
                                                Value::Bool(b) => b.to_object(py),
                                                Value::Number(n) => {
                                                    if let Some(i) = n.as_i64() {
                                                        i.to_object(py)
                                                    } else if let Some(f) = n.as_f64() {
                                                        f.to_object(py)
                                                    } else {
                                                        py.None()
                                                    }
                                                }
                                                Value::String(s) => s.to_object(py),
                                                _ => {
                                                    // For complex nested types, serialize to JSON string
                                                    serde_json::to_string(value)
                                                        .unwrap_or_default()
                                                        .to_object(py)
                                                }
                                            };
                                            py_dict.set_item(key, py_value)?;
                                        }
                                        Ok(py_dict.to_object(py))
                                    }
                                }
                            })
                            .collect();

                        let py_args =
                            py_args.map_err(|e| anyhow!("Python conversion error: {}", e))?;
                        func.call1(PyTuple::new(py, py_args))?
                    };

                    // Convert result back to JSON
                    if result.is_none() {
                        Ok(Value::Null)
                    } else if let Ok(b) = result.extract::<bool>() {
                        Ok(Value::Bool(b))
                    } else if let Ok(i) = result.extract::<i64>() {
                        Ok(Value::Number(i.into()))
                    } else if let Ok(f) = result.extract::<f64>() {
                        Ok(Value::Number(
                            serde_json::Number::from_f64(f).unwrap_or(0.into()),
                        ))
                    } else if let Ok(s) = result.extract::<String>() {
                        Ok(Value::String(s))
                    } else {
                        // Try to convert to string representation
                        let str_repr = result.str()?.to_string();
                        Ok(Value::String(str_repr))
                    }
                })
            }),
        )
        .await;

        match result {
            Ok(task_result) => task_result.map_err(|e| anyhow!("Task error: {}", e))?,
            Err(_) => {
                return Err(anyhow!(
                    "Python function call timed out after {} seconds",
                    self.config.timeout_seconds
                ))
            }
        }
    }

    /// Call a Python function without the python-driver feature
    #[cfg(not(feature = "python-driver"))]
    async fn call_python_function(&self, _func_name: &str, _args: &[Value]) -> Result<Value> {
        Err(anyhow!(
            "Python driver not compiled - missing python-driver feature"
        ))
    }

    /// Add measurement to history
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
    fn update_status(&self, status: String) {
        let mut current_status = self.status.lock().unwrap();
        *current_status = status;
    }
}

#[async_trait]
impl ActionDriver for PythonActionDriver {
    fn driver_type(&self) -> &str {
        "python"
    }

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
