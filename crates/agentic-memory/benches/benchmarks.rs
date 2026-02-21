//! Criterion benchmarks for AgenticMemory.

use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;
use tempfile::NamedTempFile;

use agentic_memory::engine::{
    PatternParams, PatternSort, QueryEngine, SimilarityParams, TraversalParams, WriteEngine,
};
use agentic_memory::format::{AmemReader, AmemWriter, MmapReader};
use agentic_memory::graph::MemoryGraph;
use agentic_memory::graph::TraversalDirection;
use agentic_memory::types::{
    CognitiveEvent, CognitiveEventBuilder, Edge, EdgeType, EventType, DEFAULT_DIMENSION,
};

// v0.2 query expansion imports
use agentic_memory::{
    AnalogicalAnchor, AnalogicalParams, BeliefRevisionParams, CentralityAlgorithm,
    CentralityParams, ConsolidationOp, ConsolidationParams, DocLengths, DriftParams,
    GapDetectionParams, GapSeverity, HybridSearchParams, ShortestPathParams, TermIndex,
    TextSearchParams, Tokenizer,
};

/// Build a large graph using from_parts for fast construction.
fn make_large_graph(node_count: usize, edges_per_node: usize) -> MemoryGraph {
    let mut rng = rand::thread_rng();
    let types = [
        EventType::Fact,
        EventType::Decision,
        EventType::Inference,
        EventType::Skill,
        EventType::Episode,
    ];
    let edge_types = [EdgeType::CausedBy, EdgeType::Supports, EdgeType::RelatedTo];

    let mut nodes: Vec<CognitiveEvent> = Vec::with_capacity(node_count);
    for i in 0..node_count {
        let et = types[i % types.len()];
        let mut fv = vec![0.0f32; DEFAULT_DIMENSION];
        for val in &mut fv {
            *val = rng.gen_range(-1.0..1.0);
        }
        let mut event = CognitiveEventBuilder::new(et, format!("node_{}", i))
            .session_id(i as u32 / 100)
            .confidence(rng.gen_range(0.1..1.0))
            .feature_vec(fv)
            .build();
        event.id = i as u64;
        nodes.push(event);
    }

    let mut edges: Vec<Edge> = Vec::with_capacity(node_count * edges_per_node);
    for i in 0..node_count {
        for _ in 0..edges_per_node {
            let target = rng.gen_range(0..node_count);
            if target != i {
                let et = edge_types[rng.gen_range(0..edge_types.len())];
                edges.push(Edge::new(
                    i as u64,
                    target as u64,
                    et,
                    rng.gen_range(0.1..1.0),
                ));
            }
        }
    }

    MemoryGraph::from_parts(nodes, edges, DEFAULT_DIMENSION).unwrap()
}

/// Build a small graph using add_node/add_edge (for benchmarking those operations).
fn make_small_graph(node_count: usize, edges_per_node: usize) -> MemoryGraph {
    let mut rng = rand::thread_rng();
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let types = [
        EventType::Fact,
        EventType::Decision,
        EventType::Inference,
        EventType::Skill,
        EventType::Episode,
    ];

    for i in 0..node_count {
        let et = types[i % types.len()];
        let mut fv = vec![0.0f32; DEFAULT_DIMENSION];
        for val in &mut fv {
            *val = rng.gen_range(-1.0..1.0);
        }
        let event = CognitiveEventBuilder::new(et, format!("node_{}", i))
            .session_id(i as u32 / 100)
            .confidence(rng.gen_range(0.1..1.0))
            .feature_vec(fv)
            .build();
        graph.add_node(event).unwrap();
    }

    let edge_types = [EdgeType::CausedBy, EdgeType::Supports, EdgeType::RelatedTo];
    for i in 0..node_count {
        for _ in 0..edges_per_node {
            let target = rng.gen_range(0..node_count);
            if target != i {
                let et = edge_types[rng.gen_range(0..edge_types.len())];
                let _ = graph.add_edge(Edge::new(
                    i as u64,
                    target as u64,
                    et,
                    rng.gen_range(0.1..1.0),
                ));
            }
        }
    }

    graph
}

fn bench_add_node(c: &mut Criterion) {
    let mut graph = make_small_graph(10_000, 3);

    c.bench_function("add_node_to_10k", |b| {
        b.iter(|| {
            let event = CognitiveEventBuilder::new(EventType::Fact, "bench node")
                .session_id(999)
                .confidence(0.9)
                .build();
            let _ = graph.add_node(event);
        })
    });
}

fn bench_add_edge(c: &mut Criterion) {
    let mut graph = make_small_graph(10_000, 3);

    c.bench_function("add_edge_to_10k", |b| {
        let mut rng = rand::thread_rng();
        b.iter(|| {
            let src = rng.gen_range(0..10_000u64);
            let tgt = rng.gen_range(0..10_000u64);
            if src != tgt {
                let _ = graph.add_edge(Edge::new(src, tgt, EdgeType::RelatedTo, 0.5));
            }
        })
    });
}

fn bench_traverse_depth_5(c: &mut Criterion) {
    let graph = make_large_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("traverse_depth5_100k", |b| {
        b.iter(|| {
            let params = TraversalParams {
                start_id: 50_000,
                edge_types: vec![EdgeType::CausedBy, EdgeType::Supports, EdgeType::RelatedTo],
                direction: TraversalDirection::Forward,
                max_depth: 5,
                max_results: 100,
                min_confidence: 0.0,
            };
            let _ = query_engine.traverse(&graph, params);
        })
    });
}

fn bench_pattern_query(c: &mut Criterion) {
    let graph = make_large_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("pattern_query_100k", |b| {
        b.iter(|| {
            let params = PatternParams {
                event_types: vec![EventType::Fact],
                min_confidence: Some(0.5),
                max_confidence: None,
                session_ids: vec![],
                created_after: None,
                created_before: None,
                min_decay_score: None,
                max_results: 50,
                sort_by: PatternSort::MostRecent,
            };
            let _ = query_engine.pattern(&graph, params);
        })
    });
}

fn bench_similarity_search_100k(c: &mut Criterion) {
    let graph = make_large_graph(100_000, 0);
    let query_engine = QueryEngine::new();
    let mut rng = rand::thread_rng();
    let query: Vec<f32> = (0..DEFAULT_DIMENSION)
        .map(|_| rng.gen_range(-1.0..1.0))
        .collect();

    c.bench_function("similarity_100k_128dim", |b| {
        b.iter(|| {
            let params = SimilarityParams {
                query_vec: query.clone(),
                top_k: 10,
                min_similarity: 0.0,
                event_types: vec![],
                skip_zero_vectors: true,
            };
            let _ = query_engine.similarity(&graph, params);
        })
    });
}

fn bench_write_file_10k(c: &mut Criterion) {
    let graph = make_large_graph(10_000, 3);
    let writer = AmemWriter::new(DEFAULT_DIMENSION);

    c.bench_function("write_file_10k", |b| {
        b.iter(|| {
            let tmp = NamedTempFile::new().unwrap();
            writer.write_to_file(&graph, tmp.path()).unwrap();
        })
    });
}

fn bench_read_file_10k(c: &mut Criterion) {
    let graph = make_large_graph(10_000, 3);
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    let tmp = NamedTempFile::new().unwrap();
    writer.write_to_file(&graph, tmp.path()).unwrap();

    c.bench_function("read_file_10k", |b| {
        b.iter(|| {
            let _ = AmemReader::read_from_file(tmp.path()).unwrap();
        })
    });
}

fn bench_mmap_node_access(c: &mut Criterion) {
    let graph = make_large_graph(100_000, 3);
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    let tmp = NamedTempFile::new().unwrap();
    writer.write_to_file(&graph, tmp.path()).unwrap();
    let reader = MmapReader::open(tmp.path()).unwrap();

    c.bench_function("mmap_node_access_100k", |b| {
        let mut rng = rand::thread_rng();
        b.iter(|| {
            let id = rng.gen_range(0..100_000u64);
            let _ = reader.read_node(id);
        })
    });
}

fn bench_mmap_batch_similarity(c: &mut Criterion) {
    let graph = make_large_graph(100_000, 0);
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    let tmp = NamedTempFile::new().unwrap();
    writer.write_to_file(&graph, tmp.path()).unwrap();
    let reader = MmapReader::open(tmp.path()).unwrap();
    let mut rng = rand::thread_rng();
    let query: Vec<f32> = (0..DEFAULT_DIMENSION)
        .map(|_| rng.gen_range(-1.0..1.0))
        .collect();

    c.bench_function("mmap_batch_similarity_100k", |b| {
        b.iter(|| {
            let _ = reader.batch_similarity(&query, 10, 0.0);
        })
    });
}

fn bench_decay_calculation(c: &mut Criterion) {
    let mut graph = make_large_graph(100_000, 0);
    let engine = WriteEngine::new(DEFAULT_DIMENSION);

    c.bench_function("decay_100k", |b| {
        b.iter(|| {
            let _ = engine.run_decay(&mut graph, agentic_memory::now_micros());
        })
    });
}

// ===========================================================================
// v0.2 Query Expansion Benchmarks
// ===========================================================================

/// Build a large graph with realistic text content for BM25 benchmarks.
fn make_text_graph(node_count: usize, edges_per_node: usize) -> MemoryGraph {
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
        "API rate limit configuration server",
        "database PostgreSQL query optimization",
        "Redis caching strategy performance",
        "authentication JWT token security",
        "deployment Kubernetes container orchestration",
        "frontend React component rendering",
        "machine learning model training inference",
        "network latency bandwidth throughput",
        "memory allocation garbage collection",
        "testing unit integration regression",
    ];

    let mut nodes: Vec<CognitiveEvent> = Vec::with_capacity(node_count);
    for i in 0..node_count {
        let et = types[i % types.len()];
        let topic = topics[i % topics.len()];
        let content = format!("{} node_{} session_{}", topic, i, i / 100);
        let mut fv = vec![0.0f32; DEFAULT_DIMENSION];
        for val in &mut fv {
            *val = rng.gen_range(-1.0..1.0);
        }
        let mut event = CognitiveEventBuilder::new(et, content)
            .session_id(i as u32 / 100)
            .confidence(rng.gen_range(0.1..1.0))
            .feature_vec(fv)
            .build();
        event.id = i as u64;
        nodes.push(event);
    }

    let mut edges: Vec<Edge> = Vec::with_capacity(node_count * edges_per_node);
    for i in 0..node_count {
        for _ in 0..edges_per_node {
            let target = rng.gen_range(0..node_count);
            if target != i {
                let et = edge_types[rng.gen_range(0..edge_types.len())];
                edges.push(Edge::new(
                    i as u64,
                    target as u64,
                    et,
                    rng.gen_range(0.1..1.0),
                ));
            }
        }
    }

    MemoryGraph::from_parts(nodes, edges, DEFAULT_DIMENSION).unwrap()
}

fn bench_bm25_text_search_100k(c: &mut Criterion) {
    let graph = make_text_graph(100_000, 3);
    let query_engine = QueryEngine::new();
    let tokenizer = Tokenizer::new();
    let term_index = TermIndex::build(&graph, &tokenizer);
    let doc_lengths = DocLengths::build(&graph, &tokenizer);

    c.bench_function("bm25_fast_100k", |b| {
        b.iter(|| {
            let params = TextSearchParams {
                query: "API rate limit".to_string(),
                max_results: 10,
                event_types: vec![],
                session_ids: vec![],
                min_score: 0.0,
            };
            let _ = query_engine.text_search(&graph, Some(&term_index), Some(&doc_lengths), params);
        })
    });
}

fn bench_bm25_slow_path_100k(c: &mut Criterion) {
    let graph = make_text_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("bm25_slow_100k", |b| {
        b.iter(|| {
            let params = TextSearchParams {
                query: "API rate limit".to_string(),
                max_results: 10,
                event_types: vec![],
                session_ids: vec![],
                min_score: 0.0,
            };
            let _ = query_engine.text_search(&graph, None, None, params);
        })
    });
}

fn bench_hybrid_search_100k(c: &mut Criterion) {
    let graph = make_text_graph(100_000, 3);
    let query_engine = QueryEngine::new();
    let tokenizer = Tokenizer::new();
    let term_index = TermIndex::build(&graph, &tokenizer);
    let doc_lengths = DocLengths::build(&graph, &tokenizer);
    let mut rng = rand::thread_rng();
    let query_vec: Vec<f32> = (0..DEFAULT_DIMENSION)
        .map(|_| rng.gen_range(-1.0..1.0))
        .collect();

    c.bench_function("hybrid_search_100k", |b| {
        b.iter(|| {
            let params = HybridSearchParams {
                query_text: "database query optimization".to_string(),
                query_vec: Some(query_vec.clone()),
                max_results: 10,
                event_types: vec![],
                text_weight: 0.5,
                vector_weight: 0.5,
                rrf_k: 60,
            };
            let _ =
                query_engine.hybrid_search(&graph, Some(&term_index), Some(&doc_lengths), params);
        })
    });
}

fn bench_pagerank_100k(c: &mut Criterion) {
    let graph = make_large_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("pagerank_100k", |b| {
        b.iter(|| {
            let params = CentralityParams {
                algorithm: CentralityAlgorithm::PageRank { damping: 0.85 },
                max_iterations: 100,
                tolerance: 1e-6,
                top_k: 10,
                event_types: vec![],
                edge_types: vec![],
            };
            let _ = query_engine.centrality(&graph, params);
        })
    });
}

fn bench_degree_centrality_100k(c: &mut Criterion) {
    let graph = make_large_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("degree_centrality_100k", |b| {
        b.iter(|| {
            let params = CentralityParams {
                algorithm: CentralityAlgorithm::Degree,
                max_iterations: 0,
                tolerance: 0.0,
                top_k: 10,
                event_types: vec![],
                edge_types: vec![],
            };
            let _ = query_engine.centrality(&graph, params);
        })
    });
}

fn bench_betweenness_centrality_100k(c: &mut Criterion) {
    let graph = make_large_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("betweenness_centrality_100k", |b| {
        b.iter(|| {
            let params = CentralityParams {
                algorithm: CentralityAlgorithm::Betweenness,
                max_iterations: 0,
                tolerance: 0.0,
                top_k: 10,
                event_types: vec![],
                edge_types: vec![],
            };
            let _ = query_engine.centrality(&graph, params);
        })
    });
}

fn bench_shortest_path_bfs_100k(c: &mut Criterion) {
    let graph = make_large_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("shortest_path_bfs_100k", |b| {
        b.iter(|| {
            let params = ShortestPathParams {
                source_id: 100,
                target_id: 99_900,
                edge_types: vec![],
                direction: TraversalDirection::Forward,
                max_depth: 20,
                weighted: false,
            };
            let _ = query_engine.shortest_path(&graph, params);
        })
    });
}

fn bench_shortest_path_dijkstra_100k(c: &mut Criterion) {
    let graph = make_large_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("shortest_path_dijkstra_100k", |b| {
        b.iter(|| {
            let params = ShortestPathParams {
                source_id: 100,
                target_id: 99_900,
                edge_types: vec![],
                direction: TraversalDirection::Forward,
                max_depth: 20,
                weighted: true,
            };
            let _ = query_engine.shortest_path(&graph, params);
        })
    });
}

fn bench_belief_revision_100k(c: &mut Criterion) {
    let graph = make_text_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("belief_revision_100k", |b| {
        b.iter(|| {
            let params = BeliefRevisionParams {
                hypothesis: "API rate limit is 200 requests per minute".to_string(),
                hypothesis_vec: None,
                contradiction_threshold: 0.5,
                max_depth: 5,
                hypothesis_confidence: 0.9,
            };
            let _ = query_engine.belief_revision(&graph, params);
        })
    });
}

fn bench_gap_detection_100k(c: &mut Criterion) {
    let graph = make_text_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("gap_detection_100k", |b| {
        b.iter(|| {
            let params = GapDetectionParams {
                confidence_threshold: 0.5,
                min_support_count: 2,
                max_results: 50,
                session_range: None,
                sort_by: GapSeverity::HighestImpact,
            };
            let _ = query_engine.gap_detection(&graph, params);
        })
    });
}

fn bench_analogical_100k(c: &mut Criterion) {
    let graph = make_text_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("analogical_100k", |b| {
        b.iter(|| {
            let params = AnalogicalParams {
                anchor: AnalogicalAnchor::Node(50_000),
                context_depth: 2,
                max_results: 5,
                min_similarity: 0.0,
                exclude_sessions: vec![],
            };
            let _ = query_engine.analogical(&graph, params);
        })
    });
}

fn bench_consolidation_dryrun_100k(c: &mut Criterion) {
    let mut graph = make_text_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("consolidation_dryrun_100k", |b| {
        b.iter(|| {
            let params = ConsolidationParams {
                session_range: None,
                operations: vec![
                    ConsolidationOp::DeduplicateFacts { threshold: 0.9 },
                    ConsolidationOp::PruneOrphans { max_decay: 0.1 },
                ],
                dry_run: true,
                backup_path: None,
            };
            let _ = query_engine.consolidate(&mut graph, params);
        })
    });
}

fn bench_drift_detection_100k(c: &mut Criterion) {
    let graph = make_text_graph(100_000, 3);
    let query_engine = QueryEngine::new();

    c.bench_function("drift_detection_100k", |b| {
        b.iter(|| {
            let params = DriftParams {
                topic: "database query optimization".to_string(),
                topic_vec: None,
                max_results: 10,
                min_relevance: 0.1,
            };
            let _ = query_engine.drift_detection(&graph, params);
        })
    });
}

criterion_group!(
    benches,
    bench_add_node,
    bench_add_edge,
    bench_traverse_depth_5,
    bench_pattern_query,
    bench_similarity_search_100k,
    bench_write_file_10k,
    bench_read_file_10k,
    bench_mmap_node_access,
    bench_mmap_batch_similarity,
    bench_decay_calculation,
);

// v0.2 query expansion benchmarks
criterion_group!(
    v02_benches,
    bench_bm25_text_search_100k,
    bench_bm25_slow_path_100k,
    bench_hybrid_search_100k,
    bench_pagerank_100k,
    bench_degree_centrality_100k,
    bench_betweenness_centrality_100k,
    bench_shortest_path_bfs_100k,
    bench_shortest_path_dijkstra_100k,
    bench_belief_revision_100k,
    bench_gap_detection_100k,
    bench_analogical_100k,
    bench_consolidation_dryrun_100k,
    bench_drift_detection_100k,
);

criterion_main!(benches, v02_benches);
