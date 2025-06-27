"""
SciPy Bandpass Filter for PythonNode

This script demonstrates implementing a bandpass filter using SciPy
for the PythonNode processing system.

Features:
- Butterworth bandpass filter (300Hz - 3000Hz)
- Zero-phase filtering to avoid distortion
- Support for SingleChannel, DualChannel, and AudioFrame data
- Automatic frequency validation

Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
This file is part of the rust-photoacoustic project and is licensed under the
SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
"""

import numpy as np
from scipy import signal
import json

# -----------------------------
# User Guide for PythonNode Scripts
# -----------------------------
#
# This script is an example for use with the Rust PythonNode processing node.
# To create your own Python processing node, you must implement the following functions:
#   - initialize(): Called once when the node is created. Return a dict with status info.
#   - process_data(data): Called for each data block. Receives a dict, must return a dict.
#   - get_status(): Return a dict describing the current status of the node.
#   - shutdown(): Called when the node is dropped. Return a dict with shutdown info.
#
# The 'data' argument to process_data will be a dictionary with at least a 'type' key.
# Supported types (see Rust ProcessingData):
#   - 'SingleChannel': {samples, sample_rate, timestamp, frame_number}
#   - 'DualChannel': {channel_a, channel_b, sample_rate, timestamp, frame_number}
#   - 'AudioFrame': {channel_a, channel_b, sample_rate, timestamp, frame_number}
#   - 'PhotoacousticResult': {signal, metadata}
#
# Your function must return a dictionary in the same format, with the same or modified data.
#
# You can use any Python library available in your environment (e.g., numpy, scipy).
#
# For more details, see the Rust documentation for PythonNode and PythonNodeConfig.
#
# -----------------------------

# Filter configuration
LOW_FREQ = 300.0   # Low cutoff frequency in Hz
HIGH_FREQ = 3000.0 # High cutoff frequency in Hz
FILTER_ORDER = 5   # Butterworth filter order


def initialize():
    """
    Called once when the node is initialized.
    You can use this to set up any state, print info, or return metadata.
    Returns:
        dict: Status and filter parameters.
    """
    print(f"SciPy bandpass filter initialized: {LOW_FREQ}Hz - {HIGH_FREQ}Hz, order {FILTER_ORDER}")
    return {
        "status": "initialized", 
        "filter_type": "scipy_butterworth_bandpass",
        "low_freq": LOW_FREQ,
        "high_freq": HIGH_FREQ,
        "order": FILTER_ORDER
    }


def design_filter(sample_rate):
    """
    Design a Butterworth bandpass filter for the given sample rate.
    Args:
        sample_rate (float): The sample rate of the input signal in Hz.
    Returns:
        sos (ndarray): Second-order sections for the filter.
    """
    nyquist = sample_rate / 2.0
    low_norm = LOW_FREQ / nyquist
    high_norm = HIGH_FREQ / nyquist
    # Ensure normalized frequencies are valid (0 < freq < 1)
    low_norm = max(0.001, min(0.999, low_norm))
    high_norm = max(low_norm + 0.001, min(0.999, high_norm))
    # Design Butterworth bandpass filter using SOS (Second-Order Sections)
    # SOS format is more numerically stable than transfer function
    sos = signal.butter(FILTER_ORDER, [low_norm, high_norm], btype='band', output='sos')
    return sos


def apply_filter(samples, sample_rate):
    """
    Apply the bandpass filter to a list of samples.
    Args:
        samples (list of float): Input audio samples.
        sample_rate (float): Sample rate in Hz.
    Returns:
        list of float: Filtered samples.
    """
    if len(samples) == 0:
        return samples
    samples_array = np.array(samples, dtype=np.float64)
    sos = design_filter(sample_rate)
    # Use sosfiltfilt for zero-phase filtering (no phase distortion)
    filtered = signal.sosfiltfilt(sos, samples_array)
    return filtered.astype(np.float32).tolist()


def process_data(data):
    """
    Main processing function called by the Rust node for each data block.
    Args:
        data (dict): Input data dictionary. Must contain a 'type' key.
    Returns:
        dict: Output data dictionary, same format as input.
    """
    data_type = data.get("type")
    # Process SingleChannel data
    if data_type == "SingleChannel":
        samples = data["samples"]
        sample_rate = data["sample_rate"]
        filtered_samples = apply_filter(samples, sample_rate)
        return {
            "type": "SingleChannel",
            "samples": filtered_samples,
            "sample_rate": sample_rate,
            "timestamp": data["timestamp"],
            "frame_number": data["frame_number"]
        }
    # Process DualChannel data
    elif data_type == "DualChannel":
        channel_a = data["channel_a"]
        channel_b = data["channel_b"]
        sample_rate = data["sample_rate"]
        filtered_a = apply_filter(channel_a, sample_rate)
        filtered_b = apply_filter(channel_b, sample_rate)
        return {
            "type": "DualChannel",
            "channel_a": filtered_a,
            "channel_b": filtered_b,
            "sample_rate": sample_rate,
            "timestamp": data["timestamp"],
            "frame_number": data["frame_number"]
        }
    # Process AudioFrame data (convert to DualChannel)
    elif data_type == "AudioFrame":
        channel_a = data["channel_a"]
        channel_b = data["channel_b"]
        sample_rate = data["sample_rate"]
        filtered_a = apply_filter(channel_a, sample_rate)
        filtered_b = apply_filter(channel_b, sample_rate)
        # Convert AudioFrame to DualChannel for further processing
        return {
            "type": "DualChannel",
            "channel_a": filtered_a,
            "channel_b": filtered_b,
            "sample_rate": sample_rate,
            "timestamp": data["timestamp"],
            "frame_number": data["frame_number"]
        }
    # Pass through other types unchanged
    return data


def get_status():
    """
    Return the current status of the node. Called by Rust for status queries.
    Returns:
        dict: Status and filter parameters.
    """
    return {
        "status": "active", 
        "type": "scipy_butterworth_bandpass",
        "low_freq": LOW_FREQ,
        "high_freq": HIGH_FREQ,
        "filter_order": FILTER_ORDER,
        "description": f"Butterworth bandpass filter {LOW_FREQ}-{HIGH_FREQ}Hz, order {FILTER_ORDER}"
    }


def shutdown():
    """
    Called when the node is being shut down. Use for cleanup if needed.
    Returns:
        dict: Shutdown status.
    """
    print("SciPy bandpass filter shutting down")
    return {"status": "shutdown"}


# -----------------------------
# Standalone test (optional)
# -----------------------------
# You can run this script directly to test the filter logic with synthetic data.
# This is not used by the Rust node, but is useful for development.
if __name__ == "__main__":
    # Test the filter with synthetic data
    import matplotlib.pyplot as plt
    # Generate test signal: sum of 100Hz, 1000Hz, 5000Hz
    fs = 44100
    t = np.linspace(0, 1, fs, False)
    signal_test = (np.sin(100 * 2 * np.pi * t) +    # 100 Hz (should be filtered out)
                   0.5 * np.sin(1000 * 2 * np.pi * t) +  # 1000 Hz (should pass)
                   0.3 * np.sin(5000 * 2 * np.pi * t))   # 5000 Hz (should be filtered out)
    # Apply filter
    filtered = apply_filter(signal_test.tolist(), fs)
    print(f"Original signal RMS: {np.sqrt(np.mean(signal_test**2)):.6f}")
    print(f"Filtered signal RMS: {np.sqrt(np.mean(np.array(filtered)**2)):.6f}")
    # Plot if matplotlib is available
    try:
        plt.figure(figsize=(12, 6))
        plt.subplot(2, 1, 1)
        plt.plot(t[:1000], signal_test[:1000])
        plt.title('Original Signal (first 1000 samples)')
        plt.xlabel('Time (s)')
        plt.ylabel('Amplitude')
        plt.subplot(2, 1, 2)
        plt.plot(t[:1000], filtered[:1000])
        plt.title('Filtered Signal (first 1000 samples)')
        plt.xlabel('Time (s)')
        plt.ylabel('Amplitude')
        plt.tight_layout()
        plt.show()
    except ImportError:
        print("Matplotlib not available for plotting")
