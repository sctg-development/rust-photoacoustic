// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Multi-Instance ConcentrationNode Demonstration
//!
//! This example demonstrates the usage of multiple ConcentrationNode instances
//! working with multiple PeakFinderNode instances in a multi-spectral gas
//! analysis scenario.

use rust_photoacoustic::processing::computing_nodes::{
    ComputingSharedData, ConcentrationNode, PeakFinderNode,
};
use rust_photoacoustic::processing::nodes::ProcessingNode;
use rust_photoacoustic::processing::ProcessingData;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::init();

    println!("🧪 Multi-Instance ConcentrationNode Demonstration");
    println!("================================================");

    // Create shared state for communication between nodes
    let shared_state = Arc::new(RwLock::new(ComputingSharedData::default()));

    // Step 1: Create PeakFinderNodes for different gas species
    println!("\n1. Creating PeakFinderNodes for different gas species...");

    let mut peak_finder_co2 = PeakFinderNode::new_with_shared_state(
        "peak_finder_co2".to_string(),
        Some(shared_state.clone()),
    )
    .with_frequency_range(2000.0, 2100.0)
    .with_detection_threshold(0.05);

    let mut peak_finder_ch4 = PeakFinderNode::new_with_shared_state(
        "peak_finder_ch4".to_string(),
        Some(shared_state.clone()),
    )
    .with_frequency_range(3000.0, 3100.0)
    .with_detection_threshold(0.03);

    let mut peak_finder_nh3 = PeakFinderNode::new_with_shared_state(
        "peak_finder_nh3".to_string(),
        Some(shared_state.clone()),
    )
    .with_frequency_range(1500.0, 1600.0)
    .with_detection_threshold(0.04);

    println!("   ✓ CO2 PeakFinder: 2000-2100 Hz, threshold 0.05");
    println!("   ✓ CH4 PeakFinder: 3000-3100 Hz, threshold 0.03");
    println!("   ✓ NH3 PeakFinder: 1500-1600 Hz, threshold 0.04");

    // Step 2: Create ConcentrationNodes with different polynomial calibrations
    println!("\n2. Creating ConcentrationNodes with gas-specific calibrations...");

    let mut concentration_co2 = ConcentrationNode::new_with_shared_state(
        "concentration_co2".to_string(),
        Some(shared_state.clone()),
    )
    .with_peak_finder_source("peak_finder_co2".to_string())
    .with_polynomial_coefficients([0.0, 850.0, -12.5, 0.0, 0.0])
    .with_spectral_line_id("CO2_4.26um".to_string())
    .with_temperature_compensation(true);

    let mut concentration_ch4 = ConcentrationNode::new_with_shared_state(
        "concentration_ch4".to_string(),
        Some(shared_state.clone()),
    )
    .with_peak_finder_source("peak_finder_ch4".to_string())
    .with_polynomial_coefficients([5.0, 1200.0, -8.3, 0.15, 0.0])
    .with_spectral_line_id("CH4_3.39um".to_string())
    .with_temperature_compensation(true);

    let mut concentration_nh3 = ConcentrationNode::new_with_shared_state(
        "concentration_nh3".to_string(),
        Some(shared_state.clone()),
    )
    .with_peak_finder_source("peak_finder_nh3".to_string())
    .with_polynomial_coefficients([2.5, 950.0, -15.2, 0.08, 0.0])
    .with_spectral_line_id("NH3_10.4um".to_string())
    .with_temperature_compensation(false);

    println!("   ✓ CO2 ConcentrationNode: 4th-degree polynomial, temp compensation ON");
    println!("   ✓ CH4 ConcentrationNode: 4th-degree polynomial, temp compensation ON");
    println!("   ✓ NH3 ConcentrationNode: 4th-degree polynomial, temp compensation OFF");

    // Step 3: Simulate peak detection results
    println!("\n3. Simulating spectral peak detection...");

    {
        let mut state = shared_state.write().await;

        // CO2 detection at 2050 Hz with 8 mV amplitude
        let co2_peak = rust_photoacoustic::processing::computing_nodes::PeakResult {
            frequency: 2050.0,
            amplitude: 0.008,
            concentration_ppm: None,
            timestamp: SystemTime::now(),
            coherence_score: 0.95,
            processing_metadata: std::collections::HashMap::new(),
        };
        state
            .peak_results
            .insert("peak_finder_co2".to_string(), co2_peak);

        // CH4 detection at 3045 Hz with 3 mV amplitude
        let ch4_peak = rust_photoacoustic::processing::computing_nodes::PeakResult {
            frequency: 3045.0,
            amplitude: 0.003,
            concentration_ppm: None,
            timestamp: SystemTime::now(),
            coherence_score: 0.88,
            processing_metadata: std::collections::HashMap::new(),
        };
        state
            .peak_results
            .insert("peak_finder_ch4".to_string(), ch4_peak);

        // NH3 detection at 1550 Hz with 12 mV amplitude
        let nh3_peak = rust_photoacoustic::processing::computing_nodes::PeakResult {
            frequency: 1550.0,
            amplitude: 0.012,
            concentration_ppm: None,
            timestamp: SystemTime::now(),
            coherence_score: 0.92,
            processing_metadata: std::collections::HashMap::new(),
        };
        state
            .peak_results
            .insert("peak_finder_nh3".to_string(), nh3_peak);
    }

    println!("   ✓ CO2 peak: 2050 Hz, 8.0 mV (coherence: 95%)");
    println!("   ✓ CH4 peak: 3045 Hz, 3.0 mV (coherence: 88%)");
    println!("   ✓ NH3 peak: 1550 Hz, 12.0 mV (coherence: 92%)");

    // Step 4: Process data through concentration nodes
    println!("\n4. Processing concentration calculations...");

    let test_data = ProcessingData::SingleChannel {
        samples: vec![0.1, 0.2, 0.3, 0.4, 0.5],
        sample_rate: 44100,
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis() as u64,
        frame_number: 1,
    };

    // Process through all concentration nodes
    concentration_co2.process(test_data.clone())?;
    concentration_ch4.process(test_data.clone())?;
    concentration_nh3.process(test_data)?;

    // Step 5: Display results
    println!("\n5. Analysis Results:");
    println!("===================");

    {
        let state = shared_state.read().await;

        // Display individual concentration results
        for (node_id, result) in &state.concentration_results {
            println!(
                "\n📊 {} Results:",
                node_id.replace("concentration_", "").to_uppercase()
            );
            println!("   Concentration: {:.3} ppm", result.concentration_ppm);
            println!(
                "   Source Peak:   {} Hz @ {:.3} mV",
                result.source_frequency,
                result.source_amplitude * 1000.0
            );
            println!(
                "   Spectral Line: {}",
                result.spectral_line_id.as_deref().unwrap_or("N/A")
            );
            println!(
                "   Temp Compensation: {}",
                if result.temperature_compensated {
                    "Enabled"
                } else {
                    "Disabled"
                }
            );
            println!("   Polynomial: {:?}", result.polynomial_coefficients);
        }

        // Display system overview
        println!("\n📈 System Overview:");
        println!(
            "   Total gas species detected: {}",
            state.concentration_results.len()
        );
        println!("   Peak finder nodes active: {}", state.peak_results.len());

        if let Some(highest_concentration) = state.concentration_results.values().max_by(|a, b| {
            a.concentration_ppm
                .partial_cmp(&b.concentration_ppm)
                .unwrap()
        }) {
            println!(
                "   Highest concentration: {:.3} ppm ({})",
                highest_concentration.concentration_ppm,
                highest_concentration
                    .source_peak_finder_id
                    .replace("peak_finder_", "")
                    .to_uppercase()
            );
        }
    }

    // Step 6: Demonstrate hot-reload capability
    println!("\n6. Demonstrating hot-reload configuration update...");

    let new_ch4_config = serde_json::json!({
        "polynomial_coefficients": [10.0, 1100.0, -6.0, 0.1, 0.0],
        "temperature_compensation": false,
        "min_amplitude_threshold": 0.002
    });

    if concentration_ch4.update_config(&new_ch4_config)? {
        println!("   ✓ CH4 configuration updated successfully");

        // Reprocess with new configuration
        let updated_test_data = ProcessingData::SingleChannel {
            samples: vec![0.2, 0.3, 0.4],
            sample_rate: 44100,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_millis() as u64,
            frame_number: 2,
        };

        concentration_ch4.process(updated_test_data)?;

        {
            let state = shared_state.read().await;
            if let Some(ch4_result) = state.concentration_results.get("concentration_ch4") {
                println!(
                    "   ✓ Updated CH4 concentration: {:.3} ppm",
                    ch4_result.concentration_ppm
                );
                println!(
                    "   ✓ Temperature compensation: {}",
                    if ch4_result.temperature_compensated {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                );
            }
        }
    }

    // Step 7: Performance statistics
    println!("\n7. Performance Statistics:");

    let (co2_processing, co2_calculations) = concentration_co2.get_statistics();
    let (ch4_processing, ch4_calculations) = concentration_ch4.get_statistics();
    let (nh3_processing, nh3_calculations) = concentration_nh3.get_statistics();

    println!(
        "   CO2 Node: {} processed, {} calculated",
        co2_processing, co2_calculations
    );
    println!(
        "   CH4 Node: {} processed, {} calculated",
        ch4_processing, ch4_calculations
    );
    println!(
        "   NH3 Node: {} processed, {} calculated",
        nh3_processing, nh3_calculations
    );

    println!("\n✅ Multi-instance demonstration completed successfully!");
    println!("\n💡 Key Features Demonstrated:");
    println!("   • Multiple ConcentrationNode instances with unique IDs");
    println!("   • Source-specific binding to PeakFinderNode instances");
    println!("   • Individual polynomial calibrations per gas species");
    println!("   • Hot-reload configuration updates");
    println!("   • HashMap-based result storage in shared state");
    println!("   • Backward compatibility with legacy fields");
    println!("   • Pass-through data processing behavior");

    Ok(())
}
