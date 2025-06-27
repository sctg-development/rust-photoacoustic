"""
Simple Gain Node for PythonNode

This script demonstrates a basic gain (amplification/attenuation) 
operation using the PythonNode processing system.

Features:
- Configurable gain factor
- Support for all audio data types
- Preserves data format and metadata

Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
This file is part of the rust-photoacoustic project and is licensed under the
SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
"""

import json

# Configuration
GAIN_FACTOR = 2.0  # 2x amplification by default

def initialize():
    """Initialize the gain node"""
    print(f"Gain node initialized with factor: {GAIN_FACTOR}")
    return {
        "status": "initialized", 
        "type": "gain_node",
        "gain_factor": GAIN_FACTOR
    }

def apply_gain(samples, gain=GAIN_FACTOR):
    """Apply gain to a list of samples"""
    return [sample * gain for sample in samples]

def process_data(data):
    """
    Apply gain to audio data
    
    Args:
        data: Dictionary containing the processing data
        
    Returns:
        Dictionary with gained data in the same format
    """
    data_type = data.get("type")
    
    if data_type == "SingleChannel":
        samples = data["samples"]
        gained_samples = apply_gain(samples)
        
        return {
            "type": "SingleChannel",
            "samples": gained_samples,
            "sample_rate": data["sample_rate"],
            "timestamp": data["timestamp"],
            "frame_number": data["frame_number"]
        }
    
    elif data_type == "DualChannel":
        channel_a = apply_gain(data["channel_a"])
        channel_b = apply_gain(data["channel_b"])
        
        return {
            "type": "DualChannel",
            "channel_a": channel_a,
            "channel_b": channel_b,
            "sample_rate": data["sample_rate"],
            "timestamp": data["timestamp"],
            "frame_number": data["frame_number"]
        }
    
    elif data_type == "AudioFrame":
        channel_a = apply_gain(data["channel_a"])
        channel_b = apply_gain(data["channel_b"])
        
        # Return as AudioFrame to maintain format
        return {
            "type": "AudioFrame",
            "channel_a": channel_a,
            "channel_b": channel_b,
            "sample_rate": data["sample_rate"],
            "timestamp": data["timestamp"],
            "frame_number": data["frame_number"]
        }
    
    elif data_type == "PhotoacousticResult":
        # Apply gain to the signal data
        signal_data = data["signal"]
        gained_signal = apply_gain(signal_data)
        
        return {
            "type": "PhotoacousticResult",
            "signal": gained_signal,
            "metadata": data["metadata"]  # Preserve metadata unchanged
        }
    
    # Pass through unknown types unchanged
    return data

def get_status():
    """Return current gain node status"""
    return {
        "status": "active",
        "type": "gain_node", 
        "gain_factor": GAIN_FACTOR,
        "gain_db": 20 * (GAIN_FACTOR ** 0.5) if GAIN_FACTOR > 0 else float('-inf')
    }

def shutdown():
    """Shutdown the gain node"""
    print("Gain node shutting down")
    return {"status": "shutdown"}

# Test function for standalone execution
if __name__ == "__main__":
    # Test with sample data
    test_data = {
        "type": "SingleChannel",
        "samples": [0.1, 0.2, -0.1, -0.2],
        "sample_rate": 44100,
        "timestamp": 1000,
        "frame_number": 1
    }
    
    print("Input data:", test_data)
    
    result = process_data(test_data)
    
    print("Output data:", result)
    print(f"Gain applied: {GAIN_FACTOR}x")
