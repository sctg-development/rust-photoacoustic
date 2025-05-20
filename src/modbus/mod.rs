// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Modbus communication module
//!
//! This module provides Modbus TCP server functionality for the photoacoustic
//! water vapor analyzer, allowing external systems to read measurement data
//! and configure the analyzer via the Modbus protocol.
//!
//! ## Key Components
//!
//! - `PhotoacousticModbusServer`: The main server implementation that handles
//!   Modbus requests and provides access to measurement data.
//!
//! ## Usage
//!
//! The Modbus server can be started as part of the application daemon:
//!
//! ```no_run
//! use rust_photoacoustic::config::Config;
//! use rust_photoacoustic::daemon::launch_daemon::Daemon;
//!
//! let config = Config::default();
//! let mut daemon = Daemon::new();
//! daemon.start(config).await.unwrap();
//! ```
//!
//! ## Register Map
//!
//! ### Input Registers (Read-Only)
//!
//! - Register 0: Resonance frequency (Hz × 10, 0.1 Hz resolution)
//! - Register 1: Signal amplitude (× 1000, 0.001 resolution)
//! - Register 2: Water vapor concentration (ppm × 10, 0.1 ppm resolution)
//! - Register 3: Timestamp low word (UNIX epoch seconds)
//! - Register 4: Timestamp high word (UNIX epoch seconds)
//! - Register 5: Status code (0=normal, 1=warning, 2=error)
//!
//! ### Holding Registers (Read/Write)
//!
//! - Register 0: Measurement interval (seconds), default: 10
//! - Register 1: Averaging count (samples), default: 20
//! - Register 2: Gain setting, default: 30
//! - Register 3: Filter strength, default: 40

pub mod modbus_server;
pub use modbus_server::PhotoacousticModbusServer;
