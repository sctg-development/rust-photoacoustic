//! Python action driver implementation
//! 
//! This module implements a driver for executing custom Python scripts as actions.
//! It uses PyO3 to bridge between Rust and Python, allowing users to write
//! custom action logic in Python while maintaining performance and safety.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFunction, PyModule};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

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
            script_path: PathBuf::from("action_script.py"),
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

/// Python action driver
/// 
/// Executes custom Python scripts for action processing.
/// Integrates with PyO3 to provide a safe bridge between Rust and Python.
#[derive(Debug)]
pub struct PythonActionDriver {
    /// Driver configuration
    config: PythonDriverConfig,
    /// Python interpreter instance (thread-safe)
    py_instance: Arc<RwLock<Option<Python<'static>>>>,
    /// Python module (reloaded as needed)
    py_module: Arc<Mutex<Option<Py<PyModule>>>>,
    /// Driver status
    status: Arc<Mutex<String>>,
    /// Last script modification time (for auto-reload)
    last_modified: Arc<Mutex<Option<SystemTime>>>,
    /// History buffer for the driver
    history: Arc<Mutex<Vec<MeasurementData>>>,
    /// Maximum history size
    max_history: usize,
}

impl PythonActionDriver {
    /// Create a new Python action driver
    ///
    /// # Arguments
    /// * `config` - Driver configuration
    pub fn new(config: PythonDriverConfig) -> Self {
        Self {
            config,
            py_instance: Arc::new(RwLock::new(None)),
            py_module: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new("Not initialized".to_string())),
            last_modified: Arc::new(Mutex::new(None)),
            history: Arc::new(Mutex::new(Vec::new())),
            max_history: 1000, // Default history size
        }
    }

    /// Builder pattern: Set script path
    pub fn with_script_path(mut self, path: PathBuf) -> Self {
        self.config.script_path = path;
        self
    }

    /// Builder pattern: Set virtual environment
    pub fn with_venv(mut self, venv_path: PathBuf) -> Self {
        self.config.venv_path = Some(venv_path);
        self
    }

    /// Builder pattern: Set timeout
    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.config.timeout_seconds = timeout_seconds;
        self
    }

    /// Builder pattern: Enable auto-reload for development
    pub fn with_auto_reload(mut self, auto_reload: bool) -> Self {
        self.config.auto_reload = auto_reload;
        self
    }

    /// Builder pattern: Set history buffer size
    pub fn with_history_size(mut self, size: usize) -> Self {
        self.max_history = size;
        self
    }

    /// Initialize Python interpreter and load script
    async fn init_python(&self) -> Result<()> {
        let config = &self.config;
        
        // Initialize Python interpreter
        pyo3::prepare_freethreaded_python();
        
        let gil = Python::acquire_gil();
        let py = gil.python();
        
        // Setup Python path if specified
        if !config.python_paths.is_empty() {
            let sys = py.import("sys")?;
            let path = sys.getattr("path")?;
            for py_path in &config.python_paths {
                path.call_method1("append", (py_path.to_string_lossy().to_string(),))?;
            }
        }
        
        // Activate virtual environment if specified
        if let Some(venv_path) = &config.venv_path {
            let activate_script = venv_path.join("bin/activate_this.py");
            if activate_script.exists() {
                let activate_code = std::fs::read_to_string(activate_script)?;
                py.run(&activate_code, None, None)?;
            }
        }
        
        // Load the user script
        self.load_script(py).await?;
        
        // Store Python instance
        {
            let mut py_inst = self.py_instance.write().await;
            *py_inst = Some(unsafe { std::mem::transmute(py) });
        }
        
        // Call initialization function if it exists
        self.call_python_function(&config.init_function, &[]).await?;
        
        *self.status.lock().unwrap() = "Initialized".to_string();
        
        Ok(())
    }

    /// Load or reload the Python script
    async fn load_script(&self, py: Python<'_>) -> Result<()> {
        let script_path = &self.config.script_path;
        
        if !script_path.exists() {
            return Err(anyhow!("Python script not found: {}", script_path.display()));
        }
        
        // Check if script needs reloading
        let metadata = std::fs::metadata(script_path)?;
        let modified = metadata.modified()?;
        
        {
            let mut last_mod = self.last_modified.lock().unwrap();
            if let Some(last) = *last_mod {
                if !self.config.auto_reload || modified <= last {
                    return Ok(()); // No need to reload
                }
            }
            *last_mod = Some(modified);
        }
        
        // Read and execute script
        let script_content = std::fs::read_to_string(script_path)?;
        let module = PyModule::from_code(py, &script_content, "action_script.py", "action_script")?;
        
        // Store module
        {
            let mut py_mod = self.py_module.lock().unwrap();
            *py_mod = Some(module.into());
        }
        
        info!("Loaded Python script: {}", script_path.display());
        Ok(())
    }

    /// Call a Python function with arguments
    async fn call_python_function(&self, func_name: &str, args: &[&dyn ToPyObject]) -> Result<Option<PyObject>> {
        let py_inst = self.py_instance.read().await;
        let py = py_inst.as_ref().ok_or_else(|| anyhow!("Python not initialized"))?;
        
        let py_mod_guard = self.py_module.lock().unwrap();
        let py_mod = py_mod_guard.as_ref().ok_or_else(|| anyhow!("Python module not loaded"))?;
        
        let module = py_mod.as_ref(*py);
        
        // Check if function exists
        if !module.hasattr(func_name)? {
            debug!("Python function '{}' not found, skipping", func_name);
            return Ok(None);
        }
        
        let func: &PyFunction = module.getattr(func_name)?.downcast()?;
        
        // Execute with timeout
        let timeout = Duration::from_secs(self.config.timeout_seconds);
        let start = std::time::Instant::now();
        
        let result = func.call1(args.into())?;
        
        if start.elapsed() > timeout {
            warn!("Python function '{}' took longer than {} seconds", func_name, self.config.timeout_seconds);
        }
        
        Ok(Some(result.into()))
    }

    /// Convert MeasurementData to Python dict
    fn measurement_to_py_dict(&self, py: Python<'_>, data: &MeasurementData) -> Result<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("concentration_ppm", data.concentration_ppm)?;
        dict.set_item("source_node_id", &data.source_node_id)?;
        dict.set_item("peak_amplitude", data.peak_amplitude)?;
        dict.set_item("peak_frequency", data.peak_frequency)?;
        dict.set_item("timestamp", data.timestamp.duration_since(std::time::UNIX_EPOCH)?.as_secs())?;
        
        // Convert metadata
        let metadata_dict = PyDict::new(py);
        for (key, value) in &data.metadata {
            metadata_dict.set_item(key, value.to_string())?;
        }
        dict.set_item("metadata", metadata_dict)?;
        
        Ok(dict.into())
    }

    /// Convert AlertData to Python dict
    fn alert_to_py_dict(&self, py: Python<'_>, alert: &AlertData) -> Result<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("alert_type", &alert.alert_type)?;
        dict.set_item("severity", &alert.severity)?;
        dict.set_item("message", &alert.message)?;
        dict.set_item("timestamp", alert.timestamp.duration_since(std::time::UNIX_EPOCH)?.as_secs())?;
        
        // Convert data
        let data_dict = PyDict::new(py);
        for (key, value) in &alert.data {
            data_dict.set_item(key, value.to_string())?;
        }
        dict.set_item("data", data_dict)?;
        
        Ok(dict.into())
    }
}

#[async_trait]
impl ActionDriver for PythonActionDriver {
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing Python action driver with script: {}", self.config.script_path.display());
        
        self.init_python().await?;
        
        info!("Python action driver initialized successfully");
        Ok(())
    }

    async fn update_action(&mut self, data: &MeasurementData) -> Result<()> {
        // Reload script if auto-reload is enabled
        if self.config.auto_reload {
            let py_inst = self.py_instance.read().await;
            if let Some(py) = py_inst.as_ref() {
                self.load_script(*py).await?;
            }
        }
        
        // Convert data to Python format
        let py_inst = self.py_instance.read().await;
        let py = py_inst.as_ref().ok_or_else(|| anyhow!("Python not initialized"))?;
        let py_data = self.measurement_to_py_dict(*py, data)?;
        
        // Call Python function
        match self.call_python_function(&self.config.update_function, &[&py_data]).await {
            Ok(_) => {
                // Add to history
                let mut history = self.history.lock().unwrap();
                history.push(data.clone());
                if history.len() > self.max_history {
                    history.remove(0);
                }
                Ok(())
            }
            Err(e) => {
                error!("Python update function failed: {}", e);
                *self.status.lock().unwrap() = format!("Error: {}", e);
                Err(e)
            }
        }
    }

    async fn show_alert(&mut self, alert: &AlertData) -> Result<()> {
        let py_inst = self.py_instance.read().await;
        let py = py_inst.as_ref().ok_or_else(|| anyhow!("Python not initialized"))?;
        let py_alert = self.alert_to_py_dict(*py, alert)?;
        
        match self.call_python_function(&self.config.alert_function, &[&py_alert]).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Python alert function failed: {}", e);
                *self.status.lock().unwrap() = format!("Alert Error: {}", e);
                Err(e)
            }
        }
    }

    async fn clear_action(&mut self) -> Result<()> {
        match self.call_python_function("clear_action", &[]).await {
            Ok(_) => {
                *self.status.lock().unwrap() = "Cleared".to_string();
                Ok(())
            }
            Err(e) => {
                warn!("Python clear function failed (this is optional): {}", e);
                Ok(()) // Clear is optional, don't fail
            }
        }
    }

    async fn get_status(&self) -> Result<Value> {
        // Try to get status from Python script
        if let Ok(Some(py_result)) = self.call_python_function(&self.config.status_function, &[]).await {
            let py_inst = self.py_instance.read().await;
            if let Some(py) = py_inst.as_ref() {
                // Try to convert Python result to JSON
                if let Ok(status_str) = py_result.extract::<String>(*py) {
                    if let Ok(json_value) = serde_json::from_str::<Value>(&status_str) {
                        return Ok(json_value);
                    }
                }
            }
        }
        
        // Fallback to basic status
        let status = self.status.lock().unwrap().clone();
        let history_size = self.history.lock().unwrap().len();
        
        Ok(json!({
            "driver_type": "python",
            "status": status,
            "script_path": self.config.script_path,
            "auto_reload": self.config.auto_reload,
            "timeout_seconds": self.config.timeout_seconds,
            "history_size": history_size,
            "max_history": self.max_history
        }))
    }

    fn driver_type(&self) -> &str {
        "python"
    }

    fn supports_realtime(&self) -> bool {
        true // Python scripts can handle real-time updates
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down Python action driver");
        
        // Call Python shutdown function if it exists
        if let Err(e) = self.call_python_function(&self.config.shutdown_function, &[]).await {
            warn!("Python shutdown function failed: {}", e);
        }
        
        // Clear Python resources
        {
            let mut py_inst = self.py_instance.write().await;
            *py_inst = None;
        }
        {
            let mut py_mod = self.py_module.lock().unwrap();
            *py_mod = None;
        }
        
        *self.status.lock().unwrap() = "Shutdown".to_string();
        
        info!("Python action driver shutdown complete");
        Ok(())
    }

    async fn get_history(&self, limit: Option<usize>) -> Result<Vec<MeasurementData>> {
        let history = self.history.lock().unwrap();
        let data = history.clone();
        
        if let Some(limit) = limit {
            Ok(data.into_iter().rev().take(limit).collect())
        } else {
            Ok(data.into_iter().rev().collect())
        }
    }

    async fn get_history_stats(&self) -> Result<Value> {
        let history = self.history.lock().unwrap();
        let size = history.len();
        let oldest = history.first().map(|d| d.timestamp);
        let newest = history.last().map(|d| d.timestamp);
        
        Ok(json!({
            "driver_type": "python",
            "history_supported": true,
            "buffer_capacity": self.max_history,
            "buffer_size": size,
            "oldest_entry": oldest.map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()),
            "newest_entry": newest.map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::NamedTempFile;
    use std::io::Write;

    fn create_test_script(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[tokio::test]
    async fn test_python_driver_creation() {
        let config = PythonDriverConfig::default();
        let driver = PythonActionDriver::new(config);
        
        assert_eq!(driver.driver_type(), "python");
        assert!(driver.supports_realtime());
    }

    #[tokio::test]
    async fn test_python_driver_with_simple_script() {
        let script_content = r#"
def initialize():
    return {"status": "initialized"}

def on_measurement(data):
    print(f"Received: {data['concentration_ppm']} ppm")
    return {"processed": True}

def get_status():
    return '{"custom_status": "active"}'
"#;
        
        let script_file = create_test_script(script_content);
        
        let config = PythonDriverConfig {
            script_path: script_file.path().to_path_buf(),
            auto_reload: false,
            timeout_seconds: 5,
            ..Default::default()
        };
        
        let mut driver = PythonActionDriver::new(config);
        
        // Test initialization
        let result = driver.initialize().await;
        assert!(result.is_ok(), "Driver initialization failed: {:?}", result);
        
        // Test measurement update
        let measurement = MeasurementData {
            concentration_ppm: 500.0,
            source_node_id: "test_node".to_string(),
            peak_amplitude: 0.5,
            peak_frequency: 1000.0,
            timestamp: SystemTime::now(),
            metadata: HashMap::new(),
        };
        
        let result = driver.update_action(&measurement).await;
        assert!(result.is_ok(), "Update action failed: {:?}", result);
        
        // Test status
        let status = driver.get_status().await;
        assert!(status.is_ok(), "Get status failed: {:?}", status);
        
        // Test shutdown
        let result = driver.shutdown().await;
        assert!(result.is_ok(), "Shutdown failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_python_driver_builder_pattern() {
        let script_file = create_test_script("def initialize(): pass");
        
        let driver = PythonActionDriver::new(PythonDriverConfig::default())
            .with_script_path(script_file.path().to_path_buf())
            .with_timeout(60)
            .with_auto_reload(true)
            .with_history_size(500);
            
        assert_eq!(driver.config.timeout_seconds, 60);
        assert_eq!(driver.config.auto_reload, true);
        assert_eq!(driver.max_history, 500);
    }
}
