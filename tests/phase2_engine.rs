//! Phase 2 tests: Write Engine + Query Engine.

use agentic_memory::engine::query::{
    CausalParams, PatternParams, PatternSort, QueryEngine, SimilarityParams, TemporalParams,
    TimeRange, TraversalParams,
};
use agentic_memory::engine::write::WriteEngine;
use agentic_memory::graph::traversal::TraversalDirection;
use agentic_memory::graph::MemoryGraph;
use agentic_memory::types::edge::{Edge, EdgeType};
use agentic_memory::types::error::AmemError;
use agentic_memory::types::event::{CognitiveEventBuilder, EventType};
use agentic_memory::types::{DEFAULT_DIMENSION, MAX_CONTENT_SIZE};

// ==================== Helper ====================

/// Create a zero feature vector of graph dimension.
fn zero_vec() -> Vec<f32> {
    vec![0.0; DEFAULT_DIMENSION]
}

/// Create a feature vector with a single non-zero element at the given index.
fn basis_vec(index: usize, value: f32) -> Vec<f32> {
    let mut v = vec![0.0; DEFAULT_DIMENSION];
    v[index] = value;
    v
}

// ==================== Write Engine Tests ====================

#[test]
fn test_ingest_single_event() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    let event = CognitiveEventBuilder::new(EventType::Fact, "The sky is blue")
        .session_id(1)
        .confidence(0.9)
        .feature_vec(zero_vec())
        .build();

    let result = engine.ingest(&mut graph, vec![event], vec![]).unwrap();

    assert_eq!(result.new_node_ids.len(), 1);
    assert_eq!(graph.node_count(), 1);
    let node = graph.get_node(result.new_node_ids[0]).unwrap();
    assert_eq!(node.content, "The sky is blue");
}

#[test]
fn test_ingest_batch() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    let mut events = Vec::new();
    for i in 0..50 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("fact_{}", i))
            .session_id(1)
            .confidence(0.9)
            .feature_vec(zero_vec())
            .build();
        events.push(event);
    }

    // Ingest all 50 events
    let result = engine.ingest(&mut graph, events, vec![]).unwrap();

    assert_eq!(result.new_node_ids.len(), 50);
    assert_eq!(graph.node_count(), 50);

    // Add edges connecting them in a chain after they exist
    let ids = &result.new_node_ids;
    for i in 0..49 {
        let edge = Edge::new(ids[i], ids[i + 1], EdgeType::TemporalNext, 1.0);
        graph.add_edge(edge).unwrap();
    }

    // Verify all nodes present and edges connected
    for &id in ids {
        assert!(graph.get_node(id).is_some());
    }
    // Chain of 49 edges
    assert_eq!(graph.edge_count(), 49);
}

#[test]
fn test_correct_node() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    // Add original fact
    let event = CognitiveEventBuilder::new(EventType::Fact, "Earth has one moon")
        .session_id(1)
        .confidence(0.9)
        .feature_vec(zero_vec())
        .build();
    let ingest_result = engine.ingest(&mut graph, vec![event], vec![]).unwrap();
    let old_id = ingest_result.new_node_ids[0];

    // Correct it
    let new_id = engine
        .correct(&mut graph, old_id, "Earth has one natural moon", 2)
        .unwrap();

    // Old node's confidence is 0.0
    let old_node = graph.get_node(old_id).unwrap();
    assert!((old_node.confidence - 0.0).abs() < f32::EPSILON);

    // New correction node exists
    let new_node = graph.get_node(new_id).unwrap();
    assert_eq!(new_node.event_type, EventType::Correction);
    assert_eq!(new_node.content, "Earth has one natural moon");

    // SUPERSEDES edge from new to old
    let edges_from_new = graph.edges_from(new_id);
    assert_eq!(edges_from_new.len(), 1);
    assert_eq!(edges_from_new[0].edge_type, EdgeType::Supersedes);
    assert_eq!(edges_from_new[0].source_id, new_id);
    assert_eq!(edges_from_new[0].target_id, old_id);
}

#[test]
fn test_compress_session() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    // Add 10 nodes in session 5
    let mut events = Vec::new();
    for i in 0..10 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("session5_fact_{}", i))
            .session_id(5)
            .confidence(0.8)
            .feature_vec(zero_vec())
            .build();
        events.push(event);
    }
    let ingest_result = engine.ingest(&mut graph, events, vec![]).unwrap();
    let session_node_ids: Vec<u64> = ingest_result.new_node_ids.clone();

    // Compress session 5
    let episode_id = engine
        .compress_session(&mut graph, 5, "Summary of session 5")
        .unwrap();

    // Episode node exists and is an Episode type
    let episode_node = graph.get_node(episode_id).unwrap();
    assert_eq!(episode_node.event_type, EventType::Episode);
    assert_eq!(episode_node.content, "Summary of session 5");

    // PART_OF edges from all 10 original nodes to the episode
    let edges_to_episode = graph.edges_to(episode_id);
    assert_eq!(edges_to_episode.len(), 10);
    for edge in &edges_to_episode {
        assert_eq!(edge.edge_type, EdgeType::PartOf);
        assert_eq!(edge.target_id, episode_id);
        assert!(session_node_ids.contains(&edge.source_id));
    }

    // Original nodes still exist
    for &id in &session_node_ids {
        assert!(graph.get_node(id).is_some());
    }
}

#[test]
fn test_touch_updates_access() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    let event = CognitiveEventBuilder::new(EventType::Fact, "touchable fact")
        .session_id(1)
        .feature_vec(zero_vec())
        .build();
    let ingest_result = engine.ingest(&mut graph, vec![event], vec![]).unwrap();
    let node_id = ingest_result.new_node_ids[0];

    // Initially access_count is 0
    let node = graph.get_node(node_id).unwrap();
    assert_eq!(node.access_count, 0);
    let original_last_accessed = node.last_accessed;

    // Small sleep to ensure time advances (microsecond precision)
    std::thread::sleep(std::time::Duration::from_millis(2));

    // Touch it
    engine.touch(&mut graph, node_id).unwrap();

    let node = graph.get_node(node_id).unwrap();
    assert_eq!(node.access_count, 1);
    assert!(node.last_accessed >= original_last_accessed);
}

#[test]
fn test_decay_calculation() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    let micros_per_day: u64 = 86_400_000_000;
    let now: u64 = 100 * micros_per_day; // day 100

    // Node A: recent, frequently accessed (created day 99, accessed many times)
    let event_a = CognitiveEventBuilder::new(EventType::Fact, "recent and popular")
        .session_id(1)
        .confidence(1.0)
        .created_at(99 * micros_per_day)
        .feature_vec(zero_vec())
        .build();

    // Node B: old, never accessed (created day 1)
    let event_b = CognitiveEventBuilder::new(EventType::Fact, "old and forgotten")
        .session_id(1)
        .confidence(1.0)
        .created_at(1 * micros_per_day)
        .feature_vec(zero_vec())
        .build();

    let result = engine
        .ingest(&mut graph, vec![event_a, event_b], vec![])
        .unwrap();
    let id_a = result.new_node_ids[0];
    let id_b = result.new_node_ids[1];

    // Touch node A many times to boost its access_count
    for _ in 0..20 {
        engine.touch(&mut graph, id_a).unwrap();
    }

    // Run decay
    engine.run_decay(&mut graph, now).unwrap();

    let node_a = graph.get_node(id_a).unwrap();
    let node_b = graph.get_node(id_b).unwrap();

    // Node A (recent, frequently accessed) should have higher decay score than Node B
    assert!(
        node_a.decay_score > node_b.decay_score,
        "Node A decay_score ({}) should be > Node B decay_score ({})",
        node_a.decay_score,
        node_b.decay_score
    );

    // Node B should have a low decay score (old, 0 access)
    assert!(
        node_b.decay_score < 0.2,
        "Old unaccessed node should have low decay_score, got {}",
        node_b.decay_score
    );
}

#[test]
fn test_decay_never_deletes() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    let micros_per_day: u64 = 86_400_000_000;

    // Add some nodes
    let mut events = Vec::new();
    for i in 0..5 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("fact_{}", i))
            .session_id(1)
            .created_at((i as u64 + 1) * micros_per_day)
            .feature_vec(zero_vec())
            .build();
        events.push(event);
    }
    engine.ingest(&mut graph, events, vec![]).unwrap();
    let count_before = graph.node_count();

    // Run decay far into the future
    let report = engine
        .run_decay(&mut graph, 10000 * micros_per_day)
        .unwrap();

    // No nodes removed
    assert_eq!(graph.node_count(), count_before);

    // But decay scores were updated
    assert!(report.nodes_decayed > 0);

    // Verify only decay_scores changed (no deletions)
    for node in graph.nodes() {
        assert!(node.decay_score >= 0.0);
        assert!(node.decay_score <= 1.0);
    }
}

#[test]
fn test_ingest_validates_content_size() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    let oversized_content = "x".repeat(MAX_CONTENT_SIZE + 1);
    let event = CognitiveEventBuilder::new(EventType::Fact, oversized_content)
        .session_id(1)
        .feature_vec(zero_vec())
        .build();

    let result = engine.ingest(&mut graph, vec![event], vec![]);
    assert!(result.is_err());
    let err = result.err().unwrap();
    match err {
        AmemError::ContentTooLarge { size, max } => {
            assert!(size > MAX_CONTENT_SIZE);
            assert_eq!(max, MAX_CONTENT_SIZE);
        }
        e => panic!("Expected ContentTooLarge, got {:?}", e),
    }
}

#[test]
fn test_ingest_validates_dimension() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    // Wrong dimension feature vector
    let event = CognitiveEventBuilder::new(EventType::Fact, "test")
        .session_id(1)
        .feature_vec(vec![1.0; DEFAULT_DIMENSION + 10]) // wrong size
        .build();

    let result = engine.ingest(&mut graph, vec![event], vec![]);
    assert!(result.is_err());
    let err = result.err().unwrap();
    match err {
        AmemError::DimensionMismatch { expected, got } => {
            assert_eq!(expected, DEFAULT_DIMENSION);
            assert_eq!(got, DEFAULT_DIMENSION + 10);
        }
        e => panic!("Expected DimensionMismatch, got {:?}", e),
    }
}

// ==================== Query Engine: Traversal Tests ====================

/// Build a linear chain A -> B -> C with CausedBy edges.
/// Returns (graph, [id_a, id_b, id_c]).
fn build_chain_abc() -> (MemoryGraph, [u64; 3]) {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    let a = CognitiveEventBuilder::new(EventType::Fact, "A")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let b = CognitiveEventBuilder::new(EventType::Fact, "B")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let c = CognitiveEventBuilder::new(EventType::Fact, "C")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();

    let id_a = graph.add_node(a).unwrap();
    let id_b = graph.add_node(b).unwrap();
    let id_c = graph.add_node(c).unwrap();

    // A -> B -> C with CausedBy edges
    graph
        .add_edge(Edge::new(id_a, id_b, EdgeType::CausedBy, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(id_b, id_c, EdgeType::CausedBy, 1.0))
        .unwrap();

    (graph, [id_a, id_b, id_c])
}

#[test]
fn test_traverse_forward() {
    let (graph, [id_a, id_b, id_c]) = build_chain_abc();
    let qe = QueryEngine::new();

    let result = qe
        .traverse(
            &graph,
            TraversalParams {
                start_id: id_a,
                edge_types: vec![EdgeType::CausedBy],
                direction: TraversalDirection::Forward,
                max_depth: 10,
                max_results: 100,
                min_confidence: 0.0,
            },
        )
        .unwrap();

    // Should visit A (start), then B, then C
    assert!(result.visited.contains(&id_a));
    assert!(result.visited.contains(&id_b));
    assert!(result.visited.contains(&id_c));
    assert_eq!(result.visited.len(), 3);
}

#[test]
fn test_traverse_backward() {
    let (graph, [id_a, id_b, id_c]) = build_chain_abc();
    let qe = QueryEngine::new();

    let result = qe
        .traverse(
            &graph,
            TraversalParams {
                start_id: id_c,
                edge_types: vec![EdgeType::CausedBy],
                direction: TraversalDirection::Backward,
                max_depth: 10,
                max_results: 100,
                min_confidence: 0.0,
            },
        )
        .unwrap();

    // Backward from C: visits C (start), then B (source of B->C), then A (source of A->B)
    assert!(result.visited.contains(&id_c));
    assert!(result.visited.contains(&id_b));
    assert!(result.visited.contains(&id_a));
    assert_eq!(result.visited.len(), 3);
}

#[test]
fn test_traverse_max_depth() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Create chain of 10 nodes: 0 -> 1 -> 2 -> ... -> 9
    let mut ids = Vec::new();
    for i in 0..10 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("node_{}", i))
            .session_id(1)
            .confidence(1.0)
            .feature_vec(zero_vec())
            .build();
        ids.push(graph.add_node(event).unwrap());
    }
    for i in 0..9 {
        graph
            .add_edge(Edge::new(ids[i], ids[i + 1], EdgeType::CausedBy, 1.0))
            .unwrap();
    }

    let qe = QueryEngine::new();
    let result = qe
        .traverse(
            &graph,
            TraversalParams {
                start_id: ids[0],
                edge_types: vec![EdgeType::CausedBy],
                direction: TraversalDirection::Forward,
                max_depth: 3,
                max_results: 100,
                min_confidence: 0.0,
            },
        )
        .unwrap();

    // Start node (depth 0) + 3 levels: nodes 0, 1, 2, 3
    assert_eq!(result.visited.len(), 4);
    assert!(result.visited.contains(&ids[0]));
    assert!(result.visited.contains(&ids[1]));
    assert!(result.visited.contains(&ids[2]));
    assert!(result.visited.contains(&ids[3]));
    // Should NOT have node at depth 4+
    assert!(!result.visited.contains(&ids[4]));
}

#[test]
fn test_traverse_respects_edge_type_filter() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    let a = CognitiveEventBuilder::new(EventType::Fact, "A")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let b = CognitiveEventBuilder::new(EventType::Fact, "B")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let c = CognitiveEventBuilder::new(EventType::Fact, "C")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();

    let id_a = graph.add_node(a).unwrap();
    let id_b = graph.add_node(b).unwrap();
    let id_c = graph.add_node(c).unwrap();

    // A -> B via CausedBy, A -> C via Supports
    graph
        .add_edge(Edge::new(id_a, id_b, EdgeType::CausedBy, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(id_a, id_c, EdgeType::Supports, 1.0))
        .unwrap();

    let qe = QueryEngine::new();
    let result = qe
        .traverse(
            &graph,
            TraversalParams {
                start_id: id_a,
                edge_types: vec![EdgeType::CausedBy], // Only follow CausedBy
                direction: TraversalDirection::Forward,
                max_depth: 10,
                max_results: 100,
                min_confidence: 0.0,
            },
        )
        .unwrap();

    assert!(result.visited.contains(&id_a));
    assert!(result.visited.contains(&id_b));
    assert!(
        !result.visited.contains(&id_c),
        "Should not follow Supports edge"
    );
}

#[test]
fn test_traverse_handles_cycles() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    let a = CognitiveEventBuilder::new(EventType::Fact, "A")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let b = CognitiveEventBuilder::new(EventType::Fact, "B")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let c = CognitiveEventBuilder::new(EventType::Fact, "C")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();

    let id_a = graph.add_node(a).unwrap();
    let id_b = graph.add_node(b).unwrap();
    let id_c = graph.add_node(c).unwrap();

    // A -> B -> C -> A (cycle)
    graph
        .add_edge(Edge::new(id_a, id_b, EdgeType::CausedBy, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(id_b, id_c, EdgeType::CausedBy, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(id_c, id_a, EdgeType::CausedBy, 1.0))
        .unwrap();

    let qe = QueryEngine::new();
    let result = qe
        .traverse(
            &graph,
            TraversalParams {
                start_id: id_a,
                edge_types: vec![EdgeType::CausedBy],
                direction: TraversalDirection::Forward,
                max_depth: 100,
                max_results: 100,
                min_confidence: 0.0,
            },
        )
        .unwrap();

    // No infinite loop; each visited exactly once
    assert_eq!(result.visited.len(), 3);
    assert!(result.visited.contains(&id_a));
    assert!(result.visited.contains(&id_b));
    assert!(result.visited.contains(&id_c));
}

#[test]
fn test_traverse_min_confidence() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    let a = CognitiveEventBuilder::new(EventType::Fact, "A")
        .session_id(1)
        .confidence(0.9)
        .feature_vec(zero_vec())
        .build();
    let b = CognitiveEventBuilder::new(EventType::Fact, "B-low-conf")
        .session_id(1)
        .confidence(0.1) // low confidence
        .feature_vec(zero_vec())
        .build();
    let c = CognitiveEventBuilder::new(EventType::Fact, "C")
        .session_id(1)
        .confidence(0.9)
        .feature_vec(zero_vec())
        .build();

    let id_a = graph.add_node(a).unwrap();
    let id_b = graph.add_node(b).unwrap();
    let id_c = graph.add_node(c).unwrap();

    // A -> B -> C
    graph
        .add_edge(Edge::new(id_a, id_b, EdgeType::CausedBy, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(id_b, id_c, EdgeType::CausedBy, 1.0))
        .unwrap();

    let qe = QueryEngine::new();
    let result = qe
        .traverse(
            &graph,
            TraversalParams {
                start_id: id_a,
                edge_types: vec![EdgeType::CausedBy],
                direction: TraversalDirection::Forward,
                max_depth: 10,
                max_results: 100,
                min_confidence: 0.5,
            },
        )
        .unwrap();

    // B is skipped due to low confidence; C is unreachable through B
    assert!(result.visited.contains(&id_a));
    assert!(
        !result.visited.contains(&id_b),
        "B should be skipped (confidence 0.1 < 0.5)"
    );
}

// ==================== Query Engine: Pattern Tests ====================

#[test]
fn test_pattern_by_type() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Mix of types
    for i in 0..5 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("fact_{}", i))
            .session_id(1)
            .feature_vec(zero_vec())
            .build();
        graph.add_node(event).unwrap();
    }
    for i in 0..3 {
        let event = CognitiveEventBuilder::new(EventType::Decision, format!("decision_{}", i))
            .session_id(1)
            .feature_vec(zero_vec())
            .build();
        graph.add_node(event).unwrap();
    }

    let qe = QueryEngine::new();
    let results = qe
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

    assert_eq!(results.len(), 5);
    for r in &results {
        assert_eq!(r.event_type, EventType::Fact);
    }
}

#[test]
fn test_pattern_by_session() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Nodes in sessions 1, 2, 3
    for session in 1..=3u32 {
        for i in 0..4 {
            let event =
                CognitiveEventBuilder::new(EventType::Fact, format!("s{}_fact_{}", session, i))
                    .session_id(session)
                    .feature_vec(zero_vec())
                    .build();
            graph.add_node(event).unwrap();
        }
    }

    let qe = QueryEngine::new();
    let results = qe
        .pattern(
            &graph,
            PatternParams {
                event_types: vec![],
                min_confidence: None,
                max_confidence: None,
                session_ids: vec![2],
                created_after: None,
                created_before: None,
                min_decay_score: None,
                max_results: 100,
                sort_by: PatternSort::MostRecent,
            },
        )
        .unwrap();

    assert_eq!(results.len(), 4);
    for r in &results {
        assert_eq!(r.session_id, 2);
    }
}

#[test]
fn test_pattern_by_time_range() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Nodes at timestamps 1000000, 2000000, 3000000, 4000000, 5000000
    for i in 1..=5u64 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("fact_t{}", i))
            .session_id(1)
            .created_at(i * 1_000_000)
            .feature_vec(zero_vec())
            .build();
        graph.add_node(event).unwrap();
    }

    let qe = QueryEngine::new();
    let results = qe
        .pattern(
            &graph,
            PatternParams {
                event_types: vec![],
                min_confidence: None,
                max_confidence: None,
                session_ids: vec![],
                created_after: Some(2_000_000),
                created_before: Some(4_000_000),
                min_decay_score: None,
                max_results: 100,
                sort_by: PatternSort::MostRecent,
            },
        )
        .unwrap();

    // Should include timestamps 2000000, 3000000, 4000000 (inclusive on both ends)
    assert_eq!(results.len(), 3);
    for r in &results {
        assert!(r.created_at >= 2_000_000);
        assert!(r.created_at <= 4_000_000);
    }
}

#[test]
fn test_pattern_by_confidence_range() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Nodes with confidence 0.1, 0.2, 0.3, ..., 1.0
    for i in 1..=10 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("fact_{}", i))
            .session_id(1)
            .confidence(i as f32 / 10.0)
            .feature_vec(zero_vec())
            .build();
        graph.add_node(event).unwrap();
    }

    let qe = QueryEngine::new();
    let results = qe
        .pattern(
            &graph,
            PatternParams {
                event_types: vec![],
                min_confidence: Some(0.3),
                max_confidence: Some(0.7),
                session_ids: vec![],
                created_after: None,
                created_before: None,
                min_decay_score: None,
                max_results: 100,
                sort_by: PatternSort::MostRecent,
            },
        )
        .unwrap();

    // Should include 0.3, 0.4, 0.5, 0.6, 0.7
    assert_eq!(results.len(), 5);
    for r in &results {
        assert!(r.confidence >= 0.3 - f32::EPSILON);
        assert!(r.confidence <= 0.7 + f32::EPSILON);
    }
}

#[test]
fn test_pattern_sort_recent() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    for i in 1..=5u64 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("fact_{}", i))
            .session_id(1)
            .created_at(i * 1_000_000)
            .feature_vec(zero_vec())
            .build();
        graph.add_node(event).unwrap();
    }

    let qe = QueryEngine::new();
    let results = qe
        .pattern(
            &graph,
            PatternParams {
                event_types: vec![],
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

    // Descending timestamp order
    for window in results.windows(2) {
        assert!(
            window[0].created_at >= window[1].created_at,
            "Expected descending timestamp order"
        );
    }
}

#[test]
fn test_pattern_sort_confidence() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    for i in 1..=5 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("fact_{}", i))
            .session_id(1)
            .confidence(i as f32 / 10.0)
            .feature_vec(zero_vec())
            .build();
        graph.add_node(event).unwrap();
    }

    let qe = QueryEngine::new();
    let results = qe
        .pattern(
            &graph,
            PatternParams {
                event_types: vec![],
                min_confidence: None,
                max_confidence: None,
                session_ids: vec![],
                created_after: None,
                created_before: None,
                min_decay_score: None,
                max_results: 100,
                sort_by: PatternSort::HighestConfidence,
            },
        )
        .unwrap();

    // Descending confidence order
    for window in results.windows(2) {
        assert!(
            window[0].confidence >= window[1].confidence,
            "Expected descending confidence order: {} >= {}",
            window[0].confidence,
            window[1].confidence
        );
    }
}

#[test]
fn test_pattern_max_results() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    for i in 0..100 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("fact_{}", i))
            .session_id(1)
            .feature_vec(zero_vec())
            .build();
        graph.add_node(event).unwrap();
    }

    let qe = QueryEngine::new();
    let results = qe
        .pattern(
            &graph,
            PatternParams {
                event_types: vec![],
                min_confidence: None,
                max_confidence: None,
                session_ids: vec![],
                created_after: None,
                created_before: None,
                min_decay_score: None,
                max_results: 10,
                sort_by: PatternSort::MostRecent,
            },
        )
        .unwrap();

    assert_eq!(results.len(), 10);
}

// ==================== Query Engine: Temporal Tests ====================

#[test]
fn test_temporal_added_nodes() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Session 1: 3 facts
    for i in 0..3 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("s1_fact_{}", i))
            .session_id(1)
            .feature_vec(zero_vec())
            .build();
        graph.add_node(event).unwrap();
    }

    // Session 2: 2 more facts
    let mut session2_ids = Vec::new();
    for i in 0..2 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("s2_fact_{}", i))
            .session_id(2)
            .feature_vec(zero_vec())
            .build();
        session2_ids.push(graph.add_node(event).unwrap());
    }

    let qe = QueryEngine::new();
    let result = qe
        .temporal(
            &graph,
            TemporalParams {
                range_a: TimeRange::Session(1),
                range_b: TimeRange::Session(2),
            },
        )
        .unwrap();

    // Added list should contain the session 2 nodes (they are new relative to session 1)
    assert_eq!(result.added.len(), 2);
    for id in &session2_ids {
        assert!(
            result.added.contains(id),
            "Session 2 node {} should be in added list",
            id
        );
    }
}

#[test]
fn test_temporal_corrected_nodes() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    // Session 1: one fact
    let event = CognitiveEventBuilder::new(EventType::Fact, "original fact")
        .session_id(1)
        .feature_vec(zero_vec())
        .build();
    let ingest_result = engine.ingest(&mut graph, vec![event], vec![]).unwrap();
    let old_id = ingest_result.new_node_ids[0];

    // Session 2: correct that fact
    let new_id = engine
        .correct(&mut graph, old_id, "corrected fact", 2)
        .unwrap();

    let qe = QueryEngine::new();
    let result = qe
        .temporal(
            &graph,
            TemporalParams {
                range_a: TimeRange::Session(1),
                range_b: TimeRange::Session(2),
            },
        )
        .unwrap();

    // Should show the correction: (old_id, new_id)
    assert!(!result.corrected.is_empty(), "Should detect the correction");
    assert!(
        result.corrected.contains(&(old_id, new_id)),
        "corrected should contain ({}, {})",
        old_id,
        new_id
    );
}

// ==================== Query Engine: Causal Tests ====================

#[test]
fn test_causal_direct_dependents() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // A is a fact
    let a = CognitiveEventBuilder::new(EventType::Fact, "A fact")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    // B is a decision caused by A
    let b = CognitiveEventBuilder::new(EventType::Decision, "B decision")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();

    let id_a = graph.add_node(a).unwrap();
    let id_b = graph.add_node(b).unwrap();

    // B depends on A: edge from B -> A (CausedBy)
    graph
        .add_edge(Edge::new(id_b, id_a, EdgeType::CausedBy, 1.0))
        .unwrap();

    let qe = QueryEngine::new();
    let result = qe
        .causal(
            &graph,
            CausalParams {
                node_id: id_a,
                max_depth: 10,
                dependency_types: vec![EdgeType::CausedBy],
            },
        )
        .unwrap();

    // B should appear as a dependent of A
    assert!(
        result.dependents.contains(&id_b),
        "B should be a dependent of A"
    );
    assert_eq!(result.affected_decisions, 1);
}

#[test]
fn test_causal_transitive_dependents() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    let a = CognitiveEventBuilder::new(EventType::Fact, "A")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let b = CognitiveEventBuilder::new(EventType::Inference, "B")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let c = CognitiveEventBuilder::new(EventType::Decision, "C")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();

    let id_a = graph.add_node(a).unwrap();
    let id_b = graph.add_node(b).unwrap();
    let id_c = graph.add_node(c).unwrap();

    // B depends on A, C depends on B: B->A, C->B (CausedBy)
    graph
        .add_edge(Edge::new(id_b, id_a, EdgeType::CausedBy, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(id_c, id_b, EdgeType::CausedBy, 1.0))
        .unwrap();

    let qe = QueryEngine::new();
    let result = qe
        .causal(
            &graph,
            CausalParams {
                node_id: id_a,
                max_depth: 10,
                dependency_types: vec![EdgeType::CausedBy],
            },
        )
        .unwrap();

    // Both B and C are dependents of A (transitively)
    assert!(result.dependents.contains(&id_b));
    assert!(result.dependents.contains(&id_c));
    assert_eq!(result.dependents.len(), 2);
}

#[test]
fn test_causal_counts_decisions_and_inferences() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Root fact
    let root = CognitiveEventBuilder::new(EventType::Fact, "root")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let id_root = graph.add_node(root).unwrap();

    // 2 decisions depend on root
    let d1 = CognitiveEventBuilder::new(EventType::Decision, "decision 1")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let d2 = CognitiveEventBuilder::new(EventType::Decision, "decision 2")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let id_d1 = graph.add_node(d1).unwrap();
    let id_d2 = graph.add_node(d2).unwrap();

    // 3 inferences depend on root
    let i1 = CognitiveEventBuilder::new(EventType::Inference, "inference 1")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let i2 = CognitiveEventBuilder::new(EventType::Inference, "inference 2")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let i3 = CognitiveEventBuilder::new(EventType::Inference, "inference 3")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let id_i1 = graph.add_node(i1).unwrap();
    let id_i2 = graph.add_node(i2).unwrap();
    let id_i3 = graph.add_node(i3).unwrap();

    // 1 fact depends on root (should NOT count as decision or inference)
    let f1 = CognitiveEventBuilder::new(EventType::Fact, "derived fact")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let id_f1 = graph.add_node(f1).unwrap();

    // All depend on root via CausedBy
    for &dep_id in &[id_d1, id_d2, id_i1, id_i2, id_i3, id_f1] {
        graph
            .add_edge(Edge::new(dep_id, id_root, EdgeType::CausedBy, 1.0))
            .unwrap();
    }

    let qe = QueryEngine::new();
    let result = qe
        .causal(
            &graph,
            CausalParams {
                node_id: id_root,
                max_depth: 10,
                dependency_types: vec![EdgeType::CausedBy],
            },
        )
        .unwrap();

    assert_eq!(result.affected_decisions, 2);
    assert_eq!(result.affected_inferences, 3);
    assert_eq!(result.dependents.len(), 6);
}

// ==================== Query Engine: Similarity Tests ====================

#[test]
fn test_similarity_basic() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Node A: feature vec with large value at index 0
    let a = CognitiveEventBuilder::new(EventType::Fact, "A")
        .session_id(1)
        .feature_vec(basis_vec(0, 1.0))
        .build();
    // Node B: feature vec with large value at index 1 (orthogonal to A)
    let b = CognitiveEventBuilder::new(EventType::Fact, "B")
        .session_id(1)
        .feature_vec(basis_vec(1, 1.0))
        .build();

    let id_a = graph.add_node(a).unwrap();
    let _id_b = graph.add_node(b).unwrap();

    let qe = QueryEngine::new();
    // Query similar to A (index 0 = 1.0)
    let results = qe
        .similarity(
            &graph,
            SimilarityParams {
                query_vec: basis_vec(0, 1.0),
                top_k: 10,
                min_similarity: 0.0,
                event_types: vec![],
                skip_zero_vectors: false,
            },
        )
        .unwrap();

    // First result should be node A (highest similarity to query)
    assert!(!results.is_empty());
    assert_eq!(results[0].node_id, id_a);
    assert!(
        results[0].similarity > 0.9,
        "Expected high similarity for matching vector"
    );
}

#[test]
fn test_similarity_respects_threshold() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Node with basis_vec(0)
    let a = CognitiveEventBuilder::new(EventType::Fact, "A")
        .session_id(1)
        .feature_vec(basis_vec(0, 1.0))
        .build();
    // Node with basis_vec(1) -- orthogonal
    let b = CognitiveEventBuilder::new(EventType::Fact, "B")
        .session_id(1)
        .feature_vec(basis_vec(1, 1.0))
        .build();

    graph.add_node(a).unwrap();
    graph.add_node(b).unwrap();

    let qe = QueryEngine::new();
    let results = qe
        .similarity(
            &graph,
            SimilarityParams {
                query_vec: basis_vec(0, 1.0),
                top_k: 10,
                min_similarity: 0.9, // High threshold
                event_types: vec![],
                skip_zero_vectors: false,
            },
        )
        .unwrap();

    // Only node A should match (similarity ~ 1.0); B is orthogonal (similarity ~ 0.0)
    assert_eq!(results.len(), 1);
    assert!(results[0].similarity >= 0.9);
}

#[test]
fn test_similarity_skips_zero_vectors() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Node with a real feature vec
    let a = CognitiveEventBuilder::new(EventType::Fact, "A")
        .session_id(1)
        .feature_vec(basis_vec(0, 1.0))
        .build();
    // Node with zero vector
    let b = CognitiveEventBuilder::new(EventType::Fact, "B-zero")
        .session_id(1)
        .feature_vec(zero_vec())
        .build();

    let id_a = graph.add_node(a).unwrap();
    let id_b = graph.add_node(b).unwrap();

    let qe = QueryEngine::new();
    let results = qe
        .similarity(
            &graph,
            SimilarityParams {
                query_vec: basis_vec(0, 1.0),
                top_k: 10,
                min_similarity: 0.0,
                event_types: vec![],
                skip_zero_vectors: true,
            },
        )
        .unwrap();

    // Node B (zero vector) should be excluded
    let result_ids: Vec<u64> = results.iter().map(|r| r.node_id).collect();
    assert!(result_ids.contains(&id_a));
    assert!(
        !result_ids.contains(&id_b),
        "Zero vector node should be skipped"
    );
}

// ==================== Query Engine: Context Tests ====================

#[test]
fn test_context_subgraph() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    let a = CognitiveEventBuilder::new(EventType::Fact, "center")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let b = CognitiveEventBuilder::new(EventType::Fact, "neighbor1")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let c = CognitiveEventBuilder::new(EventType::Fact, "neighbor2")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();
    let d = CognitiveEventBuilder::new(EventType::Fact, "far_away")
        .session_id(1)
        .confidence(1.0)
        .feature_vec(zero_vec())
        .build();

    let id_a = graph.add_node(a).unwrap();
    let id_b = graph.add_node(b).unwrap();
    let id_c = graph.add_node(c).unwrap();
    let id_d = graph.add_node(d).unwrap();

    // A <-> B, A <-> C (depth 1 neighbors)
    graph
        .add_edge(Edge::new(id_a, id_b, EdgeType::RelatedTo, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(id_c, id_a, EdgeType::Supports, 1.0))
        .unwrap();
    // C -> D (depth 2 from A)
    graph
        .add_edge(Edge::new(id_c, id_d, EdgeType::RelatedTo, 1.0))
        .unwrap();

    let qe = QueryEngine::new();
    let subgraph = qe.context(&graph, id_a, 1).unwrap();

    assert_eq!(subgraph.center_id, id_a);

    let node_ids: Vec<u64> = subgraph.nodes.iter().map(|n| n.id).collect();
    // Should contain center and direct neighbors
    assert!(node_ids.contains(&id_a));
    assert!(node_ids.contains(&id_b));
    assert!(node_ids.contains(&id_c));
    // D is at depth 2, should NOT be included at depth=1
    assert!(
        !node_ids.contains(&id_d),
        "Node D is at depth 2, should not be in depth=1 subgraph"
    );
}

// ==================== Query Engine: Resolve Tests ====================

#[test]
fn test_resolve_no_supersedes() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    let event = CognitiveEventBuilder::new(EventType::Fact, "standalone")
        .session_id(1)
        .feature_vec(zero_vec())
        .build();
    let id = graph.add_node(event).unwrap();

    let qe = QueryEngine::new();
    let resolved = qe.resolve(&graph, id).unwrap();

    // Returns the node itself
    assert_eq!(resolved.id, id);
    assert_eq!(resolved.content, "standalone");
}

#[test]
fn test_resolve_single_supersedes() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    // A is superseded by B
    let a = CognitiveEventBuilder::new(EventType::Fact, "original")
        .session_id(1)
        .feature_vec(zero_vec())
        .build();
    let ingest_result = engine.ingest(&mut graph, vec![a], vec![]).unwrap();
    let id_a = ingest_result.new_node_ids[0];

    let id_b = engine.correct(&mut graph, id_a, "corrected", 2).unwrap();

    let qe = QueryEngine::new();
    let resolved = qe.resolve(&graph, id_a).unwrap();

    // Resolve(A) should return B
    assert_eq!(resolved.id, id_b);
    assert_eq!(resolved.content, "corrected");
}

#[test]
fn test_resolve_chain() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    // A superseded by B, B superseded by C
    let a = CognitiveEventBuilder::new(EventType::Fact, "version 1")
        .session_id(1)
        .feature_vec(zero_vec())
        .build();
    let ingest_result = engine.ingest(&mut graph, vec![a], vec![]).unwrap();
    let id_a = ingest_result.new_node_ids[0];

    let id_b = engine.correct(&mut graph, id_a, "version 2", 2).unwrap();
    let id_c = engine.correct(&mut graph, id_b, "version 3", 3).unwrap();

    let qe = QueryEngine::new();

    // Resolve(A) should follow the chain to C
    let resolved = qe.resolve(&graph, id_a).unwrap();
    assert_eq!(resolved.id, id_c);
    assert_eq!(resolved.content, "version 3");

    // Resolve(B) should also return C
    let resolved_b = qe.resolve(&graph, id_b).unwrap();
    assert_eq!(resolved_b.id, id_c);
}
