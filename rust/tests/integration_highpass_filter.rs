// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

use rust_photoacoustic::config::processing::*;
use rust_photoacoustic::processing::ProcessingGraph;

#[test]
fn test_highpass_filter_from_config() {
    let config = ProcessingGraphConfig {
        id: "highpass_test".to_string(),
        nodes: vec![
            NodeConfig {
                id: "input".to_string(),
                node_type: "input".to_string(),
                parameters: serde_yml::Value::Null,
            },
            NodeConfig {
                id: "highpass".to_string(),
                node_type: "filter".to_string(),
                parameters: serde_yml::to_value(serde_yml::mapping::Mapping::from_iter([
                    ("type".into(), "highpass".into()),
                    ("cutoff_frequency".into(), 100.0.into()),
                ]))
                .unwrap(),
            },
        ],
        connections: vec![ConnectionConfig {
            from: "input".to_string(),
            to: "highpass".to_string(),
        }],
        output_node: Some("highpass".to_string()),
    };

    // Test that the graph can be created from config
    let graph = ProcessingGraph::from_config(&config);
    assert!(
        graph.is_ok(),
        "Failed to create graph with highpass filter: {:?}",
        graph.err()
    );
}

#[test]
fn test_lowpass_filter_from_config() {
    let config = ProcessingGraphConfig {
        id: "lowpass_test".to_string(),
        nodes: vec![
            NodeConfig {
                id: "input".to_string(),
                node_type: "input".to_string(),
                parameters: serde_yml::Value::Null,
            },
            NodeConfig {
                id: "lowpass".to_string(),
                node_type: "filter".to_string(),
                parameters: serde_yml::to_value(serde_yml::mapping::Mapping::from_iter([
                    ("type".into(), "lowpass".into()),
                    ("cutoff_frequency".into(), 5000.0.into()),
                ]))
                .unwrap(),
            },
        ],
        connections: vec![ConnectionConfig {
            from: "input".to_string(),
            to: "lowpass".to_string(),
        }],
        output_node: Some("lowpass".to_string()),
    };

    // Test that the graph can be created from config
    let graph = ProcessingGraph::from_config(&config);
    assert!(
        graph.is_ok(),
        "Failed to create graph with lowpass filter: {:?}",
        graph.err()
    );
}

#[test]
fn test_filter_chain_from_config() {
    let config = ProcessingGraphConfig {
        id: "filter_chain_test".to_string(),
        nodes: vec![
            NodeConfig {
                id: "input".to_string(),
                node_type: "input".to_string(),
                parameters: serde_yml::Value::Null,
            },
            NodeConfig {
                id: "highpass".to_string(),
                node_type: "filter".to_string(),
                parameters: serde_yml::to_value(serde_yml::mapping::Mapping::from_iter([
                    ("type".into(), "highpass".into()),
                    ("cutoff_frequency".into(), 100.0.into()),
                ]))
                .unwrap(),
            },
            NodeConfig {
                id: "bandpass".to_string(),
                node_type: "filter".to_string(),
                parameters: serde_yml::to_value(serde_yml::mapping::Mapping::from_iter([
                    ("type".into(), "bandpass".into()),
                    ("center_frequency".into(), 2000.0.into()),
                    ("bandwidth".into(), 200.0.into()),
                ]))
                .unwrap(),
            },
            NodeConfig {
                id: "lowpass".to_string(),
                node_type: "filter".to_string(),
                parameters: serde_yml::to_value(serde_yml::mapping::Mapping::from_iter([
                    ("type".into(), "lowpass".into()),
                    ("cutoff_frequency".into(), 5000.0.into()),
                ]))
                .unwrap(),
            },
        ],
        connections: vec![
            ConnectionConfig {
                from: "input".to_string(),
                to: "highpass".to_string(),
            },
            ConnectionConfig {
                from: "highpass".to_string(),
                to: "bandpass".to_string(),
            },
            ConnectionConfig {
                from: "bandpass".to_string(),
                to: "lowpass".to_string(),
            },
        ],
        output_node: Some("lowpass".to_string()),
    };

    // Test that a complex filter chain can be created from config
    let graph = ProcessingGraph::from_config(&config);
    assert!(
        graph.is_ok(),
        "Failed to create graph with filter chain: {:?}",
        graph.err()
    );
}
