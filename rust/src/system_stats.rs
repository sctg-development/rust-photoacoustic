// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! System statistics collection module
//!
//! This module provides cross-platform system monitoring capabilities for the
//! rust-photoacoustic application, including CPU usage, memory consumption,
//! and thread count monitoring.

use anyhow::Result;
use rocket_okapi::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, System};

/// Comprehensive system statistics for the current process
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SystemStats {
    /// CPU usage percentage (0.0 to 100.0)
    pub cpu_usage_percent: f32,
    /// Physical memory usage in megabytes
    pub memory_usage_mb: u64,
    /// Virtual memory usage in megabytes
    pub virtual_memory_mb: u64,
    /// Number of threads in the current process
    pub thread_count: usize,
    /// Total number of CPU cores in the system
    pub total_cpu_cores: usize,
    /// Available system memory in megabytes
    pub available_memory_mb: u64,
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Process uptime in seconds
    pub process_uptime_seconds: u64,
    /// Timestamp when these statistics were collected
    pub timestamp: u64,
}

/// System statistics collector with periodic refresh capability
pub struct SystemStatsCollector {
    system: System,
    current_pid: Pid,
    process_start_time: SystemTime,
}

impl SystemStatsCollector {
    /// Create a new system statistics collector
    pub fn new() -> Result<Self> {
        let mut system = System::new_all();
        system.refresh_all();

        let current_pid = sysinfo::get_current_pid()
            .map_err(|e| anyhow::anyhow!("Failed to get current process PID: {}", e))?;

        Ok(Self {
            system,
            current_pid,
            process_start_time: SystemTime::now(),
        })
    }

    /// Refresh system information and collect current statistics
    pub fn collect_stats(&mut self) -> Result<SystemStats> {
        // Refresh system information
        self.system.refresh_all();

        let process = self
            .system
            .process(self.current_pid)
            .ok_or_else(|| anyhow::anyhow!("Cannot find current process"))?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let process_uptime = self
            .process_start_time
            .elapsed()
            .unwrap_or_default()
            .as_secs();

        Ok(SystemStats {
            cpu_usage_percent: process.cpu_usage(),
            memory_usage_mb: process.memory() / 1024 / 1024,
            virtual_memory_mb: process.virtual_memory() / 1024 / 1024,
            thread_count: 1, // Default thread count - threads() method no longer supported
            total_cpu_cores: self.system.cpus().len(),
            available_memory_mb: self.system.available_memory() / 1024 / 1024,
            uptime_seconds: System::uptime(),
            process_uptime_seconds: process_uptime,
            timestamp,
        })
    }

    /// Get a quick snapshot without full refresh (faster but less accurate)
    pub fn quick_stats(&self) -> Result<SystemStats> {
        let process = self
            .system
            .process(self.current_pid)
            .ok_or_else(|| anyhow::anyhow!("Cannot find current process"))?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let process_uptime = self
            .process_start_time
            .elapsed()
            .unwrap_or_default()
            .as_secs();

        Ok(SystemStats {
            cpu_usage_percent: process.cpu_usage(),
            memory_usage_mb: process.memory() / 1024 / 1024,
            virtual_memory_mb: process.virtual_memory() / 1024 / 1024,
            thread_count: 1, // Default thread count - threads() method no longer supported
            total_cpu_cores: self.system.cpus().len(),
            available_memory_mb: self.system.available_memory() / 1024 / 1024,
            uptime_seconds: System::uptime(),
            process_uptime_seconds: process_uptime,
            timestamp,
        })
    }
}

impl SystemStats {
    /// Create a static snapshot of current system statistics
    pub fn current() -> Result<Self> {
        let mut collector = SystemStatsCollector::new()?;
        collector.collect_stats()
    }

    /// Check if memory usage is above a threshold (percentage of available memory)
    pub fn is_memory_high(&self, threshold_percent: f32) -> bool {
        let total_memory = self.memory_usage_mb + self.available_memory_mb;
        if total_memory == 0 {
            return false;
        }
        let usage_percent = (self.memory_usage_mb as f32 / total_memory as f32) * 100.0;
        usage_percent > threshold_percent
    }

    /// Check if CPU usage is above a threshold
    pub fn is_cpu_high(&self, threshold_percent: f32) -> bool {
        self.cpu_usage_percent > threshold_percent
    }

    /// Get memory usage as percentage of total system memory
    pub fn memory_usage_percent(&self) -> f32 {
        let total_memory = self.memory_usage_mb + self.available_memory_mb;
        if total_memory == 0 {
            return 0.0;
        }
        (self.memory_usage_mb as f32 / total_memory as f32) * 100.0
    }

    /// Format statistics for logging
    pub fn format_for_logging(&self) -> String {
        format!(
            "CPU: {:.1}%, RAM: {} MB ({:.1}%), Threads: {}, Uptime: {}s",
            self.cpu_usage_percent,
            self.memory_usage_mb,
            self.memory_usage_percent(),
            self.thread_count,
            self.process_uptime_seconds
        )
    }
}

impl std::fmt::Display for SystemStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "System Statistics:\n\
             CPU Usage:        {:.1}%\n\
             Memory Usage:     {} MB\n\
             Virtual Memory:   {} MB\n\
             Available Memory: {} MB\n\
             Thread Count:     {}\n\
             CPU Cores:        {}\n\
             Process Uptime:   {} seconds\n\
             System Uptime:    {} seconds",
            self.cpu_usage_percent,
            self.memory_usage_mb,
            self.virtual_memory_mb,
            self.available_memory_mb,
            self.thread_count,
            self.total_cpu_cores,
            self.process_uptime_seconds,
            self.uptime_seconds
        )
    }
}

/// Periodic system statistics monitor
pub struct SystemMonitor {
    collector: SystemStatsCollector,
    monitoring_interval: Duration,
}

impl SystemMonitor {
    /// Create a new system monitor with specified refresh interval
    pub fn new(monitoring_interval: Duration) -> Result<Self> {
        Ok(Self {
            collector: SystemStatsCollector::new()?,
            monitoring_interval,
        })
    }

    /// Start monitoring in a background task (returns handle for stopping)
    pub async fn start_monitoring<F>(&mut self, mut callback: F) -> Result<()>
    where
        F: FnMut(SystemStats) + Send + 'static,
    {
        let mut interval = tokio::time::interval(self.monitoring_interval);

        loop {
            interval.tick().await;

            match self.collector.collect_stats() {
                Ok(stats) => {
                    callback(stats);
                }
                Err(e) => {
                    log::warn!("Failed to collect system statistics: {}", e);
                }
            }
        }
    }

    /// Get current statistics
    pub fn get_current_stats(&mut self) -> Result<SystemStats> {
        self.collector.collect_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_system_stats_creation() {
        let stats = SystemStats::current().expect("Should be able to get system stats");

        assert!(stats.cpu_usage_percent >= 0.0);
        assert!(stats.memory_usage_mb > 0);
        assert!(stats.thread_count > 0);
        assert!(stats.total_cpu_cores > 0);
        assert!(stats.timestamp > 0);

        println!("System stats: {}", stats);
    }

    #[test]
    fn test_system_stats_collector() {
        let mut collector =
            SystemStatsCollector::new().expect("Should be able to create collector");

        let stats1 = collector
            .collect_stats()
            .expect("Should be able to collect stats");
        let stats2 = collector
            .quick_stats()
            .expect("Should be able to get quick stats");

        assert!(stats1.memory_usage_mb > 0);
        assert!(stats2.memory_usage_mb > 0);
        assert_eq!(stats1.total_cpu_cores, stats2.total_cpu_cores);
    }

    #[test]
    fn test_threshold_checks() {
        let stats = SystemStats::current().expect("Should be able to get system stats");

        // These should not panic and return reasonable results
        let _high_memory = stats.is_memory_high(90.0);
        let _high_cpu = stats.is_cpu_high(90.0);
        let _memory_percent = stats.memory_usage_percent();

        assert!(stats.memory_usage_percent() >= 0.0);
        assert!(stats.memory_usage_percent() <= 100.0);
    }

    #[tokio::test]
    async fn test_system_monitor() {
        let mut monitor = SystemMonitor::new(Duration::from_millis(100))
            .expect("Should be able to create monitor");

        let stats = monitor
            .get_current_stats()
            .expect("Should be able to get current stats");

        assert!(stats.memory_usage_mb > 0);
        assert!(stats.thread_count > 0);
    }

    #[test]
    fn test_stats_formatting() {
        let stats = SystemStats::current().expect("Should be able to get system stats");

        let formatted = stats.format_for_logging();
        assert!(formatted.contains("CPU:"));
        assert!(formatted.contains("RAM:"));
        assert!(formatted.contains("Threads:"));

        let display = format!("{}", stats);
        assert!(display.contains("System Statistics:"));
        assert!(display.contains("CPU Usage:"));
    }
}
