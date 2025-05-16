//! Signal preprocessing module
//! 
//! This module handles preprocessing of the acquired audio signals,
//! including filtering and differential calculation.

pub mod filters;
pub mod differential;
#[cfg(test)]
mod filters_test;
#[cfg(test)]
mod differential_test;

pub use filters::{Filter, BandpassFilter};
pub use differential::DifferentialCalculator;

use anyhow::Result;

/// Create a bandpass filter centered at the given frequency with the specified bandwidth
pub fn create_bandpass_filter(center_freq: f32, bandwidth: f32) -> Box<dyn Filter> {
    Box::new(BandpassFilter::new(center_freq, bandwidth))
}

/// Create a differential calculator
pub fn create_differential_calculator() -> Box<dyn DifferentialCalculator> {
    Box::new(differential::SimpleDifferential::new())
}
