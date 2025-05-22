// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use anyhow::Result;
use rust_photoacoustic::config;

#[test]
fn test_config_schema_output() -> Result<()> {
    // This test just ensures that the function runs without errors
    // We can't easily test the actual output as it goes to stdout, but
    // we can verify that the function doesn't panic or return an error

    // Test that the function executes without errors
    config::output_config_schema()?;
    
    // If we got here without errors, the test passes
    Ok(())
}
