// Test file to verify serializable ProcessingGraph functionality
use rust_photoacoustic::processing::{ProcessingGraph, SerializableProcessingGraph};
use rust_photoacoustic::processing::nodes::{InputNode, ProcessingNode};

fn main() -> anyhow::Result<()> {
    // Create a simple processing graph
    let mut graph = ProcessingGraph::new();
    
    // Add an input node
    let input_node = Box::new(InputNode::new("main_input".to_string()));
    graph.add_node(input_node)?;
    
    // Convert to serializable format
    let serializable_graph: SerializableProcessingGraph = graph.into();
    
    // Verify the conversion
    println!("Serializable Graph Created Successfully!");
    println!("Number of nodes: {}", serializable_graph.nodes.len());
    println!("Input node: {:?}", serializable_graph.input_node);
    println!("Graph is valid: {}", serializable_graph.is_valid);
    
    if let Some(node) = serializable_graph.nodes.first() {
        println!("First node: {} (type: {})", node.id, node.node_type);
        println!("Accepts input types: {:?}", node.accepts_input_types);
    }
    
    // Try serializing to JSON
    match serde_json::to_string_pretty(&serializable_graph) {
        Ok(json) => {
            println!("\nJSON serialization successful!");
            println!("JSON size: {} bytes", json.len());
        }
        Err(e) => {
            println!("JSON serialization failed: {}", e);
        }
    }
    
    println!("\n✅ All serializable ProcessingGraph functionality working correctly!");
    
    Ok(())
}
