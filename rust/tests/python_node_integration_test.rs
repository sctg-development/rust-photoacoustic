//! Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
//! This file is part of the rust-photoacoustic project and is licensed under the
//! SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
//!
//! Integration tests for PythonNode with SciPy bandpass filtering
//!
//! These tests demonstrate the PythonNode's ability to execute Python scripts
//! that perform advanced signal processing using scientific libraries like SciPy.

use anyhow::Result;
use rust_photoacoustic::processing::nodes::{
    ProcessingData, ProcessingNode, PythonNode, PythonNodeConfig,
};
use std::fs::write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a temporary Python script file with the given content
fn create_test_script(content: &str) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let script_path = temp_dir.path().join("test_script.py");
    write(&script_path, content).expect("Failed to write test script");
    (temp_dir, script_path)
}

/// Python script that implements a bandpass filter using SciPy
const SCIPY_BANDPASS_FILTER_SCRIPT: &str = r#"
import numpy as np
from scipy import signal
import json

def initialize():
    """Initialize the bandpass filter node"""
    print("SciPy bandpass filter initialized")
    return {"status": "initialized", "filter_type": "scipy_bandpass"}

def process_data(data):
    """
    Apply bandpass filter using SciPy to audio data
    
    Args:
        data: Dictionary containing the processing data
        
    Returns:
        Dictionary with filtered data in the same format
    """
    data_type = data.get("type")
    
    if data_type == "SingleChannel":
        samples = np.array(data["samples"], dtype=np.float32)
        sample_rate = data["sample_rate"]
        
        # Design bandpass filter: 300Hz to 3000Hz
        nyquist = sample_rate / 2.0
        low_freq = 300.0 / nyquist
        high_freq = 3000.0 / nyquist
        
        # Ensure frequencies are valid (0 < freq < 1)
        low_freq = max(0.01, min(0.99, low_freq))
        high_freq = max(low_freq + 0.01, min(0.99, high_freq))
        
        # Design Butterworth bandpass filter
        sos = signal.butter(5, [low_freq, high_freq], btype='band', output='sos')
        
        # Apply zero-phase filtering to avoid phase distortion
        filtered_samples = signal.sosfiltfilt(sos, samples)
        
        return {
            "type": "SingleChannel",
            "samples": filtered_samples.tolist(),
            "sample_rate": sample_rate,
            "timestamp": data["timestamp"],
            "frame_number": data["frame_number"]
        }
    
    elif data_type == "DualChannel":
        channel_a = np.array(data["channel_a"], dtype=np.float32)
        channel_b = np.array(data["channel_b"], dtype=np.float32)
        sample_rate = data["sample_rate"]
        
        # Design bandpass filter: 300Hz to 3000Hz
        nyquist = sample_rate / 2.0
        low_freq = 300.0 / nyquist
        high_freq = 3000.0 / nyquist
        
        # Ensure frequencies are valid
        low_freq = max(0.01, min(0.99, low_freq))
        high_freq = max(low_freq + 0.01, min(0.99, high_freq))
        
        # Design Butterworth bandpass filter
        sos = signal.butter(5, [low_freq, high_freq], btype='band', output='sos')
        
        # Apply filter to both channels
        filtered_a = signal.sosfiltfilt(sos, channel_a)
        filtered_b = signal.sosfiltfilt(sos, channel_b)
        
        return {
            "type": "DualChannel",
            "channel_a": filtered_a.tolist(),
            "channel_b": filtered_b.tolist(),
            "sample_rate": sample_rate,
            "timestamp": data["timestamp"],
            "frame_number": data["frame_number"]
        }
    
    # For AudioFrame, convert to DualChannel and process
    elif data_type == "AudioFrame":
        channel_a = np.array(data["channel_a"], dtype=np.float32)
        channel_b = np.array(data["channel_b"], dtype=np.float32)
        sample_rate = data["sample_rate"]
        
        # Design bandpass filter
        nyquist = sample_rate / 2.0
        low_freq = 300.0 / nyquist
        high_freq = 3000.0 / nyquist
        
        # Ensure frequencies are valid
        low_freq = max(0.01, min(0.99, low_freq))
        high_freq = max(low_freq + 0.01, min(0.99, high_freq))
        
        # Design Butterworth bandpass filter
        sos = signal.butter(5, [low_freq, high_freq], btype='band', output='sos')
        
        # Apply filter
        filtered_a = signal.sosfiltfilt(sos, channel_a)
        filtered_b = signal.sosfiltfilt(sos, channel_b)
        
        # Return as DualChannel for further processing
        return {
            "type": "DualChannel",
            "channel_a": filtered_a.tolist(),
            "channel_b": filtered_b.tolist(),
            "sample_rate": sample_rate,
            "timestamp": data["timestamp"],
            "frame_number": data["frame_number"]
        }
    
    # Pass through other types unchanged
    return data

def get_status():
    """Return current filter status"""
    return {
        "status": "active", 
        "type": "scipy_bandpass_filter",
        "low_freq": 300,
        "high_freq": 3000,
        "filter_order": 5
    }

def shutdown():
    """Shutdown the filter"""
    print("SciPy bandpass filter shutting down")
    return {"status": "shutdown"}
"#;

/// Simple Python script for testing basic functionality
const SIMPLE_GAIN_SCRIPT: &str = r#"
import json

def initialize():
    """Initialize the gain node"""
    return {"status": "initialized", "gain": 2.0}

def process_data(data):
    """Apply 2x gain to all samples"""
    data_type = data.get("type")
    
    if data_type == "SingleChannel":
        samples = data["samples"]
        # Apply 2x gain
        gained_samples = [s * 2.0 for s in samples]
        
        return {
            "type": "SingleChannel",
            "samples": gained_samples,
            "sample_rate": data["sample_rate"],
            "timestamp": data["timestamp"],
            "frame_number": data["frame_number"]
        }
    
    elif data_type == "DualChannel":
        channel_a = [s * 2.0 for s in data["channel_a"]]
        channel_b = [s * 2.0 for s in data["channel_b"]]
        
        return {
            "type": "DualChannel",
            "channel_a": channel_a,
            "channel_b": channel_b,
            "sample_rate": data["sample_rate"],
            "timestamp": data["timestamp"],
            "frame_number": data["frame_number"]
        }
    
    # Pass through other types
    return data

def get_status():
    return {"status": "active", "gain": 2.0}

def shutdown():
    return {"status": "shutdown"}
"#;

#[cfg(feature = "python-driver")]
#[test]
fn test_python_node_simple_gain() -> Result<()> {
    let (_temp_dir, script_path) = create_test_script(SIMPLE_GAIN_SCRIPT);

    let config = PythonNodeConfig {
        script_path,
        auto_reload: false,
        timeout_seconds: 10,
        ..Default::default()
    };

    let mut node = PythonNode::new("gain_node".to_string(), config);

    // Test with SingleChannel data
    let input = ProcessingData::SingleChannel {
        samples: vec![0.1, 0.2, 0.3, 0.4],
        sample_rate: 44100,
        timestamp: 1000,
        frame_number: 1,
    };

    let result = node.process(input)?;

    match result {
        ProcessingData::SingleChannel {
            samples,
            sample_rate,
            timestamp,
            frame_number,
        } => {
            // Check that gain was applied (2x)
            assert_eq!(samples, vec![0.2, 0.4, 0.6, 0.8]);
            assert_eq!(sample_rate, 44100);
            assert_eq!(timestamp, 1000);
            assert_eq!(frame_number, 1);
        }
        _ => panic!("Expected SingleChannel output"),
    }

    Ok(())
}

#[cfg(feature = "python-driver")]
#[test]
fn test_python_node_dual_channel_gain() -> Result<()> {
    let (_temp_dir, script_path) = create_test_script(SIMPLE_GAIN_SCRIPT);

    let config = PythonNodeConfig {
        script_path,
        auto_reload: false,
        timeout_seconds: 10,
        ..Default::default()
    };

    let mut node = PythonNode::new("dual_gain_node".to_string(), config);

    // Test with DualChannel data
    let input = ProcessingData::DualChannel {
        channel_a: vec![0.1, 0.2],
        channel_b: vec![0.3, 0.4],
        sample_rate: 44100,
        timestamp: 2000,
        frame_number: 2,
    };

    let result = node.process(input)?;

    match result {
        ProcessingData::DualChannel {
            channel_a,
            channel_b,
            sample_rate,
            timestamp,
            frame_number,
        } => {
            // Check that gain was applied to both channels
            assert_eq!(channel_a, vec![0.2, 0.4]);
            assert_eq!(channel_b, vec![0.6, 0.8]);
            assert_eq!(sample_rate, 44100);
            assert_eq!(timestamp, 2000);
            assert_eq!(frame_number, 2);
        }
        _ => panic!("Expected DualChannel output"),
    }

    Ok(())
}

#[cfg(feature = "python-driver")]
#[test]
fn test_python_node_scipy_bandpass_filter_single_channel() -> Result<()> {
    let (_temp_dir, script_path) = create_test_script(SCIPY_BANDPASS_FILTER_SCRIPT);

    let config = PythonNodeConfig {
        script_path,
        auto_reload: false,
        timeout_seconds: 30, // SciPy operations might take longer
        ..Default::default()
    };

    let mut node = PythonNode::new("scipy_bandpass".to_string(), config);

    // Create test signal: mix of 100Hz (should be filtered out), 1000Hz (should pass), and 5000Hz (should be filtered out)
    let sample_rate = 44100u32;
    let duration = 0.1; // 100ms
    let samples_count = (sample_rate as f32 * duration) as usize;

    let mut samples = Vec::with_capacity(samples_count);
    for i in 0..samples_count {
        let t = i as f32 / sample_rate as f32;
        // Mix three frequencies: 100Hz, 1000Hz, 5000Hz
        let sample = 0.3 * (2.0 * std::f32::consts::PI * 100.0 * t).sin()
            + 0.5 * (2.0 * std::f32::consts::PI * 1000.0 * t).sin()
            + 0.2 * (2.0 * std::f32::consts::PI * 5000.0 * t).sin();
        samples.push(sample);
    }

    let input = ProcessingData::SingleChannel {
        samples,
        sample_rate,
        timestamp: 1000,
        frame_number: 1,
    };

    let result = node.process(input.clone())?;

    match result {
        ProcessingData::SingleChannel {
            samples: filtered_samples,
            sample_rate: out_rate,
            ..
        } => {
            // Check basic properties
            assert_eq!(out_rate, sample_rate);
            assert_eq!(filtered_samples.len(), samples_count);

            // The filtered signal should have reduced amplitude at 100Hz and 5000Hz frequencies
            // and preserved amplitude around 1000Hz

            // Calculate RMS of original and filtered signals
            let original_samples = if let ProcessingData::SingleChannel { samples, .. } = input {
                samples
            } else {
                vec![]
            };

            let original_rms: f32 = original_samples.iter().map(|x| x * x).sum::<f32>().sqrt()
                / original_samples.len() as f32;
            let filtered_rms: f32 = filtered_samples.iter().map(|x| x * x).sum::<f32>().sqrt()
                / filtered_samples.len() as f32;

            // The filtered signal should have lower RMS due to removal of out-of-band frequencies
            // but should not be zero (1000Hz component should remain)
            assert!(filtered_rms > 0.0, "Filtered signal should not be empty");
            assert!(
                filtered_rms < original_rms,
                "Filtered signal should have lower RMS than original"
            );

            println!(
                "Original RMS: {:.6}, Filtered RMS: {:.6}",
                original_rms, filtered_rms
            );
        }
        _ => panic!("Expected SingleChannel output"),
    }

    Ok(())
}

#[cfg(feature = "python-driver")]
#[test]
fn test_python_node_scipy_bandpass_filter_dual_channel() -> Result<()> {
    let (_temp_dir, script_path) = create_test_script(SCIPY_BANDPASS_FILTER_SCRIPT);

    let config = PythonNodeConfig {
        script_path,
        auto_reload: false,
        timeout_seconds: 30,
        ..Default::default()
    };

    let mut node = PythonNode::new("scipy_bandpass_dual".to_string(), config);

    // Create test signals for both channels
    let sample_rate = 44100u32;
    let duration = 0.05; // 50ms (shorter for faster test)
    let samples_count = (sample_rate as f32 * duration) as usize;

    let mut channel_a = Vec::with_capacity(samples_count);
    let mut channel_b = Vec::with_capacity(samples_count);

    for i in 0..samples_count {
        let t = i as f32 / sample_rate as f32;
        // Channel A: 1500Hz (should pass through)
        let sample_a = 0.5 * (2.0 * std::f32::consts::PI * 1500.0 * t).sin();
        // Channel B: 5000Hz (should be filtered out)
        let sample_b = 0.5 * (2.0 * std::f32::consts::PI * 5000.0 * t).sin();

        channel_a.push(sample_a);
        channel_b.push(sample_b);
    }

    let input = ProcessingData::DualChannel {
        channel_a: channel_a.clone(),
        channel_b: channel_b.clone(),
        sample_rate,
        timestamp: 2000,
        frame_number: 2,
    };

    let result = node.process(input)?;

    match result {
        ProcessingData::DualChannel {
            channel_a: filtered_a,
            channel_b: filtered_b,
            sample_rate: out_rate,
            ..
        } => {
            assert_eq!(out_rate, sample_rate);
            assert_eq!(filtered_a.len(), samples_count);
            assert_eq!(filtered_b.len(), samples_count);

            // Channel A (1500Hz) should pass through with minimal attenuation
            let original_a_rms: f32 =
                channel_a.iter().map(|x| x * x).sum::<f32>().sqrt() / channel_a.len() as f32;
            let filtered_a_rms: f32 =
                filtered_a.iter().map(|x| x * x).sum::<f32>().sqrt() / filtered_a.len() as f32;

            // Channel B (5000Hz) should be significantly attenuated
            let original_b_rms: f32 =
                channel_b.iter().map(|x| x * x).sum::<f32>().sqrt() / channel_b.len() as f32;
            let filtered_b_rms: f32 =
                filtered_b.iter().map(|x| x * x).sum::<f32>().sqrt() / filtered_b.len() as f32;

            // Channel A should retain most of its energy (1500Hz is in passband)
            let a_retention = filtered_a_rms / original_a_rms;
            assert!(
                a_retention > 0.5,
                "Channel A should retain most energy (got {:.3})",
                a_retention
            );

            // Channel B should lose most of its energy (5000Hz is in stopband)
            let b_retention = filtered_b_rms / original_b_rms;
            assert!(
                b_retention < 0.5,
                "Channel B should lose most energy (got {:.3})",
                b_retention
            );

            println!(
                "Channel A retention: {:.3}, Channel B retention: {:.3}",
                a_retention, b_retention
            );
        }
        _ => panic!("Expected DualChannel output"),
    }

    Ok(())
}

#[cfg(feature = "python-driver")]
#[test]
fn test_python_node_type_restrictions() -> Result<()> {
    let (_temp_dir, script_path) = create_test_script(SIMPLE_GAIN_SCRIPT);

    let config = PythonNodeConfig {
        script_path,
        accepted_types: vec!["SingleChannel".to_string()], // Only accept SingleChannel
        auto_reload: false,
        timeout_seconds: 10,
        ..Default::default()
    };

    let node = PythonNode::new("restricted_node".to_string(), config);

    // Test accepted type
    let single_channel = ProcessingData::SingleChannel {
        samples: vec![0.1, 0.2],
        sample_rate: 44100,
        timestamp: 1000,
        frame_number: 1,
    };
    assert!(node.accepts_input(&single_channel));

    // Test rejected type
    let dual_channel = ProcessingData::DualChannel {
        channel_a: vec![0.1, 0.2],
        channel_b: vec![0.3, 0.4],
        sample_rate: 44100,
        timestamp: 1000,
        frame_number: 1,
    };
    assert!(!node.accepts_input(&dual_channel));

    Ok(())
}

#[cfg(feature = "python-driver")]
#[test]
fn test_python_node_output_type_validation() -> Result<()> {
    let (_temp_dir, script_path) = create_test_script(SIMPLE_GAIN_SCRIPT);

    let config = PythonNodeConfig {
        script_path,
        output_type: Some("SingleChannel".to_string()), // Expect SingleChannel output
        auto_reload: false,
        timeout_seconds: 10,
        ..Default::default()
    };

    let node = PythonNode::new("output_validated_node".to_string(), config);

    // Test expected output type
    let single_channel = ProcessingData::SingleChannel {
        samples: vec![0.1, 0.2],
        sample_rate: 44100,
        timestamp: 1000,
        frame_number: 1,
    };

    assert_eq!(
        node.output_type(&single_channel),
        Some("SingleChannel".to_string())
    );

    Ok(())
}

#[cfg(not(feature = "python-driver"))]
#[test]
fn test_python_node_without_feature() {
    let config = PythonNodeConfig::default();
    let mut node = PythonNode::new("test".to_string(), config);

    let input = ProcessingData::SingleChannel {
        samples: vec![0.1, 0.2],
        sample_rate: 44100,
        timestamp: 1000,
        frame_number: 1,
    };

    // Should fail when python-driver feature is not enabled
    let result = node.process(input);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Python driver feature not enabled"));
}

#[test]
fn test_python_node_configuration() {
    // Test default configuration
    let default_config = PythonNodeConfig::default();
    assert_eq!(default_config.script_path, PathBuf::from("processor.py"));
    assert_eq!(default_config.process_function, "process_data");
    assert_eq!(default_config.timeout_seconds, 30);
    assert_eq!(default_config.auto_reload, false);

    // Test custom configuration
    let custom_config = PythonNodeConfig {
        script_path: PathBuf::from("custom.py"),
        process_function: "custom_process".to_string(),
        timeout_seconds: 60,
        auto_reload: true,
        accepted_types: vec!["DualChannel".to_string()],
        output_type: Some("SingleChannel".to_string()),
        ..Default::default()
    };

    assert_eq!(custom_config.script_path, PathBuf::from("custom.py"));
    assert_eq!(custom_config.process_function, "custom_process");
    assert_eq!(custom_config.timeout_seconds, 60);
    assert!(custom_config.auto_reload);
    assert_eq!(custom_config.accepted_types, vec!["DualChannel"]);
    assert_eq!(custom_config.output_type, Some("SingleChannel".to_string()));
}

#[test]
fn test_python_node_creation_and_properties() {
    let config = PythonNodeConfig::default();
    let node = PythonNode::new("test_python_node".to_string(), config);

    assert_eq!(node.node_id(), "test_python_node");
    assert_eq!(node.node_type(), "python");

    // Test cloning
    let cloned = node.clone_node();
    assert_eq!(cloned.node_id(), "test_python_node");
    assert_eq!(cloned.node_type(), "python");
}
