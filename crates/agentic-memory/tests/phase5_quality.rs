//! Phase 5 tests: memory quality analysis.

use agentic_memory::{
    CognitiveEventBuilder, Edge, EdgeType, EventType, MemoryGraph, MemoryQualityParams,
    QueryEngine, DEFAULT_DIMENSION,
};

fn zero_vec() -> Vec<f32> {
    vec![0.0; DEFAULT_DIMENSION]
}

#[test]
fn test_memory_quality_detects_structural_and_confidence_issues() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    let mut fact_a = CognitiveEventBuilder::new(EventType::Fact, "Primary fact")
        .session_id(1)
        .confidence(0.2)
        .feature_vec(zero_vec())
        .build();
    fact_a.decay_score = 0.1;
    let id_a = graph.add_node(fact_a).unwrap();

    let fact_b = CognitiveEventBuilder::new(EventType::Fact, "Secondary fact")
        .session_id(1)
        .confidence(0.9)
        .feature_vec(zero_vec())
        .build();
    let id_b = graph.add_node(fact_b).unwrap();

    let decision = CognitiveEventBuilder::new(EventType::Decision, "Ship now")
        .session_id(1)
        .confidence(0.8)
        .feature_vec(zero_vec())
        .build();
    let _id_decision = graph.add_node(decision).unwrap();

    // Contradiction edge between facts.
    graph
        .add_edge(Edge::new(id_a, id_b, EdgeType::Contradicts, 1.0))
        .unwrap();

    let report = QueryEngine::new()
        .memory_quality(
            &graph,
            MemoryQualityParams {
                low_confidence_threshold: 0.45,
                stale_decay_threshold: 0.2,
                max_examples: 10,
            },
        )
        .unwrap();

    assert_eq!(report.node_count, 3);
    assert_eq!(report.contradiction_edges, 1);
    assert!(report.low_confidence_count >= 1);
    assert!(report.stale_count >= 1);
    // Decision has no CausedBy/Supports outgoing edges.
    assert_eq!(report.decisions_without_support_count, 1);
}
