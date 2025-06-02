// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Processing result types and photoacoustic analysis
//!
//! This module defines the final processing results and photoacoustic analysis structures.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Final processing result containing photoacoustic analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult {
    /// Unique identifier for this result
    pub id: String,
    /// Original frame information
    pub frame_info: FrameInfo,
    /// Photoacoustic analysis result
    pub analysis: PhotoacousticAnalysis,
    /// Processing metadata
    pub metadata: ProcessingMetadata,
    /// Timestamp when processing completed
    pub completed_at: u64,
}

/// Information about the original audio frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameInfo {
    pub frame_number: u64,
    pub timestamp: u64,
    pub sample_rate: u32,
    pub channel_a_samples: usize,
    pub channel_b_samples: usize,
}

/// Processing metadata tracking the processing chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingMetadata {
    /// List of processing steps applied
    pub processing_chain: Vec<ProcessingStep>,
    /// Total processing time in microseconds
    pub total_processing_time_us: u64,
    /// Graph configuration ID (for reproducibility)
    pub graph_config_id: String,
}

/// Individual processing step information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingStep {
    pub node_id: String,
    pub node_type: String,
    pub processing_time_us: u64,
    pub input_type: String,
    pub output_type: String,
}

/// Photoacoustic analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoacousticAnalysis {
    /// Processed signal data
    pub signal: Vec<f32>,
    /// Signal characteristics
    pub characteristics: SignalCharacteristics,
    /// Spectral analysis (if available)
    pub spectral_analysis: Option<SpectralAnalysis>,
    /// Detection results
    pub detection: DetectionResult,
}

/// Signal characteristics and quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalCharacteristics {
    /// RMS (Root Mean Square) amplitude
    pub rms_amplitude: f32,
    /// Peak amplitude
    pub peak_amplitude: f32,
    /// Signal-to-noise ratio (if calculable)
    pub snr_db: Option<f32>,
    /// Signal duration in milliseconds
    pub duration_ms: f64,
    /// Number of samples
    pub sample_count: usize,
    /// Dynamic range
    pub dynamic_range_db: f32,
}

/// Spectral analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralAnalysis {
    /// Dominant frequency in Hz
    pub dominant_frequency_hz: f32,
    /// Frequency spectrum (frequency, magnitude pairs)
    pub spectrum: Vec<(f32, f32)>,
    /// Spectral centroid
    pub spectral_centroid_hz: f32,
    /// Bandwidth
    pub bandwidth_hz: f32,
}

/// Detection and classification results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    /// Whether a photoacoustic signal was detected
    pub signal_detected: bool,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// Signal quality assessment
    pub quality_score: f32,
    /// Classification (if applicable)
    pub classification: Option<String>,
    /// Additional detection metadata
    pub metadata: DetectionMetadata,
}

/// Additional metadata for detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionMetadata {
    /// Detection threshold used
    pub threshold: f32,
    /// Number of peaks detected
    pub peak_count: usize,
    /// Time of strongest signal (ms from start)
    pub peak_time_ms: Option<f64>,
    /// False positive probability
    pub false_positive_probability: Option<f32>,
}

impl ProcessingResult {
    /// Create a new processing result
    pub fn new(
        id: String,
        frame_info: FrameInfo,
        analysis: PhotoacousticAnalysis,
        metadata: ProcessingMetadata,
    ) -> Self {
        let completed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            id,
            frame_info,
            analysis,
            metadata,
            completed_at,
        }
    }

    /// Get the processing latency in microseconds
    pub fn processing_latency_us(&self) -> u64 {
        self.metadata.total_processing_time_us
    }

    /// Check if this result indicates a successful detection
    pub fn is_detection(&self) -> bool {
        self.analysis.detection.signal_detected
    }

    /// Get the overall quality score
    pub fn quality_score(&self) -> f32 {
        self.analysis.detection.quality_score
    }

    /// Get a summary string of the result
    pub fn summary(&self) -> String {
        format!(
            "Frame {}: {} (confidence: {:.2}, quality: {:.2})",
            self.frame_info.frame_number,
            if self.is_detection() {
                "DETECTED"
            } else {
                "NO SIGNAL"
            },
            self.analysis.detection.confidence,
            self.quality_score()
        )
    }
}

impl PhotoacousticAnalysis {
    /// Create a new photoacoustic analysis from processed signal data
    pub fn from_signal(signal: Vec<f32>, sample_rate: u32) -> Self {
        let characteristics = SignalCharacteristics::calculate(&signal, sample_rate);
        let detection = DetectionResult::analyze(&signal, &characteristics);

        Self {
            signal,
            characteristics,
            spectral_analysis: None, // Could be computed later
            detection,
        }
    }

    /// Add spectral analysis to the result
    pub fn with_spectral_analysis(mut self, spectral: SpectralAnalysis) -> Self {
        self.spectral_analysis = Some(spectral);
        self
    }
}

impl SignalCharacteristics {
    /// Calculate signal characteristics from the signal data
    pub fn calculate(signal: &[f32], sample_rate: u32) -> Self {
        if signal.is_empty() {
            return Self::default();
        }

        // Calculate RMS amplitude
        let rms_amplitude =
            (signal.iter().map(|&x| x * x).sum::<f32>() / signal.len() as f32).sqrt();

        // Calculate peak amplitude
        let peak_amplitude = signal.iter().fold(0.0f32, |max, &x| max.max(x.abs()));

        // Calculate duration
        let duration_ms = (signal.len() as f64 * 1000.0) / sample_rate as f64;

        // Calculate dynamic range (difference between max and min in dB)
        let min_amplitude = signal
            .iter()
            .fold(f32::INFINITY, |min, &x| min.min(x.abs()));
        let dynamic_range_db = if min_amplitude > 0.0 && peak_amplitude > 0.0 {
            20.0 * (peak_amplitude / min_amplitude).log10()
        } else {
            0.0
        };

        Self {
            rms_amplitude,
            peak_amplitude,
            snr_db: None, // Would need noise reference to calculate
            duration_ms,
            sample_count: signal.len(),
            dynamic_range_db,
        }
    }
}

impl DetectionResult {
    /// Analyze signal and determine if photoacoustic signal is present
    pub fn analyze(signal: &[f32], characteristics: &SignalCharacteristics) -> Self {
        // Simple detection algorithm based on amplitude thresholds
        let threshold = 0.01; // Configurable threshold
        let signal_detected = characteristics.peak_amplitude > threshold;

        // Calculate confidence based on peak amplitude and SNR
        let confidence = if signal_detected {
            (characteristics.peak_amplitude / threshold).min(1.0)
        } else {
            0.0
        };

        // Quality score based on various factors
        let quality_score = calculate_quality_score(characteristics);

        // Count peaks for metadata
        let peak_count = count_peaks(signal, threshold * 0.5);

        // Find time of strongest signal
        let peak_time_ms = find_peak_time(signal, characteristics.peak_amplitude)
            .map(|idx| (idx as f64 * 1000.0) / signal.len() as f64 * characteristics.duration_ms);

        let metadata = DetectionMetadata {
            threshold,
            peak_count,
            peak_time_ms,
            false_positive_probability: None, // Would require training data
        };

        Self {
            signal_detected,
            confidence,
            quality_score,
            classification: None, // Could be added with ML models
            metadata,
        }
    }
}

impl Default for SignalCharacteristics {
    fn default() -> Self {
        Self {
            rms_amplitude: 0.0,
            peak_amplitude: 0.0,
            snr_db: None,
            duration_ms: 0.0,
            sample_count: 0,
            dynamic_range_db: 0.0,
        }
    }
}

/// Calculate quality score based on signal characteristics
fn calculate_quality_score(characteristics: &SignalCharacteristics) -> f32 {
    let mut score = 0.0;

    // Factor in RMS amplitude (normalized)
    score += (characteristics.rms_amplitude * 10.0).min(1.0) * 0.3;

    // Factor in dynamic range
    score += (characteristics.dynamic_range_db / 60.0).min(1.0) * 0.3;

    // Factor in signal duration (prefer intermediate durations)
    let duration_factor =
        if characteristics.duration_ms > 1.0 && characteristics.duration_ms < 1000.0 {
            1.0
        } else {
            0.5
        };
    score += duration_factor * 0.2;

    // Factor in sample count
    if characteristics.sample_count > 100 {
        score += 0.2;
    }

    score.min(1.0)
}

/// Count the number of peaks in the signal above a threshold
fn count_peaks(signal: &[f32], threshold: f32) -> usize {
    if signal.len() < 3 {
        return 0;
    }

    let mut count = 0;
    for i in 1..signal.len() - 1 {
        if signal[i] > threshold && signal[i] > signal[i - 1] && signal[i] > signal[i + 1] {
            count += 1;
        }
    }
    count
}

/// Find the index of the sample with the given peak amplitude
fn find_peak_time(signal: &[f32], peak_amplitude: f32) -> Option<usize> {
    signal
        .iter()
        .position(|&x| (x.abs() - peak_amplitude).abs() < f32::EPSILON)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_characteristics() {
        let signal = vec![0.1, 0.5, -0.3, 0.8, -0.2];
        let characteristics = SignalCharacteristics::calculate(&signal, 48000);

        assert!(characteristics.peak_amplitude > 0.0);
        assert!(characteristics.rms_amplitude > 0.0);
        assert_eq!(characteristics.sample_count, 5);
    }

    #[test]
    fn test_detection_result() {
        let signal = vec![0.1, 0.5, -0.3, 0.8, -0.2];
        let characteristics = SignalCharacteristics::calculate(&signal, 48000);
        let detection = DetectionResult::analyze(&signal, &characteristics);

        assert!(detection.signal_detected); // Should detect with peak of 0.8
        assert!(detection.confidence > 0.0);
    }

    #[test]
    fn test_photoacoustic_analysis() {
        let signal = vec![0.1, 0.5, -0.3, 0.8, -0.2];
        let analysis = PhotoacousticAnalysis::from_signal(signal.clone(), 48000);

        assert_eq!(analysis.signal, signal);
        assert!(analysis.detection.signal_detected);
    }
}
