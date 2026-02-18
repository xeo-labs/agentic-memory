//! 100K node performance demo.
//!
//! Uses `MemoryGraph::from_parts` for fast bulk construction.

use std::time::Instant;

use agentic_memory::*;

fn main() -> AmemResult<()> {
    let dim = DEFAULT_DIMENSION;
    let node_count = 100_000;
    let edges_per_node = 3;

    println!("Creating graph with {} nodes...", node_count);
    let start = Instant::now();

    let types = [
        EventType::Fact,
        EventType::Decision,
        EventType::Inference,
        EventType::Skill,
        EventType::Episode,
    ];

    // Build nodes
    let mut nodes = Vec::with_capacity(node_count);
    for i in 0..node_count {
        let et = types[i % types.len()];
        let mut event = CognitiveEventBuilder::new(et, format!("event_{}", i))
            .session_id(i as u32 / 100)
            .confidence(0.5 + (i as f32 % 50.0) / 100.0)
            .build();
        event.id = i as u64;
        nodes.push(event);
    }

    println!("  Nodes created in {:?}", start.elapsed());

    // Build edges
    let start = Instant::now();
    let mut edges = Vec::with_capacity(node_count * edges_per_node);
    for i in 0..node_count {
        for j in 1..=edges_per_node {
            let target = (i + j * 7) % node_count;
            if target != i {
                edges.push(Edge::new(i as u64, target as u64, EdgeType::RelatedTo, 0.5));
            }
        }
    }

    // Construct graph from parts (fast — single sort + adjacency build)
    let graph = MemoryGraph::from_parts(nodes, edges, dim)?;
    println!(
        "  Graph built in {:?} ({} nodes, {} edges)",
        start.elapsed(),
        graph.node_count(),
        graph.edge_count()
    );

    // Write to file
    let path = std::path::Path::new("/tmp/large_graph.amem");
    let start = Instant::now();
    let writer = AmemWriter::new(dim);
    writer.write_to_file(&graph, path)?;
    let file_size = std::fs::metadata(path)?.len();
    println!(
        "  Written to file in {:?} ({:.1} MB)",
        start.elapsed(),
        file_size as f64 / 1_048_576.0
    );

    // Read back
    let start = Instant::now();
    let loaded = AmemReader::read_from_file(path)?;
    println!("  Read back in {:?}", start.elapsed());
    println!(
        "  Loaded: {} nodes, {} edges",
        loaded.node_count(),
        loaded.edge_count()
    );

    // Traversal
    let query = QueryEngine::new();
    let start = Instant::now();
    let result = query.traverse(
        &loaded,
        TraversalParams {
            start_id: 50_000,
            edge_types: vec![EdgeType::RelatedTo],
            direction: TraversalDirection::Forward,
            max_depth: 5,
            max_results: 100,
            min_confidence: 0.0,
        },
    )?;
    println!(
        "  Traversal: {} nodes visited in {:?}",
        result.visited.len(),
        start.elapsed()
    );

    // Pattern query
    let start = Instant::now();
    let results = query.pattern(
        &loaded,
        PatternParams {
            event_types: vec![EventType::Fact],
            min_confidence: Some(0.7),
            max_confidence: None,
            session_ids: vec![],
            created_after: None,
            created_before: None,
            min_decay_score: None,
            max_results: 50,
            sort_by: PatternSort::MostRecent,
        },
    )?;
    println!(
        "  Pattern query: {} results in {:?}",
        results.len(),
        start.elapsed()
    );

    println!("\nDone!");
    Ok(())
}
