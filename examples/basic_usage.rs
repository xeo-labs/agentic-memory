//! Basic create -> write -> query flow.

use agentic_memory::*;

fn main() -> AmemResult<()> {
    // Create a new graph
    let mut builder = GraphBuilder::new();

    // Add some cognitive events
    let fact1 = builder.add_fact("User prefers Python for scripting", 1, 0.95);
    let fact2 = builder.add_fact("User has 5 years of experience with Python", 1, 0.9);
    let decision1 = builder.add_decision("Recommend Python for the data pipeline project", 1, 0.85);

    // Link them
    builder.link(decision1, fact1, EdgeType::CausedBy, 1.0);
    builder.link(decision1, fact2, EdgeType::Supports, 0.8);

    // Build the graph
    let graph = builder.build()?;

    println!(
        "Graph created with {} nodes and {} edges",
        graph.node_count(),
        graph.edge_count()
    );

    // Query: traverse backward from decision to find reasoning
    let query = QueryEngine::new();
    let result = query.traverse(
        &graph,
        TraversalParams {
            start_id: decision1,
            edge_types: vec![EdgeType::CausedBy, EdgeType::Supports],
            direction: TraversalDirection::Backward,
            max_depth: 5,
            max_results: 50,
            min_confidence: 0.0,
        },
    )?;

    println!("Traversal from decision node {}:", decision1);
    for &id in &result.visited {
        if let Some(node) = graph.get_node(id) {
            let depth = result.depths.get(&id).unwrap_or(&0);
            println!(
                "  [depth {}] {} ({}): {}",
                depth, id, node.event_type, node.content
            );
        }
    }

    // Save to file
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph, std::path::Path::new("/tmp/example.amem"))?;
    println!("\nSaved to /tmp/example.amem");

    // Reload and verify
    let loaded = AmemReader::read_from_file(std::path::Path::new("/tmp/example.amem"))?;
    println!(
        "Reloaded: {} nodes, {} edges",
        loaded.node_count(),
        loaded.edge_count()
    );

    Ok(())
}
