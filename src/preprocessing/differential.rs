//! Differential signal calculation

use anyhow::Result;

/// Trait for implementing differential signal calculation
pub trait DifferentialCalculator: Send + Sync {
    /// Calculate the differential signal A-B
    fn calculate(&self, channel_a: &[f32], channel_b: &[f32]) -> Result<Vec<f32>>;
}

/// A simple differential calculator that subtracts channel B from channel A
pub struct SimpleDifferential {
    // No state needed for this simple implementation
}

impl SimpleDifferential {
    /// Create a new simple differential calculator
    pub fn new() -> Self {
        Self {}
    }
}

impl DifferentialCalculator for SimpleDifferential {
    fn calculate(&self, channel_a: &[f32], channel_b: &[f32]) -> Result<Vec<f32>> {
        if channel_a.len() != channel_b.len() {
            return Err(anyhow::anyhow!(
                "Channel lengths don't match: A={}, B={}",
                channel_a.len(),
                channel_b.len()
            ));
        }
        
        let mut result = Vec::with_capacity(channel_a.len());
        
        for (&a, &b) in channel_a.iter().zip(channel_b.iter()) {
            result.push(a - b);
        }
        
        Ok(result)
    }
}
