//! Phase 5 cross-version compatibility tests.
//!
//! Verify that files written by different code versions can be read correctly,
//! and that the format is forward- and backward-compatible.

use std::path::Path;

use agentic_memory::format::{AmemReader, AmemWriter};
use agentic_memory::graph::MemoryGraph;
use agentic_memory::types::edge::{Edge, EdgeType};
use agentic_memory::types::event::{CognitiveEventBuilder, EventType};
use agentic_memory::types::header::{feature_flags, FileHeader};
use agentic_memory::types::DEFAULT_DIMENSION;
use agentic_memory::{QueryEngine, TextSearchParams};

// ==================== Compatibility Tests ====================

#[test]
fn test_v1_file_opens_in_v2_reader() {
    // Pre-generated .amem file from v0.1 (included as test fixture).
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v1_basic.amem");
    assert!(
        fixture_path.exists(),
        "Fixture file v1_basic.amem must exist at {:?}",
        fixture_path
    );

    // Read with the current (v0.2) reader.
    let graph = AmemReader::read_from_file(&fixture_path).unwrap();

    // All data should be intact.
    assert!(graph.node_count() > 0, "v1 file should have nodes");
    assert!(graph.edge_count() > 0, "v1 file should have edges");

    // Verify every node has valid content and feature vectors.
    for node in graph.nodes() {
        assert!(
            !node.content.is_empty(),
            "Node {} content should not be empty",
            node.id
        );
        assert_eq!(
            node.feature_vec.len(),
            graph.dimension(),
            "Node {} should have correct dimension feature vector",
            node.id
        );
    }

    // term_index and doc_lengths should be None (old file).
    assert!(
        graph.term_index.is_none(),
        "v1 file should not have term_index"
    );
    assert!(
        graph.doc_lengths.is_none(),
        "v1 file should not have doc_lengths"
    );

    // All new queries should work via the slow path (no BM25 indexes).
    let engine = QueryEngine::new();

    // text_search without indexes (slow path).
    let results = engine
        .text_search(
            &graph,
            None,
            None,
            TextSearchParams {
                query: graph.nodes()[0]
                    .content
                    .split_whitespace()
                    .next()
                    .unwrap_or("test")
                    .to_string(),
                max_results: 10,
                event_types: vec![],
                session_ids: vec![],
                min_score: 0.0,
            },
        )
        .unwrap();
    // The slow path should work even without indexes. Results may or may not
    // match depending on content/tokenizer, but the call should not panic.
    let _ = results.len();

    // centrality (no dependency on BM25 indexes).
    let centrality = engine
        .centrality(
            &graph,
            agentic_memory::CentralityParams {
                algorithm: agentic_memory::CentralityAlgorithm::PageRank { damping: 0.85 },
                max_iterations: 100,
                tolerance: 1e-6,
                top_k: 10,
                event_types: vec![],
                edge_types: vec![],
            },
        )
        .unwrap();
    assert!(
        !centrality.scores.is_empty(),
        "Centrality should produce scores for v1 file"
    );

    // shortest_path.
    if graph.node_count() >= 2 {
        let ids: Vec<u64> = graph.nodes().iter().map(|n| n.id).collect();
        let path_result = engine
            .shortest_path(
                &graph,
                agentic_memory::ShortestPathParams {
                    source_id: ids[0],
                    target_id: ids[1],
                    edge_types: vec![],
                    direction: agentic_memory::TraversalDirection::Both,
                    max_depth: 20,
                    weighted: false,
                },
            )
            .unwrap();
        // Path may or may not exist, but call must not panic.
        let _ = path_result.found;
    }

    // gap_detection.
    let gaps = engine
        .gap_detection(
            &graph,
            agentic_memory::GapDetectionParams {
                confidence_threshold: 0.5,
                min_support_count: 2,
                max_results: 100,
                session_range: None,
                sort_by: agentic_memory::GapSeverity::HighestImpact,
            },
        )
        .unwrap();
    let _ = gaps.gaps.len();

    // drift_detection.
    let drift = engine
        .drift_detection(
            &graph,
            agentic_memory::DriftParams {
                topic: "test".to_string(),
                topic_vec: None,
                max_results: 10,
                min_relevance: 0.1,
            },
        )
        .unwrap();
    let _ = drift.timelines.len();
}

#[test]
fn test_v2_file_gracefully_handled_by_v1_reader() {
    // Write a file with the new code (includes tags 0x05 and 0x06).
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let contents = [
        "Rust programming language is excellent for systems",
        "Python is great for data science and automation",
        "JavaScript powers the modern web frontend",
        "Go is designed for concurrent server applications",
        "TypeScript adds static typing to JavaScript",
    ];

    for (i, content) in contents.iter().enumerate() {
        let event = CognitiveEventBuilder::new(EventType::Fact, *content)
            .session_id(i as u32)
            .confidence(0.8)
            .build();
        graph.add_node(event).unwrap();
    }
    for i in 0..4u64 {
        graph
            .add_edge(Edge::new(i, i + 1, EdgeType::RelatedTo, 0.7))
            .unwrap();
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v2_format.amem");
    AmemWriter::new(DEFAULT_DIMENSION)
        .write_to_file(&graph, &path)
        .unwrap();

    // Simulate a v0.1 reader that only understands tags 0x01-0x04:
    // Read the raw bytes manually, parse header, node table, edge table,
    // content block, and feature vectors. Then encounter the index block
    // and skip unknown tags.
    let data = std::fs::read(&path).unwrap();
    let header = FileHeader::read_from(&mut std::io::Cursor::new(&data[..64])).unwrap();

    // A v0.1 reader would parse nodes, edges, content, feature vectors
    // using the same offsets. Verify the header fields are sensible.
    assert_eq!(header.node_count, 5, "Header should report 5 nodes");
    assert_eq!(header.edge_count, 4, "Header should report 4 edges");
    assert!(
        header.node_table_offset == 64,
        "Node table should start right after the 64-byte header"
    );

    // Simulate reading node table (just verify offsets are valid).
    let node_table_end = header.node_table_offset as usize + (header.node_count as usize * 72);
    assert!(
        node_table_end <= data.len(),
        "Node table should fit within file"
    );

    // Simulate reading edge table.
    let edge_table_end = header.edge_table_offset as usize + (header.edge_count as usize * 32);
    assert!(
        edge_table_end <= data.len(),
        "Edge table should fit within file"
    );

    // Feature vector block.
    let fv_end =
        header.feature_vec_offset as usize + (header.node_count as usize * DEFAULT_DIMENSION * 4);
    assert!(
        fv_end <= data.len(),
        "Feature vector block should fit within file"
    );

    // Index block starts after feature vectors.
    let index_start = fv_end;
    assert!(
        index_start < data.len(),
        "Index block should exist after feature vectors"
    );

    // Simulate a v0.1 reader encountering the index block:
    // Read tag-length-value entries and skip unknown tags.
    let mut pos = index_start;
    let mut found_tags: Vec<u8> = Vec::new();
    while pos + 9 <= data.len() {
        let tag = data[pos];
        pos += 1;
        let length = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()) as usize;
        pos += 8;

        if pos + length > data.len() {
            break;
        }

        found_tags.push(tag);

        // A v0.1 reader would recognize tags 0x01-0x04 and skip unknown tags.
        match tag {
            0x01..=0x04 => {
                // Known v0.1 tags: skip the data (already rebuilt from nodes).
                pos += length;
            }
            0x05 | 0x06 => {
                // Unknown to v0.1 reader: skip gracefully.
                pos += length;
            }
            _ => {
                // Unknown: skip.
                pos += length;
            }
        }
    }

    // Verify that tags 0x05 and 0x06 are present in the file.
    assert!(
        found_tags.contains(&0x05),
        "File should contain tag 0x05 (TermIndex)"
    );
    assert!(
        found_tags.contains(&0x06),
        "File should contain tag 0x06 (DocLengths)"
    );

    // Verify that the v0.1 reader simulation parsed all tags without error.
    assert!(
        found_tags.len() >= 6,
        "Should have found at least 6 tags (0x01-0x06), found {:?}",
        found_tags
    );

    // Now verify that the actual reader (which handles all tags) produces
    // the correct data -- the v0.1 data should be fully intact.
    let loaded = AmemReader::read_from_file(&path).unwrap();
    assert_eq!(loaded.node_count(), 5);
    assert_eq!(loaded.edge_count(), 4);
    for (i, content) in contents.iter().enumerate() {
        assert_eq!(
            loaded.get_node(i as u64).unwrap().content,
            *content,
            "Content at node {} should be intact after v0.1 skip simulation",
            i
        );
    }
}

#[test]
fn test_feature_flags_dont_crash_old_reader() {
    // Create a header with both BM25 flags set (flags = 0x03).
    // An old reader that reads the flags field as "_reserved" should
    // simply ignore it without crashing.

    // Write a file with the new writer (which sets flags).
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let event = CognitiveEventBuilder::new(EventType::Fact, "test node for flags")
        .session_id(1)
        .confidence(0.9)
        .build();
    graph.add_node(event).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("flags_test.amem");
    AmemWriter::new(DEFAULT_DIMENSION)
        .write_to_file(&graph, &path)
        .unwrap();

    // Read raw header bytes.
    let data = std::fs::read(&path).unwrap();
    let header = FileHeader::read_from(&mut std::io::Cursor::new(&data[..64])).unwrap();

    // Verify flags are set.
    assert_eq!(
        header.flags,
        feature_flags::HAS_TERM_INDEX | feature_flags::HAS_DOC_LENGTHS,
        "Flags should be 0x03"
    );

    // Simulate what an old reader does: it reads the 4 bytes at offset 0x0C
    // as a u32 "_reserved" field and ignores it. The key assertion is that
    // reading the file does not fail.
    let flags_bytes = &data[12..16];
    let reserved_value = u32::from_le_bytes(flags_bytes.try_into().unwrap());
    assert_eq!(
        reserved_value, 0x03,
        "Raw flags bytes should decode to 0x03"
    );

    // An old reader would just set _reserved = 0x03 and proceed. The rest
    // of the file structure is unchanged, so all offsets remain valid.
    assert!(
        header.node_table_offset == 64,
        "Node table offset should still be 64 regardless of flags"
    );
    assert_eq!(
        header.node_count, 1,
        "Node count should still be readable with flags set"
    );

    // Full read should succeed with no crash.
    let loaded = AmemReader::read_from_file(&path).unwrap();
    assert_eq!(loaded.node_count(), 1);
    assert_eq!(loaded.get_node(0).unwrap().content, "test node for flags");
}

#[test]
fn test_mixed_version_write_read_cycle() {
    // Simulate: v0.1 writes file -> v0.2 reads, adds nodes, writes -> v0.2 reads -> verify all data.

    // Step 1: Use the v0.1 fixture file as the starting point.
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v1_basic.amem");
    assert!(fixture_path.exists(), "Fixture v1_basic.amem must exist");

    // Step 2: Read with the v0.2 reader.
    let mut graph = AmemReader::read_from_file(&fixture_path).unwrap();
    let original_node_count = graph.node_count();
    let original_edge_count = graph.edge_count();

    // Capture original node contents for verification.
    let original_contents: Vec<(u64, String)> = graph
        .nodes()
        .iter()
        .map(|n| (n.id, n.content.clone()))
        .collect();

    // Step 3: Add new nodes and edges with the v0.2 code.
    let new_node_1 = CognitiveEventBuilder::new(
        EventType::Fact,
        "New fact added by v0.2 code: Rust is memory safe",
    )
    .session_id(99)
    .confidence(0.95)
    .build();
    let id1 = graph.add_node(new_node_1).unwrap();

    let new_node_2 = CognitiveEventBuilder::new(
        EventType::Decision,
        "New decision added by v0.2 code: use Rust for backend",
    )
    .session_id(99)
    .confidence(0.85)
    .build();
    let id2 = graph.add_node(new_node_2).unwrap();

    // Add an edge between new nodes.
    graph
        .add_edge(Edge::new(id2, id1, EdgeType::CausedBy, 0.9))
        .unwrap();

    // Also add an edge from an original node to a new node.
    if original_node_count > 0 {
        let first_original_id = original_contents[0].0;
        graph
            .add_edge(Edge::new(id1, first_original_id, EdgeType::Supports, 0.7))
            .unwrap();
    }

    // Step 4: Write with v0.2 writer (includes BM25 indexes).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixed_version.amem");
    AmemWriter::new(graph.dimension())
        .write_to_file(&graph, &path)
        .unwrap();

    // Step 5: Read back with v0.2 reader.
    let loaded = AmemReader::read_from_file(&path).unwrap();

    // Verify total counts.
    assert_eq!(
        loaded.node_count(),
        original_node_count + 2,
        "Should have original nodes plus 2 new ones"
    );
    assert_eq!(
        loaded.edge_count(),
        original_edge_count + 2,
        "Should have original edges plus 2 new ones"
    );

    // Verify all original node contents are preserved.
    for (id, original_content) in &original_contents {
        let node = loaded.get_node(*id).unwrap();
        assert_eq!(
            &node.content, original_content,
            "Original node {} content should be preserved after mixed-version cycle",
            id
        );
    }

    // Verify new nodes are present.
    let node1 = loaded.get_node(id1).unwrap();
    assert_eq!(
        node1.content,
        "New fact added by v0.2 code: Rust is memory safe"
    );
    assert_eq!(node1.session_id, 99);

    let node2 = loaded.get_node(id2).unwrap();
    assert_eq!(
        node2.content,
        "New decision added by v0.2 code: use Rust for backend"
    );

    // Verify the file now has BM25 indexes.
    assert!(
        loaded.term_index.is_some(),
        "Mixed-version output should have TermIndex"
    );
    assert!(
        loaded.doc_lengths.is_some(),
        "Mixed-version output should have DocLengths"
    );

    // Verify the header has feature flags set.
    let data = std::fs::read(&path).unwrap();
    let header = FileHeader::read_from(&mut std::io::Cursor::new(&data[..64])).unwrap();
    assert!(
        header.has_flag(feature_flags::HAS_TERM_INDEX),
        "Mixed-version file should have HAS_TERM_INDEX flag"
    );
    assert!(
        header.has_flag(feature_flags::HAS_DOC_LENGTHS),
        "Mixed-version file should have HAS_DOC_LENGTHS flag"
    );
}

#[test]
fn test_bm25_index_rebuilt_on_demand() {
    // Load v0.1 file (no BM25 index).
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v1_basic.amem");
    assert!(fixture_path.exists(), "Fixture v1_basic.amem must exist");

    let graph = AmemReader::read_from_file(&fixture_path).unwrap();

    // Confirm no BM25 indexes.
    assert!(
        graph.term_index.is_none(),
        "v1 file should not have term_index"
    );
    assert!(
        graph.doc_lengths.is_none(),
        "v1 file should not have doc_lengths"
    );

    // Run text_search with None indexes (slow path). Should work.
    let engine = QueryEngine::new();
    let slow_results = engine
        .text_search(
            &graph,
            None,
            None,
            TextSearchParams {
                query: graph.nodes()[0]
                    .content
                    .split_whitespace()
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(" "),
                max_results: 10,
                event_types: vec![],
                session_ids: vec![],
                min_score: 0.0,
            },
        )
        .unwrap();
    // Slow path should work even without indexes.
    let _ = slow_results.len();

    // Write the file with v0.2 writer -- this builds and persists BM25 indexes.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rebuilt_index.amem");
    AmemWriter::new(graph.dimension())
        .write_to_file(&graph, &path)
        .unwrap();

    // Read back -- now BM25 indexes should be present (fast path available).
    let loaded = AmemReader::read_from_file(&path).unwrap();
    assert!(
        loaded.term_index.is_some(),
        "After write-read cycle, TermIndex should be present"
    );
    assert!(
        loaded.doc_lengths.is_some(),
        "After write-read cycle, DocLengths should be present"
    );

    // Verify the header flags indicate BM25 indexes.
    let data = std::fs::read(&path).unwrap();
    let header = FileHeader::read_from(&mut std::io::Cursor::new(&data[..64])).unwrap();
    assert!(
        header.has_flag(feature_flags::HAS_TERM_INDEX),
        "Rebuilt file should have HAS_TERM_INDEX flag"
    );
    assert!(
        header.has_flag(feature_flags::HAS_DOC_LENGTHS),
        "Rebuilt file should have HAS_DOC_LENGTHS flag"
    );

    // Run text_search with the rebuilt indexes (fast path).
    let query_text = loaded.nodes()[0]
        .content
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");
    let fast_results = engine
        .text_search(
            &loaded,
            loaded.term_index.as_ref(),
            loaded.doc_lengths.as_ref(),
            TextSearchParams {
                query: query_text.clone(),
                max_results: 10,
                event_types: vec![],
                session_ids: vec![],
                min_score: 0.0,
            },
        )
        .unwrap();

    // Also run slow path on the same loaded graph for comparison.
    let slow_results_2 = engine
        .text_search(
            &loaded,
            None,
            None,
            TextSearchParams {
                query: query_text,
                max_results: 10,
                event_types: vec![],
                session_ids: vec![],
                min_score: 0.0,
            },
        )
        .unwrap();

    // Both paths should return the same set of node IDs.
    let mut fast_ids: Vec<u64> = fast_results.iter().map(|m| m.node_id).collect();
    let mut slow_ids: Vec<u64> = slow_results_2.iter().map(|m| m.node_id).collect();
    fast_ids.sort_unstable();
    slow_ids.sort_unstable();
    assert_eq!(
        fast_ids, slow_ids,
        "Fast path and slow path should return the same node IDs"
    );

    // Both paths should return the same ranking order.
    let fast_order: Vec<u64> = fast_results.iter().map(|m| m.node_id).collect();
    let slow_order: Vec<u64> = slow_results_2.iter().map(|m| m.node_id).collect();
    assert_eq!(
        fast_order, slow_order,
        "Fast path and slow path should return the same ranking order"
    );
}
