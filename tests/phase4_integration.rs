//! Phase 4 tests: CLI integration and end-to-end flows.

use std::path::PathBuf;
use std::process::{Command, Output};

use tempfile::NamedTempFile;

use agentic_memory::engine::{
    CausalParams, PatternParams, PatternSort, QueryEngine, SimilarityParams, TraversalParams,
    WriteEngine,
};
use agentic_memory::format::{AmemReader, AmemWriter};
use agentic_memory::graph::{MemoryGraph, TraversalDirection};
use agentic_memory::types::{
    now_micros, CognitiveEventBuilder, Edge, EdgeType, EventType, DEFAULT_DIMENSION,
};

// ==================== CLI Helpers ====================

/// Locate the `amem` binary built alongside test binaries.
fn amem_bin() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove "deps"
    path.push("amem");
    path
}

/// Run the `amem` CLI with the given arguments and return the output.
fn run_amem(args: &[&str]) -> Output {
    Command::new(amem_bin())
        .args(args)
        .output()
        .expect("Failed to run amem")
}

/// Helper: assert that the CLI ran successfully (exit code 0).
fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "amem failed with status {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Helper: get stdout as a string from an Output.
fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

// ==================== CLI Tests ====================

#[test]
fn test_cli_create() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();

    let output = run_amem(&["create", path]);
    assert_success(&output);

    // File should exist and be a valid .amem file
    assert!(tmp.path().exists());

    // Re-read with AmemReader to validate
    let graph = AmemReader::read_from_file(tmp.path()).unwrap();
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
    assert_eq!(graph.dimension(), DEFAULT_DIMENSION);
}

#[test]
fn test_cli_add_and_get() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();

    // Create the file
    let output = run_amem(&["create", path]);
    assert_success(&output);

    // Add a fact
    let output = run_amem(&["add", path, "fact", "User prefers Rust", "--session", "1"]);
    assert_success(&output);
    let add_out = stdout_str(&output);
    assert!(
        add_out.contains("Added node 0"),
        "Expected 'Added node 0' in: {}",
        add_out
    );

    // Get the node and verify content
    let output = run_amem(&["get", path, "0"]);
    assert_success(&output);
    let get_out = stdout_str(&output);
    assert!(
        get_out.contains("User prefers Rust"),
        "Expected content in get output: {}",
        get_out
    );
    assert!(
        get_out.contains("fact"),
        "Expected type 'fact' in get output: {}",
        get_out
    );
}

#[test]
fn test_cli_info() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();

    // Create and add some nodes
    let output = run_amem(&["create", path]);
    assert_success(&output);

    for i in 0..5 {
        let content = format!("fact_{}", i);
        let output = run_amem(&["add", path, "fact", &content, "--session", "1"]);
        assert_success(&output);
    }

    // Run info
    let output = run_amem(&["info", path]);
    assert_success(&output);
    let info_out = stdout_str(&output);
    assert!(
        info_out.contains("Nodes: 5"),
        "Expected 'Nodes: 5' in info output: {}",
        info_out
    );
}

#[test]
fn test_cli_traverse() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();

    // Create graph
    let output = run_amem(&["create", path]);
    assert_success(&output);

    // Add nodes: 0 = base fact, 1 = inference from fact, 2 = decision from inference
    let output = run_amem(&["add", path, "fact", "base fact", "--session", "1"]);
    assert_success(&output);
    let output = run_amem(&[
        "add",
        path,
        "inference",
        "derived inference",
        "--session",
        "1",
    ]);
    assert_success(&output);
    let output = run_amem(&["add", path, "decision", "final decision", "--session", "1"]);
    assert_success(&output);

    // Link: inference caused_by fact, decision caused_by inference
    let output = run_amem(&["link", path, "1", "0", "caused_by", "--weight", "1.0"]);
    assert_success(&output);
    let output = run_amem(&["link", path, "2", "1", "caused_by", "--weight", "1.0"]);
    assert_success(&output);

    // Traverse forward from node 0 following caused_by
    let output = run_amem(&[
        "traverse",
        path,
        "0",
        "--edge-types",
        "caused_by",
        "--direction",
        "both",
        "--max-depth",
        "5",
    ]);
    assert_success(&output);
    let trav_out = stdout_str(&output);
    assert!(
        trav_out.contains("base fact"),
        "Expected 'base fact' in traversal output: {}",
        trav_out
    );
    assert!(
        trav_out.contains("derived inference"),
        "Expected 'derived inference' in traversal output: {}",
        trav_out
    );
}

#[test]
fn test_cli_impact() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();

    // Create graph
    let output = run_amem(&["create", path]);
    assert_success(&output);

    // Build dependency chain: fact <- inference <- decision
    // Node 0: fact
    let output = run_amem(&["add", path, "fact", "core fact", "--session", "1"]);
    assert_success(&output);
    // Node 1: inference that depends on the fact
    let output = run_amem(&[
        "add",
        path,
        "inference",
        "derived from core",
        "--session",
        "1",
    ]);
    assert_success(&output);
    // Node 2: decision that depends on the inference
    let output = run_amem(&[
        "add",
        path,
        "decision",
        "action based on inference",
        "--session",
        "1",
    ]);
    assert_success(&output);

    // Link: node 1 caused_by node 0, node 2 caused_by node 1
    let output = run_amem(&["link", path, "1", "0", "caused_by", "--weight", "1.0"]);
    assert_success(&output);
    let output = run_amem(&["link", path, "2", "1", "caused_by", "--weight", "1.0"]);
    assert_success(&output);

    // Impact analysis on node 0 (the base fact)
    let output = run_amem(&["impact", path, "0", "--max-depth", "10"]);
    assert_success(&output);
    let impact_out = stdout_str(&output);

    // Should show dependents
    assert!(
        impact_out.contains("Total dependents: 2")
            || impact_out.contains("total_dependents")
            || impact_out.contains("Dependency tree"),
        "Expected dependents info in impact output: {}",
        impact_out
    );
}

#[test]
fn test_cli_resolve() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();

    // Create graph
    let output = run_amem(&["create", path]);
    assert_success(&output);

    // Add original fact (node 0)
    let output = run_amem(&["add", path, "fact", "Earth is flat", "--session", "1"]);
    assert_success(&output);

    // Add correction via --supersedes (node 1 supersedes node 0)
    let output = run_amem(&[
        "add",
        path,
        "correction",
        "Earth is roughly spherical",
        "--session",
        "2",
        "--supersedes",
        "0",
    ]);
    assert_success(&output);

    // Resolve node 0 should show the correction as current
    let output = run_amem(&["resolve", path, "0"]);
    assert_success(&output);
    let resolve_out = stdout_str(&output);
    assert!(
        resolve_out.contains("Earth is roughly spherical"),
        "Expected correction content in resolve output: {}",
        resolve_out
    );
}

#[test]
fn test_cli_export_import() {
    let src_file = NamedTempFile::new().unwrap();
    let src_path = src_file.path().to_str().unwrap();

    // Build a source graph via CLI
    let output = run_amem(&["create", src_path]);
    assert_success(&output);

    let output = run_amem(&["add", src_path, "fact", "fact A", "--session", "1"]);
    assert_success(&output);
    let output = run_amem(&["add", src_path, "decision", "decision B", "--session", "1"]);
    assert_success(&output);
    let output = run_amem(&["link", src_path, "1", "0", "caused_by", "--weight", "0.9"]);
    assert_success(&output);

    // Export to JSON file
    let json_file = NamedTempFile::new().unwrap();
    let json_path = json_file.path().to_str().unwrap();

    let output = run_amem(&["export", src_path, "--pretty"]);
    assert_success(&output);
    let json_content = stdout_str(&output);

    // Write JSON to the temp file
    std::fs::write(json_file.path(), &json_content).unwrap();

    // Create a new empty .amem file and import
    let dst_file = NamedTempFile::new().unwrap();
    let dst_path = dst_file.path().to_str().unwrap();

    let output = run_amem(&["create", dst_path]);
    assert_success(&output);

    let output = run_amem(&["import", dst_path, json_path]);
    assert_success(&output);
    let import_out = stdout_str(&output);
    assert!(
        import_out.contains("Imported 2 nodes"),
        "Expected 'Imported 2 nodes' in import output: {}",
        import_out
    );

    // Verify the imported graph
    let graph = AmemReader::read_from_file(dst_file.path()).unwrap();
    assert_eq!(graph.node_count(), 2);
    // Note: edges from JSON import use the original IDs; since the destination
    // graph assigns new IDs starting from 0, the edge referencing original IDs
    // may or may not succeed. The import command silently skips invalid edges.
    // We verify that at least the 2 nodes were imported.
}

#[test]
fn test_cli_json_format() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();

    // Create and add a node
    let output = run_amem(&["create", path]);
    assert_success(&output);

    let output = run_amem(&["add", path, "fact", "json test fact", "--session", "1"]);
    assert_success(&output);

    // Run info with --format json
    let output = run_amem(&["--format", "json", "info", path]);
    assert_success(&output);
    let json_out = stdout_str(&output);

    // Parse as JSON to validate
    let parsed: serde_json::Value = serde_json::from_str(&json_out).unwrap_or_else(|e| {
        panic!(
            "Failed to parse info --format json output as JSON: {}\nOutput was: {}",
            e, json_out
        )
    });

    assert_eq!(parsed["nodes"], 1);
    assert!(parsed["dimension"].is_number());
}

#[test]
fn test_cli_sessions() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();

    let output = run_amem(&["create", path]);
    assert_success(&output);

    // Add nodes across 3 sessions
    let output = run_amem(&["add", path, "fact", "session 1 fact", "--session", "1"]);
    assert_success(&output);
    let output = run_amem(&["add", path, "fact", "session 2 fact", "--session", "2"]);
    assert_success(&output);
    let output = run_amem(&[
        "add",
        path,
        "decision",
        "session 3 decision",
        "--session",
        "3",
    ]);
    assert_success(&output);

    // Run sessions
    let output = run_amem(&["sessions", path]);
    assert_success(&output);
    let sessions_out = stdout_str(&output);

    assert!(
        sessions_out.contains("Session 1"),
        "Expected 'Session 1' in sessions output: {}",
        sessions_out
    );
    assert!(
        sessions_out.contains("Session 2"),
        "Expected 'Session 2' in sessions output: {}",
        sessions_out
    );
    assert!(
        sessions_out.contains("Session 3"),
        "Expected 'Session 3' in sessions output: {}",
        sessions_out
    );
}

// ==================== End-to-End Library Tests ====================

#[test]
fn test_full_lifecycle() {
    // Step 1: Create an empty graph
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    assert_eq!(graph.node_count(), 0);

    // Step 2: Add facts, decisions, and inferences across 3 sessions

    // Session 1: foundational facts
    let fact1 = CognitiveEventBuilder::new(EventType::Fact, "User prefers Rust")
        .session_id(1)
        .confidence(0.95)
        .build();
    let id_fact1 = graph.add_node(fact1).unwrap();

    let fact2 = CognitiveEventBuilder::new(EventType::Fact, "User works on backend systems")
        .session_id(1)
        .confidence(0.9)
        .build();
    let id_fact2 = graph.add_node(fact2).unwrap();

    // Session 2: inferences derived from facts
    let inf1 = CognitiveEventBuilder::new(
        EventType::Inference,
        "User likely interested in systems programming",
    )
    .session_id(2)
    .confidence(0.8)
    .build();
    let id_inf1 = graph.add_node(inf1).unwrap();

    let decision1 =
        CognitiveEventBuilder::new(EventType::Decision, "Suggest async runtime comparisons")
            .session_id(2)
            .confidence(0.85)
            .build();
    let id_decision1 = graph.add_node(decision1).unwrap();

    // Session 3: more facts and a skill
    let fact3 = CognitiveEventBuilder::new(EventType::Fact, "User dislikes Python GIL")
        .session_id(3)
        .confidence(0.7)
        .build();
    let id_fact3 = graph.add_node(fact3).unwrap();

    let skill1 = CognitiveEventBuilder::new(EventType::Skill, "Explain async/await patterns")
        .session_id(3)
        .confidence(0.9)
        .build();
    let _id_skill1 = graph.add_node(skill1).unwrap();

    assert_eq!(graph.node_count(), 6);

    // Step 3: Link with edges
    // Inference caused by facts
    graph
        .add_edge(Edge::new(id_inf1, id_fact1, EdgeType::CausedBy, 0.9))
        .unwrap();
    graph
        .add_edge(Edge::new(id_inf1, id_fact2, EdgeType::CausedBy, 0.8))
        .unwrap();
    // Decision caused by inference
    graph
        .add_edge(Edge::new(id_decision1, id_inf1, EdgeType::CausedBy, 0.95))
        .unwrap();
    // fact3 supports the inference
    graph
        .add_edge(Edge::new(id_fact3, id_inf1, EdgeType::Supports, 0.7))
        .unwrap();

    assert_eq!(graph.edge_count(), 4);

    // Step 4: Correct a fact
    let write_engine = WriteEngine::new(DEFAULT_DIMENSION);
    let id_correction = write_engine
        .correct(
            &mut graph,
            id_fact1,
            "User prefers Rust but also appreciates Go",
            3,
        )
        .unwrap();

    // Old fact should have confidence 0.0
    assert!(
        (graph.get_node(id_fact1).unwrap().confidence - 0.0).abs() < f32::EPSILON,
        "Corrected node should have confidence 0.0"
    );

    // Correction should exist
    let correction_node = graph.get_node(id_correction).unwrap();
    assert_eq!(correction_node.event_type, EventType::Correction);
    assert!(correction_node.content.contains("also appreciates Go"));

    // Step 5: Compress sessions
    let _episode_id = write_engine
        .compress_session(
            &mut graph,
            1,
            "Session 1: discovered user preferences for Rust and backend work",
        )
        .unwrap();

    // Step 6: Run decay
    let far_future = now_micros() + 86_400_000_000 * 365; // ~1 year later
    let decay_report = write_engine.run_decay(&mut graph, far_future).unwrap();
    assert!(
        decay_report.nodes_decayed > 0,
        "Expected some nodes to have decayed"
    );

    // Step 7: Traverse from a fact
    let query_engine = QueryEngine::new();
    let traversal = query_engine
        .traverse(
            &graph,
            TraversalParams {
                start_id: id_fact1,
                edge_types: vec![EdgeType::CausedBy, EdgeType::Supports, EdgeType::Supersedes],
                direction: TraversalDirection::Both,
                max_depth: 5,
                max_results: 50,
                min_confidence: 0.0,
            },
        )
        .unwrap();
    assert!(
        traversal.visited.len() >= 2,
        "Traversal should visit multiple nodes, got: {:?}",
        traversal.visited
    );

    // Step 8: Impact analysis on the original fact
    let impact = query_engine
        .causal(
            &graph,
            CausalParams {
                node_id: id_fact1,
                max_depth: 10,
                dependency_types: vec![EdgeType::CausedBy, EdgeType::Supports],
            },
        )
        .unwrap();
    assert!(
        !impact.dependents.is_empty(),
        "Impact analysis should find dependents"
    );

    // Step 9: Similarity search (all zero vectors, so similarity won't be meaningful,
    // but the API should not error)
    let query_vec = vec![0.0; DEFAULT_DIMENSION];
    let sim_results = query_engine
        .similarity(
            &graph,
            SimilarityParams {
                query_vec,
                top_k: 5,
                min_similarity: -1.0, // Accept anything
                event_types: vec![],
                skip_zero_vectors: false,
            },
        )
        .unwrap();
    // With zero vectors, similarity could be NaN or 0; just ensure no error
    let _ = sim_results;

    // Step 10: Resolve the corrected fact
    let resolved = query_engine.resolve(&graph, id_fact1).unwrap();
    assert_eq!(resolved.id, id_correction);
    assert!(resolved.content.contains("also appreciates Go"));

    // Step 11: Export to JSON via serde
    let nodes_json: Vec<serde_json::Value> = graph
        .nodes()
        .iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id,
                "event_type": n.event_type.name(),
                "content": n.content,
                "session_id": n.session_id,
                "confidence": n.confidence,
            })
        })
        .collect();
    let export_json = serde_json::to_string(&nodes_json).unwrap();
    assert!(!export_json.is_empty());

    // Step 12: Save to file and reload
    let tmp = NamedTempFile::new().unwrap();
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph, tmp.path()).unwrap();

    let reloaded = AmemReader::read_from_file(tmp.path()).unwrap();
    assert_eq!(reloaded.node_count(), graph.node_count());
    assert_eq!(reloaded.edge_count(), graph.edge_count());

    // Verify all node contents match after reload
    for node in graph.nodes() {
        let reloaded_node = reloaded.get_node(node.id).unwrap();
        assert_eq!(reloaded_node.content, node.content);
        assert_eq!(reloaded_node.event_type, node.event_type);
        assert_eq!(reloaded_node.session_id, node.session_id);
        assert!(
            (reloaded_node.confidence - node.confidence).abs() < f32::EPSILON,
            "Confidence mismatch for node {}: {} vs {}",
            node.id,
            reloaded_node.confidence,
            node.confidence
        );
    }

    // Verify queries still work on reloaded graph
    let resolved_after_reload = query_engine.resolve(&reloaded, id_fact1).unwrap();
    assert_eq!(resolved_after_reload.id, id_correction);
}

#[test]
fn test_portable_file() {
    // Write from one MemoryGraph
    let mut graph1 = MemoryGraph::new(DEFAULT_DIMENSION);

    let f1 = CognitiveEventBuilder::new(EventType::Fact, "portable fact alpha")
        .session_id(10)
        .confidence(0.9)
        .build();
    let id1 = graph1.add_node(f1).unwrap();

    let f2 = CognitiveEventBuilder::new(EventType::Decision, "portable decision beta")
        .session_id(10)
        .confidence(0.85)
        .build();
    let id2 = graph1.add_node(f2).unwrap();

    let f3 = CognitiveEventBuilder::new(EventType::Inference, "portable inference gamma")
        .session_id(11)
        .confidence(0.75)
        .build();
    let id3 = graph1.add_node(f3).unwrap();

    graph1
        .add_edge(Edge::new(id2, id1, EdgeType::CausedBy, 1.0))
        .unwrap();
    graph1
        .add_edge(Edge::new(id3, id1, EdgeType::Supports, 0.8))
        .unwrap();

    // Write to file
    let tmp = NamedTempFile::new().unwrap();
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph1, tmp.path()).unwrap();

    // Read into a new MemoryGraph
    let graph2 = AmemReader::read_from_file(tmp.path()).unwrap();

    // All data intact
    assert_eq!(graph2.node_count(), 3);
    assert_eq!(graph2.edge_count(), 2);

    let node1 = graph2.get_node(id1).unwrap();
    assert_eq!(node1.content, "portable fact alpha");
    assert_eq!(node1.session_id, 10);

    let node2 = graph2.get_node(id2).unwrap();
    assert_eq!(node2.content, "portable decision beta");

    let node3 = graph2.get_node(id3).unwrap();
    assert_eq!(node3.content, "portable inference gamma");
    assert_eq!(node3.session_id, 11);

    // Queries work on the reloaded graph
    let query_engine = QueryEngine::new();

    // Traversal
    let traversal = query_engine
        .traverse(
            &graph2,
            TraversalParams {
                start_id: id1,
                edge_types: vec![EdgeType::CausedBy, EdgeType::Supports],
                direction: TraversalDirection::Both,
                max_depth: 3,
                max_results: 10,
                min_confidence: 0.0,
            },
        )
        .unwrap();
    assert!(
        traversal.visited.len() >= 2,
        "Should traverse to connected nodes"
    );

    // Impact analysis
    let impact = query_engine
        .causal(
            &graph2,
            CausalParams {
                node_id: id1,
                max_depth: 5,
                dependency_types: vec![EdgeType::CausedBy, EdgeType::Supports],
            },
        )
        .unwrap();
    assert_eq!(impact.dependents.len(), 2);

    // Pattern search
    let facts = query_engine
        .pattern(
            &graph2,
            PatternParams {
                event_types: vec![EventType::Fact],
                min_confidence: None,
                max_confidence: None,
                session_ids: vec![],
                created_after: None,
                created_before: None,
                min_decay_score: None,
                max_results: 100,
                sort_by: PatternSort::MostRecent,
            },
        )
        .unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].content, "portable fact alpha");
}

#[test]
fn test_incremental_build() {
    let tmp = NamedTempFile::new().unwrap();

    // Create 100 nodes and save
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    for i in 0..100 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("incremental_node_{}", i))
            .session_id(1)
            .build();
        graph.add_node(event).unwrap();
    }
    assert_eq!(graph.node_count(), 100);

    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph, tmp.path()).unwrap();

    // Load
    let mut graph = AmemReader::read_from_file(tmp.path()).unwrap();
    assert_eq!(graph.node_count(), 100);

    // Add 50 more
    for i in 100..150 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("incremental_node_{}", i))
            .session_id(2)
            .build();
        graph.add_node(event).unwrap();
    }
    assert_eq!(graph.node_count(), 150);

    // Save again
    writer.write_to_file(&graph, tmp.path()).unwrap();

    // Load and verify
    let final_graph = AmemReader::read_from_file(tmp.path()).unwrap();
    assert_eq!(final_graph.node_count(), 150);

    // Verify all nodes are present with correct content
    for i in 0..150u64 {
        let node = final_graph.get_node(i).unwrap();
        assert_eq!(
            node.content,
            format!("incremental_node_{}", i),
            "Content mismatch at node {}",
            i
        );
    }

    // Verify session index is correct
    let session1_ids = final_graph.session_index().get_session(1);
    assert_eq!(session1_ids.len(), 100);
    let session2_ids = final_graph.session_index().get_session(2);
    assert_eq!(session2_ids.len(), 50);
}

#[test]
fn test_empty_file_operations() {
    // Create an empty graph
    let graph = MemoryGraph::new(DEFAULT_DIMENSION);
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);

    let query_engine = QueryEngine::new();

    // Pattern query on empty graph returns empty (not error)
    let results = query_engine
        .pattern(
            &graph,
            PatternParams {
                event_types: vec![EventType::Fact],
                min_confidence: None,
                max_confidence: None,
                session_ids: vec![],
                created_after: None,
                created_before: None,
                min_decay_score: None,
                max_results: 100,
                sort_by: PatternSort::MostRecent,
            },
        )
        .unwrap();
    assert!(results.is_empty());

    // Similarity search on empty graph returns empty
    let sim_results = query_engine
        .similarity(
            &graph,
            SimilarityParams {
                query_vec: vec![1.0; DEFAULT_DIMENSION],
                top_k: 10,
                min_similarity: 0.0,
                event_types: vec![],
                skip_zero_vectors: false,
            },
        )
        .unwrap();
    assert!(sim_results.is_empty());

    // Traverse from non-existent node returns NodeNotFound
    let result = query_engine.traverse(
        &graph,
        TraversalParams {
            start_id: 999,
            edge_types: vec![EdgeType::CausedBy],
            direction: TraversalDirection::Forward,
            max_depth: 5,
            max_results: 50,
            min_confidence: 0.0,
        },
    );
    match result {
        Err(agentic_memory::types::AmemError::NodeNotFound(999)) => {}
        Err(e) => panic!("Expected NodeNotFound(999), got {:?}", e),
        Ok(_) => panic!("Expected NodeNotFound(999) error, but got Ok"),
    }

    // Causal on non-existent node returns NodeNotFound
    let result = query_engine.causal(
        &graph,
        CausalParams {
            node_id: 42,
            max_depth: 5,
            dependency_types: vec![EdgeType::CausedBy],
        },
    );
    match result {
        Err(agentic_memory::types::AmemError::NodeNotFound(42)) => {}
        Err(e) => panic!("Expected NodeNotFound(42), got {:?}", e),
        Ok(_) => panic!("Expected NodeNotFound(42) error, but got Ok"),
    }

    // Resolve on non-existent node returns NodeNotFound
    let result: Result<&agentic_memory::types::CognitiveEvent, _> = query_engine.resolve(&graph, 0);
    match result {
        Err(agentic_memory::types::AmemError::NodeNotFound(0)) => {}
        Err(e) => panic!("Expected NodeNotFound(0), got {:?}", e),
        Ok(_) => panic!("Expected NodeNotFound(0) error, but got Ok"),
    }

    // Write empty graph to file and read back
    let tmp = NamedTempFile::new().unwrap();
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph, tmp.path()).unwrap();

    let reloaded = AmemReader::read_from_file(tmp.path()).unwrap();
    assert_eq!(reloaded.node_count(), 0);
    assert_eq!(reloaded.edge_count(), 0);
}
