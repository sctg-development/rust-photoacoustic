// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Utility module for common utilities used throughout the project

pub mod noise_generator;

// Re-export the NoiseGenerator for use in other modules
pub use noise_generator::NoiseGenerator;
