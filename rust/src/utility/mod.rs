// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Utility module for common utilities used throughout the project

pub mod certificate_utilities;
pub mod cpal;
pub mod data_source;
pub mod noise_generator;
#[cfg(test)]
pub mod noise_generator_test;
pub mod temperature_conversion;

// Re-exports for use in other modules
pub use data_source::PhotoacousticDataSource;
pub use temperature_conversion::convert_voltage_to_temperature;

/// Macro to include a PNG file as a base64-encoded string
/// This macro reads a PNG file at compile time and encodes it in base64 format.
/// The resulting string can be used directly in HTML or CSS as a data URL.
#[macro_export]
macro_rules! include_png_as_base64 {
    ($path:expr) => {{
        use ::base64::prelude::{Engine as _, BASE64_STANDARD};
        let png_data = include_bytes!($path);
        let base64 = BASE64_STANDARD.encode(png_data);
        format!("data:image/png;base64,{}", base64)
    }};
}

/// Macro to include a SVG file as a base64-encoded string
/// This macro reads a SVG file at compile time and encodes it in base64 format.
/// The resulting string can be used directly in HTML or CSS as a data URL.
/// It is useful for embedding SVG images directly into web pages or stylesheets.
#[macro_export]
macro_rules! include_svg_as_base64 {
    ($path:expr) => {{
        use ::base64::prelude::{Engine as _, BASE64_STANDARD};
        let svg_data = include_bytes!($path);
        let base64 = BASE64_STANDARD.encode(svg_data);
        format!("data:image/svg+xml;base64,{}", base64)
    }};
}
