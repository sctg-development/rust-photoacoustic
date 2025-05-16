// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Gaussian noise generator implementation

use std::time::SystemTime;

/// Random number generator using XORShift algorithm
pub struct NoiseGenerator {
    rng_state: u32,
}

impl NoiseGenerator {
    /// Create a new noise generator with a given seed
    pub fn new(seed: u32) -> Self {
        Self { rng_state: seed }
    }
    
    /// Create a new noise generator with a seed from the system time
    pub fn new_from_system_time() -> Self {
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u32;
        Self::new(seed)
    }
    
    /// Generate a random f32 between -1.0 and 1.0
    pub fn random_float(&mut self) -> f32 {
        // XOR Shift algorithm for pseudo-random numbers
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        
        // Convert to float between -1.0 and 1.0
        (self.rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    
    /// Generate a random value from a standard Gaussian distribution
    pub fn random_gaussian(&mut self) -> f32 {
        // Use Box-Muller transform to generate gaussian distributed values
        let u1 = (self.random_float() + 1.0) / 2.0; // remap to (0,1)
        let u2 = (self.random_float() + 1.0) / 2.0;
        
        // Avoid ln(0)
        let u1 = if u1 < 0.0001 { 0.0001 } else { u1 };
        
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
        z
    }
    
    /// Generate a buffer of mono Gaussian white noise
    pub fn generate_mono(&mut self, num_samples: u32, amplitude: f32) -> Vec<i16> {
        let mut samples = Vec::with_capacity(num_samples as usize);
        
        for _ in 0..num_samples {
            let sample = self.random_gaussian() * amplitude;
            let value = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            samples.push(value);
        }
        
        samples
    }
    
    /// Generate a buffer of stereo Gaussian white noise (independent channels)
    pub fn generate_stereo(&mut self, num_samples: u32, amplitude: f32) -> Vec<i16> {
        let mut samples = Vec::with_capacity((num_samples * 2) as usize);
        
        for _ in 0..num_samples {
            let sample_left = self.random_gaussian() * amplitude;
            let sample_right = self.random_gaussian() * amplitude;
            
            let value_left = (sample_left * 32767.0).clamp(-32768.0, 32767.0) as i16;
            let value_right = (sample_right * 32767.0).clamp(-32768.0, 32767.0) as i16;
            
            samples.push(value_left);
            samples.push(value_right);
        }
        
        samples
    }
    
    /// Generate a buffer of stereo Gaussian white noise with correlation between channels
    pub fn generate_correlated_stereo(&mut self, num_samples: u32, amplitude: f32, correlation: f32) -> Vec<i16> {
        let mut samples = Vec::with_capacity((num_samples * 2) as usize);
        let sqrt_one_minus_corr_squared = (1.0 - correlation * correlation).sqrt();
        
        for _ in 0..num_samples {
            let sample1 = self.random_gaussian() * amplitude;
            let independent_sample = self.random_gaussian();
            let sample2 = (correlation * sample1 + sqrt_one_minus_corr_squared * independent_sample) * amplitude;
            
            let value1 = (sample1 * 32767.0).clamp(-32768.0, 32767.0) as i16;
            let value2 = (sample2 * 32767.0).clamp(-32768.0, 32767.0) as i16;
            
            samples.push(value1);
            samples.push(value2);
        }
        
        samples
    }
}
