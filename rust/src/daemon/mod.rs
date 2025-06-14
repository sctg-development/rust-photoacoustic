//! # Daemon Module
//!
//! The daemon module provides functionality for running and managing background
//! services in the photoacoustic application. This includes the web server for
//! visualization, data acquisition tasks, and system monitoring.
//!
//! ## Components
//!
//! * **Launch Daemon**: Core implementation for starting, monitoring, and gracefully
//!   shutting down background tasks
//!
//! ## Usage
//!
//! ```no_run
//! use rust_photoacoustic::{config::Config, daemon::launch_daemon::Daemon};
//! use std::sync::Arc;
//!
//! async fn run() -> anyhow::Result<()> {
//!     let config = Config::from_file("config.yaml")?;
//!     let config_arc = Arc::new(config);
//!     
//!     // Create and launch daemon
//!     let mut daemon = Daemon::new();
//!     daemon.launch(config_arc).await?;
//!     
//!     // Wait for shutdown signal (e.g., Ctrl+C)
//!     tokio::signal::ctrl_c().await?;
//!     
//!     // Clean shutdown
//!     daemon.shutdown();
//!     daemon.join().await?;
//!     
//!     Ok(())
//! }
//! ```

// Re-export the Daemon struct for convenience

pub mod launch_daemon;
