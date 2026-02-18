//! Phase 3 tests: Indexes, mmap, and performance.

use std::time::Instant;

use rand::Rng;
use tempfile::NamedTempFile;

use agentic_memory::format::{AmemWriter, MmapReader};
use agentic_memory::graph::MemoryGraph;
use agentic_memory::index::{ClusterMap, TemporalIndex, TypeIndex};
use agentic_memory::types::{
    CognitiveEvent, CognitiveEventBuilder, Edge, EdgeType, EventType, DEFAULT_DIMENSION,
};
use agentic_memory::{
    PatternParams, PatternSort, QueryEngine, SimilarityParams, TraversalDirection, TraversalParams,
};

// ==================== Helpers ====================

/// Build a random feature vector of the given dimension, centered around `center`
/// with some noise.
fn random_feature_vec(rng: &mut impl Rng, dim: usize, center: &[f32], noise: f32) -> Vec<f32> {
    center
        .iter()
        .map(|&c| c + rng.gen_range(-noise..noise))
        .chain(std::iter::repeat(0.0))
        .take(dim)
        .collect()
}

/// Create a large test graph with `node_count` nodes and approximately
/// `edges_per_node` random edges per node.
///
/// Uses `MemoryGraph::from_parts` to avoid the O(n^2) cost of
/// rebuilding adjacency on every `add_edge` call.
fn make_test_graph(node_count: usize, edges_per_node: usize) -> MemoryGraph {
    let mut rng = rand::thread_rng();

    let types = [
        EventType::Fact,
        EventType::Decision,
        EventType::Inference,
        EventType::Correction,
        EventType::Skill,
        EventType::Episode,
    ];
    let edge_type_list = [
        EdgeType::CausedBy,
        EdgeType::Supports,
        EdgeType::Contradicts,
        EdgeType::RelatedTo,
        EdgeType::PartOf,
        EdgeType::TemporalNext,
    ];

    // Build nodes
    let base_ts = 1_000_000_000u64;
    let mut nodes: Vec<CognitiveEvent> = Vec::with_capacity(node_count);
    for i in 0..node_count {
        let et = types[i % types.len()];
        let confidence: f32 = rng.gen_range(0.1..1.0);
        let session_id = (i / 100) as u32;
        let ts = base_ts + i as u64;

        let mut fv = vec![0.0f32; DEFAULT_DIMENSION];
        for val in fv.iter_mut() {
            *val = rng.gen_range(-1.0..1.0);
        }

        let mut event = CognitiveEventBuilder::new(et, format!("node_content_{}", i))
            .session_id(session_id)
            .confidence(confidence)
            .feature_vec(fv)
            .created_at(ts)
            .build();
        event.id = i as u64;
        nodes.push(event);
    }

    // Build edges
    let mut edges: Vec<Edge> = Vec::with_capacity(node_count * edges_per_node);
    for i in 0..node_count {
        for _ in 0..edges_per_node {
            let target = rng.gen_range(0..node_count) as u64;
            if target == i as u64 {
                continue;
            }
            let et = edge_type_list[rng.gen_range(0..edge_type_list.len())];
            let weight: f32 = rng.gen_range(0.1..1.0);
            edges.push(Edge::new(i as u64, target, et, weight));
        }
    }

    MemoryGraph::from_parts(nodes, edges, DEFAULT_DIMENSION).unwrap()
}

/// Write a graph to a temp file and return the path handle.
fn write_graph_to_temp(graph: &MemoryGraph) -> NamedTempFile {
    let tmp = tempfile::Builder::new().suffix(".amem").tempfile().unwrap();
    let writer = AmemWriter::new(graph.dimension());
    writer.write_to_file(graph, tmp.path()).unwrap();
    tmp
}

// ==================== Index Tests ====================

#[test]
fn test_type_index_build() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Add 50 facts
    for i in 0..50 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("fact_{}", i)).build();
        graph.add_node(event).unwrap();
    }
    // Add 30 decisions
    for i in 0..30 {
        let event =
            CognitiveEventBuilder::new(EventType::Decision, format!("decision_{}", i)).build();
        graph.add_node(event).unwrap();
    }
    // Add 20 inferences
    for i in 0..20 {
        let event =
            CognitiveEventBuilder::new(EventType::Inference, format!("inference_{}", i)).build();
        graph.add_node(event).unwrap();
    }

    let ti = graph.type_index();
    assert_eq!(ti.count(EventType::Fact), 50);
    assert_eq!(ti.count(EventType::Decision), 30);
    assert_eq!(ti.count(EventType::Inference), 20);
    assert_eq!(ti.count(EventType::Correction), 0);
    assert_eq!(ti.count(EventType::Skill), 0);
    assert_eq!(ti.count(EventType::Episode), 0);

    // Verify the IDs in the fact list are the first 50
    let fact_ids = ti.get(EventType::Fact);
    assert_eq!(fact_ids.len(), 50);
    for i in 0..50u64 {
        assert!(fact_ids.contains(&i));
    }

    // Decision IDs should be 50..80
    let decision_ids = ti.get(EventType::Decision);
    assert_eq!(decision_ids.len(), 30);
    for i in 50..80u64 {
        assert!(decision_ids.contains(&i));
    }
}

#[test]
fn test_type_index_incremental() {
    let mut index = TypeIndex::new();

    // Build initial index from some events
    let events: Vec<CognitiveEvent> = (0..10)
        .map(|i| {
            let mut e = CognitiveEventBuilder::new(EventType::Fact, format!("fact_{}", i)).build();
            e.id = i;
            e
        })
        .collect();
    index.rebuild(&events);
    assert_eq!(index.count(EventType::Fact), 10);

    // Incrementally add a new node
    let mut new_event =
        CognitiveEventBuilder::new(EventType::Decision, "new_decision".to_string()).build();
    new_event.id = 10;
    index.add_node(&new_event);

    assert_eq!(index.count(EventType::Fact), 10);
    assert_eq!(index.count(EventType::Decision), 1);

    // Verify the new node appears in the Decision list
    let decision_ids = index.get(EventType::Decision);
    assert_eq!(decision_ids.len(), 1);
    assert_eq!(decision_ids[0], 10);
}

#[test]
fn test_temporal_index_range() {
    let mut index = TemporalIndex::new();

    // Create events with known timestamps
    let base_ts = 1_000_000u64;
    let events: Vec<CognitiveEvent> = (0..20)
        .map(|i| {
            let mut e = CognitiveEventBuilder::new(EventType::Fact, format!("event_{}", i))
                .created_at(base_ts + i * 100)
                .build();
            e.id = i;
            e
        })
        .collect();
    index.rebuild(&events);

    // Range query: timestamps 500 to 1500 (inclusive).
    // Nodes with created_at in [base_ts + 500, base_ts + 1500]:
    //   i=5 (ts=1_000_500), i=6 (ts=1_000_600), ..., i=15 (ts=1_001_500)
    let result = index.range(base_ts + 500, base_ts + 1500);
    assert_eq!(result.len(), 11); // i in 5..=15
    for &id in &result {
        let ts = base_ts + id * 100;
        assert!(ts >= base_ts + 500);
        assert!(ts <= base_ts + 1500);
    }

    // Boundary test: exact start and end
    let result = index.range(base_ts, base_ts);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], 0);

    // Boundary test: last element
    let result = index.range(base_ts + 1900, base_ts + 1900);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], 19);
}

#[test]
fn test_temporal_index_most_recent() {
    let mut index = TemporalIndex::new();

    let base_ts = 1_000_000u64;
    let events: Vec<CognitiveEvent> = (0..20)
        .map(|i| {
            let mut e = CognitiveEventBuilder::new(EventType::Fact, format!("event_{}", i))
                .created_at(base_ts + i * 100)
                .build();
            e.id = i;
            e
        })
        .collect();
    index.rebuild(&events);

    let recent = index.most_recent(5);
    assert_eq!(recent.len(), 5);

    // most_recent returns them in reverse chronological order (newest first)
    assert_eq!(recent[0], 19);
    assert_eq!(recent[1], 18);
    assert_eq!(recent[2], 17);
    assert_eq!(recent[3], 16);
    assert_eq!(recent[4], 15);
}

#[test]
fn test_session_index_get_session() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Session 0: nodes 0..5
    for i in 0..5 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("s0_{}", i))
            .session_id(0)
            .build();
        graph.add_node(event).unwrap();
    }
    // Session 1: nodes 5..12
    for i in 0..7 {
        let event = CognitiveEventBuilder::new(EventType::Decision, format!("s1_{}", i))
            .session_id(1)
            .build();
        graph.add_node(event).unwrap();
    }
    // Session 2: nodes 12..15
    for i in 0..3 {
        let event = CognitiveEventBuilder::new(EventType::Inference, format!("s2_{}", i))
            .session_id(2)
            .build();
        graph.add_node(event).unwrap();
    }

    let si = graph.session_index();

    let s2_ids = si.get_session(2);
    assert_eq!(s2_ids.len(), 3);
    assert!(s2_ids.contains(&12));
    assert!(s2_ids.contains(&13));
    assert!(s2_ids.contains(&14));

    let s0_ids = si.get_session(0);
    assert_eq!(s0_ids.len(), 5);

    let s1_ids = si.get_session(1);
    assert_eq!(s1_ids.len(), 7);
}

#[test]
fn test_session_index_session_count() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    for session in 0..3u32 {
        for i in 0..4 {
            let event = CognitiveEventBuilder::new(EventType::Fact, format!("s{}_{}", session, i))
                .session_id(session)
                .build();
            graph.add_node(event).unwrap();
        }
    }

    let si = graph.session_index();
    assert_eq!(si.session_count(), 3);
    assert_eq!(si.node_count(0), 4);
    assert_eq!(si.node_count(1), 4);
    assert_eq!(si.node_count(2), 4);
    assert_eq!(si.node_count(99), 0); // Non-existent session
}

#[test]
fn test_cluster_map_build() {
    let mut rng = rand::thread_rng();
    let dim = DEFAULT_DIMENSION;
    let mut cluster_map = ClusterMap::new(dim);

    // Create 3 natural clusters
    let center_a: Vec<f32> = (0..dim)
        .map(|i| if i < dim / 3 { 1.0 } else { 0.0 })
        .collect();
    let center_b: Vec<f32> = (0..dim)
        .map(|i| {
            if i >= dim / 3 && i < 2 * dim / 3 {
                1.0
            } else {
                0.0
            }
        })
        .collect();
    let center_c: Vec<f32> = (0..dim)
        .map(|i| if i >= 2 * dim / 3 { 1.0 } else { 0.0 })
        .collect();

    let mut nodes: Vec<(u64, Vec<f32>)> = Vec::new();
    for i in 0..34 {
        nodes.push((i, random_feature_vec(&mut rng, dim, &center_a, 0.1)));
    }
    for i in 34..67 {
        nodes.push((i, random_feature_vec(&mut rng, dim, &center_b, 0.1)));
    }
    for i in 67..100 {
        nodes.push((i, random_feature_vec(&mut rng, dim, &center_c, 0.1)));
    }

    let node_refs: Vec<(u64, &[f32])> = nodes.iter().map(|(id, v)| (*id, v.as_slice())).collect();
    cluster_map.build(&node_refs, 50);

    // k = sqrt(100) = 10
    let k = cluster_map.cluster_count();
    assert!(k > 0, "Should have at least 1 cluster");
    assert!(
        k <= 20,
        "With 100 nodes, should not have more than 20 clusters, got {}",
        k
    );

    // Verify every node is assigned to exactly one cluster
    let mut assigned_count = 0;
    for ci in 0..k {
        assigned_count += cluster_map.get_cluster(ci).len();
    }
    assert_eq!(
        assigned_count, 100,
        "All 100 nodes should be assigned, got {}",
        assigned_count
    );
}

#[test]
fn test_cluster_map_assign_new() {
    let mut rng = rand::thread_rng();
    let dim = DEFAULT_DIMENSION;
    let mut cluster_map = ClusterMap::new(dim);

    // Build initial clusters from 50 nodes
    let center_a: Vec<f32> = (0..dim)
        .map(|i| if i < dim / 2 { 1.0 } else { 0.0 })
        .collect();
    let center_b: Vec<f32> = (0..dim)
        .map(|i| if i >= dim / 2 { 1.0 } else { 0.0 })
        .collect();

    let mut nodes: Vec<(u64, Vec<f32>)> = Vec::new();
    for i in 0..25 {
        nodes.push((i, random_feature_vec(&mut rng, dim, &center_a, 0.1)));
    }
    for i in 25..50 {
        nodes.push((i, random_feature_vec(&mut rng, dim, &center_b, 0.1)));
    }

    let node_refs: Vec<(u64, &[f32])> = nodes.iter().map(|(id, v)| (*id, v.as_slice())).collect();
    cluster_map.build(&node_refs, 50);

    let k = cluster_map.cluster_count();
    assert!(k > 0);

    let total_before: usize = (0..k).map(|ci| cluster_map.get_cluster(ci).len()).sum();
    assert_eq!(total_before, 50);

    // Assign a new node close to center_a
    let new_vec = random_feature_vec(&mut rng, dim, &center_a, 0.05);
    cluster_map.assign_node(999, &new_vec);

    let total_after: usize = (0..k).map(|ci| cluster_map.get_cluster(ci).len()).sum();
    assert_eq!(total_after, 51);

    // Find which cluster the new node was assigned to
    let mut found = false;
    for ci in 0..k {
        if cluster_map.get_cluster(ci).contains(&999) {
            found = true;
            break;
        }
    }
    assert!(found, "New node should be assigned to some cluster");
}

#[test]
fn test_cluster_map_nearest() {
    let mut rng = rand::thread_rng();
    let dim = DEFAULT_DIMENSION;
    let mut cluster_map = ClusterMap::new(dim);

    // Two distinct clusters with very different directions
    let center_a: Vec<f32> = (0..dim)
        .map(|i| if i < dim / 2 { 1.0 } else { 0.0 })
        .collect();
    let center_b: Vec<f32> = (0..dim)
        .map(|i| if i >= dim / 2 { 1.0 } else { 0.0 })
        .collect();

    let mut nodes: Vec<(u64, Vec<f32>)> = Vec::new();
    for i in 0..25 {
        nodes.push((i, random_feature_vec(&mut rng, dim, &center_a, 0.05)));
    }
    for i in 25..50 {
        nodes.push((i, random_feature_vec(&mut rng, dim, &center_b, 0.05)));
    }

    let node_refs: Vec<(u64, &[f32])> = nodes.iter().map(|(id, v)| (*id, v.as_slice())).collect();
    cluster_map.build(&node_refs, 50);

    // Query with a vector very close to center_a
    let query_a = random_feature_vec(&mut rng, dim, &center_a, 0.01);
    let nearest_a = cluster_map.nearest_cluster(&query_a).unwrap();

    // The nearest cluster should contain mostly "group A" nodes (IDs 0..25)
    let cluster_members = cluster_map.get_cluster(nearest_a);
    let group_a_count = cluster_members.iter().filter(|&&id| id < 25).count();
    assert!(
        group_a_count > cluster_members.len() / 2,
        "Nearest cluster for center_a query should contain mostly group A nodes, \
         got {}/{} group A nodes",
        group_a_count,
        cluster_members.len()
    );

    // Query with a vector very close to center_b
    let query_b = random_feature_vec(&mut rng, dim, &center_b, 0.01);
    let nearest_b = cluster_map.nearest_cluster(&query_b).unwrap();

    // The nearest cluster should contain mostly "group B" nodes (IDs 25..50)
    let cluster_members_b = cluster_map.get_cluster(nearest_b);
    let group_b_count = cluster_members_b.iter().filter(|&&id| id >= 25).count();
    assert!(
        group_b_count > cluster_members_b.len() / 2,
        "Nearest cluster for center_b query should contain mostly group B nodes, \
         got {}/{} group B nodes",
        group_b_count,
        cluster_members_b.len()
    );

    // The two queries should map to different clusters
    assert_ne!(
        nearest_a, nearest_b,
        "Different cluster regions should map to different clusters"
    );
}

// ==================== Mmap Tests ====================

/// Helper: build a small graph with known data for mmap tests.
fn build_mmap_test_graph() -> MemoryGraph {
    let mut rng = rand::thread_rng();
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    let types = [
        EventType::Fact,
        EventType::Decision,
        EventType::Inference,
        EventType::Fact,
        EventType::Skill,
        EventType::Episode,
        EventType::Correction,
        EventType::Fact,
        EventType::Decision,
        EventType::Inference,
    ];

    for i in 0..10 {
        let mut fv = vec![0.0f32; DEFAULT_DIMENSION];
        for val in fv.iter_mut() {
            *val = rng.gen_range(-1.0..1.0);
        }
        let event = CognitiveEventBuilder::new(types[i], format!("mmap_content_{}", i))
            .session_id(i as u32 / 3)
            .confidence(0.5 + (i as f32) * 0.05)
            .feature_vec(fv)
            .build();
        graph.add_node(event).unwrap();
    }

    // Add some edges
    let edge_pairs = [
        (0u64, 1u64),
        (0, 2),
        (1, 3),
        (2, 4),
        (3, 5),
        (5, 6),
        (7, 8),
        (8, 9),
    ];
    for &(src, tgt) in &edge_pairs {
        graph
            .add_edge(Edge::new(src, tgt, EdgeType::RelatedTo, 0.8))
            .unwrap();
    }

    graph
}

#[test]
fn test_mmap_reader_open() {
    let graph = build_mmap_test_graph();
    let tmp = write_graph_to_temp(&graph);

    let reader = MmapReader::open(tmp.path()).unwrap();
    let header = reader.header();

    assert_eq!(header.node_count, 10);
    assert_eq!(header.edge_count, 8);
    assert_eq!(header.dimension, DEFAULT_DIMENSION as u32);
    assert_eq!(header.version, 1);
    assert_eq!(header.magic, [0x41, 0x4D, 0x45, 0x4D]); // "AMEM"
}

#[test]
fn test_mmap_read_node() {
    let graph = build_mmap_test_graph();
    let tmp = write_graph_to_temp(&graph);
    let reader = MmapReader::open(tmp.path()).unwrap();

    // Read node 5 and verify fields match the original graph
    let original = graph.get_node(5).unwrap();
    let mmap_node = reader.read_node(5).unwrap();

    assert_eq!(mmap_node.id, original.id);
    assert_eq!(mmap_node.event_type, original.event_type);
    assert_eq!(mmap_node.session_id, original.session_id);
    assert_eq!(mmap_node.created_at, original.created_at);
    assert!((mmap_node.confidence - original.confidence).abs() < f32::EPSILON);
    assert_eq!(mmap_node.access_count, original.access_count);
    assert_eq!(mmap_node.content, original.content);

    // Also verify a few other nodes
    for id in [0u64, 3, 7, 9] {
        let orig = graph.get_node(id).unwrap();
        let mmap_n = reader.read_node(id).unwrap();
        assert_eq!(mmap_n.id, orig.id);
        assert_eq!(mmap_n.content, orig.content);
        assert_eq!(mmap_n.event_type, orig.event_type);
    }
}

#[test]
fn test_mmap_read_content() {
    let graph = build_mmap_test_graph();
    let tmp = write_graph_to_temp(&graph);
    let reader = MmapReader::open(tmp.path()).unwrap();

    for id in 0..10u64 {
        let original_content = &graph.get_node(id).unwrap().content;
        let mmap_content = reader.read_content(id).unwrap();
        assert_eq!(
            &mmap_content, original_content,
            "Content mismatch for node {}",
            id
        );
    }
}

#[test]
fn test_mmap_read_feature_vec() {
    let graph = build_mmap_test_graph();
    let tmp = write_graph_to_temp(&graph);
    let reader = MmapReader::open(tmp.path()).unwrap();

    for id in 0..10u64 {
        let original_fv = &graph.get_node(id).unwrap().feature_vec;
        let mmap_fv = reader.read_feature_vec(id).unwrap();

        assert_eq!(mmap_fv.len(), original_fv.len());
        for (j, (&a, &b)) in original_fv.iter().zip(mmap_fv.iter()).enumerate() {
            assert!(
                (a - b).abs() < 1e-6,
                "Feature vec mismatch at node {} dim {}: {} vs {}",
                id,
                j,
                a,
                b
            );
        }
    }
}

#[test]
fn test_mmap_read_edges() {
    let graph = build_mmap_test_graph();
    let tmp = write_graph_to_temp(&graph);
    let reader = MmapReader::open(tmp.path()).unwrap();

    // Node 0 has edges to nodes 1 and 2
    let edges_0 = reader.read_edges(0).unwrap();
    assert_eq!(edges_0.len(), 2);
    let targets: Vec<u64> = edges_0.iter().map(|e| e.target_id).collect();
    assert!(targets.contains(&1));
    assert!(targets.contains(&2));
    for e in &edges_0 {
        assert_eq!(e.source_id, 0);
        assert_eq!(e.edge_type, EdgeType::RelatedTo);
        assert!((e.weight - 0.8).abs() < f32::EPSILON);
    }

    // Node 8 has edge to node 9
    let edges_8 = reader.read_edges(8).unwrap();
    assert_eq!(edges_8.len(), 1);
    assert_eq!(edges_8[0].source_id, 8);
    assert_eq!(edges_8[0].target_id, 9);

    // Node 9 has no outgoing edges
    let edges_9 = reader.read_edges(9).unwrap();
    assert_eq!(edges_9.len(), 0);
}

#[test]
fn test_mmap_batch_similarity() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    // Create nodes with known feature vectors
    // Node 0: all 1s (normalized direction)
    let fv_ones: Vec<f32> = vec![1.0; DEFAULT_DIMENSION];
    let event0 = CognitiveEventBuilder::new(EventType::Fact, "ones")
        .feature_vec(fv_ones.clone())
        .build();
    graph.add_node(event0).unwrap();

    // Node 1: all -1s (opposite direction)
    let fv_neg: Vec<f32> = vec![-1.0; DEFAULT_DIMENSION];
    let event1 = CognitiveEventBuilder::new(EventType::Fact, "neg_ones")
        .feature_vec(fv_neg)
        .build();
    graph.add_node(event1).unwrap();

    // Node 2: first half 1s, second half 0s
    let mut fv_half: Vec<f32> = vec![0.0; DEFAULT_DIMENSION];
    for i in 0..DEFAULT_DIMENSION / 2 {
        fv_half[i] = 1.0;
    }
    let event2 = CognitiveEventBuilder::new(EventType::Fact, "half")
        .feature_vec(fv_half)
        .build();
    graph.add_node(event2).unwrap();

    // Node 3: all zeros (should be skipped by similarity)
    let event3 = CognitiveEventBuilder::new(EventType::Fact, "zeros").build();
    graph.add_node(event3).unwrap();

    let tmp = write_graph_to_temp(&graph);
    let reader = MmapReader::open(tmp.path()).unwrap();

    // Query with all 1s - should find node 0 as most similar (top_k=10 to get all non-zero nodes)
    let results = reader.batch_similarity(&fv_ones, 10, -2.0).unwrap();

    // Node 0 should be first (similarity ~ 1.0)
    assert!(!results.is_empty());
    assert_eq!(results[0].node_id, 0);
    assert!((results[0].similarity - 1.0).abs() < 1e-5);

    // Node 1 should appear (similarity ~ -1.0)
    let node1_result = results.iter().find(|r| r.node_id == 1);
    assert!(
        node1_result.is_some(),
        "Node 1 (all -1s) should appear in results, got {:?}",
        results
            .iter()
            .map(|r| (r.node_id, r.similarity))
            .collect::<Vec<_>>()
    );
    assert!((node1_result.unwrap().similarity - (-1.0)).abs() < 1e-5);

    // Node 2 should appear (similarity ~ 0.707)
    let node2_result = results.iter().find(|r| r.node_id == 2);
    assert!(node2_result.is_some());
    assert!(node2_result.unwrap().similarity > 0.5);

    // Test with min_similarity threshold
    let results_positive = reader.batch_similarity(&fv_ones, 10, 0.5).unwrap();
    // Only nodes 0 and 2 should pass (node 0 ~ 1.0, node 2 ~ 0.707)
    assert!(results_positive.len() >= 1);
    for r in &results_positive {
        assert!(
            r.similarity >= 0.5,
            "Result node {} has sim {} below threshold",
            r.node_id,
            r.similarity
        );
    }
    // Node 1 (sim ~ -1.0) should NOT appear in positive results
    assert!(
        results_positive.iter().all(|r| r.node_id != 1),
        "Node 1 should not appear with min_similarity=0.5"
    );
}

// ==================== Performance Tests ====================

#[test]
fn test_large_graph_creation() {
    let start = Instant::now();
    let graph = make_test_graph(100_000, 3);
    let elapsed = start.elapsed();

    assert_eq!(graph.node_count(), 100_000);
    // Each node tries to add ~3 edges but some may fail (self-edge, too many edges)
    assert!(
        graph.edge_count() > 100_000,
        "Should have many edges, got {}",
        graph.edge_count()
    );
    println!(
        "Large graph creation: {} nodes, {} edges in {:?}",
        graph.node_count(),
        graph.edge_count(),
        elapsed
    );
}

#[test]
fn test_large_graph_write_read() {
    let graph = make_test_graph(100_000, 3);
    let tmp = write_graph_to_temp(&graph);

    // File size check: < 500 MB (100K nodes x 128-dim feature vectors alone is ~49MB)
    let file_size = std::fs::metadata(tmp.path()).unwrap().len();
    assert!(
        file_size < 500 * 1024 * 1024,
        "File size {} bytes exceeds 500MB",
        file_size
    );
    println!(
        "100K graph file size: {:.2} MB",
        file_size as f64 / (1024.0 * 1024.0)
    );

    // Read back with MmapReader
    let reader = MmapReader::open(tmp.path()).unwrap();
    let header = reader.header();

    assert_eq!(header.node_count, graph.node_count() as u64);
    assert_eq!(header.edge_count, graph.edge_count() as u64);
    assert_eq!(header.dimension, DEFAULT_DIMENSION as u32);

    // Spot check a few nodes
    for id in [0u64, 50_000, 99_999] {
        let original = graph.get_node(id).unwrap();
        let mmap_node = reader.read_node(id).unwrap();
        assert_eq!(mmap_node.id, original.id);
        assert_eq!(mmap_node.content, original.content);
        assert_eq!(mmap_node.event_type, original.event_type);
    }
}

#[test]
fn test_large_graph_traversal_speed() {
    let graph = make_test_graph(100_000, 3);

    let params = TraversalParams {
        start_id: 0,
        edge_types: vec![
            EdgeType::CausedBy,
            EdgeType::Supports,
            EdgeType::RelatedTo,
            EdgeType::PartOf,
            EdgeType::TemporalNext,
        ],
        direction: TraversalDirection::Both,
        max_depth: 3,
        max_results: 1000,
        min_confidence: 0.0,
    };

    let engine = QueryEngine::new();

    let start = Instant::now();
    let result = engine.traverse(&graph, params).unwrap();
    let elapsed = start.elapsed();

    assert!(!result.visited.is_empty());
    // In release mode this is < 10ms. Debug mode is significantly slower.
    assert!(
        elapsed.as_secs() < 2,
        "Traversal took {:?}, expected < 2s",
        elapsed
    );
    println!(
        "Traversal: {} nodes visited in {:?}",
        result.visited.len(),
        elapsed
    );
}

#[test]
fn test_large_graph_pattern_query_speed() {
    let graph = make_test_graph(100_000, 3);

    let params = PatternParams {
        event_types: vec![EventType::Fact],
        min_confidence: Some(0.5),
        max_confidence: None,
        session_ids: vec![],
        created_after: None,
        created_before: None,
        min_decay_score: None,
        max_results: 100,
        sort_by: PatternSort::HighestConfidence,
    };

    let engine = QueryEngine::new();

    let start = Instant::now();
    let result = engine.pattern(&graph, params).unwrap();
    let elapsed = start.elapsed();

    assert!(!result.is_empty());
    assert!(result.len() <= 100);
    // All results should be Facts with confidence >= 0.5
    for node in &result {
        assert_eq!(node.event_type, EventType::Fact);
        assert!(node.confidence >= 0.5);
    }
    // In release mode this is < 1s. Debug mode is significantly slower.
    assert!(
        elapsed.as_secs() < 10,
        "Pattern query took {:?}, expected < 10s",
        elapsed
    );
    println!("Pattern query: {} results in {:?}", result.len(), elapsed);
}

#[test]
fn test_large_graph_similarity_speed() {
    let graph = make_test_graph(100_000, 3);

    // Build a query vector
    let mut rng = rand::thread_rng();
    let mut query_vec = vec![0.0f32; DEFAULT_DIMENSION];
    for val in query_vec.iter_mut() {
        *val = rng.gen_range(-1.0..1.0);
    }

    let params = SimilarityParams {
        query_vec,
        top_k: 10,
        min_similarity: 0.0,
        event_types: vec![],
        skip_zero_vectors: true,
    };

    let engine = QueryEngine::new();

    let start = Instant::now();
    let result = engine.similarity(&graph, params).unwrap();
    let elapsed = start.elapsed();

    assert!(!result.is_empty());
    assert!(result.len() <= 10);

    // Results should be sorted by similarity descending
    for i in 1..result.len() {
        assert!(result[i - 1].similarity >= result[i].similarity);
    }

    // In release mode this is < 100ms. Debug mode is significantly slower.
    assert!(
        elapsed.as_secs() < 2,
        "Similarity search took {:?}, expected < 2s",
        elapsed
    );
    println!(
        "Similarity search: {} results in {:?}",
        result.len(),
        elapsed
    );
}
