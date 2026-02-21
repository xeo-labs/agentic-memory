use rand::Rng;
use std::time::Instant;

use agentic_memory::engine::QueryEngine;
use agentic_memory::graph::MemoryGraph;
use agentic_memory::graph::TraversalDirection;
use agentic_memory::types::{CognitiveEventBuilder, Edge, EdgeType, EventType, DEFAULT_DIMENSION};
use agentic_memory::{
    BeliefRevisionParams, CentralityAlgorithm, CentralityParams, DocLengths, DriftParams,
    HybridSearchParams, ShortestPathParams, TermIndex, TextSearchParams, Tokenizer,
};

fn make_graph(n: usize, epn: usize) -> MemoryGraph {
    let mut rng = rand::thread_rng();
    let types = [
        EventType::Fact,
        EventType::Decision,
        EventType::Inference,
        EventType::Skill,
        EventType::Episode,
    ];
    let edge_types = [
        EdgeType::CausedBy,
        EdgeType::Supports,
        EdgeType::RelatedTo,
        EdgeType::Contradicts,
        EdgeType::Supersedes,
    ];
    let topics = [
        "API rate limit config",
        "database query optimization",
        "Redis caching strategy",
        "JWT token security",
        "Kubernetes deployment",
        "React component render",
        "ML model training",
        "network latency",
        "memory allocation",
        "testing regression",
    ];
    let mut nodes = Vec::with_capacity(n);
    for i in 0..n {
        let et = types[i % types.len()];
        let content = format!(
            "{} node_{} session_{}",
            topics[i % topics.len()],
            i,
            i / 100
        );
        let mut fv = vec![0.0f32; DEFAULT_DIMENSION];
        for val in &mut fv {
            *val = rng.gen_range(-1.0..1.0);
        }
        let mut ev = CognitiveEventBuilder::new(et, content)
            .session_id(i as u32 / 100)
            .confidence(rng.gen_range(0.1..1.0))
            .feature_vec(fv)
            .build();
        ev.id = i as u64;
        nodes.push(ev);
    }
    let mut edges = Vec::with_capacity(n * epn);
    for i in 0..n {
        for _ in 0..epn {
            let t = rng.gen_range(0..n);
            if t != i {
                let et = edge_types[rng.gen_range(0..edge_types.len())];
                edges.push(Edge::new(i as u64, t as u64, et, rng.gen_range(0.1..1.0)));
            }
        }
    }
    MemoryGraph::from_parts(nodes, edges, DEFAULT_DIMENSION).unwrap()
}

fn main() {
    let qe = QueryEngine::new();
    let tokenizer = Tokenizer::new();
    let n = 10_000usize;
    println!("=== 10K node benchmarks (single run, release mode) ===");
    let graph = make_graph(n, 3);
    let ti = TermIndex::build(&graph, &tokenizer);
    let dl = DocLengths::build(&graph, &tokenizer);
    let mut rng = rand::thread_rng();
    let qv: Vec<f32> = (0..DEFAULT_DIMENSION)
        .map(|_| rng.gen_range(-1.0..1.0))
        .collect();

    // BM25 fast
    let s = Instant::now();
    for _ in 0..10 {
        let _ = qe.text_search(
            &graph,
            Some(&ti),
            Some(&dl),
            TextSearchParams {
                query: "API rate limit".into(),
                max_results: 10,
                event_types: vec![],
                session_ids: vec![],
                min_score: 0.0,
            },
        );
    }
    println!("bm25_fast_10k (avg 10): {:?}", s.elapsed() / 10);

    // BM25 slow
    let s = Instant::now();
    for _ in 0..10 {
        let _ = qe.text_search(
            &graph,
            None,
            None,
            TextSearchParams {
                query: "API rate limit".into(),
                max_results: 10,
                event_types: vec![],
                session_ids: vec![],
                min_score: 0.0,
            },
        );
    }
    println!("bm25_slow_10k (avg 10): {:?}", s.elapsed() / 10);

    // Hybrid
    let s = Instant::now();
    for _ in 0..10 {
        let _ = qe.hybrid_search(
            &graph,
            Some(&ti),
            Some(&dl),
            HybridSearchParams {
                query_text: "database query optimization".into(),
                query_vec: Some(qv.clone()),
                max_results: 10,
                event_types: vec![],
                text_weight: 0.5,
                vector_weight: 0.5,
                rrf_k: 60,
            },
        );
    }
    println!("hybrid_10k (avg 10): {:?}", s.elapsed() / 10);

    // PageRank
    let s = Instant::now();
    for _ in 0..10 {
        let _ = qe.centrality(
            &graph,
            CentralityParams {
                algorithm: CentralityAlgorithm::PageRank { damping: 0.85 },
                max_iterations: 100,
                tolerance: 1e-6,
                top_k: 10,
                event_types: vec![],
                edge_types: vec![],
            },
        );
    }
    println!("pagerank_10k (avg 10): {:?}", s.elapsed() / 10);

    // Degree
    let s = Instant::now();
    for _ in 0..10 {
        let _ = qe.centrality(
            &graph,
            CentralityParams {
                algorithm: CentralityAlgorithm::Degree,
                max_iterations: 0,
                tolerance: 0.0,
                top_k: 10,
                event_types: vec![],
                edge_types: vec![],
            },
        );
    }
    println!("degree_10k (avg 10): {:?}", s.elapsed() / 10);

    // Betweenness
    let s = Instant::now();
    let _ = qe.centrality(
        &graph,
        CentralityParams {
            algorithm: CentralityAlgorithm::Betweenness,
            max_iterations: 0,
            tolerance: 0.0,
            top_k: 10,
            event_types: vec![],
            edge_types: vec![],
        },
    );
    println!("betweenness_10k: {:?}", s.elapsed());

    // BFS
    let s = Instant::now();
    for _ in 0..100 {
        let _ = qe.shortest_path(
            &graph,
            ShortestPathParams {
                source_id: 100,
                target_id: 9900,
                edge_types: vec![],
                direction: TraversalDirection::Forward,
                max_depth: 20,
                weighted: false,
            },
        );
    }
    println!("bfs_10k (avg 100): {:?}", s.elapsed() / 100);

    // Dijkstra
    let s = Instant::now();
    for _ in 0..10 {
        let _ = qe.shortest_path(
            &graph,
            ShortestPathParams {
                source_id: 100,
                target_id: 9900,
                edge_types: vec![],
                direction: TraversalDirection::Forward,
                max_depth: 20,
                weighted: true,
            },
        );
    }
    println!("dijkstra_10k (avg 10): {:?}", s.elapsed() / 10);

    // Belief revision
    let s = Instant::now();
    for _ in 0..10 {
        let _ = qe.belief_revision(
            &graph,
            BeliefRevisionParams {
                hypothesis: "API rate limit is 200 per minute".into(),
                hypothesis_vec: None,
                contradiction_threshold: 0.5,
                max_depth: 5,
                hypothesis_confidence: 0.9,
            },
        );
    }
    println!("belief_revision_10k (avg 10): {:?}", s.elapsed() / 10);

    // Drift
    let s = Instant::now();
    for _ in 0..10 {
        let _ = qe.drift_detection(
            &graph,
            DriftParams {
                topic: "database query optimization".into(),
                topic_vec: None,
                max_results: 10,
                min_relevance: 0.1,
            },
        );
    }
    println!("drift_10k (avg 10): {:?}", s.elapsed() / 10);
}
