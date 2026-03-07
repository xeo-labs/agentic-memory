//! Comprehensive tests for the V4 Longevity Engine.

use super::*;
use super::capture::{CaptureRole, CaptureSource};

// ═══════════════════════════════════════════════════════════════════
// PHASE 1: HIERARCHY & TYPES
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_memory_layer_ordering() {
    assert!(MemoryLayer::Raw < MemoryLayer::Episode);
    assert!(MemoryLayer::Episode < MemoryLayer::Summary);
    assert!(MemoryLayer::Summary < MemoryLayer::Pattern);
    assert!(MemoryLayer::Pattern < MemoryLayer::Trait);
    assert!(MemoryLayer::Trait < MemoryLayer::Identity);
}

#[test]
fn test_memory_layer_from_u8() {
    assert_eq!(MemoryLayer::from_u8(0), Some(MemoryLayer::Raw));
    assert_eq!(MemoryLayer::from_u8(1), Some(MemoryLayer::Episode));
    assert_eq!(MemoryLayer::from_u8(2), Some(MemoryLayer::Summary));
    assert_eq!(MemoryLayer::from_u8(3), Some(MemoryLayer::Pattern));
    assert_eq!(MemoryLayer::from_u8(4), Some(MemoryLayer::Trait));
    assert_eq!(MemoryLayer::from_u8(5), Some(MemoryLayer::Identity));
    assert_eq!(MemoryLayer::from_u8(6), None);
}

#[test]
fn test_memory_layer_next() {
    assert_eq!(MemoryLayer::Raw.next_layer(), Some(MemoryLayer::Episode));
    assert_eq!(MemoryLayer::Trait.next_layer(), Some(MemoryLayer::Identity));
    assert_eq!(MemoryLayer::Identity.next_layer(), None);
}

#[test]
fn test_memory_layer_content_types() {
    assert_eq!(MemoryLayer::Raw.content_type(), "event");
    assert_eq!(MemoryLayer::Episode.content_type(), "episode");
    assert_eq!(MemoryLayer::Summary.content_type(), "summary");
    assert_eq!(MemoryLayer::Pattern.content_type(), "pattern");
    assert_eq!(MemoryLayer::Trait.content_type(), "trait");
    assert_eq!(MemoryLayer::Identity.content_type(), "identity");
}

#[test]
fn test_memory_layer_all() {
    let all = MemoryLayer::all();
    assert_eq!(all.len(), 6);
}

#[test]
fn test_memory_record_new_raw() {
    let record = MemoryRecord::new_raw(
        "test-id".to_string(),
        serde_json::json!({"text": "hello world"}),
        "project-1".to_string(),
        Some("session-1".to_string()),
    );
    assert_eq!(record.layer, MemoryLayer::Raw);
    assert_eq!(record.significance, 0.5);
    assert_eq!(record.access_count, 0);
    assert_eq!(record.project_id, "project-1");
}

#[test]
fn test_memory_record_new_compressed() {
    let record = MemoryRecord::new_compressed(
        "episode-1".to_string(),
        MemoryLayer::Episode,
        serde_json::json!({"summary": "test episode"}),
        vec!["raw-1".to_string(), "raw-2".to_string()],
        "project-1".to_string(),
    );
    assert_eq!(record.layer, MemoryLayer::Episode);
    assert_eq!(record.original_ids.as_ref().unwrap().len(), 2);
}

#[test]
fn test_memory_record_extract_text() {
    let record = MemoryRecord::new_raw(
        "id".to_string(),
        serde_json::json!({"text": "hello", "summary": "world"}),
        "proj".to_string(),
        None,
    );
    let text = record.extract_text();
    assert!(text.contains("hello"));
    assert!(text.contains("world"));
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 2: LONGEVITY STORE (SQLite)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_store_open_memory() {
    let store = LongevityStore::open_memory().unwrap();
    let version = store.current_schema_version().unwrap();
    assert_eq!(version, 1);
}

#[test]
fn test_store_insert_and_get() {
    let store = LongevityStore::open_memory().unwrap();
    let record = MemoryRecord::new_raw(
        "test-1".to_string(),
        serde_json::json!({"text": "hello world"}),
        "project-1".to_string(),
        Some("session-1".to_string()),
    );

    store.insert_memory(&record).unwrap();
    let retrieved = store.get_memory("test-1").unwrap();
    assert!(retrieved.is_some());

    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id, "test-1");
    assert_eq!(retrieved.layer, MemoryLayer::Raw);
    assert_eq!(retrieved.project_id, "project-1");
}

#[test]
fn test_store_query_by_layer() {
    let store = LongevityStore::open_memory().unwrap();

    for i in 0..5 {
        let record = MemoryRecord::new_raw(
            format!("raw-{}", i),
            serde_json::json!({"text": format!("event {}", i)}),
            "project-1".to_string(),
            None,
        );
        store.insert_memory(&record).unwrap();
    }

    let episode = MemoryRecord::new_compressed(
        "episode-1".to_string(),
        MemoryLayer::Episode,
        serde_json::json!({"summary": "test episode"}),
        vec!["raw-0".to_string()],
        "project-1".to_string(),
    );
    store.insert_memory(&episode).unwrap();

    let raw = store.query_by_layer("project-1", MemoryLayer::Raw, 100).unwrap();
    assert_eq!(raw.len(), 5);

    let episodes = store.query_by_layer("project-1", MemoryLayer::Episode, 100).unwrap();
    assert_eq!(episodes.len(), 1);
}

#[test]
fn test_store_query_by_significance() {
    let store = LongevityStore::open_memory().unwrap();

    let mut low = MemoryRecord::new_raw(
        "low".to_string(),
        serde_json::json!({"text": "not important"}),
        "project-1".to_string(),
        None,
    );
    low.significance = 0.1;
    store.insert_memory(&low).unwrap();

    let mut high = MemoryRecord::new_raw(
        "high".to_string(),
        serde_json::json!({"text": "very important"}),
        "project-1".to_string(),
        None,
    );
    high.significance = 0.9;
    store.insert_memory(&high).unwrap();

    let low_results = store.query_by_significance("project-1", 0.0, 0.5, 100).unwrap();
    assert_eq!(low_results.len(), 1);
    assert_eq!(low_results[0].id, "low");

    let high_results = store.query_by_significance("project-1", 0.5, 1.0, 100).unwrap();
    assert_eq!(high_results.len(), 1);
    assert_eq!(high_results[0].id, "high");
}

#[test]
fn test_store_fulltext_search() {
    let store = LongevityStore::open_memory().unwrap();

    let r1 = MemoryRecord::new_raw(
        "r1".to_string(),
        serde_json::json!({"text": "rust programming language"}),
        "project-1".to_string(),
        None,
    );
    store.insert_memory(&r1).unwrap();

    let r2 = MemoryRecord::new_raw(
        "r2".to_string(),
        serde_json::json!({"text": "python scripting"}),
        "project-1".to_string(),
        None,
    );
    store.insert_memory(&r2).unwrap();

    let results = store.search_fulltext("project-1", "rust", 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "r1");
}

#[test]
fn test_store_update_significance() {
    let store = LongevityStore::open_memory().unwrap();
    let record = MemoryRecord::new_raw(
        "test".to_string(),
        serde_json::json!({"text": "test"}),
        "project-1".to_string(),
        None,
    );
    store.insert_memory(&record).unwrap();

    store.update_significance("test", 0.95).unwrap();
    let updated = store.get_memory("test").unwrap().unwrap();
    assert!((updated.significance - 0.95).abs() < 0.001);
}

#[test]
fn test_store_record_access() {
    let store = LongevityStore::open_memory().unwrap();
    let record = MemoryRecord::new_raw(
        "test".to_string(),
        serde_json::json!({"text": "test"}),
        "project-1".to_string(),
        None,
    );
    store.insert_memory(&record).unwrap();

    store.record_access("test").unwrap();
    store.record_access("test").unwrap();

    let updated = store.get_memory("test").unwrap().unwrap();
    assert_eq!(updated.access_count, 2);
    assert!(updated.last_accessed.is_some());
}

#[test]
fn test_store_delete_memories() {
    let store = LongevityStore::open_memory().unwrap();
    let record = MemoryRecord::new_raw(
        "to-delete".to_string(),
        serde_json::json!({"text": "goodbye"}),
        "project-1".to_string(),
        None,
    );
    store.insert_memory(&record).unwrap();

    let deleted = store.delete_memories(&["to-delete".to_string()]).unwrap();
    assert_eq!(deleted, 1);
    assert!(store.get_memory("to-delete").unwrap().is_none());
}

#[test]
fn test_store_hierarchy_stats() {
    let store = LongevityStore::open_memory().unwrap();

    for i in 0..3 {
        store.insert_memory(&MemoryRecord::new_raw(
            format!("raw-{}", i),
            serde_json::json!({"text": "test"}),
            "project-1".to_string(),
            None,
        )).unwrap();
    }

    store.insert_memory(&MemoryRecord::new_compressed(
        "ep-1".to_string(),
        MemoryLayer::Episode,
        serde_json::json!({"summary": "episode"}),
        vec![],
        "project-1".to_string(),
    )).unwrap();

    let stats = store.hierarchy_stats("project-1").unwrap();
    assert_eq!(stats.raw_count, 3);
    assert_eq!(stats.episode_count, 1);
    assert_eq!(stats.total_count, 4);
}

#[test]
fn test_store_consolidation_log() {
    let store = LongevityStore::open_memory().unwrap();
    store.log_consolidation(
        "log-1", MemoryLayer::Raw, MemoryLayer::Episode,
        10, 2, 5.0, "algorithmic", 150,
    ).unwrap();

    let logs = store.get_consolidation_log(10).unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].memories_processed, 10);
    assert_eq!(logs[0].memories_created, 2);
}

#[test]
fn test_store_embedding_models() {
    let store = LongevityStore::open_memory().unwrap();
    store.register_embedding_model("model-1", "test-embed", 384, "local").unwrap();

    let active = store.get_active_embedding_model().unwrap();
    assert!(active.is_some());
    assert_eq!(active.unwrap().model_id, "model-1");

    store.retire_embedding_model("model-1", Some("model-2")).unwrap();
    let active = store.get_active_embedding_model().unwrap();
    assert!(active.is_none());
}

#[test]
fn test_store_schema_history() {
    let store = LongevityStore::open_memory().unwrap();
    let history = store.schema_history().unwrap();
    assert!(!history.is_empty());
    assert_eq!(history[0].version, 1);
}

#[test]
fn test_store_integrity_proofs() {
    let store = LongevityStore::open_memory().unwrap();
    store.store_integrity_proof("proof-1", "merkle_root", "abc123", 100).unwrap();

    let proof = store.latest_integrity_proof().unwrap();
    assert!(proof.is_some());
    assert_eq!(proof.unwrap().root_hash, "abc123");
}

#[test]
fn test_store_database_size() {
    let store = LongevityStore::open_memory().unwrap();
    let size = store.database_size_bytes().unwrap();
    assert!(size > 0);
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 3: SIGNIFICANCE SCORER
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_significance_default_weights_sum_to_one() {
    let weights = significance::SignificanceWeights::default();
    let sum = weights.recency
        + weights.access_frequency
        + weights.referential_weight
        + weights.causal_depth
        + weights.emotional_valence
        + weights.contradiction_signal
        + weights.uniqueness;
    assert!((sum - 1.0).abs() < 0.001);
}

#[test]
fn test_significance_user_marked_always_max() {
    let scorer = SignificanceScorer::new();
    let memory = MemoryRecord::new_raw(
        "id".to_string(),
        serde_json::json!({"text": "boring text"}),
        "proj".to_string(),
        None,
    );
    let ctx = significance::ScoringContext {
        user_marked: true,
        ..Default::default()
    };
    let breakdown = scorer.score(&memory, &ctx);
    assert_eq!(breakdown.final_score, 1.0);
    assert_eq!(breakdown.threshold, "immune");
}

#[test]
fn test_significance_simple_scoring() {
    let scorer = SignificanceScorer::new();
    let memory = MemoryRecord::new_raw(
        "id".to_string(),
        serde_json::json!({"text": "test"}),
        "proj".to_string(),
        None,
    );
    let score = scorer.score_simple(&memory);
    assert!(score >= 0.0 && score <= 1.0);
}

#[test]
fn test_significance_recent_scores_higher() {
    let scorer = SignificanceScorer::new();

    let recent = MemoryRecord::new_raw(
        "recent".to_string(),
        serde_json::json!({"text": "recent event"}),
        "proj".to_string(),
        None,
    );
    // recent.created_at is now

    let mut old = MemoryRecord::new_raw(
        "old".to_string(),
        serde_json::json!({"text": "old event"}),
        "proj".to_string(),
        None,
    );
    old.created_at = (chrono::Utc::now() - chrono::Duration::days(365)).to_rfc3339();

    let recent_score = scorer.score_simple(&recent);
    let old_score = scorer.score_simple(&old);
    assert!(recent_score > old_score, "Recent should score higher: {} vs {}", recent_score, old_score);
}

#[test]
fn test_significance_emotional_content_scores_higher() {
    let scorer = SignificanceScorer::new();

    let emotional = MemoryRecord::new_raw(
        "e".to_string(),
        serde_json::json!({"text": "I love this approach! It's IMPORTANT and CRITICAL!"}),
        "proj".to_string(),
        None,
    );
    let bland = MemoryRecord::new_raw(
        "b".to_string(),
        serde_json::json!({"text": "used method x"}),
        "proj".to_string(),
        None,
    );

    let e_score = scorer.score_simple(&emotional);
    let b_score = scorer.score_simple(&bland);
    assert!(e_score > b_score, "Emotional should score higher: {} vs {}", e_score, b_score);
}

#[test]
fn test_significance_thresholds() {
    use significance::SignificanceThreshold;
    assert!(matches!(SignificanceThreshold::from_score(0.9), SignificanceThreshold::Immune));
    assert!(matches!(SignificanceThreshold::from_score(0.6), SignificanceThreshold::Normal));
    assert!(matches!(SignificanceThreshold::from_score(0.3), SignificanceThreshold::Accelerated));
    assert!(matches!(SignificanceThreshold::from_score(0.1), SignificanceThreshold::Forgettable));
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 4: CONSOLIDATION ENGINE
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_consolidation_schedule_transitions() {
    assert_eq!(
        ConsolidationSchedule::Nightly.layer_transition(),
        Some((MemoryLayer::Raw, MemoryLayer::Episode))
    );
    assert_eq!(
        ConsolidationSchedule::Weekly.layer_transition(),
        Some((MemoryLayer::Episode, MemoryLayer::Summary))
    );
    assert_eq!(
        ConsolidationSchedule::Monthly.layer_transition(),
        Some((MemoryLayer::Summary, MemoryLayer::Pattern))
    );
    assert_eq!(
        ConsolidationSchedule::Quarterly.layer_transition(),
        Some((MemoryLayer::Pattern, MemoryLayer::Trait))
    );
}

#[test]
fn test_consolidation_empty_store() {
    let store = LongevityStore::open_memory().unwrap();
    let engine = ConsolidationEngine::new();
    let task = ConsolidationTask {
        schedule: ConsolidationSchedule::Nightly,
        from_layer: MemoryLayer::Raw,
        to_layer: MemoryLayer::Episode,
        project_id: "project-1".to_string(),
        max_memories: 1000,
    };

    let result = engine.run(&store, &task).unwrap();
    assert_eq!(result.memories_processed, 0);
    assert_eq!(result.memories_created, 0);
}

#[test]
fn test_consolidation_raw_to_episode() {
    let store = LongevityStore::open_memory().unwrap();
    let engine = ConsolidationEngine::new();

    // Insert old raw memories (older than 24 hours)
    for i in 0..10 {
        let mut record = MemoryRecord::new_raw(
            format!("raw-{}", i),
            serde_json::json!({
                "text": format!("User said thing number {}", i),
                "role": "user"
            }),
            "project-1".to_string(),
            Some("session-1".to_string()),
        );
        // Make them old enough
        record.created_at = (chrono::Utc::now() - chrono::Duration::hours(48)).to_rfc3339();
        record.significance = 0.3; // Below preservation threshold
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
    assert!(result.memories_processed > 0);
    assert!(result.memories_created > 0);
    assert!(result.compression_ratio > 1.0);

    // Check episodes were created
    let episodes = store.query_by_layer("project-1", MemoryLayer::Episode, 100).unwrap();
    assert!(!episodes.is_empty());
}

#[test]
fn test_consolidation_preserves_significant() {
    let store = LongevityStore::open_memory().unwrap();
    let engine = ConsolidationEngine::new();

    // Insert a high-significance memory
    let mut important = MemoryRecord::new_raw(
        "important".to_string(),
        serde_json::json!({"text": "CRITICAL decision: use Rust"}),
        "project-1".to_string(),
        None,
    );
    important.created_at = (chrono::Utc::now() - chrono::Duration::hours(48)).to_rfc3339();
    important.significance = 0.95;
    store.insert_memory(&important).unwrap();

    let task = ConsolidationTask {
        schedule: ConsolidationSchedule::Nightly,
        from_layer: MemoryLayer::Raw,
        to_layer: MemoryLayer::Episode,
        project_id: "project-1".to_string(),
        max_memories: 1000,
    };

    let result = engine.run(&store, &task).unwrap();
    assert_eq!(result.memories_preserved, 1);

    // Important memory should still exist
    let retrieved = store.get_memory("important").unwrap();
    assert!(retrieved.is_some());
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 5: HIERARCHY OPERATIONS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_hierarchy_group_for_episodes() {
    let mut memories = Vec::new();
    for i in 0..15 {
        memories.push(MemoryRecord::new_raw(
            format!("raw-{}", i),
            serde_json::json!({"text": format!("event {}", i)}),
            "proj".to_string(),
            Some("session-1".to_string()),
        ));
    }

    let groups = MemoryHierarchy::group_for_episodes(&memories);
    assert!(!groups.is_empty());
}

#[test]
fn test_hierarchy_create_episode_summary() {
    let memories: Vec<MemoryRecord> = (0..5)
        .map(|i| MemoryRecord::new_raw(
            format!("raw-{}", i),
            serde_json::json!({
                "text": format!("Working on file_{}.rs", i),
                "path": format!("src/file_{}.rs", i),
                "tool_name": "read_file",
            }),
            "proj".to_string(),
            Some("session-1".to_string()),
        ))
        .collect();

    let refs: Vec<&MemoryRecord> = memories.iter().collect();
    let episode = MemoryHierarchy::create_episode_summary(&refs);

    assert!(episode.get("summary").is_some());
    assert_eq!(episode["event_count"], 5);
}

#[test]
fn test_hierarchy_extract_patterns() {
    let mut summaries: Vec<MemoryRecord> = Vec::new();
    for i in 0..5 {
        summaries.push(MemoryRecord::new_compressed(
            format!("sum-{}", i),
            MemoryLayer::Summary,
            serde_json::json!({
                "files_touched": ["src/main.rs", "src/lib.rs", "src/auth.rs"],
                "tools_used": ["read_file", "edit_file", "grep"],
                "decisions": [format!("decision {}", i)],
            }),
            vec![],
            "proj".to_string(),
        ));
    }

    let refs: Vec<&MemoryRecord> = summaries.iter().collect();
    let patterns = MemoryHierarchy::extract_patterns(&refs);
    assert!(!patterns.is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 6: CAPTURE & DEDUP
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_content_dedup_new_content() {
    let dedup = ContentDedup::new(100);
    assert!(!dedup.is_duplicate("hello", 1000));
    assert!(!dedup.is_duplicate("world", 1000));
}

#[test]
fn test_content_dedup_same_content_same_window() {
    let dedup = ContentDedup::new(100);
    assert!(!dedup.is_duplicate("hello", 1000));
    assert!(dedup.is_duplicate("hello", 1001)); // Same 2-second window
}

#[test]
fn test_content_dedup_same_content_different_window() {
    let dedup = ContentDedup::new(100);
    assert!(!dedup.is_duplicate("hello", 1000));
    assert!(!dedup.is_duplicate("hello", 1003)); // Different 2-second window
}

#[test]
fn test_content_dedup_cleanup() {
    let dedup = ContentDedup::new(10);
    for i in 0..20 {
        dedup.is_duplicate(&format!("msg-{}", i), i * 3);
    }
    assert!(dedup.cache_size() <= 15); // Cleaned up to ~half
}

#[test]
fn test_capture_daemon_basic() {
    let daemon = CaptureDaemon::new();
    assert_eq!(daemon.buffer_size(), 0);

    let event = CaptureEvent {
        role: CaptureRole::User,
        content: "hello".to_string(),
        timestamp: 1000,
        source: CaptureSource::Manual,
        session_id: None,
        project_path: None,
    };

    assert!(daemon.capture(event.clone()));
    assert_eq!(daemon.buffer_size(), 1);

    // Same event should be deduped
    assert!(!daemon.capture(event));
    assert_eq!(daemon.buffer_size(), 1);
}

#[test]
fn test_capture_daemon_drain() {
    let daemon = CaptureDaemon::new();
    for i in 0..5 {
        daemon.capture(CaptureEvent {
            role: CaptureRole::User,
            content: format!("msg-{}", i),
            timestamp: i * 3000, // Different windows
            source: CaptureSource::Manual,
            session_id: None,
            project_path: None,
        });
    }

    let drained = daemon.drain_buffer();
    assert_eq!(drained.len(), 5);
    assert_eq!(daemon.buffer_size(), 0);
}

#[test]
fn test_client_log_monitor_detection() {
    let dedup = std::sync::Arc::new(ContentDedup::default());
    let monitor = ClientLogMonitor::new(dedup);
    // Just verify it doesn't crash — detection depends on local environment
    let _ = monitor.watch_targets();
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 7: FORGETTING PROTOCOL
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_forgetting_eligibility_checks() {
    let store = LongevityStore::open_memory().unwrap();
    let protocol = ForgettingProtocol::new().with_min_age(0.0);

    // Low significance, old enough
    let mut candidate = MemoryRecord::new_raw(
        "forget-me".to_string(),
        serde_json::json!({"text": "minor note"}),
        "project-1".to_string(),
        None,
    );
    candidate.significance = 0.1;
    candidate.created_at = (chrono::Utc::now() - chrono::Duration::days(60)).to_rfc3339();
    store.insert_memory(&candidate).unwrap();

    let verdicts = protocol.evaluate_candidates(&store, "project-1", 100).unwrap();
    assert_eq!(verdicts.len(), 1);
    assert!(verdicts[0].eligible);
}

#[test]
fn test_forgetting_protects_identity() {
    let store = LongevityStore::open_memory().unwrap();
    let protocol = ForgettingProtocol::new().with_min_age(0.0);

    let mut identity = MemoryRecord::new_compressed(
        "identity-core".to_string(),
        MemoryLayer::Identity,
        serde_json::json!({"trait": "Rust developer"}),
        vec![],
        "project-1".to_string(),
    );
    identity.significance = 0.1; // Even low significance
    identity.created_at = (chrono::Utc::now() - chrono::Duration::days(365)).to_rfc3339();
    store.insert_memory(&identity).unwrap();

    let verdicts = protocol.evaluate_candidates(&store, "project-1", 100).unwrap();
    assert_eq!(verdicts.len(), 1);
    assert!(!verdicts[0].eligible); // Identity is protected
}

#[test]
fn test_forgetting_execute() {
    let store = LongevityStore::open_memory().unwrap();
    let protocol = ForgettingProtocol::new().with_min_age(0.0);

    let mut candidate = MemoryRecord::new_raw(
        "forget-me".to_string(),
        serde_json::json!({"text": "trivial"}),
        "project-1".to_string(),
        None,
    );
    candidate.significance = 0.05;
    candidate.created_at = (chrono::Utc::now() - chrono::Duration::days(60)).to_rfc3339();
    store.insert_memory(&candidate).unwrap();

    let result = protocol.execute(&store, &["forget-me".to_string()]).unwrap();
    assert_eq!(result.forgotten_count, 1);
    assert!(store.get_memory("forget-me").unwrap().is_none());
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 8: BUDGET MANAGEMENT
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_budget_default() {
    let budget = StorageBudget::new();
    assert_eq!(budget.total_budget_bytes, 10 * 1024 * 1024 * 1024);
}

#[test]
fn test_budget_layer_allocations() {
    let budget = StorageBudget::new();
    let stats = hierarchy::HierarchyStats::default();
    let layers = budget.layer_budgets(&stats);
    assert_eq!(layers.len(), 6);

    // All should be healthy when empty
    for layer in &layers {
        assert!(matches!(layer.status.alert, BudgetAlert::Healthy));
    }
}

#[test]
fn test_budget_warning_threshold() {
    let budget = StorageBudget::with_budget(1000); // 1KB budget for testing
    let mut stats = hierarchy::HierarchyStats::default();
    stats.raw_bytes = 200; // Over 80% of 150 (15% allocation)

    let layers = budget.layer_budgets(&stats);
    let raw = layers.iter().find(|l| l.layer == "event").unwrap();
    assert!(matches!(raw.status.alert, BudgetAlert::Critical));
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 9: SCHEMA VERSIONING
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_schema_migration_not_needed() {
    let store = LongevityStore::open_memory().unwrap();
    let applied = schema::MigrationEngine::migrate_if_needed(&store).unwrap();
    assert!(applied.is_empty()); // Already at v1
}

#[test]
fn test_schema_all_migrations() {
    let migrations = schema::MigrationEngine::all_migrations();
    assert!(!migrations.is_empty());
    assert_eq!(migrations[0].version, 1);
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 10: INTEGRITY VERIFICATION
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_integrity_verify_empty() {
    let store = LongevityStore::open_memory().unwrap();
    let report = IntegrityVerifier::verify(&store, "project-1").unwrap();
    assert!(report.database_ok);
    assert_eq!(report.total_memories, 0);
}

#[test]
fn test_integrity_merkle_proof() {
    let store = LongevityStore::open_memory().unwrap();

    // Insert some data
    for i in 0..5 {
        store.insert_memory(&MemoryRecord::new_raw(
            format!("m-{}", i),
            serde_json::json!({"text": format!("memory {}", i)}),
            "project-1".to_string(),
            None,
        )).unwrap();
    }

    let proof = IntegrityVerifier::create_merkle_proof(&store, "project-1").unwrap();
    assert!(!proof.root_hash.is_empty());
    assert_eq!(proof.leaf_count, 5);

    // Verify against proof
    let verified = IntegrityVerifier::verify_against_proof(
        &store, "project-1", &proof.root_hash,
    ).unwrap();
    assert!(verified);
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 11: SYNC PROTOCOL
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_sync_captures_to_sqlite() {
    let store = LongevityStore::open_memory().unwrap();
    let events = vec![
        CaptureEvent {
            role: CaptureRole::User,
            content: "hello world".to_string(),
            timestamp: 1000,
            source: CaptureSource::Manual,
            session_id: Some("session-1".to_string()),
            project_path: None,
        },
        CaptureEvent {
            role: CaptureRole::Assistant,
            content: "hi there".to_string(),
            timestamp: 2000,
            source: CaptureSource::Manual,
            session_id: Some("session-1".to_string()),
            project_path: None,
        },
    ];

    let result = SyncProtocol::sync_captures_to_sqlite(&store, &events, "project-1").unwrap();
    assert_eq!(result.records_synced, 2);
    assert!(result.errors.is_empty());

    let total = store.total_count("project-1").unwrap();
    assert_eq!(total, 2);
}

#[test]
fn test_sync_load_session_context() {
    let store = LongevityStore::open_memory().unwrap();

    // Insert some memories at various layers
    store.insert_memory(&MemoryRecord::new_raw(
        "raw-1".to_string(),
        serde_json::json!({"text": "recent event"}),
        "project-1".to_string(),
        Some("session-1".to_string()),
    )).unwrap();

    let mut pattern = MemoryRecord::new_compressed(
        "pat-1".to_string(),
        MemoryLayer::Pattern,
        serde_json::json!({"pattern": "prefers Rust"}),
        vec![],
        "project-1".to_string(),
    );
    pattern.significance = 0.9;
    store.insert_memory(&pattern).unwrap();

    let ctx = SyncProtocol::load_session_context(&store, "project-1", 4096).unwrap();
    assert!(!ctx.parts.is_empty());
    assert!(ctx.tokens_used > 0);
}

#[test]
fn test_session_context_ghost_writer_format() {
    let ctx = sync::SessionContext {
        parts: vec![
            sync::ContextPart {
                layer: "identity".to_string(),
                content: "Senior Rust developer".to_string(),
                significance: 0.95,
            },
            sync::ContextPart {
                layer: "trait".to_string(),
                content: "Prefers explicit error handling".to_string(),
                significance: 0.8,
            },
        ],
        tokens_used: 100,
        last_session_summary: Some("Last session: 50 events".to_string()),
        pattern_count: 5,
        trait_count: 3,
    };

    let formatted = ctx.to_ghost_writer_format();
    assert!(formatted.contains("CRITICAL INSTRUCTION"));
    assert!(formatted.contains("memory_capture_message"));
    assert!(formatted.contains("Senior Rust developer"));
    assert!(formatted.contains("Prefers explicit error handling"));
    assert!(formatted.contains("Last session: 50 events"));
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 12: BACKUP
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_backup_config_default() {
    let config = BackupConfig::default();
    assert_eq!(config.schedule, BackupSchedule::Daily);
    assert_eq!(config.mode, BackupMode::Full);
}

#[test]
fn test_backup_retention_default() {
    let retention = backup::RetentionPolicy::default();
    assert_eq!(retention.daily_count, 7);
    assert_eq!(retention.weekly_count, 4);
    assert_eq!(retention.monthly_count, 12);
}

#[test]
fn test_backup_to_local() {
    let config = BackupConfig::default();
    let daemon = BackupDaemon::new(config);

    let temp = tempfile::tempdir().unwrap();
    let amem_path = temp.path().join("test.amem");
    std::fs::write(&amem_path, b"test amem data").unwrap();

    let longevity_path = temp.path().join("test.longevity.db");
    let _store = LongevityStore::open(&longevity_path).unwrap();

    let backup_dest = temp.path().join("backups");
    std::fs::create_dir_all(&backup_dest).unwrap();

    let result = daemon.backup_to_local(&amem_path, &longevity_path, &backup_dest).unwrap();
    assert!(result.success);
    assert!(result.size_bytes > 0);
    assert!(!result.files_backed_up.is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 13: EMBEDDING MIGRATION
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_embedding_register_model() {
    let store = LongevityStore::open_memory().unwrap();
    EmbeddingMigrator::register_model(
        &store, "model-v1", "test-embed-v1", 384, "local",
    ).unwrap();

    let models = EmbeddingMigrator::list_models(&store).unwrap();
    assert_eq!(models.len(), 1);
    assert!(models[0].is_active);
}

#[test]
fn test_embedding_switch_model() {
    let store = LongevityStore::open_memory().unwrap();
    EmbeddingMigrator::register_model(&store, "v1", "embed-v1", 384, "local").unwrap();
    EmbeddingMigrator::register_model(&store, "v2", "embed-v2", 512, "local").unwrap();

    EmbeddingMigrator::switch_model(&store, "v1", "v2").unwrap();

    let models = EmbeddingMigrator::list_models(&store).unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].model_id, "v2");
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 14: ENCRYPTION ROTATION
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_encryption_rotate_key() {
    let store = LongevityStore::open_memory().unwrap();
    let key_id = EncryptionRotator::rotate_key(&store, "AES-256-GCM").unwrap();
    assert!(!key_id.is_empty());

    let current = EncryptionRotator::current_key(&store).unwrap();
    assert!(current.is_some());
    assert_eq!(current.unwrap().key_id, key_id);
}

#[test]
fn test_encryption_key_rotation_retires_old() {
    let store = LongevityStore::open_memory().unwrap();
    let key1 = EncryptionRotator::rotate_key(&store, "AES-256-GCM").unwrap();
    let key2 = EncryptionRotator::rotate_key(&store, "AES-256-GCM").unwrap();

    let current = EncryptionRotator::current_key(&store).unwrap().unwrap();
    assert_eq!(current.key_id, key2);
    assert_ne!(current.key_id, key1);
}

// ═══════════════════════════════════════════════════════════════════
// PHASE 15: INTEGRATION (END-TO-END)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_end_to_end_capture_consolidate_query() {
    let store = LongevityStore::open_memory().unwrap();
    let engine = ConsolidationEngine::new();

    // 1. Simulate capture: insert 20 raw memories
    for i in 0..20 {
        let mut record = MemoryRecord::new_raw(
            format!("raw-{}", i),
            serde_json::json!({
                "text": format!("User discussed topic {} in detail", i),
                "role": "user"
            }),
            "project-1".to_string(),
            Some("session-1".to_string()),
        );
        record.created_at = (chrono::Utc::now() - chrono::Duration::hours(48)).to_rfc3339();
        record.significance = 0.3;
        store.insert_memory(&record).unwrap();
    }

    // 2. Consolidate Raw → Episode
    let task = ConsolidationTask {
        schedule: ConsolidationSchedule::Nightly,
        from_layer: MemoryLayer::Raw,
        to_layer: MemoryLayer::Episode,
        project_id: "project-1".to_string(),
        max_memories: 1000,
    };
    let result = engine.run(&store, &task).unwrap();
    assert!(result.memories_created > 0);

    // 3. Verify hierarchy
    let stats = store.hierarchy_stats("project-1").unwrap();
    assert!(stats.episode_count > 0);

    // 4. Verify consolidation was logged
    let logs = store.get_consolidation_log(10).unwrap();
    assert!(!logs.is_empty());

    // 5. Load session context
    let ctx = SyncProtocol::load_session_context(&store, "project-1", 4096).unwrap();
    // Should have loaded some context
    let ghost_writer_output = ctx.to_ghost_writer_format();
    assert!(ghost_writer_output.contains("CRITICAL INSTRUCTION"));
}

#[test]
fn test_end_to_end_full_lifecycle() {
    let store = LongevityStore::open_memory().unwrap();

    // 1. Register embedding model
    EmbeddingMigrator::register_model(&store, "embed-v1", "test-embed", 384, "local").unwrap();

    // 2. Store memories
    for i in 0..10 {
        store.insert_memory(&MemoryRecord::new_raw(
            format!("m-{}", i),
            serde_json::json!({"text": format!("memory {}", i)}),
            "project-1".to_string(),
            None,
        )).unwrap();
    }

    // 3. Create integrity proof
    let proof = IntegrityVerifier::create_merkle_proof(&store, "project-1").unwrap();
    assert_eq!(proof.leaf_count, 10);

    // 4. Check budget
    let budget = StorageBudget::new();
    let stats = store.hierarchy_stats("project-1").unwrap();
    let status = budget.overall_status(&stats);
    assert!(matches!(status.alert, BudgetAlert::Healthy));

    // 5. Schema is at v1
    let version = store.current_schema_version().unwrap();
    assert_eq!(version, 1);

    // 6. Verify integrity
    let report = IntegrityVerifier::verify(&store, "project-1").unwrap();
    assert!(report.database_ok);
    assert_eq!(report.total_memories, 10);
}
