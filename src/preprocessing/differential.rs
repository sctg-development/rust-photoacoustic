//! Differential signal calculation

use anyhow::Result;

/// Calculate the differential signal between two i16 sample vectors
/// 
/// This function calculates signal_a - signal_b for each sample pair.
/// Used primarily by the differential binary utility for WAV file processing.
/// 
/// # Arguments
/// 
/// * `signal_a` - First signal
/// * `signal_b` - Second signal (to be subtracted from signal_a)
/// 
/// # Returns
/// 
/// A new vector containing the sample-wise difference
pub fn calculate_differential(signal_a: &[i16], signal_b: &[i16]) -> Vec<i16> {
    let mut result = Vec::with_capacity(signal_a.len());
    
    let length = std::cmp::min(signal_a.len(), signal_b.len());
    for i in 0..length {
        // Calculate difference with saturation to prevent overflow
        let diff = match signal_a[i].checked_sub(signal_b[i]) {
            Some(val) => val,
            None => {
                // Handle underflow with saturation
                if signal_a[i] < signal_b[i] {
                    i16::MIN
                } else {
                    i16::MAX
                }
            }
        };
        result.push(diff);
    }
    
    result
}

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
