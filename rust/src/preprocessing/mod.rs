// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
//! Signal preprocessing module
//!
//! This module handles preprocessing of the acquired audio signals,
//! including filtering and differential calculation.

pub mod differential;
#[cfg(test)]
mod differential_test;
pub mod filter;
#[cfg(test)]
mod filters_test;

pub use differential::DifferentialCalculator;
pub use filter::{BandpassFilter, Filter, HighpassFilter, LowpassFilter};

/// Create a bandpass filter centered at the given frequency with the specified bandwidth
pub fn create_bandpass_filter(center_freq: f32, bandwidth: f32) -> Box<dyn Filter> {
    Box::new(BandpassFilter::new(center_freq, bandwidth))
}

/// Create a differential calculator
pub fn create_differential_calculator() -> Box<dyn DifferentialCalculator> {
    Box::new(differential::SimpleDifferential::new())
}
