//! Python action driver implementation
//!
//! This module implements a driver for executing custom Python scripts as actions.
//! It uses PyO3 to bridge between Rust and Python, allowing users to write
//! custom action logic in Python while maintaining performance and safety.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use log::{debug, error, info, warn};
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
    
    #[cfg(feature = "python-driver")]
    /// Python interpreter instance (thread-safe)
    py_instance: Arc<RwLock<Option<Python<'static>>>>,
    #[cfg(feature = "python-driver")]
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
            #[cfg(feature = "python-driver")]
            py_instance: Arc::new(RwLock::new(None)),
            #[cfg(feature = "python-driver")]
            py_module: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new("Not initialized".to_string())),
            last_modified: Arc::new(Mutex::new(None)),
            history: Arc::new(Mutex::new(Vec::new())),
            max_history: 1000, // Default history size
        }
    }

    /// Create from configuration value
    pub fn from_config(config: Value) -> Result<Self> {
        let script_path = config.get("script_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing script_path in Python driver config"))?;

        let mut driver_config = PythonDriverConfig {
            script_path: PathBuf::from(script_path),
            ..Default::default()
        };

        // Optional configurations
        if let Some(venv_path) = config.get("venv_path").and_then(|v| v.as_str()) {
            driver_config.venv_path = Some(PathBuf::from(venv_path));
        }

        if let Some(timeout) = config.get("timeout_seconds").and_then(|v| v.as_u64()) {
            driver_config.timeout_seconds = timeout;
        }

        if let Some(auto_reload) = config.get("auto_reload").and_then(|v| v.as_bool()) {
            driver_config.auto_reload = auto_reload;
        }

        if let Some(paths) = config.get("python_paths").and_then(|v| v.as_array()) {
            driver_config.python_paths = paths.iter()
                .filter_map(|v| v.as_str())
                .map(PathBuf::from)
                .collect();
        }

        // Function names
        if let Some(functions) = config.get("functions") {
            if let Some(update) = functions.get("update").and_then(|v| v.as_str()) {
                driver_config.update_function = update.to_string();
            }
            if let Some(alert) = functions.get("alert").and_then(|v| v.as_str()) {
                driver_config.alert_function = alert.to_string();
            }
            if let Some(init) = functions.get("init").and_then(|v| v.as_str()) {
                driver_config.init_function = init.to_string();
            }
            if let Some(shutdown) = functions.get("shutdown").and_then(|v| v.as_str()) {
                driver_config.shutdown_function = shutdown.to_string();
            }
            if let Some(status) = functions.get("status").and_then(|v| v.as_str()) {
                driver_config.status_function = status.to_string();
            }
        }

        Ok(Self::new(driver_config))
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

    #[cfg(feature = "python-driver")]
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
            } else {
                // Try alternative activation method for conda/anaconda
                let python_executable = venv_path.join("bin/python");
                if python_executable.exists() {
                    let sys = py.import("sys")?;
                    sys.setattr("executable", python_executable.to_string_lossy().to_string())?;
                    let site_packages = venv_path.join("lib").join("python3.11").join("site-packages");
                    if site_packages.exists() {
                        let path = sys.getattr("path")?;
                        path.call_method1("insert", (0, site_packages.to_string_lossy().to_string()))?;
                    }
                }
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

    #[cfg(not(feature = "python-driver"))]
    async fn init_python(&self) -> Result<()> {
        Err(anyhow!("Python driver not compiled - enable 'python-driver' feature"))
    }

    #[cfg(feature = "python-driver")]
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

    #[cfg(not(feature = "python-driver"))]
    async fn load_script(&self, _py: ()) -> Result<()> {
        Err(anyhow!("Python driver not compiled"))
    }

    #[cfg(feature = "python-driver")]
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

    #[cfg(not(feature = "python-driver"))]
    async fn call_python_function(&self, _func_name: &str, _args: &[()]) -> Result<Option<()>> {
        Err(anyhow!("Python driver not compiled"))
    }

    #[cfg(feature = "python-driver")]
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

    #[cfg(feature = "python-driver")]
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
        
        #[cfg(feature = "python-driver")]
        {
            self.init_python().await?;
            info!("Python action driver initialized successfully");
        }
        
        #[cfg(not(feature = "python-driver"))]
        {
            *self.status.lock().unwrap() = "Python driver not compiled".to_string();
            return Err(anyhow!("Python driver not compiled - enable 'python-driver' feature"));
        }
        
        Ok(())
    }

    async fn update_action(&mut self, data: &MeasurementData) -> Result<()> {
        #[cfg(feature = "python-driver")]
        {
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
        
        #[cfg(not(feature = "python-driver"))]
        {
            let _ = data; // Suppress unused variable warning
            Err(anyhow!("Python driver not compiled"))
        }
    }

    async fn show_alert(&mut self, alert: &AlertData) -> Result<()> {
        #[cfg(feature = "python-driver")]
        {
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
        
        #[cfg(not(feature = "python-driver"))]
        {
            let _ = alert; // Suppress unused variable warning
            Err(anyhow!("Python driver not compiled"))
        }
    }

    async fn clear_action(&mut self) -> Result<()> {
        #[cfg(feature = "python-driver")]
        {
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
        
        #[cfg(not(feature = "python-driver"))]
        Err(anyhow!("Python driver not compiled"))
    }

    async fn get_status(&self) -> Result<Value> {
        #[cfg(feature = "python-driver")]
        {
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
            "max_history": self.max_history,
            "python_enabled": cfg!(feature = "python-driver")
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
        
        #[cfg(feature = "python-driver")]
        {
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
            "newest_entry": newest.map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()),
            "python_enabled": cfg!(feature = "python-driver")
        }))
    }
}
