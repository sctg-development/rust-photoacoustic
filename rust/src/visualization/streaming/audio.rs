// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Audio streaming API endpoints
//!
//! This module provides HTTP endpoints for streaming audio data and spectral analysis
//! to web clients in real-time using Server-Sent Events (SSE).

use crate::acquisition::{AudioFrame, AudioStreamConsumer, SharedAudioStream, StreamStats};
use crate::visualization::api_auth::AuthenticatedUser;
use auth_macros::protect_get;
use rocket::serde::json::Json;
use rocket::{
    response::stream::{Event, EventStream},
    State,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Audio streaming state managed by Rocket
pub struct AudioStreamState {
    pub stream: Arc<SharedAudioStream>,
}

/// Response structure for audio frame data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFrameResponse {
    /// Channel A audio data
    pub channel_a: Vec<f32>,
    /// Channel B audio data  
    pub channel_b: Vec<f32>,
    /// Sample rate of the audio data
    pub sample_rate: u32,
    /// Timestamp when the frame was captured
    pub timestamp: u64,
    /// Sequential frame number
    pub frame_number: u64,
    /// Duration of this frame in milliseconds
    pub duration_ms: f64,
}

impl From<AudioFrame> for AudioFrameResponse {
    fn from(frame: AudioFrame) -> Self {
        let duration_ms = frame.duration_ms();
        Self {
            channel_a: frame.channel_a,
            channel_b: frame.channel_b,
            sample_rate: frame.sample_rate,
            timestamp: frame.timestamp,
            frame_number: frame.frame_number,
            duration_ms,
        }
    }
}

/// Spectral analysis data for real-time visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralDataResponse {
    /// Frequency bins (Hz)
    pub frequencies: Vec<f32>,
    /// Magnitude spectrum for channel A
    pub magnitude_a: Vec<f32>,
    /// Magnitude spectrum for channel B
    pub magnitude_b: Vec<f32>,
    /// Phase spectrum for channel A (optional)
    pub phase_a: Option<Vec<f32>>,
    /// Phase spectrum for channel B (optional)
    pub phase_b: Option<Vec<f32>>,
    /// Frame metadata
    pub frame_number: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Sample rate
    pub sample_rate: u32,
}

/// Get current stream statistics
///
/// Returns information about the audio stream including frame rates,
/// subscriber count, and other metrics.
#[protect_get("/stream/stats", "read_api")]
pub async fn get_stream_stats(
    _user: AuthenticatedUser,
    stream_state: &State<AudioStreamState>,
) -> Json<StreamStats> {
    let stats = stream_state.stream.get_stats().await;
    Json(stats)
}

/// Get the latest audio frame
///
/// Returns the most recent audio frame without subscribing to the stream.
/// Useful for getting current state or testing connectivity.
#[protect_get("/stream/latest", "read_api")]
pub async fn get_latest_frame(
    stream_state: &State<AudioStreamState>,
) -> Option<Json<AudioFrameResponse>> {
    let frame = stream_state.stream.get_latest_frame().await;
    match frame {
        Some(frame) => Some(Json(frame.into())),
        None => None,
    }
}

/// Stream audio frames via Server-Sent Events
///
/// Provides a continuous stream of audio frames to web clients using
/// Server-Sent Events. Each event contains a complete audio frame with
/// both channels of data.
///
/// # Authentication
/// Requires a valid JWT token with appropriate read permissions.
///
/// # Response Format
/// The stream sends JSON-encoded audio frames as SSE events:
/// ```json
/// data: {"channel_a": [...], "channel_b": [...], ...}
///
/// ```
#[protect_get("/stream/audio", "read:api")]
pub fn stream_audio(stream_state: &State<AudioStreamState>) -> EventStream![Event] {
    let stream = stream_state.stream.clone();

    EventStream! {
        let mut consumer = AudioStreamConsumer::new(&stream);

        loop {            // Set a timeout to prevent hanging if no frames are available
             match timeout(Duration::from_secs(5), consumer.next_frame()).await {
                Ok(Some(frame)) => {
                    let response = AudioFrameResponse::from(frame);
                    // Unwrap le Result de Event::json
                    yield Event::json(&response);
                },
                Ok(None) => {
                    // Stream closed
                    log::info!("Audio stream closed");
                    break;
                },
                Err(_) => {
                    // Timeout - send heartbeat
                    yield Event::data(r#"{"type":"heartbeat"}"#);
                }
            }
        }
    }
}
/// Stream spectral analysis data via Server-Sent Events
///
/// Provides real-time spectral analysis data computed from the audio frames.
/// The analysis includes FFT magnitude and optionally phase information for
/// both audio channels.
///
/// # Authentication
/// Requires a valid JWT token with appropriate read permissions.
///
/// # Response Format
/// The stream sends JSON-encoded spectral data as SSE events:
/// ```json
/// data: {"frequencies": [...], "magnitude_a": [...], "magnitude_b": [...], ...}
///
/// ```
#[protect_get("/stream/spectral", "read:api")]
pub fn stream_spectral_analysis(stream_state: &State<AudioStreamState>) -> EventStream![Event] {
    let stream = stream_state.stream.clone();

    EventStream! {
        let mut consumer = AudioStreamConsumer::new(&stream);

        loop {            match timeout(Duration::from_secs(5), consumer.next_frame()).await {
                Ok(Some(frame)) => {
                    // Perform FFT analysis on the frame
                    let spectral_data = compute_spectral_analysis(&frame);
                    yield Event::json(&spectral_data);
                },Ok(None) => {
                    log::info!("Audio stream closed for spectral analysis stream");
                    break;
                },
                Err(_) => {
                    // Timeout - send heartbeat
                    yield Event::data(r#"{"type":"heartbeat"}"#);
                }
            }
        }
    }
}

/// Compute spectral analysis for an audio frame
///
/// Performs FFT analysis on both channels of the audio frame and returns
/// frequency domain representation including magnitude spectra.
fn compute_spectral_analysis(frame: &AudioFrame) -> SpectralDataResponse {
    use rustfft::{num_complex::Complex, FftPlanner};

    let n = frame.channel_a.len();
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);

    // Convert to complex numbers for FFT
    let mut buffer_a: Vec<Complex<f32>> = frame
        .channel_a
        .iter()
        .map(|&x| Complex::new(x, 0.0))
        .collect();
    let mut buffer_b: Vec<Complex<f32>> = frame
        .channel_b
        .iter()
        .map(|&x| Complex::new(x, 0.0))
        .collect();

    // Perform FFT
    fft.process(&mut buffer_a);
    fft.process(&mut buffer_b);

    // Compute magnitude spectra
    let magnitude_a: Vec<f32> = buffer_a
        .iter()
        .take(n / 2) // Only positive frequencies
        .map(|c| c.norm())
        .collect();
    let magnitude_b: Vec<f32> = buffer_b.iter().take(n / 2).map(|c| c.norm()).collect();

    // Generate frequency bins
    let sample_rate = frame.sample_rate as f32;
    let frequencies: Vec<f32> = (0..n / 2)
        .map(|i| i as f32 * sample_rate / n as f32)
        .collect();

    SpectralDataResponse {
        frequencies,
        magnitude_a,
        magnitude_b,
        phase_a: None, // Could compute phase if needed
        phase_b: None,
        frame_number: frame.frame_number,
        timestamp: frame.timestamp,
        sample_rate: frame.sample_rate,
    }
}

/// Get all audio streaming routes
///
/// Returns a vector of all route handlers for audio streaming functionality.
pub fn get_audio_streaming_routes() -> Vec<rocket::Route> {
    rocket::routes![
        get_stream_stats,
        get_latest_frame,
        stream_audio,
        stream_spectral_analysis,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::AudioFrame;

    #[test]
    fn test_audio_frame_response_conversion() {
        let frame = AudioFrame::new(vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6], 48000, 42);

        let response = AudioFrameResponse::from(frame.clone());
        assert_eq!(response.channel_a, vec![0.1, 0.2, 0.3]);
        assert_eq!(response.channel_b, vec![0.4, 0.5, 0.6]);
        assert_eq!(response.sample_rate, 48000);
        assert_eq!(response.frame_number, 42);
        assert!(response.duration_ms > 0.0);
    }

    #[test]
    fn test_spectral_analysis() {
        let frame = AudioFrame::new(vec![0.1, 0.2, 0.3, 0.4], vec![0.4, 0.5, 0.6, 0.7], 48000, 1);

        let spectral = compute_spectral_analysis(&frame);
        assert_eq!(spectral.magnitude_a.len(), 2); // n/2 frequencies
        assert_eq!(spectral.magnitude_b.len(), 2);
        assert_eq!(spectral.frequencies.len(), 2);
        assert_eq!(spectral.sample_rate, 48000);
    }
}
