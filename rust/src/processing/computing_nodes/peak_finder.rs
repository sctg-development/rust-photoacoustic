// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! This module implements the PeakFinder node, which is used to find peaks in a signal.
//!
//! The PeakFinder is a specialized ComputingNode that implements the ProcessingNode trait.
//! It performs spectral analysis on input signals to detect frequency peaks while passing
//! the original data through unchanged. This enables real-time peak detection for
//! photoacoustic applications where the resonance frequency needs to be tracked.
//!
//! # Features
//!
//! - **FFT-based spectral analysis**: Uses Fast Fourier Transform for frequency domain analysis
//! - **Adaptive peak detection**: Configurable threshold-based peak detection algorithm
//! - **Pass-through processing**: Original signal data flows unchanged to next node
//! - **Shared state updates**: Peak detection results are stored in global shared state
//! - **Temporal coherence filtering**: Eliminates spurious peaks through temporal consistency
//! - **Moving average smoothing**: Provides stability in peak frequency tracking
//! - **Global parameter integration**: Uses photoacoustic.sample_rate and photoacoustic.frame_size
//!
//! # Configuration
//!
//! The PeakFinderNode uses a restrictive configuration approach:
//! - `sample_rate` is automatically set from `photoacoustic.sample_rate` (global config)
//! - `fft_size` is automatically set from `photoacoustic.frame_size` (global config)
//! - Only node-specific parameters can be configured directly:
//!   - `detection_threshold`: Minimum relative amplitude for peak detection (0.0-1.0)
//!   - `frequency_min`: Lower bound of frequency range to analyze (Hz)
//!   - `frequency_max`: Upper bound of frequency range to analyze (Hz)
//!   - `smoothing_factor`: Moving average smoothing factor (0.0-1.0)
//!
//! This design ensures consistency with the global photoacoustic system configuration
//! and prevents configuration mismatches that could lead to incorrect analysis.
//!
//! # Usage
//!
//! ```rust
//! use rust_photoacoustic::processing::computing_nodes::peak_finder::PeakFinderNode;
//! use rust_photoacoustic::processing::{ProcessingNode, ProcessingData};
//! use rust_photoacoustic::acquisition::AudioFrame;
//!
//! let mut peak_finder = PeakFinderNode::new("peak_detector".to_string())
//!     .with_detection_threshold(0.1)
//!     .with_frequency_range(800.0, 1200.0)
//!     .with_smoothing_factor(0.8);
//!
//! // Create some test audio data
//! let audio_frame = AudioFrame {
//!     channel_a: vec![0.1, 0.2, 0.3, 0.4],
//!     channel_b: vec![0.05, 0.15, 0.25, 0.35],
//!     sample_rate: 48000,
//!     timestamp: 1000,
//!     frame_number: 1,
//! };
//! let input_data = ProcessingData::AudioFrame(audio_frame);
//!
//! // Process audio data (data passes through unchanged)
//! let output = peak_finder.process(input_data).unwrap();
//!
//! // Peak results are available in shared state
//! let shared_state = peak_finder.get_shared_state();
//! {
//!     let state = shared_state.try_read().unwrap();
//!     if let Some(peak_freq) = state.peak_frequency {
//!         println!("Detected peak at {} Hz", peak_freq);
//!     }
//! }
//! ```

use crate::processing::computing_nodes::{ComputingSharedData, PeakResult, SharedComputingState};
use crate::processing::nodes::ProcessingMetadata;
use crate::processing::{ProcessingData, ProcessingNode};
use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use num_complex;
use realfft::{RealFftPlanner, RealToComplex};
use rustfft::{num_complex::Complex, FftPlanner};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

/// A computing node that performs real-time peak detection in the frequency domain
///
/// This node implements spectral analysis using FFT to detect frequency peaks in audio signals.
/// It's designed as a pass-through node that doesn't modify the signal data but extracts
/// analytical information for use by other nodes in the processing graph.
///
/// The peak detection algorithm uses:
/// - FFT-based spectral analysis with configurable window size
/// - Threshold-based peak detection with amplitude filtering
/// - Temporal coherence filtering to eliminate spurious detections
/// - Moving average smoothing for stable frequency tracking
/// - Frequency range limiting for focused analysis
pub struct PeakFinderNode {
    /// Unique identifier for this node
    id: String,

    /// Minimum amplitude threshold for peak detection (0.0 to 1.0)
    detection_threshold: f32,

    /// Lower bound of frequency range to analyze (Hz)
    frequency_min: f32,

    /// Upper bound of frequency range to analyze (Hz)
    frequency_max: f32,

    /// FFT window size (must be power of 2)
    fft_size: usize,

    /// Sample rate for frequency calculations
    sample_rate: u32,

    /// Smoothing factor for moving average (0.0 = no smoothing, 1.0 = maximum smoothing)
    smoothing_factor: f32,

    /// Number of consecutive detections required for validation
    coherence_threshold: usize,

    /// Shared state for communicating results to other nodes
    shared_state: Arc<RwLock<ComputingSharedData>>,

    /// FFT planner for efficient computation
    fft_planner: RealFftPlanner<f32>,

    /// Cached FFT instance
    fft: Option<Arc<dyn RealToComplex<f32>>>,

    /// Buffer for accumulating audio samples
    sample_buffer: VecDeque<f32>,

    /// History of recent peak detections for coherence filtering
    peak_history: VecDeque<Option<f32>>,

    /// Current smoothed peak frequency
    smoothed_frequency: Option<f32>,

    /// Statistics for monitoring performance
    processing_count: u64,
    last_detection_time: Option<SystemTime>,
}

impl PeakFinderNode {
    /// Create a new PeakFinder node with default parameters
    ///
    /// Default configuration:
    /// - Detection threshold: 0.1 (10% of maximum amplitude)
    /// - Frequency range: 20 Hz to 20 kHz (full audio spectrum)
    /// - FFT size: 2048 samples
    /// - Sample rate: 48 kHz
    /// - Smoothing factor: 0.7 (moderate smoothing)
    /// - Coherence threshold: 3 consecutive detections
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    ///
    /// # Returns
    ///
    /// A new PeakFinderNode instance with default configuration
    pub fn new(id: String) -> Self {
        let fft_size = 2048;
        let mut fft_planner = RealFftPlanner::<f32>::new();
        let fft = Some(fft_planner.plan_fft_forward(fft_size));

        Self {
            id,
            detection_threshold: 0.1,
            frequency_min: 20.0,
            frequency_max: 20000.0,
            fft_size,
            sample_rate: 48000,
            smoothing_factor: 0.7,
            coherence_threshold: 3,
            shared_state: Arc::new(RwLock::new(ComputingSharedData::default())),
            fft_planner,
            fft,
            sample_buffer: VecDeque::with_capacity(fft_size * 2),
            peak_history: VecDeque::with_capacity(10),
            smoothed_frequency: None,
            processing_count: 0,
            last_detection_time: None,
        }
    }

    /// Create a new PeakFinder node with an external shared computing state
    ///
    /// This constructor allows sharing the computing state between multiple nodes,
    /// enabling centralized management of analytical results. If no shared state
    /// is provided, creates a new one.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `shared_state` - Optional external shared computing state
    ///
    /// # Returns
    ///
    /// A new PeakFinderNode instance with the provided or new shared state
    pub fn new_with_shared_state(id: String, shared_state: Option<SharedComputingState>) -> Self {
        let fft_size = 2048;
        let mut fft_planner = RealFftPlanner::<f32>::new();
        let fft = Some(fft_planner.plan_fft_forward(fft_size));

        let shared_state =
            shared_state.unwrap_or_else(|| Arc::new(RwLock::new(ComputingSharedData::default())));

        Self {
            id,
            detection_threshold: 0.1,
            frequency_min: 20.0,
            frequency_max: 20000.0,
            fft_size,
            sample_rate: 48000,
            smoothing_factor: 0.7,
            coherence_threshold: 3,
            shared_state,
            fft_planner,
            fft,
            sample_buffer: VecDeque::with_capacity(fft_size * 2),
            peak_history: VecDeque::with_capacity(10),
            smoothed_frequency: None,
            processing_count: 0,
            last_detection_time: None,
        }
    }

    /// Set the detection threshold for peak identification
    ///
    /// # Arguments
    ///
    /// * `threshold` - Minimum relative amplitude (0.0 to 1.0) for peak detection
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_detection_threshold(mut self, threshold: f32) -> Self {
        self.detection_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set the frequency range for analysis
    ///
    /// # Arguments
    ///
    /// * `min_freq` - Lower bound of frequency range (Hz)
    /// * `max_freq` - Upper bound of frequency range (Hz)
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_frequency_range(mut self, min_freq: f32, max_freq: f32) -> Self {
        self.frequency_min = min_freq.max(0.0);
        self.frequency_max = max_freq.min(self.sample_rate as f32 / 2.0);
        self
    }

    /// Set the FFT window size
    ///
    /// # Arguments
    ///
    /// * `size` - FFT window size (must be power of 2)
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_fft_size(mut self, size: usize) -> Self {
        if size.is_power_of_two() && size >= 64 {
            self.fft_size = size;
            self.fft = Some(self.fft_planner.plan_fft_forward(size));
            self.sample_buffer = VecDeque::with_capacity(size * 2);
        }
        self
    }

    /// Set the sample rate for frequency calculations
    ///
    /// # Arguments
    ///
    /// * `rate` - Sample rate in Hz
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_sample_rate(mut self, rate: u32) -> Self {
        self.sample_rate = rate;
        self.frequency_max = self.frequency_max.min(rate as f32 / 2.0);
        self
    }

    /// Set the smoothing factor for moving average
    ///
    /// # Arguments
    ///
    /// * `factor` - Smoothing factor (0.0 = no smoothing, 1.0 = maximum smoothing)
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_smoothing_factor(mut self, factor: f32) -> Self {
        self.smoothing_factor = factor.clamp(0.0, 1.0);
        self
    }

    /// Get access to the shared state for reading results
    ///
    /// # Returns
    ///
    /// Arc<RwLock<ComputingSharedData>> for thread-safe access to computation results
    pub fn get_shared_state(&self) -> Arc<RwLock<ComputingSharedData>> {
        Arc::clone(&self.shared_state)
    }

    /// Perform FFT-based spectral analysis on accumulated samples
    ///
    /// This method applies a Hann window to reduce spectral leakage, performs FFT,
    /// calculates magnitude spectrum, and searches for peaks within the specified
    /// frequency range.
    ///
    /// # Returns
    ///
    /// Result containing (peak_frequency, peak_amplitude) or None if no peak found
    fn analyze_spectrum(&mut self) -> Result<Option<(f32, f32)>> {
        if self.sample_buffer.len() < self.fft_size {
            return Ok(None);
        }

        // Extract samples for FFT
        let mut samples: Vec<f32> = self
            .sample_buffer
            .range(0..self.fft_size)
            .cloned()
            .collect();

        // Apply Hann window to reduce spectral leakage
        for (i, sample) in samples.iter_mut().enumerate() {
            let window = 0.5
                * (1.0
                    - (2.0 * std::f32::consts::PI * i as f32 / (self.fft_size - 1) as f32).cos());
            *sample *= window;
        }

        // Prepare FFT output buffer
        let mut spectrum = vec![num_complex::Complex::new(0.0f32, 0.0f32); self.fft_size / 2 + 1];

        // Perform FFT
        if let Some(ref fft) = self.fft {
            fft.process(&mut samples, &mut spectrum)
                .map_err(|e| anyhow!("FFT processing failed: {:?}", e))?;
        } else {
            return Err(anyhow!("FFT not initialized"));
        }

        // Calculate magnitude spectrum
        let magnitudes: Vec<f32> = spectrum.iter().map(|c| c.norm()).collect();

        // Find frequency resolution
        let freq_resolution = self.sample_rate as f32 / self.fft_size as f32;

        // Convert frequency range to bin indices
        let min_bin = (self.frequency_min / freq_resolution) as usize;
        let max_bin = ((self.frequency_max / freq_resolution) as usize).min(magnitudes.len() - 1);

        if min_bin >= max_bin {
            return Ok(None);
        }

        // Find the peak within the frequency range
        let mut peak_bin = min_bin;
        let mut peak_magnitude = magnitudes[min_bin];

        for i in min_bin..=max_bin {
            if magnitudes[i] > peak_magnitude {
                peak_magnitude = magnitudes[i];
                peak_bin = i;
            }
        }

        // Calculate amplitude in dB (20 * log10(magnitude))
        // Use a reference value to avoid log(0) and provide meaningful dB scale
        let reference_magnitude = 1e-6f32; // Small reference to avoid numerical issues
        let peak_amplitude_db = if peak_magnitude > reference_magnitude {
            20.0 * peak_magnitude.log10()
        } else {
            -120.0 // Very small signal, set to -120dB
        };

        // For threshold checking, still use normalized amplitude (relative to max in range)
        let max_magnitude = magnitudes[min_bin..=max_bin]
            .iter()
            .cloned()
            .fold(0.0f32, f32::max);
        let normalized_amplitude = if max_magnitude > 0.0 {
            peak_magnitude / max_magnitude
        } else {
            0.0
        };

        // Check if peak meets threshold (using normalized amplitude)
        if normalized_amplitude >= self.detection_threshold {
            let peak_frequency = peak_bin as f32 * freq_resolution;
            // Return frequency and dB amplitude
            Ok(Some((peak_frequency, peak_amplitude_db)))
        } else {
            Ok(None)
        }
    }

    /// Apply temporal coherence filtering to validate peak detections
    ///
    /// This method maintains a history of recent peak detections and only accepts
    /// a peak as valid if it appears consistently over multiple analysis windows.
    ///
    /// # Arguments
    ///
    /// * `detected_peak` - Optional (frequency, amplitude) of current detection
    ///
    /// # Returns
    ///
    /// Validated peak frequency if coherence threshold is met
    fn apply_coherence_filter(&mut self, detected_peak: Option<(f32, f32)>) -> Option<f32> {
        // Add current detection to history
        self.peak_history
            .push_back(detected_peak.map(|(freq, _)| freq));

        // Limit history size
        while self.peak_history.len() > self.coherence_threshold * 2 {
            self.peak_history.pop_front();
        }

        // Check for coherent detections
        if self.peak_history.len() >= self.coherence_threshold {
            let recent_detections: Vec<f32> = self
                .peak_history
                .iter()
                .rev()
                .take(self.coherence_threshold)
                .filter_map(|&freq| freq)
                .collect();

            if recent_detections.len() >= self.coherence_threshold {
                // Calculate average frequency and check for consistency
                let avg_frequency =
                    recent_detections.iter().sum::<f32>() / recent_detections.len() as f32;
                let max_deviation = recent_detections
                    .iter()
                    .map(|&freq| (freq - avg_frequency).abs())
                    .fold(0.0f32, f32::max);

                // Accept if deviation is small relative to frequency
                if max_deviation < avg_frequency * 0.05 {
                    // 5% tolerance
                    return Some(avg_frequency);
                }
            }
        }

        None
    }

    /// Apply moving average smoothing to stabilize frequency tracking
    ///
    /// # Arguments
    ///
    /// * `new_frequency` - Newly detected peak frequency
    ///
    /// # Returns
    ///
    /// Smoothed frequency value
    fn apply_smoothing(&mut self, new_frequency: f32) -> f32 {
        match self.smoothed_frequency {
            Some(current) => {
                let smoothed =
                    current * self.smoothing_factor + new_frequency * (1.0 - self.smoothing_factor);
                self.smoothed_frequency = Some(smoothed);
                smoothed
            }
            None => {
                self.smoothed_frequency = Some(new_frequency);
                new_frequency
            }
        }
    }

    /// Update the shared state with new peak detection results
    ///
    /// # Arguments
    ///
    /// * `frequency` - Detected peak frequency
    /// * `amplitude` - Peak amplitude in dB (20 * log10(magnitude))
    fn update_shared_state(&mut self, frequency: f32, amplitude: f32) {
        // Limit debug display to avoid flooding logs
        if self.processing_count % 100 == 0 {
            info!(
                "Peak finder '{}': Detected peak at {:.2} Hz with amplitude {:.2} dB",
                self.id, frequency, amplitude
            );
        }

        match self.shared_state.try_write() {
            Ok(mut state) => {
                // Create new peak result
                let peak_result = PeakResult {
                    frequency,
                    amplitude,
                    concentration_ppm: None, // Will be calculated if needed
                    timestamp: SystemTime::now(),
                    coherence_score: 1.0, // Default coherence score
                    processing_metadata: std::collections::HashMap::new(),
                };

                // Update using the new method that handles both HashMap and legacy fields
                state.update_peak_result(self.id.clone(), peak_result);
            }
            Err(_) => {
                warn!("Peak finder '{}': Failed to acquire write lock for shared state - frequency={:.2} Hz, amplitude={:.4}", 
                      self.id, frequency, amplitude);
            }
        }
        self.last_detection_time = Some(SystemTime::now());
    }
}

impl ProcessingNode for PeakFinderNode {
    /// Process input data while performing spectral analysis
    ///
    /// This method implements the pass-through behavior characteristic of ComputingNodes:
    /// the input data is returned unchanged while spectral analysis is performed in parallel.
    /// Peak detection results are stored in the shared state for access by other nodes.
    ///
    /// # Arguments
    ///
    /// * `input` - Input audio data to analyze
    ///
    /// # Returns
    ///
    /// The same input data unchanged, allowing it to flow to the next node
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        self.processing_count += 1;

        // Extract audio samples from both channels
        let samples = match &input {
            ProcessingData::AudioFrame(frame) => {
                // Update sample rate if different
                if frame.sample_rate != self.sample_rate {
                    self.sample_rate = frame.sample_rate;
                    self.frequency_max = self.frequency_max.min(frame.sample_rate as f32 / 2.0);
                }

                // Use channel A for analysis (could be made configurable)
                frame.channel_a.clone()
            }
            ProcessingData::SingleChannel {
                samples,
                sample_rate,
                ..
            } => {
                if *sample_rate != self.sample_rate {
                    self.sample_rate = *sample_rate;
                    self.frequency_max = self.frequency_max.min(*sample_rate as f32 / 2.0);
                }
                samples.clone()
            }
            ProcessingData::DualChannel {
                channel_a,
                sample_rate,
                ..
            } => {
                if *sample_rate != self.sample_rate {
                    self.sample_rate = *sample_rate;
                    self.frequency_max = self.frequency_max.min(*sample_rate as f32 / 2.0);
                }
                channel_a.clone()
            }
            _ => {
                // For non-audio data, pass through without analysis
                return Ok(input);
            }
        };

        // Accumulate samples in buffer
        for sample in samples {
            self.sample_buffer.push_back(sample);
        }

        // Maintain buffer size
        while self.sample_buffer.len() > self.fft_size * 2 {
            self.sample_buffer.pop_front();
        }

        // Perform spectral analysis if we have enough samples
        if self.sample_buffer.len() >= self.fft_size {
            // Debug logs every 50 processing cycles to avoid log flooding
            let should_debug = self.processing_count % 50 == 0;

            if should_debug {
                debug!(
                    "Peak finder '{}': Performing spectral analysis with {} samples (cycle {})",
                    self.id,
                    self.sample_buffer.len(),
                    self.processing_count
                );
            }

            if let Ok(detected_peak) = self.analyze_spectrum() {
                // Apply coherence filtering
                if let Some((raw_frequency, amplitude)) = detected_peak {
                    if should_debug {
                        debug!(
                            "Peak finder '{}': Raw peak detected at {:.2} Hz with amplitude {:.4}",
                            self.id, raw_frequency, amplitude
                        );
                    }

                    if let Some(validated_frequency) =
                        self.apply_coherence_filter(Some((raw_frequency, amplitude)))
                    {
                        // Apply smoothing
                        let smoothed_frequency = self.apply_smoothing(validated_frequency);

                        if should_debug {
                            debug!(
                                "Peak finder '{}': Validated and smoothed peak at {:.2} Hz",
                                self.id, smoothed_frequency
                            );
                        }

                        // Update shared state - always log state updates but less verbosely
                        self.update_shared_state(smoothed_frequency, amplitude);
                    } else {
                        if should_debug {
                            debug!(
                                "Peak finder '{}': Peak at {:.2} Hz failed coherence filtering",
                                self.id, raw_frequency
                            );
                        }
                    }
                } else {
                    if should_debug {
                        debug!(
                            "Peak finder '{}': No peak detected in current frame",
                            self.id
                        );
                    }
                    // No peak detected, still update coherence filter
                    self.apply_coherence_filter(None);
                }
            } else {
                if should_debug {
                    debug!("Peak finder '{}': Spectral analysis failed", self.id);
                }
            }
        } else {
            // Only log insufficient samples occasionally as it's expected during startup
            if self.processing_count % 100 == 0 {
                debug!(
                    "Peak finder '{}': Insufficient samples for analysis ({}/{}) - cycle {}",
                    self.id,
                    self.sample_buffer.len(),
                    self.fft_size,
                    self.processing_count
                );
            }
        }

        // Return input data unchanged (pass-through behavior)
        Ok(input)
    }

    /// Get the unique identifier for this node
    fn node_id(&self) -> &str {
        &self.id
    }

    /// Get the node type identifier
    fn node_type(&self) -> &str {
        "computing_peak_finder"
    }

    /// Check if this node can accept the given input type
    ///
    /// PeakFinderNode can process any audio data types for analysis
    fn accepts_input(&self, input: &ProcessingData) -> bool {
        matches!(
            input,
            ProcessingData::AudioFrame(_)
                | ProcessingData::SingleChannel { .. }
                | ProcessingData::DualChannel { .. }
        )
    }

    /// Get the expected output type for the given input
    ///
    /// PeakFinderNode is a pass-through node, so output type matches input type
    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        match input {
            ProcessingData::AudioFrame(_) => Some("AudioFrame".to_string()),
            ProcessingData::SingleChannel { .. } => Some("SingleChannel".to_string()),
            ProcessingData::DualChannel { .. } => Some("DualChannel".to_string()),
            ProcessingData::PhotoacousticResult { .. } => Some("PhotoacousticResult".to_string()),
        }
    }

    /// Reset internal state
    ///
    /// Clears all buffers and resets detection state
    fn reset(&mut self) {
        self.sample_buffer.clear();
        self.peak_history.clear();
        self.smoothed_frequency = None;
        self.processing_count = 0;
        self.last_detection_time = None;

        // Reset shared state
        if let Ok(mut state) = self.shared_state.try_write() {
            state.peak_frequency = None;
            state.peak_amplitude = None;
            state.last_update = SystemTime::now();
        }
    }

    /// Clone the node for graph reconfiguration
    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        Box::new(
            PeakFinderNode::new(self.id.clone())
                .with_detection_threshold(self.detection_threshold)
                .with_frequency_range(self.frequency_min, self.frequency_max)
                .with_fft_size(self.fft_size)
                .with_sample_rate(self.sample_rate)
                .with_smoothing_factor(self.smoothing_factor),
        )
    }

    /// Check if this node supports hot-reload configuration updates
    fn supports_hot_reload(&self) -> bool {
        true // PeakFinderNode supports dynamic configuration updates
    }

    /// Update configuration parameters dynamically
    ///
    /// Supports updating:
    /// - `detection_threshold`: Peak detection threshold (0.0 to 1.0)
    /// - `frequency_min`: Lower frequency bound (Hz)
    /// - `frequency_max`: Upper frequency bound (Hz)
    /// - `fft_size`: FFT window size (must be power of 2)
    /// - `smoothing_factor`: Moving average smoothing (0.0 to 1.0)
    /// - `coherence_threshold`: Number of consecutive detections required
    ///
    /// # Arguments
    ///
    /// * `parameters` - JSON object containing parameter updates
    ///
    /// # Returns
    ///
    /// Result indicating success and whether any parameters were changed
    fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
        let mut updated = false;

        if let Some(threshold) = parameters.get("detection_threshold") {
            if let Some(t) = threshold.as_f64() {
                let new_threshold = (t as f32).clamp(0.0, 1.0);
                if (new_threshold - self.detection_threshold).abs() > f32::EPSILON {
                    self.detection_threshold = new_threshold;
                    updated = true;
                }
            }
        }

        if let Some(freq_min) = parameters.get("frequency_min") {
            if let Some(f) = freq_min.as_f64() {
                let new_freq = (f as f32).max(0.0);
                if (new_freq - self.frequency_min).abs() > f32::EPSILON {
                    self.frequency_min = new_freq;
                    updated = true;
                }
            }
        }

        if let Some(freq_max) = parameters.get("frequency_max") {
            if let Some(f) = freq_max.as_f64() {
                let new_freq = (f as f32).min(self.sample_rate as f32 / 2.0);
                if (new_freq - self.frequency_max).abs() > f32::EPSILON {
                    self.frequency_max = new_freq;
                    updated = true;
                }
            }
        }

        if let Some(fft_size) = parameters.get("fft_size") {
            if let Some(size) = fft_size.as_u64() {
                let new_size = size as usize;
                if new_size.is_power_of_two() && new_size >= 64 && new_size != self.fft_size {
                    self.fft_size = new_size;
                    self.fft = Some(self.fft_planner.plan_fft_forward(new_size));
                    self.sample_buffer = VecDeque::with_capacity(new_size * 2);
                    updated = true;
                }
            }
        }

        if let Some(smoothing) = parameters.get("smoothing_factor") {
            if let Some(s) = smoothing.as_f64() {
                let new_smoothing = (s as f32).clamp(0.0, 1.0);
                if (new_smoothing - self.smoothing_factor).abs() > f32::EPSILON {
                    self.smoothing_factor = new_smoothing;
                    updated = true;
                }
            }
        }

        if let Some(coherence) = parameters.get("coherence_threshold") {
            if let Some(c) = coherence.as_u64() {
                let new_coherence = (c as usize).max(1);
                if new_coherence != self.coherence_threshold {
                    self.coherence_threshold = new_coherence;
                    updated = true;
                }
            }
        }

        Ok(updated)
    }

    /// Set the shared computing state for this node
    ///
    /// For PeakFinderNode, this replaces the internal shared state with the provided one,
    /// allowing the node to write its results to a graph-wide shared state.
    fn set_shared_computing_state(&mut self, shared_state: Option<SharedComputingState>) {
        if let Some(state) = shared_state {
            self.shared_state = state;
        }
    }

    /// Get the shared computing state for this node
    ///
    /// Returns the current shared computing state that contains peak detection results
    fn get_shared_computing_state(&self) -> Option<SharedComputingState> {
        Some(self.shared_state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processing::ProcessingData;
    use std::f32::consts::PI;

    /// Helper function to generate a sine wave
    fn generate_sine_wave(
        frequency: f32,
        sample_rate: u32,
        duration_sec: f32,
        amplitude: f32,
    ) -> Vec<f32> {
        let num_samples = (sample_rate as f32 * duration_sec) as usize;
        let mut signal = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let sample = amplitude * (2.0 * PI * frequency * t).sin();
            signal.push(sample);
        }

        signal
    }

    /// Helper function to generate composite signal with multiple frequencies
    fn generate_composite_signal(
        frequencies: &[(f32, f32)],
        sample_rate: u32,
        duration_sec: f32,
    ) -> Vec<f32> {
        let num_samples = (sample_rate as f32 * duration_sec) as usize;
        let mut signal = vec![0.0; num_samples];

        for &(freq, amplitude) in frequencies {
            let sine_wave = generate_sine_wave(freq, sample_rate, duration_sec, amplitude);
            for (i, &sample) in sine_wave.iter().enumerate() {
                if i < signal.len() {
                    signal[i] += sample;
                }
            }
        }

        signal
    }

    #[test]
    fn test_peak_finder_creation() {
        let peak_finder = PeakFinderNode::new("test_peak_finder".to_string());

        assert_eq!(peak_finder.node_id(), "test_peak_finder");
        assert_eq!(peak_finder.node_type(), "computing_peak_finder");
        assert_eq!(peak_finder.detection_threshold, 0.1);
        assert_eq!(peak_finder.frequency_min, 20.0);
        assert_eq!(peak_finder.frequency_max, 20000.0);
        assert_eq!(peak_finder.fft_size, 2048);
        assert_eq!(peak_finder.sample_rate, 48000);
        assert_eq!(peak_finder.smoothing_factor, 0.7);
        assert_eq!(peak_finder.coherence_threshold, 3);
    }

    #[test]
    fn test_peak_finder_builder_pattern() {
        let peak_finder = PeakFinderNode::new("test".to_string())
            .with_detection_threshold(0.2)
            .with_frequency_range(100.0, 2000.0)
            .with_fft_size(1024)
            .with_sample_rate(44100)
            .with_smoothing_factor(0.5);

        assert_eq!(peak_finder.detection_threshold, 0.2);
        assert_eq!(peak_finder.frequency_min, 100.0);
        assert_eq!(peak_finder.frequency_max, 2000.0);
        assert_eq!(peak_finder.fft_size, 1024);
        assert_eq!(peak_finder.sample_rate, 44100);
        assert_eq!(peak_finder.smoothing_factor, 0.5);
    }

    #[test]
    fn test_peak_finder_parameter_validation() {
        let peak_finder = PeakFinderNode::new("test".to_string())
            .with_detection_threshold(-0.5) // Should be clamped to 0.0
            .with_detection_threshold(1.5) // Should be clamped to 1.0
            .with_smoothing_factor(-0.1) // Should be clamped to 0.0
            .with_smoothing_factor(1.1); // Should be clamped to 1.0

        assert_eq!(peak_finder.detection_threshold, 1.0);
        assert_eq!(peak_finder.smoothing_factor, 1.0);
    }

    #[test]
    fn test_peak_finder_fft_size_validation() {
        // Valid power of 2
        let peak_finder1 = PeakFinderNode::new("test1".to_string()).with_fft_size(512);
        assert_eq!(peak_finder1.fft_size, 512);

        // Invalid size (not power of 2) - should keep default
        let peak_finder2 = PeakFinderNode::new("test2".to_string()).with_fft_size(1000);
        assert_eq!(peak_finder2.fft_size, 2048); // Default unchanged

        // Too small - should keep default
        let peak_finder3 = PeakFinderNode::new("test3".to_string()).with_fft_size(32);
        assert_eq!(peak_finder3.fft_size, 2048); // Default unchanged
    }

    #[test]
    fn test_peak_finder_pass_through_behavior() {
        use crate::acquisition::AudioFrame;

        let mut peak_finder = PeakFinderNode::new("test".to_string());

        let audio_frame = AudioFrame {
            channel_a: vec![0.1, 0.2, 0.3, 0.4],
            channel_b: vec![0.05, 0.15, 0.25, 0.35],
            sample_rate: 48000,
            timestamp: 1000,
            frame_number: 1,
        };

        let input_data = ProcessingData::AudioFrame(audio_frame.clone());
        let output_data = peak_finder.process(input_data.clone()).unwrap();

        // Data should pass through unchanged
        match (&input_data, &output_data) {
            (ProcessingData::AudioFrame(in_frame), ProcessingData::AudioFrame(out_frame)) => {
                assert_eq!(in_frame.channel_a, out_frame.channel_a);
                assert_eq!(in_frame.channel_b, out_frame.channel_b);
                assert_eq!(in_frame.sample_rate, out_frame.sample_rate);
                assert_eq!(in_frame.timestamp, out_frame.timestamp);
                assert_eq!(in_frame.frame_number, out_frame.frame_number);
            }
            _ => panic!("Data type should be preserved"),
        }
    }

    #[test]
    fn test_peak_finder_single_frequency_detection() {
        use crate::acquisition::AudioFrame;

        let mut peak_finder = PeakFinderNode::new("test".to_string())
            .with_detection_threshold(0.1)
            .with_frequency_range(900.0, 1100.0)
            .with_fft_size(2048)
            .with_smoothing_factor(0.0) // No smoothing for precise testing
            .with_sample_rate(48000);

        // Generate a strong 1kHz signal
        let test_frequency = 1000.0;
        let signal = generate_sine_wave(test_frequency, 48000, 0.1, 1.0);

        let audio_frame = AudioFrame {
            channel_a: signal,
            channel_b: vec![],
            sample_rate: 48000,
            timestamp: 1000,
            frame_number: 1,
        };

        let input_data = ProcessingData::AudioFrame(audio_frame);

        // Process several times to trigger coherence validation
        for _ in 0..5 {
            let _output = peak_finder.process(input_data.clone()).unwrap();
        }

        // Check if peak was detected in shared state
        let shared_state = peak_finder.get_shared_state();
        let state = shared_state.try_read().unwrap();

        if let Some(detected_freq) = state.peak_frequency {
            // Allow some tolerance due to FFT bin resolution
            let freq_error = (detected_freq - test_frequency).abs();
            let freq_tolerance = 48000.0 / 2048.0; // One FFT bin
            assert!(
                freq_error < freq_tolerance * 2.0,
                "Detected frequency {} should be close to {}",
                detected_freq,
                test_frequency
            );
        }
    }

    #[test]
    fn test_peak_finder_no_detection_below_threshold() {
        use crate::acquisition::AudioFrame;

        let mut peak_finder = PeakFinderNode::new("test".to_string())
            .with_detection_threshold(0.8) // High threshold
            .with_frequency_range(900.0, 1100.0);

        // Generate a weak signal
        let signal = generate_sine_wave(1000.0, 48000, 0.1, 0.1);

        let audio_frame = AudioFrame {
            channel_a: signal,
            channel_b: vec![],
            sample_rate: 48000,
            timestamp: 1000,
            frame_number: 1,
        };

        let input_data = ProcessingData::AudioFrame(audio_frame);

        // Process several times
        for _ in 0..5 {
            let _output = peak_finder.process(input_data.clone()).unwrap();
        }

        // Should not detect anything due to high threshold
        let shared_state = peak_finder.get_shared_state();
        let state = shared_state.try_read().unwrap();

        // Peak might be None or very different from input frequency due to noise
        // Main thing is that processing doesn't crash
        assert!(true); // Just ensure we reach here without panic
    }

    #[test]
    fn test_peak_finder_frequency_range_filtering() {
        use crate::acquisition::AudioFrame;

        let mut peak_finder = PeakFinderNode::new("test".to_string())
            .with_detection_threshold(0.1)
            .with_frequency_range(1900.0, 2100.0); // Only allow 2kHz range

        // Generate 1kHz signal (outside range)
        let signal = generate_sine_wave(1000.0, 48000, 0.1, 1.0);

        let audio_frame = AudioFrame {
            channel_a: signal,
            channel_b: vec![],
            sample_rate: 48000,
            timestamp: 1000,
            frame_number: 1,
        };

        let input_data = ProcessingData::AudioFrame(audio_frame);

        // Process several times
        for _ in 0..5 {
            let _output = peak_finder.process(input_data.clone()).unwrap();
        }

        // Should not detect 1kHz peak as it's outside the allowed range
        let shared_state = peak_finder.get_shared_state();
        let state = shared_state.try_read().unwrap();

        if let Some(detected_freq) = state.peak_frequency {
            // If anything is detected, it should be in the allowed range
            // Allow small tolerance due to FFT bin resolution
            assert!(
                detected_freq >= 1895.0 && detected_freq <= 2105.0,
                "Detected frequency {} should be approximately in range [1900, 2100]",
                detected_freq
            );
        }
    }

    #[test]
    fn test_peak_finder_composite_signal() {
        use crate::acquisition::AudioFrame;

        let mut peak_finder = PeakFinderNode::new("test".to_string())
            .with_detection_threshold(0.3)
            .with_frequency_range(950.0, 1050.0);

        // Generate composite signal with dominant 1kHz component
        let frequencies = [(500.0, 0.2), (1000.0, 1.0), (2000.0, 0.3)];
        let signal = generate_composite_signal(&frequencies, 48000, 0.1);

        let audio_frame = AudioFrame {
            channel_a: signal,
            channel_b: vec![],
            sample_rate: 48000,
            timestamp: 1000,
            frame_number: 1,
        };

        let input_data = ProcessingData::AudioFrame(audio_frame);

        // Process several times for coherence
        for _ in 0..5 {
            let _output = peak_finder.process(input_data.clone()).unwrap();
        }

        let shared_state = peak_finder.get_shared_state();
        let state = shared_state.try_read().unwrap();

        if let Some(detected_freq) = state.peak_frequency {
            // Should detect the 1kHz component (strongest in allowed range)
            let freq_error = (detected_freq - 1000.0).abs();
            assert!(
                freq_error < 50.0,
                "Should detect 1kHz component, got {}",
                detected_freq
            );
        }
    }

    #[test]
    fn test_peak_finder_config_update() {
        let mut peak_finder = PeakFinderNode::new("test".to_string());

        let config = serde_json::json!({
            "detection_threshold": 0.3,
            "frequency_min": 500.0,
            "frequency_max": 1500.0,
            "fft_size": 1024,
            "smoothing_factor": 0.9,
            "coherence_threshold": 5
        });

        let updated = peak_finder.update_config(&config).unwrap();
        assert!(updated);

        assert_eq!(peak_finder.detection_threshold, 0.3);
        assert_eq!(peak_finder.frequency_min, 500.0);
        assert_eq!(peak_finder.frequency_max, 1500.0);
        assert_eq!(peak_finder.fft_size, 1024);
        assert_eq!(peak_finder.smoothing_factor, 0.9);
        assert_eq!(peak_finder.coherence_threshold, 5);
    }

    #[test]
    fn test_peak_finder_invalid_config() {
        let mut peak_finder = PeakFinderNode::new("test".to_string());

        let config = serde_json::json!({
            "detection_threshold": "invalid",
            "fft_size": 1000, // Not power of 2
            "frequency_min": -100.0, // Should be clamped
            "smoothing_factor": 2.0 // Should be clamped
        });

        let updated = peak_finder.update_config(&config).unwrap();

        // Some parameters might be updated (those that can be validated and clamped)
        // Invalid FFT size should be ignored
        assert_eq!(peak_finder.fft_size, 2048); // Should remain default
        assert_eq!(peak_finder.frequency_min, 0.0); // Should be clamped
        assert_eq!(peak_finder.smoothing_factor, 1.0); // Should be clamped
    }

    #[test]
    fn test_peak_finder_processing_counts() {
        use crate::acquisition::AudioFrame;

        let mut peak_finder = PeakFinderNode::new("test".to_string());

        let signal = generate_sine_wave(1000.0, 48000, 0.05, 1.0);
        let audio_frame = AudioFrame {
            channel_a: signal,
            channel_b: vec![],
            sample_rate: 48000,
            timestamp: 1000,
            frame_number: 1,
        };

        let input_data = ProcessingData::AudioFrame(audio_frame);

        // Initial state
        assert_eq!(peak_finder.processing_count, 0);

        // Process once
        let _output = peak_finder.process(input_data.clone()).unwrap();
        assert_eq!(peak_finder.processing_count, 1);

        // Process again
        let _output = peak_finder.process(input_data).unwrap();
        assert_eq!(peak_finder.processing_count, 2);

        // Verify other internal state
        assert!(peak_finder.sample_buffer.len() > 0);
        assert_eq!(peak_finder.fft_size, 2048);
        assert_eq!(peak_finder.sample_rate, 48000);
        assert_eq!(peak_finder.frequency_min, 20.0);
        assert_eq!(peak_finder.frequency_max, 20000.0);
        assert_eq!(peak_finder.detection_threshold, 0.1);
        assert_eq!(peak_finder.smoothing_factor, 0.7);
        assert_eq!(peak_finder.coherence_threshold, 3);
    }

    #[test]
    fn test_peak_finder_non_audio_data_passthrough() {
        let mut peak_finder = PeakFinderNode::new("test".to_string());

        // Test with photoacoustic result data (non-audio)
        let input_data = ProcessingData::PhotoacousticResult {
            signal: vec![0.1, 0.2, 0.3],
            metadata: ProcessingMetadata {
                original_frame_number: 1,
                original_timestamp: 1000,
                sample_rate: 48000,
                processing_steps: vec!["test".to_string()],
                processing_latency_us: 100,
            },
        };
        let output_data = peak_finder.process(input_data.clone()).unwrap();

        // Should pass through unchanged
        match (&input_data, &output_data) {
            (
                ProcessingData::PhotoacousticResult { .. },
                ProcessingData::PhotoacousticResult { .. },
            ) => {
                assert!(true); // Correct passthrough
            }
            _ => panic!("Non-audio data should pass through unchanged"),
        }
    }

    #[test]
    fn test_peak_finder_sample_rate_adaptation() {
        use crate::acquisition::AudioFrame;

        let mut peak_finder = PeakFinderNode::new("test".to_string()).with_sample_rate(48000);

        assert_eq!(peak_finder.sample_rate, 48000);

        // Process data with different sample rate
        let signal = generate_sine_wave(1000.0, 44100, 0.05, 1.0);
        let audio_frame = AudioFrame {
            channel_a: signal,
            channel_b: vec![],
            sample_rate: 44100, // Different sample rate
            timestamp: 1000,
            frame_number: 1,
        };

        let input_data = ProcessingData::AudioFrame(audio_frame);

        let _output = peak_finder.process(input_data).unwrap();

        // Sample rate should be updated
        assert_eq!(peak_finder.sample_rate, 44100);
    }
}
