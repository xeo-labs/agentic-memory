use rand::Rng;
use std::time::Instant;

use agentic_memory::engine::QueryEngine;
use agentic_memory::graph::MemoryGraph;
use agentic_memory::types::{CognitiveEventBuilder, Edge, EdgeType, EventType, DEFAULT_DIMENSION};
use agentic_memory::{
    AnalogicalAnchor, AnalogicalParams, ConsolidationOp, ConsolidationParams, DriftParams,
    GapDetectionParams, GapSeverity,
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

    for &n in &[10_000usize, 100_000] {
        println!("\n=== Graph size: {} nodes ===", n);
        let mut graph = make_graph(n, 3);

        // Gap detection
        let start = Instant::now();
        let params = GapDetectionParams {
            confidence_threshold: 0.5,
            min_support_count: 2,
            max_results: 50,
            session_range: None,
            sort_by: GapSeverity::HighestImpact,
        };
        let r = qe.gap_detection(&graph, params);
        let elapsed = start.elapsed();
        println!("gap_detection_{}: {:?} (ok={})", n, elapsed, r.is_ok());

        // Analogical
        let start = Instant::now();
        let params = AnalogicalParams {
            anchor: AnalogicalAnchor::Node(n as u64 / 2),
            context_depth: 2,
            max_results: 5,
            min_similarity: 0.0,
            exclude_sessions: vec![],
        };
        let r = qe.analogical(&graph, params);
        let elapsed = start.elapsed();
        println!("analogical_{}: {:?} (ok={})", n, elapsed, r.is_ok());

        // Consolidation (dry run)
        let start = Instant::now();
        let params = ConsolidationParams {
            session_range: None,
            operations: vec![
                ConsolidationOp::DeduplicateFacts { threshold: 0.9 },
                ConsolidationOp::PruneOrphans { max_decay: 0.1 },
            ],
            dry_run: true,
            backup_path: None,
        };
        let r = qe.consolidate(&mut graph, params);
        let elapsed = start.elapsed();
        println!(
            "consolidation_dryrun_{}: {:?} (ok={})",
            n,
            elapsed,
            r.is_ok()
        );

        // Drift detection
        let start = Instant::now();
        let params = DriftParams {
            topic: "database query optimization".to_string(),
            topic_vec: None,
            max_results: 10,
            min_relevance: 0.1,
        };
        let r = qe.drift_detection(&graph, params);
        let elapsed = start.elapsed();
        println!("drift_detection_{}: {:?} (ok={})", n, elapsed, r.is_ok());
    }
}
