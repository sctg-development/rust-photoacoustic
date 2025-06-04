// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Simulated Photoacoustic Real-time Audio Source
//!
//! This module provides a comprehensive simulated photoacoustic audio source that uses
//! the `generate_universal_photoacoustic_stereo` function to create realistic synthetic
//! photoacoustic signals for testing and development purposes.

use super::{AudioFrame, RealTimeAudioSource, SharedAudioStream};
use crate::config::{PhotoacousticConfig, SimulatedSourceConfig};
use crate::utility::noise_generator::NoiseGenerator;
use anyhow::Result;
use async_trait::async_trait;
use log::{debug, error};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

/// Advanced simulated photoacoustic real-time audio source
///
/// This source implements comprehensive photoacoustic physics simulation using the
/// `generate_universal_photoacoustic_stereo` function. Unlike the simple `MockSource`,
/// this provides realistic modeling of:
///
/// **For Physics PhD Specialists:**
/// - Helmholtz resonance cell dynamics with configurable Q-factor
/// - Dual-microphone differential configuration with phase opposition
/// - Gas flow noise with 1/f characteristics from turbulent flow
/// - Thermal drift effects on frequency and phase stability
/// - Molecular concentration variations (random walk simulation)
/// - Laser modulation (both amplitude and pulsed modes)
/// - Environmental perturbations and system noise
///
/// **For Rust Developers:**
/// This struct implements the `RealTimeAudioSource` trait and provides streaming
/// audio data via the `SharedAudioStream` interface. It does NOT implement the
/// deprecated `AudioSource` trait.
pub struct SimulatedPhotoacousticRealtimeAudioSource {
    /// Noise generator for deterministic pseudo-random sequences
    generator: NoiseGenerator,
    /// Audio sample rate in Hz
    sample_rate: u32,
    /// Number of samples per frame per channel
    frame_size: usize,
    /// Original photoacoustic configuration
    config: PhotoacousticConfig,
    /// Simulation parameters
    simulation_config: SimulatedSourceConfig,
    /// Timing control for real-time simulation
    last_frame_time: Option<Instant>,
    /// Duration of each frame for timing control
    frame_duration: Duration,
    /// Whether to simulate real-time timing
    real_time_mode: bool,
    /// Atomic flag for streaming state
    streaming: Arc<AtomicBool>,
    /// Handle to the streaming task
    stream_handle: Option<tokio::task::JoinHandle<()>>,
}

impl SimulatedPhotoacousticRealtimeAudioSource {
    /// Create a new simulated photoacoustic real-time audio source
    ///
    /// # Arguments
    ///
    /// * `config` - PhotoacousticConfig containing frequency, sample_rate, and precision settings
    /// * `simulation_config` - SimulatedSourceConfig with comprehensive simulation parameters
    ///
    /// # Returns
    ///
    /// A new SimulatedPhotoacousticRealtimeAudioSource instance
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::acquisition::SimulatedPhotoacousticRealtimeAudioSource;
    /// use rust_photoacoustic::config::{PhotoacousticConfig, SimulatedSourceConfig};
    ///
    /// let pa_config = PhotoacousticConfig::default();
    /// let sim_config = SimulatedSourceConfig::default();
    /// let source = SimulatedPhotoacousticRealtimeAudioSource::new(pa_config, sim_config)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new(
        config: PhotoacousticConfig,
        simulation_config: SimulatedSourceConfig,
    ) -> Result<Self> {
        let generator = NoiseGenerator::new_from_system_time();
        let sample_rate = config.sample_rate as u32;
        let frame_size = config.frame_size as usize;

        // Calculate frame duration for real-time simulation
        let frame_duration = Duration::from_secs_f64(frame_size as f64 / sample_rate as f64);

        debug!("Creating SimulatedPhotoacousticRealtimeAudioSource with config:");
        debug!("  Sample rate: {} Hz", sample_rate);
        debug!("  Frame size: {} samples per channel", frame_size);
        debug!(
            "  Frame duration: {:.1}ms",
            frame_duration.as_secs_f64() * 1000.0
        );
        debug!("  Expected FPS: {:.1}", 1.0 / frame_duration.as_secs_f64());
        debug!("  Resonance frequency: {} Hz", simulation_config.resonance_frequency);
        debug!("  Laser modulation depth: {:.1}%", simulation_config.laser_modulation_depth * 100.0);
        debug!("  Signal amplitude: {:.1}%", simulation_config.signal_amplitude * 100.0);
        debug!("  Phase opposition: {}°", simulation_config.phase_opposition_degrees);
        debug!("  SNR factor: {} dB", simulation_config.snr_factor);
        debug!("  Modulation mode: {}", simulation_config.modulation_mode);

        Ok(Self {
            generator,
            sample_rate,
            frame_size,
            config,
            simulation_config,
            last_frame_time: None,
            frame_duration,
            real_time_mode: true, // Enable real-time simulation by default
            streaming: Arc::new(AtomicBool::new(false)),
            stream_handle: None,
        })
    }

    /// Enable or disable real-time simulation timing
    ///
    /// When enabled, the source will respect real-time timing constraints.
    /// When disabled, frames are generated as quickly as possible.
    pub fn set_real_time_mode(&mut self, enabled: bool) {
        self.real_time_mode = enabled;
    }

    /// Update the simulation configuration
    ///
    /// This allows runtime modification of simulation parameters without
    /// recreating the entire source.
    pub fn update_simulation_config(&mut self, new_config: SimulatedSourceConfig) {
        self.simulation_config = new_config;
        debug!("Updated simulation configuration");
        debug!("  Resonance frequency: {} Hz", self.simulation_config.resonance_frequency);
        debug!("  Laser modulation depth: {:.1}%", self.simulation_config.laser_modulation_depth * 100.0);
        debug!("  Signal amplitude: {:.1}%", self.simulation_config.signal_amplitude * 100.0);
        debug!("  SNR factor: {} dB", self.simulation_config.snr_factor);
    }

    /// Generate a single frame of simulated photoacoustic data
    ///
    /// Uses the `generate_universal_photoacoustic_stereo` function to create
    /// realistic photoacoustic signals with comprehensive physics modeling.
    fn generate_frame(&mut self) -> Vec<i16> {
        self.generator.generate_universal_photoacoustic_stereo(
            self.frame_size as u32,
            self.sample_rate,
            self.simulation_config.background_noise_amplitude,
            self.simulation_config.resonance_frequency,
            self.simulation_config.laser_modulation_depth,
            self.simulation_config.signal_amplitude,
            self.simulation_config.phase_opposition_degrees,
            self.simulation_config.temperature_drift_factor,
            self.simulation_config.gas_flow_noise_factor,
            self.simulation_config.snr_factor,
            &self.simulation_config.modulation_mode,
            self.simulation_config.pulse_width_seconds,
            self.simulation_config.pulse_frequency_hz,
        )
    }
}

#[async_trait]
impl RealTimeAudioSource for SimulatedPhotoacousticRealtimeAudioSource {
    async fn start_streaming(&mut self, stream: Arc<SharedAudioStream>) -> Result<()> {
        if self.streaming.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.streaming.store(true, Ordering::Relaxed);
        debug!("Starting simulated photoacoustic streaming");

        let sample_rate = self.sample_rate;
        let frame_size = self.frame_size;
        let frame_duration = self.frame_duration;
        let real_time_mode = self.real_time_mode;
        let streaming = Arc::clone(&self.streaming);
        
        // Clone simulation config for the async task
        let simulation_config = self.simulation_config.clone();

        let handle = tokio::spawn(async move {
            let mut generator = NoiseGenerator::new_from_system_time();
            let mut frame_number = 0u64;
            let mut last_time = Instant::now();

            while streaming.load(Ordering::Relaxed) {
                // Real-time timing control
                if real_time_mode {
                    let elapsed = last_time.elapsed();
                    if elapsed < frame_duration {
                        tokio::time::sleep(frame_duration - elapsed).await;
                    }
                    last_time = Instant::now();
                }

                // Generate comprehensive photoacoustic simulation data
                let samples = generator.generate_universal_photoacoustic_stereo(
                    frame_size as u32,
                    sample_rate,
                    simulation_config.background_noise_amplitude,
                    simulation_config.resonance_frequency,
                    simulation_config.laser_modulation_depth,
                    simulation_config.signal_amplitude,
                    simulation_config.phase_opposition_degrees,
                    simulation_config.temperature_drift_factor,
                    simulation_config.gas_flow_noise_factor,
                    simulation_config.snr_factor,
                    &simulation_config.modulation_mode,
                    simulation_config.pulse_width_seconds,
                    simulation_config.pulse_frequency_hz,
                );

                // Convert interleaved stereo i16 samples to separate f32 channels
                let mut channel_a = Vec::with_capacity(frame_size);
                let mut channel_b = Vec::with_capacity(frame_size);

                // Conversion function from i16 to f32 in range [-1.0, 1.0]
                let i16_to_f32 = |sample: i16| -> f32 {
                    if sample >= 0 {
                        sample as f32 / i16::MAX as f32
                    } else {
                        sample as f32 / -(i16::MIN as f32)
                    }
                };

                // Deinterleave stereo samples into separate channels
                for chunk in samples.chunks_exact(2) {
                    let left = i16_to_f32(chunk[0]);
                    let right = i16_to_f32(chunk[1]);
                    channel_a.push(left);
                    channel_b.push(right);
                }

                frame_number += 1;
                let audio_frame = AudioFrame::new(channel_a, channel_b, sample_rate, frame_number);

                if let Err(e) = stream.publish(audio_frame).await {
                    error!("Failed to publish simulated photoacoustic frame: {}", e);
                    break;
                }
            }

            debug!("Simulated photoacoustic streaming stopped");
        });

        self.stream_handle = Some(handle);
        Ok(())
    }

    async fn stop_streaming(&mut self) -> Result<()> {
        self.streaming.store(false, Ordering::Relaxed);
        debug!("Stopping simulated photoacoustic streaming");

        if let Some(handle) = self.stream_handle.take() {
            handle.abort();
        }

        Ok(())
    }

    fn is_streaming(&self) -> bool {
        self.streaming.load(Ordering::Relaxed)
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}
