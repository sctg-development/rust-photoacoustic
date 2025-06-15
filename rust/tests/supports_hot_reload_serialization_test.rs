use rust_photoacoustic::preprocessing::filters::BandpassFilter;
use rust_photoacoustic::processing::graph::{ProcessingGraph, SerializableProcessingGraph};
use rust_photoacoustic::processing::nodes::filter::FilterNode;
use rust_photoacoustic::processing::nodes::input::InputNode;
use rust_photoacoustic::processing::nodes::output::PhotoacousticOutputNode;
use rust_photoacoustic::processing::ChannelTarget;
use rust_photoacoustic::processing::ProcessingNode;
use serde_json;

#[test]
fn test_serializable_node_supports_hot_reload_field() {
    // Create a simple processing graph with nodes that have different hot-reload support
    let mut graph = ProcessingGraph::new();

    // Add an InputNode (supports hot-reload: false)
    let input_node = InputNode::new("input".to_string());
    let input_supports_hot_reload = input_node.supports_hot_reload();
    graph
        .add_node(Box::new(input_node))
        .expect("Failed to add input node");

    // Add a FilterNode (supports hot-reload: true)
    let filter = BandpassFilter::new(1000.0, 100.0);
    let filter_node = FilterNode::new("filter".to_string(), Box::new(filter), ChannelTarget::Both);
    let filter_supports_hot_reload = filter_node.supports_hot_reload();
    graph
        .add_node(Box::new(filter_node))
        .expect("Failed to add filter node");

    // Add an OutputNode (supports hot-reload: false)
    let output_node = PhotoacousticOutputNode::new("output".to_string());
    let output_supports_hot_reload = output_node.supports_hot_reload();
    graph
        .add_node(Box::new(output_node))
        .expect("Failed to add output node");

    // Verify our expectations about hot-reload support
    assert!(
        !input_supports_hot_reload,
        "InputNode should not support hot-reload"
    );
    assert!(
        filter_supports_hot_reload,
        "FilterNode should support hot-reload"
    );
    assert!(
        !output_supports_hot_reload,
        "OutputNode should not support hot-reload"
    );

    // Serialize the graph (we don't need the all_parameters HashMap)
    let serializable_graph = graph.to_serializable();

    // Verify that the supports_hot_reload field is correctly set for each node
    assert_eq!(serializable_graph.nodes.len(), 3);

    for node in &serializable_graph.nodes {
        match node.id.as_str() {
            "input" => {
                assert!(
                    !node.supports_hot_reload,
                    "InputNode should have supports_hot_reload: false"
                );
            }
            "filter" => {
                assert!(
                    node.supports_hot_reload,
                    "FilterNode should have supports_hot_reload: true"
                );
            }
            "output" => {
                assert!(
                    !node.supports_hot_reload,
                    "OutputNode should have supports_hot_reload: false"
                );
            }
            _ => panic!("Unexpected node id: {}", node.id),
        }
    }

    // Test JSON serialization/deserialization to ensure the field is properly included
    let json =
        serde_json::to_string_pretty(&serializable_graph).expect("Failed to serialize to JSON");
    println!("Serialized graph JSON:\n{}", json);

    // Verify that the JSON contains the supports_hot_reload field
    assert!(
        json.contains("supports_hot_reload"),
        "JSON should contain the supports_hot_reload field"
    );
    assert!(
        json.contains("\"supports_hot_reload\": true"),
        "JSON should contain supports_hot_reload: true for FilterNode"
    );
    assert!(
        json.contains("\"supports_hot_reload\": false"),
        "JSON should contain supports_hot_reload: false for other nodes"
    );

    // Test deserialization
    let deserialized: SerializableProcessingGraph =
        serde_json::from_str(&json).expect("Failed to deserialize from JSON");

    // Verify deserialized data matches original
    assert_eq!(deserialized.nodes.len(), 3);
    for (original, deserialized) in serializable_graph
        .nodes
        .iter()
        .zip(deserialized.nodes.iter())
    {
        assert_eq!(original.id, deserialized.id);
        assert_eq!(
            original.supports_hot_reload,
            deserialized.supports_hot_reload
        );
    }
}
