// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

// This module provides a list of available audio devices using the cpal library
use cpal::traits::{DeviceTrait, HostTrait};

/// List available audio input devices
/// This function retrieves the names of all available audio input devices
/// and returns them as a vector of strings.
///
/// # Returns
/// A Result containing a vector of device names or an error if the operation fails.
pub fn list_audio_devices() -> Result<Vec<String>, anyhow::Error> {
    // Get the default host
    let host = cpal::default_host();

    // Get the list of available input devices
    let devices = host
        .input_devices()
        .map_err(|e| anyhow::anyhow!("Failed to get input devices: {}", e))?;

    // Collect device names into a vector
    Ok(devices
        .into_iter()
        .map(|device| {
            device
                .name()
                .unwrap_or_else(|_| "Unknown Device".to_string())
        })
        .collect())
}
