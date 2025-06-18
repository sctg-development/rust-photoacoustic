// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! I2C drivers for thermal regulation hardware
//!
//! This module provides different I2C bus driver implementations:
//! - Native: Direct access to Raspberry Pi I2C hardware
//! - CP2112: USB-to-I2C bridge driver
//! - Mock: Simulation driver for testing and development

pub mod cp2112;
pub mod mock;
pub mod native;

pub use cp2112::Cp2112Driver;
pub use mock::MockI2CDriver;
pub use native::NativeI2CDriver;
