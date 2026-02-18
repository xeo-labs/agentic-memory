//! Phase 1 tests: Data structures + file format.

use agentic_memory::format::{AmemReader, AmemWriter};
use agentic_memory::graph::MemoryGraph;
use agentic_memory::types::edge::{Edge, EdgeType};
use agentic_memory::types::error::AmemError;
use agentic_memory::types::event::{CognitiveEventBuilder, EventType};
use agentic_memory::types::header::FileHeader;
use agentic_memory::types::{AMEM_MAGIC, DEFAULT_DIMENSION, FORMAT_VERSION};

use std::io::Cursor;
use tempfile::NamedTempFile;

// ==================== Data Structure Tests ====================

#[test]
fn test_event_type_roundtrip() {
    for val in 0u8..=5 {
        let et = EventType::from_u8(val).unwrap();
        assert_eq!(et as u8, val);
        assert_eq!(EventType::from_u8(et as u8), Some(et));
    }
}

#[test]
fn test_event_type_invalid() {
    assert!(EventType::from_u8(255).is_none());
    assert!(EventType::from_u8(6).is_none());
}

#[test]
fn test_edge_type_roundtrip() {
    for val in 0u8..=6 {
        let et = EdgeType::from_u8(val).unwrap();
        assert_eq!(et as u8, val);
        assert_eq!(EdgeType::from_u8(et as u8), Some(et));
    }
}

#[test]
fn test_cognitive_event_creation() {
    let event = CognitiveEventBuilder::new(EventType::Fact, "User prefers Python")
        .session_id(1)
        .confidence(0.95)
        .build();

    assert_eq!(event.event_type, EventType::Fact);
    assert_eq!(event.content, "User prefers Python");
    assert_eq!(event.session_id, 1);
    assert!((event.confidence - 0.95).abs() < f32::EPSILON);
    assert_eq!(event.access_count, 0);
    assert!((event.decay_score - 1.0).abs() < f32::EPSILON);
    assert_eq!(event.feature_vec.len(), DEFAULT_DIMENSION);
}

#[test]
fn test_confidence_clamping() {
    let event = CognitiveEventBuilder::new(EventType::Fact, "test")
        .confidence(1.5)
        .build();
    assert!((event.confidence - 1.0).abs() < f32::EPSILON);

    let event = CognitiveEventBuilder::new(EventType::Fact, "test")
        .confidence(-0.5)
        .build();
    assert!((event.confidence - 0.0).abs() < f32::EPSILON);
}

#[test]
fn test_edge_creation() {
    let edge = Edge::new(0, 1, EdgeType::CausedBy, 0.8);
    assert_eq!(edge.source_id, 0);
    assert_eq!(edge.target_id, 1);
    assert_eq!(edge.edge_type, EdgeType::CausedBy);
    assert!((edge.weight - 0.8).abs() < f32::EPSILON);
}

#[test]
fn test_self_edge_rejected() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let event = CognitiveEventBuilder::new(EventType::Fact, "test").build();
    let id = graph.add_node(event).unwrap();

    let edge = Edge::new(id, id, EdgeType::CausedBy, 1.0);
    let result = graph.add_edge(edge);
    assert!(result.is_err());
    match result.unwrap_err() {
        AmemError::SelfEdge(_) => {}
        e => panic!("Expected SelfEdge error, got {:?}", e),
    }
}

#[test]
fn test_weight_validation() {
    // Edge weight is clamped, not rejected
    let edge = Edge::new(0, 1, EdgeType::CausedBy, 1.5);
    assert!((edge.weight - 1.0).abs() < f32::EPSILON);

    let edge = Edge::new(0, 1, EdgeType::CausedBy, -0.5);
    assert!((edge.weight - 0.0).abs() < f32::EPSILON);
}

// ==================== File Header Tests ====================

#[test]
fn test_header_write_read_roundtrip() {
    let header = FileHeader {
        magic: AMEM_MAGIC,
        version: FORMAT_VERSION,
        dimension: 128,
        node_count: 42,
        edge_count: 100,
        node_table_offset: 64,
        edge_table_offset: 1000,
        content_block_offset: 2000,
        feature_vec_offset: 3000,
    };

    let mut buf = Vec::new();
    header.write_to(&mut buf).unwrap();

    let read_header = FileHeader::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(header, read_header);
}

#[test]
fn test_header_size_is_64_bytes() {
    let header = FileHeader::new(128);
    let mut buf = Vec::new();
    header.write_to(&mut buf).unwrap();
    assert_eq!(buf.len(), 64);
}

#[test]
fn test_header_magic_validation() {
    let header = FileHeader::new(128);
    let mut buf = Vec::new();
    header.write_to(&mut buf).unwrap();

    // Corrupt magic bytes
    buf[0] = 0xFF;
    let result = FileHeader::read_from(&mut Cursor::new(&buf));
    assert!(result.is_err());
    match result.unwrap_err() {
        AmemError::InvalidMagic => {}
        e => panic!("Expected InvalidMagic error, got {:?}", e),
    }
}

#[test]
fn test_header_version_validation() {
    let header = FileHeader::new(128);
    let mut buf = Vec::new();
    header.write_to(&mut buf).unwrap();

    // Set version to 99
    let version_bytes = 99u32.to_le_bytes();
    buf[4..8].copy_from_slice(&version_bytes);

    let result = FileHeader::read_from(&mut Cursor::new(&buf));
    assert!(result.is_err());
    match result.unwrap_err() {
        AmemError::UnsupportedVersion(99) => {}
        e => panic!("Expected UnsupportedVersion(99), got {:?}", e),
    }
}

#[test]
fn test_header_little_endian() {
    let header = FileHeader {
        magic: AMEM_MAGIC,
        version: FORMAT_VERSION,
        dimension: 128,
        node_count: 0x0102030405060708,
        edge_count: 0,
        node_table_offset: 64,
        edge_table_offset: 64,
        content_block_offset: 64,
        feature_vec_offset: 64,
    };

    let mut buf = Vec::new();
    header.write_to(&mut buf).unwrap();

    // node_count is at offset 0x10 (16) and is 8 bytes
    let node_count_bytes = &buf[16..24];
    // Little-endian: least significant byte first
    assert_eq!(node_count_bytes[0], 0x08);
    assert_eq!(node_count_bytes[1], 0x07);
    assert_eq!(node_count_bytes[2], 0x06);
    assert_eq!(node_count_bytes[3], 0x05);
    assert_eq!(node_count_bytes[4], 0x04);
    assert_eq!(node_count_bytes[5], 0x03);
    assert_eq!(node_count_bytes[6], 0x02);
    assert_eq!(node_count_bytes[7], 0x01);
}

// ==================== Memory Graph Basic Tests ====================

#[test]
fn test_empty_graph() {
    let graph = MemoryGraph::new(DEFAULT_DIMENSION);
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
}

#[test]
fn test_add_single_node() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let event = CognitiveEventBuilder::new(EventType::Fact, "test").build();
    let id = graph.add_node(event).unwrap();

    assert_eq!(id, 0);
    assert_eq!(graph.node_count(), 1);
    assert!(graph.get_node(0).is_some());
    assert!(graph.get_node(1).is_none());
}

#[test]
fn test_add_multiple_nodes() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    for i in 0..10 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("fact {}", i)).build();
        let id = graph.add_node(event).unwrap();
        assert_eq!(id, i as u64);
    }
    assert_eq!(graph.node_count(), 10);
    for i in 0..10 {
        assert!(graph.get_node(i as u64).is_some());
    }
}

#[test]
fn test_add_edge() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let e1 = CognitiveEventBuilder::new(EventType::Fact, "fact").build();
    let e2 = CognitiveEventBuilder::new(EventType::Decision, "decision").build();
    let id1 = graph.add_node(e1).unwrap();
    let id2 = graph.add_node(e2).unwrap();

    let edge = Edge::new(id2, id1, EdgeType::CausedBy, 1.0);
    graph.add_edge(edge).unwrap();

    assert_eq!(graph.edge_count(), 1);
    let edges = graph.edges_from(id2);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].target_id, id1);

    let incoming = graph.edges_to(id1);
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].source_id, id2);
}

#[test]
fn test_add_edge_invalid_source() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let e1 = CognitiveEventBuilder::new(EventType::Fact, "fact").build();
    graph.add_node(e1).unwrap();

    let edge = Edge::new(999, 0, EdgeType::CausedBy, 1.0);
    let result = graph.add_edge(edge);
    assert!(result.is_err());
    match result.unwrap_err() {
        AmemError::NodeNotFound(999) => {}
        e => panic!("Expected NodeNotFound(999), got {:?}", e),
    }
}

#[test]
fn test_add_edge_invalid_target() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let e1 = CognitiveEventBuilder::new(EventType::Fact, "fact").build();
    graph.add_node(e1).unwrap();

    let edge = Edge::new(0, 999, EdgeType::CausedBy, 1.0);
    let result = graph.add_edge(edge);
    assert!(result.is_err());
    match result.unwrap_err() {
        AmemError::InvalidEdgeTarget(999) => {}
        e => panic!("Expected InvalidEdgeTarget(999), got {:?}", e),
    }
}

#[test]
fn test_remove_node() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let e0 = CognitiveEventBuilder::new(EventType::Fact, "a").build();
    let e1 = CognitiveEventBuilder::new(EventType::Fact, "b").build();
    let e2 = CognitiveEventBuilder::new(EventType::Fact, "c").build();
    let id0 = graph.add_node(e0).unwrap();
    let id1 = graph.add_node(e1).unwrap();
    let id2 = graph.add_node(e2).unwrap();

    graph
        .add_edge(Edge::new(id0, id1, EdgeType::RelatedTo, 1.0))
        .unwrap();
    graph
        .add_edge(Edge::new(id1, id2, EdgeType::RelatedTo, 1.0))
        .unwrap();

    // Remove middle node
    graph.remove_node(id1).unwrap();

    assert_eq!(graph.node_count(), 2);
    assert!(graph.get_node(id1).is_none());
    assert_eq!(graph.edge_count(), 0); // All edges involving id1 removed
}

#[test]
fn test_graph_from_parts() {
    let events = vec![
        {
            let mut e = CognitiveEventBuilder::new(EventType::Fact, "a").build();
            e.id = 0;
            e.feature_vec = vec![0.0; DEFAULT_DIMENSION];
            e
        },
        {
            let mut e = CognitiveEventBuilder::new(EventType::Decision, "b").build();
            e.id = 1;
            e.feature_vec = vec![0.0; DEFAULT_DIMENSION];
            e
        },
    ];
    let edges = vec![Edge::new(1, 0, EdgeType::CausedBy, 1.0)];

    let graph = MemoryGraph::from_parts(events, edges, DEFAULT_DIMENSION).unwrap();
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edge_count(), 1);
}

// ==================== File Format Tests ====================

#[test]
fn test_write_read_empty_graph() {
    let graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let tmp = NamedTempFile::new().unwrap();
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph, tmp.path()).unwrap();

    let loaded = AmemReader::read_from_file(tmp.path()).unwrap();
    assert_eq!(loaded.node_count(), 0);
    assert_eq!(loaded.edge_count(), 0);
}

#[test]
fn test_write_read_single_node() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let event = CognitiveEventBuilder::new(EventType::Fact, "User prefers Python")
        .session_id(1)
        .confidence(0.95)
        .build();
    graph.add_node(event).unwrap();

    let tmp = NamedTempFile::new().unwrap();
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph, tmp.path()).unwrap();

    let loaded = AmemReader::read_from_file(tmp.path()).unwrap();
    assert_eq!(loaded.node_count(), 1);

    let node = loaded.get_node(0).unwrap();
    assert_eq!(node.content, "User prefers Python");
    assert_eq!(node.event_type, EventType::Fact);
    assert_eq!(node.session_id, 1);
    assert!((node.confidence - 0.95).abs() < f32::EPSILON);
}

#[test]
fn test_write_read_many_nodes() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let types = [
        EventType::Fact,
        EventType::Decision,
        EventType::Inference,
        EventType::Correction,
        EventType::Skill,
        EventType::Episode,
    ];

    for i in 0..100 {
        let et = types[i % types.len()];
        let event = CognitiveEventBuilder::new(et, format!("content_{}", i))
            .session_id(i as u32 / 10)
            .confidence((i as f32 / 100.0).clamp(0.0, 1.0))
            .build();
        graph.add_node(event).unwrap();
    }

    let tmp = NamedTempFile::new().unwrap();
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph, tmp.path()).unwrap();

    let loaded = AmemReader::read_from_file(tmp.path()).unwrap();
    assert_eq!(loaded.node_count(), 100);

    for i in 0..100u64 {
        let node = loaded.get_node(i).unwrap();
        assert_eq!(node.content, format!("content_{}", i));
    }
}

#[test]
fn test_write_read_with_edges() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    for i in 0..10 {
        let event = CognitiveEventBuilder::new(EventType::Fact, format!("node_{}", i)).build();
        graph.add_node(event).unwrap();
    }

    // Create 20 edges
    for i in 0..10u64 {
        graph
            .add_edge(Edge::new(i, (i + 1) % 10, EdgeType::RelatedTo, 0.8))
            .unwrap();
        graph
            .add_edge(Edge::new(i, (i + 2) % 10, EdgeType::Supports, 0.6))
            .unwrap();
    }

    let tmp = NamedTempFile::new().unwrap();
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph, tmp.path()).unwrap();

    let loaded = AmemReader::read_from_file(tmp.path()).unwrap();
    assert_eq!(loaded.node_count(), 10);
    assert_eq!(loaded.edge_count(), 20);

    // Verify edge types
    for edge in loaded.edges() {
        assert!(edge.edge_type == EdgeType::RelatedTo || edge.edge_type == EdgeType::Supports);
    }
}

#[test]
fn test_write_read_feature_vectors() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);

    let mut fv: Vec<f32> = vec![0.0; DEFAULT_DIMENSION];
    for (i, val) in fv.iter_mut().enumerate() {
        *val = (i as f32) / (DEFAULT_DIMENSION as f32);
    }

    let event = CognitiveEventBuilder::new(EventType::Fact, "test")
        .feature_vec(fv.clone())
        .build();
    graph.add_node(event).unwrap();

    let tmp = NamedTempFile::new().unwrap();
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph, tmp.path()).unwrap();

    let loaded = AmemReader::read_from_file(tmp.path()).unwrap();
    let node = loaded.get_node(0).unwrap();

    for i in 0..DEFAULT_DIMENSION {
        assert!(
            (node.feature_vec[i] - fv[i]).abs() < 1e-6,
            "Feature vec mismatch at index {}: {} vs {}",
            i,
            node.feature_vec[i],
            fv[i]
        );
    }
}

#[test]
fn test_write_read_large_content() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let large_content = "x".repeat(60_000); // Near MAX_CONTENT_SIZE

    let event = CognitiveEventBuilder::new(EventType::Fact, &large_content).build();
    graph.add_node(event).unwrap();

    let tmp = NamedTempFile::new().unwrap();
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph, tmp.path()).unwrap();

    let loaded = AmemReader::read_from_file(tmp.path()).unwrap();
    let node = loaded.get_node(0).unwrap();
    assert_eq!(node.content, large_content);
}

#[test]
fn test_content_compression_actually_compresses() {
    let mut graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let repetitive = "hello world ".repeat(1000);

    let event = CognitiveEventBuilder::new(EventType::Fact, &repetitive).build();
    graph.add_node(event).unwrap();

    let tmp = NamedTempFile::new().unwrap();
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph, tmp.path()).unwrap();

    let file_size = std::fs::metadata(tmp.path()).unwrap().len();
    let uncompressed_size = repetitive.len();
    assert!(
        file_size < uncompressed_size as u64,
        "File size {} should be less than uncompressed content size {}",
        file_size,
        uncompressed_size
    );
}

#[test]
fn test_file_extension() {
    let tmp = tempfile::Builder::new().suffix(".amem").tempfile().unwrap();
    let graph = MemoryGraph::new(DEFAULT_DIMENSION);
    let writer = AmemWriter::new(DEFAULT_DIMENSION);
    writer.write_to_file(&graph, tmp.path()).unwrap();
    assert!(tmp.path().extension().unwrap() == "amem");
}
