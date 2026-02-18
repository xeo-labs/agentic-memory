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
criterion_main!(benches);
