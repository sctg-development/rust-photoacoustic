// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Record node implementation for stream recording
//!
//! This module provides the `RecordNode` for recording audio streams to PCM files.
//! The record node acts as a pass-through node that saves audio data while allowing
//! it to continue through the processing pipeline.
//!
//! ## Features
//!
//! - Records audio streams in PCM format (mono or stereo)
//! - Configurable file rotation based on size limits
//! - Automatic file cleanup when enabled
//! - Pass-through design - doesn't modify the audio stream
//! - Supports both single and dual channel data
//!
//! ## Configuration
//!
//! The node supports three main parameters:
//! - `record_file`: Output file path (PathBuf)
//! - `max_size`: Maximum file size in kilobytes before rotation (usize)
//! - `auto_delete`: Whether to automatically delete old files (bool)
//!
//! ## Examples
//!
//! Basic usage in a processing graph:
//!
//! ```no_run
//! use rust_photoacoustic::processing::{RecordNode, ProcessingNode, ProcessingData};
//! use std::path::PathBuf;
//!
//! let mut record_node = RecordNode::new(
//!     "record_1".to_string(),
//!     PathBuf::from("recording.wav"),
//!     1024, // 1MB max size
//!     false // don't auto-delete
//! );
//!
//! let input = ProcessingData::SingleChannel {
//!     samples: vec![0.1, 0.2, 0.3, 0.4],
//!     sample_rate: 44100,
//!     timestamp: 1000,
//!     frame_number: 1,
//! };
//!
//! let result = record_node.process(input.clone())?;
//! // result is identical to input, but audio has been recorded to file
//! # Ok::<(), anyhow::Error>(())
//! ```

use super::{ProcessingNode, ProcessingData};
use anyhow::{anyhow, Result};
use hound::{SampleFormat, WavSpec, WavWriter};
use log::{debug, error, info, warn};
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Record node that records audio streams to PCM files while passing data through
///
/// The `RecordNode` acts as a transparent recording device in the processing pipeline.
/// It saves incoming audio data to WAV files without modifying the stream, allowing
/// the data to continue to downstream nodes.
///
/// # Recording Features
///
/// - **Format**: Records in PCM WAV format (16-bit integer)
/// - **Channels**: Automatically detects mono/stereo from input data
/// - **File Rotation**: Creates new files when size limit is reached
/// - **Pass-through**: Input data is returned unchanged
///
/// # File Management
///
/// When `max_size` is exceeded, the node will:
/// 1. Close the current file
/// 2. Create a new file with timestamp suffix
/// 3. Optionally delete the old file if `auto_delete` is true
///
/// # Thread Safety
///
/// The node maintains internal state for the WAV writer and implements proper
/// cleanup when dropped. However, it's designed for single-threaded use within
/// the processing graph.
///
/// # Examples
///
/// Creating a record node with file rotation:
///
/// ```no_run
/// use rust_photoacoustic::processing::{RecordNode, ProcessingNode, ProcessingData};
/// use std::path::PathBuf;
///
/// let mut record_node = RecordNode::new(
///     "stream_recorder".to_string(),
///     PathBuf::from("/tmp/audio_stream.wav"),
///     2048, // 2MB files
///     true  // auto-delete old files
/// );
///
/// // Record both mono and stereo data
/// let mono_data = ProcessingData::SingleChannel {
///     samples: vec![0.1; 1024],
///     sample_rate: 48000,
///     timestamp: 1000,
///     frame_number: 1,
/// };
///
/// let stereo_data = ProcessingData::DualChannel {
///     channel_a: vec![0.1; 1024],
///     channel_b: vec![0.2; 1024],
///     sample_rate: 48000,
///     timestamp: 2000,
///     frame_number: 2,
/// };
///
/// // Both calls return the input unchanged while recording
/// let mono_result = record_node.process(mono_data)?;
/// let stereo_result = record_node.process(stereo_data)?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub struct RecordNode {
    /// Node identifier
    id: String,
    /// Output file path
    record_file: PathBuf,
    /// Maximum file size in kilobytes
    max_size_kb: usize,
    /// Whether to automatically delete old files
    auto_delete: bool,
    /// Current WAV writer (if recording)
    wav_writer: Option<WavWriter<BufWriter<File>>>,
    /// Current recording specifications
    current_spec: Option<WavSpec>,
    /// Current file size in bytes
    current_size_bytes: usize,
    /// Current file index for rotation
    file_index: u32,
}

impl RecordNode {
    /// Create a new record node
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `record_file` - Path where recordings will be saved
    /// * `max_size_kb` - Maximum file size in kilobytes before rotation
    /// * `auto_delete` - Whether to automatically delete old files
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_photoacoustic::processing::RecordNode;
    /// use std::path::PathBuf;
    ///
    /// let record_node = RecordNode::new(
    ///     "my_recorder".to_string(),
    ///     PathBuf::from("output.wav"),
    ///     1024, // 1MB max
    ///     false // keep old files
    /// );
    /// ```
    pub fn new(id: String, record_file: PathBuf, max_size_kb: usize, auto_delete: bool) -> Self {
        Self {
            id,
            record_file,
            max_size_kb,
            auto_delete,
            wav_writer: None,
            current_spec: None,
            current_size_bytes: 0,
            file_index: 0,
        }
    }

    /// Initialize or rotate the WAV writer
    fn ensure_wav_writer(&mut self, spec: WavSpec) -> Result<()> {
        // Check if we need to rotate the file
        let max_size_bytes = self.max_size_kb * 1024;
        let needs_rotation = self.current_size_bytes >= max_size_bytes;
        let spec_changed = self.current_spec.as_ref() != Some(&spec);

        if self.wav_writer.is_none() || needs_rotation || spec_changed {
            self.rotate_file(spec)?;
        }

        Ok(())
    }

    /// Rotate to a new recording file
    fn rotate_file(&mut self, spec: WavSpec) -> Result<()> {
        // Close current writer
        if let Some(writer) = self.wav_writer.take() {
            if let Err(e) = writer.finalize() {
                error!("Failed to finalize WAV file: {}", e);
            } else {
                info!("Finalized recording file");
            }
        }

        // Handle file rotation if we have an existing file
        if self.file_index > 0 {
            let old_file = self.get_current_file_path();
            
            if self.auto_delete && old_file.exists() {
                if let Err(e) = fs::remove_file(&old_file) {
                    warn!("Failed to delete old recording file {:?}: {}", old_file, e);
                } else {
                    debug!("Deleted old recording file: {:?}", old_file);
                }
            }
        }

        // Increment file index for new file
        self.file_index += 1;
        let new_file_path = self.get_current_file_path();

        // Create directory if it doesn't exist
        if let Some(parent_dir) = new_file_path.parent() {
            if !parent_dir.exists() {
                fs::create_dir_all(parent_dir)
                    .map_err(|e| anyhow!("Failed to create recording directory: {}", e))?;
            }
        }

        // Create new WAV writer
        let writer = WavWriter::create(&new_file_path, spec)
            .map_err(|e| anyhow!("Failed to create WAV writer for {:?}: {}", new_file_path, e))?;

        info!("Started new recording file: {:?} ({}Hz, {} channels)", 
              new_file_path, spec.sample_rate, spec.channels);

        self.wav_writer = Some(writer);
        self.current_spec = Some(spec);
        self.current_size_bytes = 0;

        Ok(())
    }

    /// Get the current file path with index/timestamp
    fn get_current_file_path(&self) -> PathBuf {
        if self.file_index == 1 {
            // First file uses original name
            self.record_file.clone()
        } else {
            // Subsequent files get timestamp suffix
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let stem = self.record_file.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("recording");
            let extension = self.record_file.extension()
                .and_then(|s| s.to_str())
                .unwrap_or("wav");

            self.record_file.with_file_name(
                format!("{}_{}.{}", stem, timestamp, extension)
            )
        }
    }

    /// Record audio data to file
    fn record_audio_data(&mut self, data: &ProcessingData) -> Result<()> {
        let (samples, channels, sample_rate) = match data {
            ProcessingData::SingleChannel { samples, sample_rate, .. } => {
                (samples.clone(), 1, *sample_rate)
            }
            ProcessingData::DualChannel { channel_a, channel_b, sample_rate, .. } => {
                // Interleave channels for stereo recording
                let mut interleaved = Vec::with_capacity(channel_a.len() + channel_b.len());
                for (a, b) in channel_a.iter().zip(channel_b.iter()) {
                    interleaved.push(*a);
                    interleaved.push(*b);
                }
                (interleaved, 2, *sample_rate)
            }
            ProcessingData::AudioFrame(frame) => {
                // Interleave channels from AudioFrame
                let mut interleaved = Vec::with_capacity(frame.channel_a.len() + frame.channel_b.len());
                for (a, b) in frame.channel_a.iter().zip(frame.channel_b.iter()) {
                    interleaved.push(*a);
                    interleaved.push(*b);
                }
                (interleaved, 2, frame.sample_rate)
            }
            ProcessingData::PhotoacousticResult { .. } => {
                debug!("Skipping recording of PhotoacousticResult data");
                return Ok(());
            }
        };

        // Create WAV specification
        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        // Ensure we have a writer
        self.ensure_wav_writer(spec)?;

        // Write samples
        if let Some(writer) = &mut self.wav_writer {
            for &sample in &samples {
                // Convert f32 to i16 with proper scaling and clipping
                let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                writer.write_sample(sample_i16)
                    .map_err(|e| anyhow!("Failed to write audio sample: {}", e))?;
            }

            // Update size tracking (2 bytes per i16 sample)
            self.current_size_bytes += samples.len() * 2;
        }

        Ok(())
    }
}

impl ProcessingNode for RecordNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Record the audio data
        if let Err(e) = self.record_audio_data(&input) {
            error!("Recording failed for node '{}': {}", self.id, e);
            // Continue processing even if recording fails
        }

        // Pass through the input unchanged
        Ok(input)
    }

    fn node_id(&self) -> &str {
        &self.id
    }

    fn node_type(&self) -> &str {
        "record"
    }

    fn accepts_input(&self, input: &ProcessingData) -> bool {
        // Accept all audio data types except PhotoacousticResult
        matches!(input, 
            ProcessingData::SingleChannel { .. } |
            ProcessingData::DualChannel { .. } |
            ProcessingData::AudioFrame(_)
        )
    }

    fn output_type(&self, input: &ProcessingData) -> Option<String> {
        // Pass through the input type unchanged
        match input {
            ProcessingData::SingleChannel { .. } => Some("SingleChannel".to_string()),
            ProcessingData::DualChannel { .. } => Some("DualChannel".to_string()),
            ProcessingData::AudioFrame(_) => Some("AudioFrame".to_string()),
            ProcessingData::PhotoacousticResult { .. } => Some("PhotoacousticResult".to_string()),
        }
    }

    fn reset(&mut self) {
        // Close current recording and reset state
        if let Some(writer) = self.wav_writer.take() {
            if let Err(e) = writer.finalize() {
                error!("Failed to finalize WAV file during reset: {}", e);
            }
        }
        
        self.current_spec = None;
        self.current_size_bytes = 0;
        // Don't reset file_index to avoid overwriting files
        
        debug!("Record node '{}' reset", self.id);
    }

    fn clone_node(&self) -> Box<dyn ProcessingNode> {
        Box::new(RecordNode::new(
            self.id.clone(),
            self.record_file.clone(),
            self.max_size_kb,
            self.auto_delete,
        ))
    }
}

impl Drop for RecordNode {
    fn drop(&mut self) {
        // Ensure the WAV file is properly finalized when the node is dropped
        if let Some(writer) = self.wav_writer.take() {
            if let Err(e) = writer.finalize() {
                error!("Failed to finalize WAV file in Drop: {}", e);
            } else {
                debug!("WAV file finalized in Drop for node '{}'", self.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_record_node_creation() {
        let record_node = RecordNode::new(
            "test_record".to_string(),
            PathBuf::from("test.wav"),
            1024,
            false,
        );

        assert_eq!(record_node.node_id(), "test_record");
        assert_eq!(record_node.node_type(), "record");
        assert_eq!(record_node.max_size_kb, 1024);
        assert!(!record_node.auto_delete);
    }

    #[test]
    fn test_accepts_input() {
        let record_node = RecordNode::new(
            "test".to_string(),
            PathBuf::from("test.wav"),
            1024,
            false,
        );

        let single_channel = ProcessingData::SingleChannel {
            samples: vec![0.1, 0.2],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let dual_channel = ProcessingData::DualChannel {
            channel_a: vec![0.1, 0.2],
            channel_b: vec![0.3, 0.4],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        assert!(record_node.accepts_input(&single_channel));
        assert!(record_node.accepts_input(&dual_channel));
    }

    #[test]
    fn test_record_single_channel() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test_mono.wav");
        
        let mut record_node = RecordNode::new(
            "test_mono".to_string(),
            file_path.clone(),
            1024,
            false,
        );

        let input = ProcessingData::SingleChannel {
            samples: vec![0.1, 0.2, 0.3, 0.4],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        let output = record_node.process(input.clone())?;

        // Verify pass-through behavior
        match (&input, &output) {
            (ProcessingData::SingleChannel { samples: in_samples, .. },
             ProcessingData::SingleChannel { samples: out_samples, .. }) => {
                assert_eq!(in_samples, out_samples);
            }
            _ => panic!("Expected SingleChannel data"),
        }

        // Finalize the recording
        drop(record_node);

        // Verify file was created
        assert!(file_path.exists());

        Ok(())
    }

    #[test]
    fn test_record_dual_channel() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test_stereo.wav");
        
        let mut record_node = RecordNode::new(
            "test_stereo".to_string(),
            file_path.clone(),
            1024,
            false,
        );

        let input = ProcessingData::DualChannel {
            channel_a: vec![0.1, 0.2],
            channel_b: vec![0.3, 0.4],
            sample_rate: 48000,
            timestamp: 1000,
            frame_number: 1,
        };

        let output = record_node.process(input.clone())?;

        // Verify pass-through behavior
        match (&input, &output) {
            (ProcessingData::DualChannel { channel_a: in_a, channel_b: in_b, .. },
             ProcessingData::DualChannel { channel_a: out_a, channel_b: out_b, .. }) => {
                assert_eq!(in_a, out_a);
                assert_eq!(in_b, out_b);
            }
            _ => panic!("Expected DualChannel data"),
        }

        // Finalize the recording
        drop(record_node);

        // Verify file was created
        assert!(file_path.exists());

        Ok(())
    }

    #[test]
    fn test_file_rotation_by_size() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test_rotation.wav");
        
        // Very small max size to trigger rotation
        let mut record_node = RecordNode::new(
            "test_rotation".to_string(),
            file_path.clone(),
            1, // 1KB max size
            false,
        );

        // Generate enough data to exceed 1KB
        let large_samples = vec![0.1; 1000]; // 1000 samples * 2 bytes = 2KB
        let input = ProcessingData::SingleChannel {
            samples: large_samples,
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };

        // Process the large input
        record_node.process(input)?;

        // Process another input to trigger rotation
        let input2 = ProcessingData::SingleChannel {
            samples: vec![0.2; 100],
            sample_rate: 44100,
            timestamp: 2000,
            frame_number: 2,
        };
        record_node.process(input2)?;

        // Finalize recording
        drop(record_node);

        // Should have created the original file
        assert!(file_path.exists());

        Ok(())
    }

    #[test]
    fn test_auto_delete() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test_delete.wav");
        
        let mut record_node = RecordNode::new(
            "test_delete".to_string(),
            file_path.clone(),
            1, // Small size to trigger rotation
            true, // Enable auto-delete
        );

        // Process enough data to trigger rotation
        let input1 = ProcessingData::SingleChannel {
            samples: vec![0.1; 500],
            sample_rate: 44100,
            timestamp: 1000,
            frame_number: 1,
        };
        record_node.process(input1)?;

        let input2 = ProcessingData::SingleChannel {
            samples: vec![0.2; 500],
            sample_rate: 44100,
            timestamp: 2000,
            frame_number: 2,
        };
        record_node.process(input2)?;

        // The test passes if no errors occur during auto-deletion
        drop(record_node);

        Ok(())
    }
}
