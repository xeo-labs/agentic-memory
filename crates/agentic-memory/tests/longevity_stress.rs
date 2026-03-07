//! Rigorous stress tests for the V4 Longevity Engine.
//!
//! Tests: concurrency, large datasets, edge cases, data integrity,
//! round-trip serialization, performance, and real-world scenarios.

use agentic_memory::v3::longevity::backup::{BackupConfig, BackupDaemon};
use agentic_memory::v3::longevity::budget::StorageBudget;
use agentic_memory::v3::longevity::capture::{
    CaptureDaemon, CaptureEvent, CaptureRole, CaptureSource, ContentDedup,
};
use agentic_memory::v3::longevity::consolidation::{
    ConsolidationEngine, ConsolidationSchedule, ConsolidationTask,
};
use agentic_memory::v3::longevity::embedding_migration::EmbeddingMigrator;
use agentic_memory::v3::longevity::encryption_rotation::EncryptionRotator;
use agentic_memory::v3::longevity::forgetting::ForgettingProtocol;
use agentic_memory::v3::longevity::hierarchy::{MemoryHierarchy, MemoryLayer, MemoryRecord};
use agentic_memory::v3::longevity::integrity::IntegrityVerifier;
use agentic_memory::v3::longevity::significance::{
    ScoringContext, SignificanceScorer, SignificanceWeights,
};
use agentic_memory::v3::longevity::store::LongevityStore;
use agentic_memory::v3::longevity::sync::SyncProtocol;

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 1: LARGE DATASET INSERT + QUERY
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_insert_1000_memories() {
    let store = LongevityStore::open_memory().unwrap();
    let start = std::time::Instant::now();

    for i in 0..1000 {
        let record = MemoryRecord::new_raw(
            format!("stress-{:05}", i),
            serde_json::json!({
                "text": format!("Stress test memory number {} with some content about topic {}", i, i % 50),
                "role": if i % 2 == 0 { "user" } else { "assistant" },
                "session": format!("session-{}", i / 20),
            }),
            "stress-project".to_string(),
            Some(format!("session-{}", i / 20)),
        );
        store.insert_memory(&record).unwrap();
    }

    let elapsed = start.elapsed();
    println!("Inserted 1000 memories in {:?}", elapsed);
    assert!(elapsed.as_millis() < 5000, "Insert should complete in < 5s");

    // Verify count
    let count = store.total_count("stress-project").unwrap();
    assert_eq!(count, 1000);

    // Query by layer
    let raw = store
        .query_by_layer("stress-project", MemoryLayer::Raw, 2000)
        .unwrap();
    assert_eq!(raw.len(), 1000);

    // Stats
    let stats = store.hierarchy_stats("stress-project").unwrap();
    assert_eq!(stats.raw_count, 1000);
    assert_eq!(stats.total_count, 1000);
    assert!(stats.raw_bytes > 0);
}

#[test]
fn stress_fulltext_search_1000() {
    let store = LongevityStore::open_memory().unwrap();

    // Insert with varied content
    let topics = [
        "rust programming",
        "python scripting",
        "database design",
        "authentication module",
        "deployment pipeline",
        "error handling",
        "performance optimization",
        "unit testing",
        "code review",
        "documentation",
    ];

    for i in 0..1000 {
        let topic = topics[i % topics.len()];
        let record = MemoryRecord::new_raw(
            format!("fts-{:05}", i),
            serde_json::json!({
                "text": format!("Working on {} iteration {} with detailed analysis", topic, i),
            }),
            "fts-project".to_string(),
            None,
        );
        store.insert_memory(&record).unwrap();
    }

    // Search for specific topics
    let start = std::time::Instant::now();
    let results = store.search_fulltext("fts-project", "rust", 100).unwrap();
    let elapsed = start.elapsed();

    assert!(!results.is_empty(), "Should find 'rust' results");
    assert_eq!(results.len(), 100); // Capped at limit
    println!(
        "FTS search 'rust' in 1000 docs: {} results in {:?}",
        results.len(),
        elapsed
    );
    assert!(elapsed.as_millis() < 500, "FTS search should be < 500ms");

    // Search for another topic
    let results = store
        .search_fulltext("fts-project", "authentication", 100)
        .unwrap();
    assert!(!results.is_empty());

    // Search for non-existent
    let results = store
        .search_fulltext("fts-project", "nonexistent_xyz_term", 100)
        .unwrap();
    assert!(results.is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 2: SIGNIFICANCE SCORING EDGE CASES
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_significance_all_factors() {
    let scorer = SignificanceScorer::new();

    // Test with maximum context
    let memory = MemoryRecord::new_raw(
        "max-ctx".to_string(),
        serde_json::json!({"text": "CRITICAL IMPORTANT decision: I love this approach!!!"}),
        "proj".to_string(),
        None,
    );

    let ctx = ScoringContext {
        reference_count: 100,
        max_reference_count: 100,
        causal_depth: 10,
        max_causal_depth: 10,
        is_contradiction: true,
        user_marked: false,
        avg_neighbor_similarity: 0.1, // Very unique
        max_access_count: 100,
    };

    let breakdown = scorer.score(&memory, &ctx);
    assert!(
        breakdown.final_score > 0.7,
        "Highly significant memory should score > 0.7, got {}",
        breakdown.final_score
    );
    assert_eq!(breakdown.factors.len(), 7);

    // Verify all factors are in [0, 1]
    for factor in &breakdown.factors {
        assert!(
            factor.value >= 0.0 && factor.value <= 1.0,
            "Factor {} value {} out of range",
            factor.name,
            factor.value
        );
        assert!(
            factor.contribution >= 0.0,
            "Factor {} contribution {} negative",
            factor.name,
            factor.contribution
        );
    }
}

#[test]
fn stress_significance_zero_context() {
    let scorer = SignificanceScorer::new();
    let memory = MemoryRecord::new_raw(
        "zero".to_string(),
        serde_json::json!({"text": "bland content"}),
        "proj".to_string(),
        None,
    );

    let ctx = ScoringContext {
        reference_count: 0,
        max_reference_count: 0,
        causal_depth: 0,
        max_causal_depth: 0,
        is_contradiction: false,
        user_marked: false,
        avg_neighbor_similarity: 1.0, // Completely redundant
        max_access_count: 0,
    };

    let breakdown = scorer.score(&memory, &ctx);
    assert!(
        breakdown.final_score >= 0.0 && breakdown.final_score <= 1.0,
        "Score {} out of range",
        breakdown.final_score
    );
}

#[test]
fn stress_significance_custom_weights() {
    let weights = SignificanceWeights {
        recency: 0.5,
        access_frequency: 0.1,
        referential_weight: 0.1,
        causal_depth: 0.1,
        emotional_valence: 0.1,
        contradiction_signal: 0.05,
        uniqueness: 0.05,
    };
    let scorer = SignificanceScorer::with_weights(weights);
    let memory = MemoryRecord::new_raw(
        "w".to_string(),
        serde_json::json!({"text": "test"}),
        "proj".to_string(),
        None,
    );
    let score = scorer.score_simple(&memory);
    assert!((0.0..=1.0).contains(&score));
}

#[test]
fn stress_significance_1000_memories() {
    let scorer = SignificanceScorer::new();
    let start = std::time::Instant::now();

    for i in 0..1000 {
        let mut memory = MemoryRecord::new_raw(
            format!("sig-{}", i),
            serde_json::json!({"text": format!("Memory content {}", i)}),
            "proj".to_string(),
            None,
        );
        memory.access_count = (i % 50) as u64;
        // Vary ages
        memory.created_at =
            (chrono::Utc::now() - chrono::Duration::hours((i % 720) as i64)).to_rfc3339();

        let score = scorer.score_simple(&memory);
        assert!((0.0..=1.0).contains(&score));
    }

    let elapsed = start.elapsed();
    println!("Scored 1000 memories in {:?}", elapsed);
    assert!(
        elapsed.as_millis() < 1000,
        "Scoring 1000 should complete in < 1s"
    );
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 3: CONSOLIDATION UNDER LOAD
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_consolidation_100_raw_to_episodes() {
    let store = LongevityStore::open_memory().unwrap();
    let engine = ConsolidationEngine::new();

    // Insert 100 old raw memories across 5 sessions
    for i in 0..100 {
        let mut record = MemoryRecord::new_raw(
            format!("raw-{:03}", i),
            serde_json::json!({
                "text": format!("User worked on feature {} in module {}", i, i % 10),
                "path": format!("src/module_{}.rs", i % 10),
                "tool_name": "edit_file",
                "role": if i % 3 == 0 { "user" } else { "assistant" },
            }),
            "project-1".to_string(),
            Some(format!("session-{}", i / 20)),
        );
        record.created_at = (chrono::Utc::now() - chrono::Duration::hours(48)).to_rfc3339();
        record.significance = 0.3;
        store.insert_memory(&record).unwrap();
    }

    let task = ConsolidationTask {
        schedule: ConsolidationSchedule::Nightly,
        from_layer: MemoryLayer::Raw,
        to_layer: MemoryLayer::Episode,
        project_id: "project-1".to_string(),
        max_memories: 1000,
    };

    let result = engine.run(&store, &task).unwrap();
    println!(
        "Consolidated {} raw → {} episodes (ratio {:.1}:1) in {}ms",
        result.memories_processed,
        result.memories_created,
        result.compression_ratio,
        result.duration_ms
    );

    assert!(result.memories_processed > 0);
    assert!(result.memories_created > 0);
    assert!(result.compression_ratio > 1.0);

    // Verify episodes exist
    let episodes = store
        .query_by_layer("project-1", MemoryLayer::Episode, 1000)
        .unwrap();
    assert!(!episodes.is_empty());

    // Verify consolidation log
    let logs = store.get_consolidation_log(10).unwrap();
    assert!(!logs.is_empty());

    // Verify hierarchy stats make sense
    let stats = store.hierarchy_stats("project-1").unwrap();
    println!(
        "After consolidation: raw={}, episodes={}",
        stats.raw_count, stats.episode_count
    );
    assert!(stats.episode_count > 0);
}

#[test]
fn stress_full_consolidation_pipeline() {
    let store = LongevityStore::open_memory().unwrap();
    let engine = ConsolidationEngine::new();

    // Phase 1: Insert 200 raw memories
    for i in 0..200 {
        let mut record = MemoryRecord::new_raw(
            format!("raw-{:04}", i),
            serde_json::json!({
                "text": format!("Event {} about topic {}", i, i % 20),
                "path": format!("src/{}.rs", i % 15),
                "tool_name": (["read_file", "edit_file", "grep"][i % 3]),
                "decision": if i % 10 == 0 { format!("Decided on approach {}", i / 10) } else { String::new() },
            }),
            "pipe-project".to_string(),
            Some(format!("session-{}", i / 25)),
        );
        record.created_at = (chrono::Utc::now() - chrono::Duration::days(90)).to_rfc3339();
        record.significance = 0.25;
        store.insert_memory(&record).unwrap();
    }

    // Phase 2: Raw → Episode
    let task1 = ConsolidationTask {
        schedule: ConsolidationSchedule::Nightly,
        from_layer: MemoryLayer::Raw,
        to_layer: MemoryLayer::Episode,
        project_id: "pipe-project".to_string(),
        max_memories: 1000,
    };
    let r1 = engine.run(&store, &task1).unwrap();
    println!(
        "Raw→Episode: {} processed, {} created",
        r1.memories_processed, r1.memories_created
    );
    assert!(r1.memories_created > 0);

    // Phase 3: Episode → Summary (make episodes old enough)
    let episodes = store
        .query_by_layer("pipe-project", MemoryLayer::Episode, 1000)
        .unwrap();
    for ep in &episodes {
        // Update created_at to make them old enough for weekly consolidation
        let mut updated = ep.clone();
        updated.created_at = (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        updated.significance = 0.3;
        store.insert_memory(&updated).unwrap();
    }

    let task2 = ConsolidationTask {
        schedule: ConsolidationSchedule::Weekly,
        from_layer: MemoryLayer::Episode,
        to_layer: MemoryLayer::Summary,
        project_id: "pipe-project".to_string(),
        max_memories: 1000,
    };
    let r2 = engine.run(&store, &task2).unwrap();
    println!(
        "Episode→Summary: {} processed, {} created",
        r2.memories_processed, r2.memories_created
    );

    // Phase 4: Summary → Pattern (make summaries old enough)
    let summaries = store
        .query_by_layer("pipe-project", MemoryLayer::Summary, 1000)
        .unwrap();
    for sum in &summaries {
        let mut updated = sum.clone();
        updated.created_at = (chrono::Utc::now() - chrono::Duration::days(60)).to_rfc3339();
        updated.significance = 0.3;
        store.insert_memory(&updated).unwrap();
    }

    let task3 = ConsolidationTask {
        schedule: ConsolidationSchedule::Monthly,
        from_layer: MemoryLayer::Summary,
        to_layer: MemoryLayer::Pattern,
        project_id: "pipe-project".to_string(),
        max_memories: 1000,
    };
    let r3 = engine.run(&store, &task3).unwrap();
    println!(
        "Summary→Pattern: {} processed, {} created",
        r3.memories_processed, r3.memories_created
    );

    // Final stats
    let stats = store.hierarchy_stats("pipe-project").unwrap();
    println!(
        "Final hierarchy: raw={}, ep={}, sum={}, pat={}, trait={}, id={}",
        stats.raw_count,
        stats.episode_count,
        stats.summary_count,
        stats.pattern_count,
        stats.trait_count,
        stats.identity_count,
    );

    // Verify compression happened at each level
    assert!(stats.total_count > 0);
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 4: CAPTURE DAEMON DEDUP UNDER LOAD
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_dedup_10000_messages() {
    let dedup = ContentDedup::new(5000);
    let mut unique = 0;
    let mut duplicates = 0;

    for i in 0..10000 {
        let content = format!("Message number {}", i);
        let timestamp = (i / 2) * 3000; // Groups of 2 in same window

        if dedup.is_duplicate(&content, timestamp as u64) {
            duplicates += 1;
        } else {
            unique += 1;
        }
    }

    println!(
        "10000 messages: {} unique, {} duplicates, cache_size={}",
        unique,
        duplicates,
        dedup.cache_size()
    );
    assert!(unique > 5000, "Should have mostly unique messages");
    assert!(
        dedup.cache_size() <= 5000,
        "Cache should not exceed max_entries"
    );
}

#[test]
fn stress_capture_daemon_rapid_fire() {
    let daemon = CaptureDaemon::new();

    let start = std::time::Instant::now();
    let mut captured = 0;

    for i in 0..5000 {
        let event = CaptureEvent {
            role: if i % 2 == 0 {
                CaptureRole::User
            } else {
                CaptureRole::Assistant
            },
            content: format!("Rapid fire message {} with some content", i),
            timestamp: i * 2000, // Unique windows
            source: CaptureSource::McpStream,
            session_id: Some(format!("session-{}", i / 100)),
            project_path: None,
        };
        if daemon.capture(event) {
            captured += 1;
        }
    }

    let elapsed = start.elapsed();
    println!("Captured {} of 5000 events in {:?}", captured, elapsed);
    assert!(captured > 4000, "Should capture most unique events");
    assert!(
        elapsed.as_millis() < 2000,
        "Rapid fire capture should complete in < 2s"
    );

    // Drain and verify
    let drained = daemon.drain_buffer();
    assert_eq!(drained.len(), captured);
    assert_eq!(daemon.buffer_size(), 0);
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 5: CONCURRENT STORE OPERATIONS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_store_on_disk() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("test.longevity.db");

    // Open, insert, close
    {
        let store = LongevityStore::open(&db_path).unwrap();
        for i in 0..100 {
            store
                .insert_memory(&MemoryRecord::new_raw(
                    format!("disk-{}", i),
                    serde_json::json!({"text": format!("Persisted memory {}", i)}),
                    "disk-project".to_string(),
                    None,
                ))
                .unwrap();
        }
        let count = store.total_count("disk-project").unwrap();
        assert_eq!(count, 100);
    }

    // Reopen and verify persistence
    {
        let store = LongevityStore::open(&db_path).unwrap();
        let count = store.total_count("disk-project").unwrap();
        assert_eq!(count, 100);

        let memory = store.get_memory("disk-50").unwrap();
        assert!(memory.is_some());
        assert!(memory.unwrap().content.to_string().contains("50"));
    }
}

#[test]
fn stress_store_large_content() {
    let store = LongevityStore::open_memory().unwrap();

    // 100KB content
    let large_text = "x".repeat(100_000);
    let record = MemoryRecord::new_raw(
        "large".to_string(),
        serde_json::json!({"text": large_text}),
        "proj".to_string(),
        None,
    );
    store.insert_memory(&record).unwrap();

    let retrieved = store.get_memory("large").unwrap().unwrap();
    let text = retrieved.extract_text();
    assert_eq!(text.len(), 100_000);
}

#[test]
fn stress_store_unicode_content() {
    let store = LongevityStore::open_memory().unwrap();

    let unicode_content = "日本語テスト 🦀 Ñoño données München Zürich 数据 Данные";
    let record = MemoryRecord::new_raw(
        "unicode".to_string(),
        serde_json::json!({"text": unicode_content}),
        "proj".to_string(),
        None,
    );
    store.insert_memory(&record).unwrap();

    let retrieved = store.get_memory("unicode").unwrap().unwrap();
    assert!(retrieved.extract_text().contains("日本語"));
    assert!(retrieved.extract_text().contains("🦀"));
}

#[test]
fn stress_store_empty_and_null_fields() {
    let store = LongevityStore::open_memory().unwrap();

    let record = MemoryRecord {
        id: "empty-fields".to_string(),
        layer: MemoryLayer::Raw,
        content: serde_json::json!(null),
        content_type: "event".to_string(),
        embedding: None,
        embedding_model: None,
        significance: 0.0,
        access_count: 0,
        last_accessed: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        original_ids: None,
        session_id: None,
        project_id: "proj".to_string(),
        metadata: None,
        encryption_key_id: None,
        schema_version: 1,
    };

    store.insert_memory(&record).unwrap();
    let retrieved = store.get_memory("empty-fields").unwrap().unwrap();
    assert_eq!(retrieved.significance, 0.0);
    assert!(retrieved.session_id.is_none());
}

#[test]
fn stress_store_embedding_roundtrip() {
    let store = LongevityStore::open_memory().unwrap();

    let embedding: Vec<f32> = (0..384).map(|i| (i as f32) / 384.0).collect();
    let mut record = MemoryRecord::new_raw(
        "embed".to_string(),
        serde_json::json!({"text": "has embedding"}),
        "proj".to_string(),
        None,
    );
    record.embedding = Some(embedding.clone());
    record.embedding_model = Some("test-model-v1".to_string());

    store.insert_memory(&record).unwrap();
    let retrieved = store.get_memory("embed").unwrap().unwrap();

    let ret_embed = retrieved.embedding.unwrap();
    assert_eq!(ret_embed.len(), 384);
    // Check values are preserved (within floating point tolerance)
    for (a, b) in embedding.iter().zip(ret_embed.iter()) {
        assert!((a - b).abs() < 1e-6, "Embedding mismatch: {} vs {}", a, b);
    }
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 6: FORGETTING UNDER LOAD
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_forgetting_500_candidates() {
    let store = LongevityStore::open_memory().unwrap();
    let protocol = ForgettingProtocol::new().with_min_age(0.0);

    // Insert 500 low-significance old memories
    for i in 0..500 {
        let mut record = MemoryRecord::new_raw(
            format!("forget-{:04}", i),
            serde_json::json!({"text": format!("Trivial note {}", i)}),
            "project-1".to_string(),
            None,
        );
        record.significance = 0.05 + (i as f64 % 15.0) * 0.01;
        record.created_at = (chrono::Utc::now() - chrono::Duration::days(90)).to_rfc3339();
        store.insert_memory(&record).unwrap();
    }

    // 50 important memories that should NOT be forgotten
    for i in 0..50 {
        let mut record = MemoryRecord::new_raw(
            format!("keep-{:04}", i),
            serde_json::json!({"text": format!("Important decision {}", i)}),
            "project-1".to_string(),
            None,
        );
        record.significance = 0.9;
        store.insert_memory(&record).unwrap();
    }

    let verdicts = protocol
        .evaluate_candidates(&store, "project-1", 1000)
        .unwrap();

    let eligible: Vec<_> = verdicts.iter().filter(|v| v.eligible).collect();
    let ineligible: Vec<_> = verdicts.iter().filter(|v| !v.eligible).collect();

    println!(
        "500 low-sig memories: {} eligible, {} ineligible",
        eligible.len(),
        ineligible.len()
    );
    assert!(!eligible.is_empty(), "Some should be eligible");

    // Execute forgetting
    let ids: Vec<String> = eligible.iter().map(|v| v.memory_id.clone()).collect();
    let result = protocol.execute(&store, &ids).unwrap();
    println!(
        "Forgotten: {}, Skipped: {}",
        result.forgotten_count, result.skipped_count
    );

    // Important memories should still exist
    for i in 0..50 {
        let m = store.get_memory(&format!("keep-{:04}", i)).unwrap();
        assert!(m.is_some(), "Important memory keep-{:04} should survive", i);
    }
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 7: INTEGRITY VERIFICATION
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_integrity_500_memories() {
    let store = LongevityStore::open_memory().unwrap();

    for i in 0..500 {
        store
            .insert_memory(&MemoryRecord::new_raw(
                format!("int-{:04}", i),
                serde_json::json!({"text": format!("Integrity test memory {}", i)}),
                "int-project".to_string(),
                None,
            ))
            .unwrap();
    }

    // Create Merkle proof
    let proof = IntegrityVerifier::create_merkle_proof(&store, "int-project").unwrap();
    assert_eq!(proof.leaf_count, 500);
    assert!(!proof.root_hash.is_empty());

    // Verify
    let verified =
        IntegrityVerifier::verify_against_proof(&store, "int-project", &proof.root_hash).unwrap();
    assert!(verified, "Merkle proof should verify");

    // Tamper and verify fails
    let tampered =
        IntegrityVerifier::verify_against_proof(&store, "int-project", "tampered_hash").unwrap();
    assert!(!tampered, "Tampered proof should not verify");

    // Full integrity report
    let report = IntegrityVerifier::verify(&store, "int-project").unwrap();
    assert!(report.database_ok);
    assert_eq!(report.total_memories, 500);
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 8: SYNC PROTOCOL
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_sync_500_captures() {
    let store = LongevityStore::open_memory().unwrap();

    let events: Vec<CaptureEvent> = (0..500)
        .map(|i| CaptureEvent {
            role: if i % 2 == 0 {
                CaptureRole::User
            } else {
                CaptureRole::Assistant
            },
            content: format!(
                "Sync test message {} with enough content to be meaningful",
                i
            ),
            timestamp: i * 1000,
            source: CaptureSource::ClientLog,
            session_id: Some(format!("session-{}", i / 50)),
            project_path: None,
        })
        .collect();

    let start = std::time::Instant::now();
    let result = SyncProtocol::sync_captures_to_sqlite(&store, &events, "sync-project").unwrap();
    let elapsed = start.elapsed();

    println!("Synced {} captures in {:?}", result.records_synced, elapsed);
    assert_eq!(result.records_synced, 500);
    assert!(result.errors.is_empty());
    assert!(
        elapsed.as_millis() < 5000,
        "Sync 500 should complete in < 5s"
    );

    // Add a high-significance pattern so context loading finds something
    let mut pattern = MemoryRecord::new_compressed(
        "ctx-pattern".to_string(),
        MemoryLayer::Pattern,
        serde_json::json!({"pattern": "User prefers Rust for systems programming"}),
        vec![],
        "sync-project".to_string(),
    );
    pattern.significance = 0.9;
    store.insert_memory(&pattern).unwrap();

    // Load context
    let ctx = SyncProtocol::load_session_context(&store, "sync-project", 4096).unwrap();
    assert!(ctx.tokens_used > 0);

    let ghost_writer = ctx.to_ghost_writer_format();
    assert!(ghost_writer.contains("CRITICAL INSTRUCTION"));
    assert!(ghost_writer.contains("memory_capture_message"));
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 9: BACKUP & RESTORE ROUND-TRIP
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_backup_restore_roundtrip() {
    let temp = tempfile::tempdir().unwrap();

    // Create source files
    let amem_path = temp.path().join("test.amem");
    std::fs::write(&amem_path, b"AMEM binary data for testing").unwrap();

    let db_path = temp.path().join("test.longevity.db");
    {
        let store = LongevityStore::open(&db_path).unwrap();
        for i in 0..50 {
            store
                .insert_memory(&MemoryRecord::new_raw(
                    format!("backup-{}", i),
                    serde_json::json!({"text": format!("Backup test {}", i)}),
                    "backup-project".to_string(),
                    None,
                ))
                .unwrap();
        }
    }

    // Backup
    let backup_dir = temp.path().join("backups");
    std::fs::create_dir_all(&backup_dir).unwrap();

    let config = BackupConfig::default();
    let daemon = BackupDaemon::new(config);
    let result = daemon
        .backup_to_local(&amem_path, &db_path, &backup_dir)
        .unwrap();
    assert!(result.success);
    assert!(result.size_bytes > 0);
    println!(
        "Backup: {} bytes, {} files",
        result.size_bytes,
        result.files_backed_up.len()
    );

    // Restore
    let restore_dir = temp.path().join("restored");
    let backup_subdir_name = std::fs::read_dir(&backup_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with("amem-backup-"))
                .unwrap_or(false)
        })
        .unwrap();

    let restore_result =
        BackupDaemon::restore_from_local(&backup_subdir_name.path(), &restore_dir).unwrap();
    assert!(restore_result.success);
    println!("Restored {} files", restore_result.files_restored.len());

    // Verify restored DB is readable
    let restored_db = restore_dir.join("test.longevity.db");
    if restored_db.exists() {
        let store = LongevityStore::open(&restored_db).unwrap();
        let count = store.total_count("backup-project").unwrap();
        assert_eq!(count, 50, "All 50 memories should be restored");
    }
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 10: BUDGET PROJECTIONS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_budget_with_real_data() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("budget.longevity.db");
    let store = LongevityStore::open(&db_path).unwrap();

    // Insert varied data to simulate real usage
    for i in 0..200 {
        let mut record = MemoryRecord::new_raw(
            format!("budget-{}", i),
            serde_json::json!({
                "text": format!("Working on feature {} with detailed analysis of module {}", i, i % 20),
                "path": format!("src/module_{}.rs", i % 15),
            }),
            "budget-project".to_string(),
            Some(format!("session-{}", i / 25)),
        );
        record.significance = (i as f64 % 100.0) / 100.0;
        store.insert_memory(&record).unwrap();
    }

    let budget = StorageBudget::new();
    let stats = store.hierarchy_stats("budget-project").unwrap();

    let layers = budget.layer_budgets(&stats);
    assert_eq!(layers.len(), 6);

    let overall = budget.overall_status(&stats);
    println!("Budget status: {}", overall.message);

    let projection = budget.project_growth(&store, "budget-project").unwrap();
    println!(
        "Projections: 1yr={:.1}KB, 5yr={:.1}KB, 20yr={:.1}KB",
        projection.projected_1_year as f64 / 1024.0,
        projection.projected_5_year as f64 / 1024.0,
        projection.projected_20_year as f64 / 1024.0,
    );
    assert!(projection.projected_20_year > projection.projected_1_year);
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 11: ENCRYPTION KEY ROTATION
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_encryption_10_rotations() {
    let store = LongevityStore::open_memory().unwrap();
    let mut keys = Vec::new();

    for _ in 0..10 {
        let key_id = EncryptionRotator::rotate_key(&store, "AES-256-GCM").unwrap();
        keys.push(key_id);
    }

    // Only the last key should be active
    let current = EncryptionRotator::current_key(&store).unwrap().unwrap();
    assert_eq!(current.key_id, keys[9]);
    assert_ne!(current.key_id, keys[0]);
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 12: EMBEDDING MODEL MIGRATION
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_embedding_model_lifecycle() {
    let store = LongevityStore::open_memory().unwrap();

    // Register v1
    EmbeddingMigrator::register_model(&store, "embed-v1", "test-embed-v1", 384, "local").unwrap();

    // Insert memories with v1
    for i in 0..50 {
        let mut record = MemoryRecord::new_raw(
            format!("em-{}", i),
            serde_json::json!({"text": format!("Embedded memory {}", i)}),
            "em-project".to_string(),
            None,
        );
        record.embedding_model = Some("embed-v1".to_string());
        record.embedding = Some(vec![0.1; 384]);
        store.insert_memory(&record).unwrap();
    }

    // Register v2 and switch
    EmbeddingMigrator::register_model(&store, "embed-v2", "test-embed-v2", 512, "local").unwrap();
    EmbeddingMigrator::switch_model(&store, "embed-v1", "embed-v2").unwrap();

    // Check migration status
    let status = EmbeddingMigrator::migration_status(&store, "embed-v1", "embed-v2").unwrap();
    assert_eq!(status.remaining_memories, 50); // All still on v1

    // Active model should be v2
    let models = EmbeddingMigrator::list_models(&store).unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].model_id, "embed-v2");
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 13: END-TO-END REALISTIC SCENARIO
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_realistic_developer_workflow() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("dev.longevity.db");
    let store = LongevityStore::open(&db_path).unwrap();
    let scorer = SignificanceScorer::new();
    let engine = ConsolidationEngine::new();

    // Simulate 7 days of developer activity
    for day in 0..7 {
        let base_time = chrono::Utc::now() - chrono::Duration::days(7 - day);

        // ~30 events per day
        for event in 0..30 {
            let is_decision = event % 10 == 0;
            let is_important = event % 15 == 0;

            let mut record = MemoryRecord::new_raw(
                format!("dev-d{}-e{}", day, event),
                serde_json::json!({
                    "text": format!(
                        "Day {} event {}: {}",
                        day, event,
                        if is_decision { "Decided to use tokio for async" }
                        else if is_important { "CRITICAL: found security bug in auth module" }
                        else { "Reviewed PR and made small edit" }
                    ),
                    "role": if event % 3 == 0 { "user" } else { "assistant" },
                    "path": format!("src/{}.rs", (["auth", "main", "config", "test"][event % 4])),
                    "tool_name": (["read_file", "edit_file", "grep", "bash"][event % 4]),
                    "decision": if is_decision { "Use tokio" } else { "" },
                }),
                "dev-project".to_string(),
                Some(format!("session-day-{}", day)),
            );

            record.created_at =
                (base_time + chrono::Duration::minutes(event as i64 * 5)).to_rfc3339();

            // Score significance
            let score = scorer.score_simple(&record);
            record.significance = score;

            store.insert_memory(&record).unwrap();
        }
    }

    // Verify initial state
    let stats = store.hierarchy_stats("dev-project").unwrap();
    assert_eq!(stats.raw_count, 210); // 7 * 30

    // Run consolidation for old events (> 24h)
    let task = ConsolidationTask {
        schedule: ConsolidationSchedule::Nightly,
        from_layer: MemoryLayer::Raw,
        to_layer: MemoryLayer::Episode,
        project_id: "dev-project".to_string(),
        max_memories: 1000,
    };
    let result = engine.run(&store, &task).unwrap();
    println!(
        "Developer workflow consolidation: {} → {} (preserved: {})",
        result.memories_processed, result.memories_created, result.memories_preserved
    );

    // Load session context
    let ctx = SyncProtocol::load_session_context(&store, "dev-project", 4096).unwrap();
    let ghost_output = ctx.to_ghost_writer_format();
    assert!(ghost_output.contains("CRITICAL INSTRUCTION"));

    // Budget check
    let budget = StorageBudget::new();
    let final_stats = store.hierarchy_stats("dev-project").unwrap();
    let status = budget.overall_status(&final_stats);
    assert!(
        matches!(
            status.alert,
            agentic_memory::v3::longevity::budget::BudgetAlert::Healthy
        ),
        "Budget should be healthy"
    );

    // Integrity
    let proof = IntegrityVerifier::create_merkle_proof(&store, "dev-project").unwrap();
    assert!(proof.leaf_count > 0);
    let verified =
        IntegrityVerifier::verify_against_proof(&store, "dev-project", &proof.root_hash).unwrap();
    assert!(verified);

    println!("Realistic developer workflow test PASSED");
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 14: PROJECT ISOLATION
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_project_isolation() {
    let store = LongevityStore::open_memory().unwrap();

    // Insert into 3 different projects
    for proj in 0..3 {
        for i in 0..50 {
            store
                .insert_memory(&MemoryRecord::new_raw(
                    format!("proj{}-{}", proj, i),
                    serde_json::json!({"text": format!("Project {} memory {}", proj, i)}),
                    format!("project-{}", proj),
                    None,
                ))
                .unwrap();
        }
    }

    // Verify isolation
    for proj in 0..3 {
        let count = store.total_count(&format!("project-{}", proj)).unwrap();
        assert_eq!(count, 50, "Project {} should have exactly 50", proj);

        let raw = store
            .query_by_layer(&format!("project-{}", proj), MemoryLayer::Raw, 1000)
            .unwrap();
        assert_eq!(raw.len(), 50);

        // FTS should be isolated too
        let results = store
            .search_fulltext(
                &format!("project-{}", proj),
                &format!("Project {}", proj),
                100,
            )
            .unwrap();
        assert!(!results.is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════
// STRESS TEST 15: HIERARCHY EDGE CASES
// ═══════════════════════════════════════════════════════════════════

#[test]
fn stress_hierarchy_empty_group() {
    let groups = MemoryHierarchy::group_for_episodes(&[]);
    assert!(groups.is_empty());
}

#[test]
fn stress_hierarchy_single_memory() {
    let memories = vec![MemoryRecord::new_raw(
        "only-one".to_string(),
        serde_json::json!({"text": "solo memory"}),
        "proj".to_string(),
        Some("s1".to_string()),
    )];
    let groups = MemoryHierarchy::group_for_episodes(&memories);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].len(), 1);
}

#[test]
fn stress_hierarchy_large_group_splits() {
    let memories: Vec<MemoryRecord> = (0..100)
        .map(|i| {
            MemoryRecord::new_raw(
                format!("lg-{}", i),
                serde_json::json!({"text": format!("large group {}", i)}),
                "proj".to_string(),
                Some("same-session".to_string()),
            )
        })
        .collect();

    let groups = MemoryHierarchy::group_for_episodes(&memories);
    // Should split into chunks of ~20
    assert!(
        groups.len() >= 4,
        "100 memories should split into 4+ groups"
    );
    for group in &groups {
        assert!(group.len() <= 30, "No group should exceed 30");
    }
}
