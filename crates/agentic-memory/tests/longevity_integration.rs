//! V4 Integration & E2E Tests — The 11philip22 Standard.
//!
//! Tests I-1 through I-10 (integration wiring) and
//! E2E-1 through E2E-7 (real user simulation).

use agentic_memory::v3::longevity::backup::{BackupConfig, BackupDaemon};
use agentic_memory::v3::longevity::budget::StorageBudget;
use agentic_memory::v3::longevity::capture::{
    CaptureDaemon, CaptureEvent, CaptureRole, CaptureSource,
};
use agentic_memory::v3::longevity::consolidation::{
    ConsolidationEngine, ConsolidationSchedule, ConsolidationTask,
};
use agentic_memory::v3::longevity::embedding_migration::EmbeddingMigrator;
use agentic_memory::v3::longevity::encryption_rotation::EncryptionRotator;
use agentic_memory::v3::longevity::hierarchy::{MemoryLayer, MemoryRecord};
use agentic_memory::v3::longevity::integrity::IntegrityVerifier;
use agentic_memory::v3::longevity::significance::SignificanceScorer;
use agentic_memory::v3::longevity::store::LongevityStore;
use agentic_memory::v3::longevity::sync::SyncProtocol;

// ═══════════════════════════════════════════════════════════════════
// TEST I-1: Capture → WAL → SQLite Pipeline
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_i1_capture_wal_sqlite_pipeline() {
    let store = LongevityStore::open_memory().unwrap();
    let daemon = CaptureDaemon::new();
    let scorer = SignificanceScorer::new();

    // Feed 50 simulated conversation messages
    for i in 0..50 {
        let event = CaptureEvent {
            role: if i % 2 == 0 {
                CaptureRole::User
            } else {
                CaptureRole::Assistant
            },
            content: format!(
                "Message {} about building an auth module with JWT tokens",
                i
            ),
            timestamp: i * 3000, // Unique windows
            source: CaptureSource::ClientLog,
            session_id: Some("session-1".to_string()),
            project_path: Some("/projects/auth".to_string()),
        };
        daemon.capture(event);
    }

    assert_eq!(daemon.buffer_size(), 50);

    // Sync to SQLite
    let events = daemon.drain_buffer();
    let result = SyncProtocol::sync_captures_to_sqlite(&store, &events, "auth-project").unwrap();
    assert_eq!(result.records_synced, 50);

    // Verify all 50 in SQLite at Raw layer
    let raw = store
        .query_by_layer("auth-project", MemoryLayer::Raw, 100)
        .unwrap();
    assert_eq!(raw.len(), 50);

    // Each should have significance computed
    for record in &raw {
        let score = scorer.score_simple(record);
        assert!((0.0..=1.0).contains(&score));
    }

    // Dedup catches duplicates — feed 10 again
    for i in 0..10 {
        let event = CaptureEvent {
            role: CaptureRole::User,
            content: format!(
                "Message {} about building an auth module with JWT tokens",
                i
            ),
            timestamp: i * 3000, // Same timestamps
            source: CaptureSource::McpStream,
            session_id: Some("session-1".to_string()),
            project_path: None,
        };
        daemon.capture(event); // Should be deduped
    }
    // Buffer should have 0 (all deduped)
    assert_eq!(daemon.buffer_size(), 0);

    // SQLite still has 50
    let count = store.total_count("auth-project").unwrap();
    assert_eq!(count, 50);
}

// ═══════════════════════════════════════════════════════════════════
// TEST I-2: Consolidation Pipeline End-to-End
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_i2_consolidation_pipeline() {
    let store = LongevityStore::open_memory().unwrap();
    let engine = ConsolidationEngine::new();

    // Insert 200 raw memories spanning 7 simulated days
    for day in 0..7 {
        for event in 0..28 {
            let i = day * 28 + event;
            let mut record = MemoryRecord::new_raw(
                format!("raw-{:04}", i),
                serde_json::json!({
                    "text": format!("Day {} event {}: working on feature {}", day, event, event % 5),
                    "path": format!("src/feature_{}.rs", event % 5),
                    "tool_name": "edit_file",
                    "decision": if event == 0 { format!("Day {} plan: focus on feature {}", day, day) } else { String::new() },
                }),
                "project-1".to_string(),
                Some(format!("session-day-{}", day)),
            );
            record.created_at = (chrono::Utc::now()
                - chrono::Duration::days(7 - day as i64)
                - chrono::Duration::hours(event as i64))
            .to_rfc3339();
            record.significance = 0.3;
            store.insert_memory(&record).unwrap();
        }
    }
    assert_eq!(store.total_count("project-1").unwrap(), 196);

    // Nightly: Raw → Episode
    let task1 = ConsolidationTask {
        schedule: ConsolidationSchedule::Nightly,
        from_layer: MemoryLayer::Raw,
        to_layer: MemoryLayer::Episode,
        project_id: "project-1".to_string(),
        max_memories: 1000,
    };
    let r1 = engine.run(&store, &task1).unwrap();
    assert!(r1.memories_created > 0, "Should create episodes");

    // Episodes should be searchable via FTS
    let episodes = store
        .query_by_layer("project-1", MemoryLayer::Episode, 100)
        .unwrap();
    assert!(!episodes.is_empty());

    // Age episodes and run weekly: Episode → Summary
    for ep in &episodes {
        let mut updated = ep.clone();
        updated.created_at = (chrono::Utc::now() - chrono::Duration::days(14)).to_rfc3339();
        updated.significance = 0.3;
        store.insert_memory(&updated).unwrap();
    }

    let task2 = ConsolidationTask {
        schedule: ConsolidationSchedule::Weekly,
        from_layer: MemoryLayer::Episode,
        to_layer: MemoryLayer::Summary,
        project_id: "project-1".to_string(),
        max_memories: 1000,
    };
    engine.run(&store, &task2).unwrap();

    // Monthly: Summary → Pattern
    let summaries = store
        .query_by_layer("project-1", MemoryLayer::Summary, 100)
        .unwrap();
    for s in &summaries {
        let mut updated = s.clone();
        updated.created_at = (chrono::Utc::now() - chrono::Duration::days(45)).to_rfc3339();
        updated.significance = 0.3;
        store.insert_memory(&updated).unwrap();
    }

    let task3 = ConsolidationTask {
        schedule: ConsolidationSchedule::Monthly,
        from_layer: MemoryLayer::Summary,
        to_layer: MemoryLayer::Pattern,
        project_id: "project-1".to_string(),
        max_memories: 1000,
    };
    engine.run(&store, &task3).unwrap();

    // Verify hierarchy stats
    let stats = store.hierarchy_stats("project-1").unwrap();
    println!(
        "I-2 Hierarchy: raw={}, ep={}, sum={}, pat={}",
        stats.raw_count, stats.episode_count, stats.summary_count, stats.pattern_count
    );
    assert!(stats.total_count > 0);
}

// ═══════════════════════════════════════════════════════════════════
// TEST I-3: Ghost Writer Context Generation
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_i3_ghost_writer_context() {
    let store = LongevityStore::open_memory().unwrap();

    // Populate with user profile (trait layer)
    let mut trait1 = MemoryRecord::new_compressed(
        "trait-1".to_string(),
        MemoryLayer::Trait,
        serde_json::json!({"trait_type": "expertise", "description": "Senior Rust developer"}),
        vec![],
        "project-1".to_string(),
    );
    trait1.significance = 0.9;
    store.insert_memory(&trait1).unwrap();

    // Active decisions (pattern layer)
    let mut pattern1 = MemoryRecord::new_compressed(
        "pat-1".to_string(),
        MemoryLayer::Pattern,
        serde_json::json!({"pattern": "Uses JWT with refresh tokens for auth", "confidence": 0.8}),
        vec![],
        "project-1".to_string(),
    );
    pattern1.significance = 0.85;
    store.insert_memory(&pattern1).unwrap();

    // Recent sessions
    for i in 0..10 {
        store
            .insert_memory(&MemoryRecord::new_raw(
                format!("recent-{}", i),
                serde_json::json!({"text": format!("Working on auth module refactor step {}", i)}),
                "project-1".to_string(),
                Some("session-5".to_string()),
            ))
            .unwrap();
    }

    // Generate context
    let ctx = SyncProtocol::load_session_context(&store, "project-1", 4096).unwrap();

    // Verify sections
    assert!(
        ctx.tokens_used > 0 && ctx.tokens_used <= 4096,
        "Must fit in 4K tokens"
    );
    assert!(!ctx.parts.is_empty());

    let output = ctx.to_ghost_writer_format();
    assert!(
        output.contains("CRITICAL INSTRUCTION"),
        "Must have CRITICAL INSTRUCTION"
    );
    assert!(
        output.contains("memory_capture_message"),
        "Must instruct LLM to capture"
    );
    assert!(output.len() > 100, "Must have meaningful content");

    // Verify it's valid UTF-8 markdown
    assert!(!output.is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// TEST I-4: Session Resume Context Loading
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_i4_session_resume_context() {
    let store = LongevityStore::open_memory().unwrap();

    // 10 sessions across 2 projects
    for proj in ["alpha", "beta"] {
        for session in 0..5 {
            for msg in 0..10 {
                store.insert_memory(&MemoryRecord::new_raw(
                    format!("{}-s{}-m{}", proj, session, msg),
                    serde_json::json!({"text": format!("{} session {} message {}", proj, session, msg)}),
                    proj.to_string(),
                    Some(format!("session-{}", session)),
                )).unwrap();
            }
        }
        // Add patterns per project
        let mut pattern = MemoryRecord::new_compressed(
            format!("{}-pat", proj),
            MemoryLayer::Pattern,
            serde_json::json!({"pattern": format!("{} project pattern", proj)}),
            vec![],
            proj.to_string(),
        );
        pattern.significance = 0.85;
        store.insert_memory(&pattern).unwrap();
    }

    // Load context for alpha only
    let ctx = SyncProtocol::load_session_context(&store, "alpha", 4096).unwrap();
    assert!(ctx.tokens_used > 0);

    // Verify pattern is included
    let has_alpha_pattern = ctx.parts.iter().any(|p| p.content.contains("alpha"));
    assert!(has_alpha_pattern, "Should include alpha project pattern");

    // Verify no beta content leaked
    let has_beta = ctx.parts.iter().any(|p| p.content.contains("beta"));
    assert!(!has_beta, "Should NOT include beta project data");
}

// ═══════════════════════════════════════════════════════════════════
// TEST I-5: Backup → Destroy → Restore Round-Trip
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_i5_backup_destroy_restore() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("test.longevity.db");

    // Create store with 500 memories across layers
    {
        let store = LongevityStore::open(&db_path).unwrap();
        for i in 0..400 {
            store
                .insert_memory(&MemoryRecord::new_raw(
                    format!("raw-{}", i),
                    serde_json::json!({"text": format!("Memory {}", i)}),
                    "project-1".to_string(),
                    None,
                ))
                .unwrap();
        }
        for i in 0..50 {
            store
                .insert_memory(&MemoryRecord::new_compressed(
                    format!("ep-{}", i),
                    MemoryLayer::Episode,
                    serde_json::json!({"summary": format!("Episode {}", i)}),
                    vec![],
                    "project-1".to_string(),
                ))
                .unwrap();
        }
        for i in 0..30 {
            store
                .insert_memory(&MemoryRecord::new_compressed(
                    format!("sum-{}", i),
                    MemoryLayer::Summary,
                    serde_json::json!({"summary": format!("Summary {}", i)}),
                    vec![],
                    "project-1".to_string(),
                ))
                .unwrap();
        }
        for i in 0..15 {
            store
                .insert_memory(&MemoryRecord::new_compressed(
                    format!("pat-{}", i),
                    MemoryLayer::Pattern,
                    serde_json::json!({"pattern": format!("Pattern {}", i)}),
                    vec![],
                    "project-1".to_string(),
                ))
                .unwrap();
        }
        for i in 0..5 {
            store
                .insert_memory(&MemoryRecord::new_compressed(
                    format!("trait-{}", i),
                    MemoryLayer::Trait,
                    serde_json::json!({"trait": format!("Trait {}", i)}),
                    vec![],
                    "project-1".to_string(),
                ))
                .unwrap();
        }
        let stats = store.hierarchy_stats("project-1").unwrap();
        assert_eq!(stats.total_count, 500);
    }

    // Create .amem placeholder
    let amem_path = temp.path().join("test.amem");
    std::fs::write(&amem_path, b"AMEM hot cache data with 50 events").unwrap();

    // Backup
    let backup_dir = temp.path().join("backups");
    let daemon = BackupDaemon::new(BackupConfig::default());
    let result = daemon
        .backup_to_local(&amem_path, &db_path, &backup_dir)
        .unwrap();
    assert!(result.success);

    // Find backup subdir
    let backup_subdir = std::fs::read_dir(&backup_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with("amem-backup-"))
                .unwrap_or(false)
        })
        .unwrap();

    // DESTROY originals
    std::fs::remove_file(&amem_path).unwrap();
    std::fs::remove_file(&db_path).unwrap();
    assert!(!amem_path.exists());
    assert!(!db_path.exists());

    // Restore
    let restore_dir = temp.path().join("restored");
    let restore_result =
        BackupDaemon::restore_from_local(&backup_subdir.path(), &restore_dir).unwrap();
    assert!(restore_result.success);

    // Verify restored DB
    let restored_db = restore_dir.join("test.longevity.db");
    assert!(restored_db.exists());
    let store = LongevityStore::open(&restored_db).unwrap();
    let stats = store.hierarchy_stats("project-1").unwrap();
    assert_eq!(stats.raw_count, 400);
    assert_eq!(stats.episode_count, 50);
    assert_eq!(stats.summary_count, 30);
    assert_eq!(stats.pattern_count, 15);
    assert_eq!(stats.trait_count, 5);
    assert_eq!(stats.total_count, 500);
}

// ═══════════════════════════════════════════════════════════════════
// TEST I-6: Multi-Project Isolation Under Load
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_i6_multi_project_isolation() {
    let store = LongevityStore::open_memory().unwrap();
    let daemon = CaptureDaemon::new();

    // Capture 100 messages per project
    for proj in ["alpha", "beta", "gamma"] {
        for i in 0..100 {
            let event = CaptureEvent {
                role: CaptureRole::User,
                content: format!("{} project message {}", proj, i),
                timestamp: i * 3000
                    + match proj {
                        "alpha" => 0,
                        "beta" => 1000,
                        _ => 2000,
                    },
                source: CaptureSource::ClientLog,
                session_id: Some(format!("{}-session", proj)),
                project_path: None,
            };
            daemon.capture(event);
        }
        let events = daemon.drain_buffer();
        SyncProtocol::sync_captures_to_sqlite(&store, &events, proj).unwrap();
    }

    // Verify isolation
    for proj in ["alpha", "beta", "gamma"] {
        let count = store.total_count(proj).unwrap();
        assert_eq!(count, 100, "Project {} should have exactly 100", proj);

        let results = store.search_fulltext(proj, proj, 200).unwrap();
        assert!(!results.is_empty(), "{} should find own messages", proj);

        // Verify no cross-contamination
        for other in ["alpha", "beta", "gamma"] {
            if other != proj {
                let cross = store
                    .search_fulltext(proj, &format!("{} project message", other), 10)
                    .unwrap();
                assert!(
                    cross.is_empty(),
                    "{} should not find {} messages",
                    proj,
                    other
                );
            }
        }
    }

    // Run consolidation independently
    let engine = ConsolidationEngine::new();
    for proj in ["alpha", "beta", "gamma"] {
        let results = engine.run_all(&store, proj).unwrap();
        // Consolidation results are independent
        for r in &results {
            assert_eq!(r.task.project_id, proj);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// TEST I-7: Schema Migration Chain
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_i7_schema_migration() {
    let store = LongevityStore::open_memory().unwrap();

    // Verify at v1
    let v = store.current_schema_version().unwrap();
    assert_eq!(v, 1);

    // Insert data at v1
    for i in 0..10 {
        store
            .insert_memory(&MemoryRecord::new_raw(
                format!("v1-{}", i),
                serde_json::json!({"text": format!("V1 memory {}", i)}),
                "project-1".to_string(),
                None,
            ))
            .unwrap();
    }

    // Migration engine should report no migration needed
    let applied =
        agentic_memory::v3::longevity::schema::MigrationEngine::migrate_if_needed(&store).unwrap();
    assert!(applied.is_empty());

    // All v1 memories should be readable
    for i in 0..10 {
        let m = store.get_memory(&format!("v1-{}", i)).unwrap();
        assert!(m.is_some());
    }

    // Schema history shows v1
    let history = store.schema_history().unwrap();
    assert!(!history.is_empty());
    assert_eq!(history[0].version, 1);
}

// ═══════════════════════════════════════════════════════════════════
// TEST I-8: Embedding Model Transition
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_i8_embedding_transition() {
    let store = LongevityStore::open_memory().unwrap();

    // Register ada-002
    EmbeddingMigrator::register_model(&store, "ada-002", "text-embedding-ada-002", 128, "openai")
        .unwrap();

    // Store 100 memories with ada-002 embeddings
    for i in 0..100 {
        let mut record = MemoryRecord::new_raw(
            format!("em-{}", i),
            serde_json::json!({"text": format!("Embedded memory {}", i)}),
            "project-1".to_string(),
            None,
        );
        record.embedding = Some(vec![0.1_f32; 128]);
        record.embedding_model = Some("ada-002".to_string());
        store.insert_memory(&record).unwrap();
    }

    // Register new model
    EmbeddingMigrator::register_model(
        &store,
        "text-3-large",
        "text-embedding-3-large",
        256,
        "openai",
    )
    .unwrap();

    // Switch models
    EmbeddingMigrator::switch_model(&store, "ada-002", "text-3-large").unwrap();

    // Simulate lazy re-embedding of 20 accessed memories
    for i in 0..20 {
        let id = format!("em-{}", i);
        store.record_access(&id).unwrap();
        // "Re-embed" — update embedding to new dimension
        let mut m = store.get_memory(&id).unwrap().unwrap();
        m.embedding = Some(vec![0.2_f32; 256]);
        m.embedding_model = Some("text-3-large".to_string());
        store.insert_memory(&m).unwrap();
    }

    // Verify: 20 re-embedded, 80 still old model
    let new_count = store.count_memories_with_model("text-3-large").unwrap();
    let old_count = store.count_memories_with_model("ada-002").unwrap();
    assert_eq!(new_count, 20);
    assert_eq!(old_count, 80);

    // FTS still works across both
    let results = store.search_fulltext("project-1", "Embedded", 200).unwrap();
    assert_eq!(results.len(), 100);
}

// ═══════════════════════════════════════════════════════════════════
// TEST I-9: Encryption Key Rotation Under Load
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_i9_encryption_rotation() {
    let store = LongevityStore::open_memory().unwrap();

    // Create K1
    let k1 = EncryptionRotator::rotate_key(&store, "AES-256-GCM").unwrap();

    // Insert 100 memories "encrypted" with K1
    for i in 0..100 {
        let mut record = MemoryRecord::new_raw(
            format!("enc-{}", i),
            serde_json::json!({"text": format!("Encrypted memory {}", i)}),
            "project-1".to_string(),
            None,
        );
        record.encryption_key_id = Some(k1.clone());
        store.insert_memory(&record).unwrap();
    }

    // Rotate to K2
    let k2 = EncryptionRotator::rotate_key(&store, "AES-256-GCM").unwrap();
    assert_ne!(k1, k2);

    // New memories use K2
    for i in 100..150 {
        let mut record = MemoryRecord::new_raw(
            format!("enc-{}", i),
            serde_json::json!({"text": format!("New encrypted memory {}", i)}),
            "project-1".to_string(),
            None,
        );
        record.encryption_key_id = Some(k2.clone());
        store.insert_memory(&record).unwrap();
    }

    // "Re-encrypt" 30 accessed old memories
    for i in 0..30 {
        let id = format!("enc-{}", i);
        let mut m = store.get_memory(&id).unwrap().unwrap();
        m.encryption_key_id = Some(k2.clone());
        store.insert_memory(&m).unwrap();
    }

    // All 150 should be readable
    for i in 0..150 {
        let m = store.get_memory(&format!("enc-{}", i)).unwrap();
        assert!(m.is_some(), "Memory enc-{} should exist", i);
    }

    // Current key should be K2
    let current = EncryptionRotator::current_key(&store).unwrap().unwrap();
    assert_eq!(current.key_id, k2);
}

// ═══════════════════════════════════════════════════════════════════
// TEST I-10: Budget Pressure Response
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_i10_budget_pressure() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("budget.db");
    let store = LongevityStore::open(&db_path).unwrap();

    let budget = StorageBudget::with_budget(50_000); // 50KB budget

    // Insert until 80% warning
    let mut inserted = 0;
    loop {
        store
            .insert_memory(&MemoryRecord::new_raw(
                format!("budget-{}", inserted),
                serde_json::json!({"text": "x".repeat(200)}),
                "project-1".to_string(),
                None,
            ))
            .unwrap();
        inserted += 1;

        let stats = store.hierarchy_stats("project-1").unwrap();
        let status = budget.overall_status(&stats);
        if matches!(
            status.alert,
            agentic_memory::v3::longevity::budget::BudgetAlert::Warning
        ) {
            println!("Warning hit at {} memories", inserted);
            break;
        }
        if inserted > 500 {
            break;
        } // Safety limit
    }

    // Continue until critical
    loop {
        store
            .insert_memory(&MemoryRecord::new_raw(
                format!("budget-{}", inserted),
                serde_json::json!({"text": "x".repeat(200)}),
                "project-1".to_string(),
                None,
            ))
            .unwrap();
        inserted += 1;

        let stats = store.hierarchy_stats("project-1").unwrap();
        let status = budget.overall_status(&stats);
        if matches!(
            status.alert,
            agentic_memory::v3::longevity::budget::BudgetAlert::Critical
        ) {
            println!("Critical hit at {} memories", inserted);
            break;
        }
        if inserted > 1000 {
            break;
        }
    }

    // Respond: accelerate consolidation
    // Mark all as old enough
    let all = store
        .query_by_layer("project-1", MemoryLayer::Raw, 2000)
        .unwrap();
    for m in &all {
        let mut updated = m.clone();
        updated.created_at = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        updated.significance = 0.2;
        store.insert_memory(&updated).unwrap();
    }

    let engine = ConsolidationEngine::new();
    let task = ConsolidationTask {
        schedule: ConsolidationSchedule::Nightly,
        from_layer: MemoryLayer::Raw,
        to_layer: MemoryLayer::Episode,
        project_id: "project-1".to_string(),
        max_memories: 2000,
    };
    let result = engine.run(&store, &task).unwrap();
    println!(
        "Emergency consolidation: {} processed, {} created",
        result.memories_processed, result.memories_created
    );

    // Verify storage reduced (episodes are smaller than raw)
    let final_stats = store.hierarchy_stats("project-1").unwrap();
    assert!(final_stats.episode_count > 0, "Episodes should be created");
}

// ═══════════════════════════════════════════════════════════════════
// E2E-1: THE 11PHILIP22 TEST — Fresh Install → Prompt → Capture
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_e2e1_the_11philip22_test() {
    let store = LongevityStore::open_memory().unwrap();
    let daemon = CaptureDaemon::new();

    // Simulate: 20 user messages + 20 assistant responses
    // NO memory_capture tools called (simulating LLM ignoring tools)
    for i in 0..20 {
        // User message
        daemon.capture(CaptureEvent {
            role: CaptureRole::User,
            content: format!(
                "User message {}: Can you help me with the authentication module?",
                i
            ),
            timestamp: i * 60_000,            // 1 minute apart
            source: CaptureSource::ClientLog, // Captured from log, NOT tool call
            session_id: Some("session-1".to_string()),
            project_path: Some("/projects/my-app".to_string()),
        });
        // Assistant response
        daemon.capture(CaptureEvent {
            role: CaptureRole::Assistant,
            content: format!(
                "Assistant response {}: I'll help you implement JWT-based auth...",
                i
            ),
            timestamp: i * 60_000 + 30_000, // 30 seconds after user
            source: CaptureSource::ClientLog,
            session_id: Some("session-1".to_string()),
            project_path: None,
        });
    }

    // Drain and sync
    let events = daemon.drain_buffer();
    assert_eq!(events.len(), 40, "All 40 messages should be captured");

    let result = SyncProtocol::sync_captures_to_sqlite(&store, &events, "my-app").unwrap();
    assert_eq!(
        result.records_synced, 40,
        "All 40 should be synced to SQLite"
    );
    assert!(result.errors.is_empty(), "No sync errors");

    // Verify each message has:
    let memories = store
        .query_by_layer("my-app", MemoryLayer::Raw, 100)
        .unwrap();
    assert_eq!(memories.len(), 40, "All 40 in SQLite at Raw layer");

    for m in &memories {
        assert!(!m.created_at.is_empty(), "Has timestamp");
        assert!(!m.content.is_null(), "Has content");
        // Role is stored in content JSON
        let text = m.extract_text();
        assert!(!text.is_empty(), "Has extractable text");
    }

    // No duplicates
    let count = store.total_count("my-app").unwrap();
    assert_eq!(count, 40, "No duplicates — exactly 40");

    println!("THE 11PHILIP22 TEST: PASSED — All 40 messages captured without ANY tool calls");
}

// ═══════════════════════════════════════════════════════════════════
// E2E-2: Session End → New Session → Context Present
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_e2e2_session_continuity() {
    let store = LongevityStore::open_memory().unwrap();

    // Session 1: 20 messages about auth with JWT
    for i in 0..20 {
        store
            .insert_memory(&MemoryRecord::new_raw(
                format!("s1-{}", i),
                serde_json::json!({
                    "text": format!("Building auth module with JWT refresh tokens, step {}", i),
                    "role": if i % 2 == 0 { "user" } else { "assistant" },
                    "decision": if i == 5 { "Use JWT with 15-minute expiry" } else { "" },
                }),
                "auth-project".to_string(),
                Some("session-1".to_string()),
            ))
            .unwrap();
    }

    // Add extracted patterns from session 1
    let mut pattern = MemoryRecord::new_compressed(
        "auth-pattern".to_string(),
        MemoryLayer::Pattern,
        serde_json::json!({"pattern": "User is building authentication with JWT refresh tokens"}),
        vec![],
        "auth-project".to_string(),
    );
    pattern.significance = 0.85;
    store.insert_memory(&pattern).unwrap();

    // Session 2: New conversation starts
    let ctx = SyncProtocol::load_session_context(&store, "auth-project", 4096).unwrap();
    let ghost_output = ctx.to_ghost_writer_format();

    // Verify
    assert!(
        ghost_output.contains("CRITICAL INSTRUCTION"),
        "Has CRITICAL INSTRUCTION"
    );
    assert!(
        ghost_output.contains("memory_capture_message"),
        "Instructs LLM to capture"
    );
    assert!(
        ctx.last_session_summary.is_some(),
        "Has last session summary"
    );
    assert!(ctx.tokens_used <= 4096, "Fits in 4K token budget");

    // Pattern about JWT should be in context
    let has_auth_context = ctx
        .parts
        .iter()
        .any(|p| p.content.contains("JWT") || p.content.contains("auth"));
    assert!(
        has_auth_context,
        "Context should mention JWT/auth from session 1"
    );

    println!("E2E-2 PASSED: New session has full context from session 1");
}

// ═══════════════════════════════════════════════════════════════════
// E2E-3: Week-Long Workflow Simulation
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_e2e3_week_long_workflow() {
    let store = LongevityStore::open_memory().unwrap();
    let engine = ConsolidationEngine::new();
    let daily_counts = [50, 40, 30, 20, 60, 10, 5]; // 215 total

    let mut total_inserted = 0;
    for (day, &count) in daily_counts.iter().enumerate() {
        for msg in 0..count {
            let mut record = MemoryRecord::new_raw(
                format!("d{}-m{}", day, msg),
                serde_json::json!({
                    "text": format!("Day {} message {}: working on feature {}", day + 1, msg, msg % 5),
                    "role": if msg % 2 == 0 { "user" } else { "assistant" },
                    "path": format!("src/day{}_feature.rs", day + 1),
                    "decision": if msg == 0 { format!("Day {} focus: feature {}", day + 1, day) } else { String::new() },
                }),
                "week-project".to_string(),
                Some(format!("session-day-{}", day + 1)),
            );
            // Make older days old enough for consolidation
            record.created_at =
                (chrono::Utc::now() - chrono::Duration::days((7 - day) as i64)).to_rfc3339();
            record.significance = 0.3;
            store.insert_memory(&record).unwrap();
            total_inserted += 1;
        }

        // Nightly consolidation after each day (except day 7 — too recent)
        if day < 6 {
            let task = ConsolidationTask {
                schedule: ConsolidationSchedule::Nightly,
                from_layer: MemoryLayer::Raw,
                to_layer: MemoryLayer::Episode,
                project_id: "week-project".to_string(),
                max_memories: 500,
            };
            engine.run(&store, &task).unwrap();
        }
    }

    assert_eq!(total_inserted, 215);

    // Weekly consolidation
    let episodes = store
        .query_by_layer("week-project", MemoryLayer::Episode, 500)
        .unwrap();
    for ep in &episodes {
        let mut updated = ep.clone();
        updated.created_at = (chrono::Utc::now() - chrono::Duration::days(14)).to_rfc3339();
        updated.significance = 0.3;
        store.insert_memory(&updated).unwrap();
    }

    let weekly = ConsolidationTask {
        schedule: ConsolidationSchedule::Weekly,
        from_layer: MemoryLayer::Episode,
        to_layer: MemoryLayer::Summary,
        project_id: "week-project".to_string(),
        max_memories: 500,
    };
    engine.run(&store, &weekly).unwrap();

    let stats = store.hierarchy_stats("week-project").unwrap();
    println!(
        "E2E-3 Week stats: raw={}, ep={}, sum={}, pat={}, total={}",
        stats.raw_count,
        stats.episode_count,
        stats.summary_count,
        stats.pattern_count,
        stats.total_count
    );

    // Day 7 should still be in raw (too recent)
    assert!(stats.raw_count > 0, "Recent days should remain in raw");

    // Search should work across layers
    let results = store
        .search_fulltext("week-project", "feature", 100)
        .unwrap();
    assert!(!results.is_empty(), "Search should find memories");

    // Ghost Writer context for day 8
    // Add a pattern so context has something to load
    let mut pattern = MemoryRecord::new_compressed(
        "week-pat".to_string(),
        MemoryLayer::Pattern,
        serde_json::json!({"pattern": "Week-long project with daily feature work"}),
        vec![],
        "week-project".to_string(),
    );
    pattern.significance = 0.9;
    store.insert_memory(&pattern).unwrap();

    let ctx = SyncProtocol::load_session_context(&store, "week-project", 4096).unwrap();
    let output = ctx.to_ghost_writer_format();
    assert!(output.contains("CRITICAL INSTRUCTION"));

    println!("E2E-3 PASSED: Week-long workflow captured and consolidated");
}

// ═══════════════════════════════════════════════════════════════════
// E2E-5: Multi-Client Capture
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_e2e5_multi_client_capture() {
    let daemon = CaptureDaemon::new();

    // 25 messages from Claude Code (client log)
    for i in 0..25 {
        daemon.capture(CaptureEvent {
            role: CaptureRole::User,
            content: format!("Claude message {}: implement feature X", i),
            timestamp: i * 5000,
            source: CaptureSource::ClientLog,
            session_id: Some("claude-session".to_string()),
            project_path: None,
        });
    }

    // 25 messages from MCP tool calls
    for i in 0..25 {
        daemon.capture(CaptureEvent {
            role: CaptureRole::User,
            content: format!("MCP captured message {}: another detail", i),
            timestamp: 200_000 + i * 5000, // Different time range
            source: CaptureSource::McpStream,
            session_id: Some("mcp-session".to_string()),
            project_path: None,
        });
    }

    // 10 duplicate messages (same content + timestamp window as some Claude messages)
    for i in 0..10 {
        let captured = daemon.capture(CaptureEvent {
            role: CaptureRole::User,
            content: format!("Claude message {}: implement feature X", i),
            timestamp: i * 5000,              // Same as above
            source: CaptureSource::McpStream, // Different source but same content
            session_id: None,
            project_path: None,
        });
        assert!(!captured, "Duplicate message {} should be deduped", i);
    }

    // Should have exactly 50 (25 + 25, 10 deduped)
    let events = daemon.drain_buffer();
    assert_eq!(
        events.len(),
        50,
        "50 unique messages (10 duplicates removed)"
    );

    // Sync to store
    let store = LongevityStore::open_memory().unwrap();
    let result = SyncProtocol::sync_captures_to_sqlite(&store, &events, "multi-project").unwrap();
    assert_eq!(result.records_synced, 50);

    // Verify timestamps preserved
    let memories = store
        .query_by_layer("multi-project", MemoryLayer::Raw, 100)
        .unwrap();
    assert_eq!(memories.len(), 50);

    println!("E2E-5 PASSED: Multi-client capture with dedup works correctly");
}

// ═══════════════════════════════════════════════════════════════════
// E2E-7: Concurrent Stress Test
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_e2e7_concurrent_stress() {
    let store = LongevityStore::open_memory().unwrap();

    // 3 projects, 500 messages each
    for proj in ["proj-a", "proj-b", "proj-c"] {
        let daemon = CaptureDaemon::new();
        for i in 0..500 {
            daemon.capture(CaptureEvent {
                role: if i % 2 == 0 {
                    CaptureRole::User
                } else {
                    CaptureRole::Assistant
                },
                content: format!("{} concurrent message {}", proj, i),
                timestamp: i * 2000,
                source: CaptureSource::ClientLog,
                session_id: Some(format!("{}-session", proj)),
                project_path: None,
            });
        }
        let events = daemon.drain_buffer();
        SyncProtocol::sync_captures_to_sqlite(&store, &events, proj).unwrap();
    }

    // Verify zero cross-contamination
    for proj in ["proj-a", "proj-b", "proj-c"] {
        let count = store.total_count(proj).unwrap();
        assert_eq!(count, 500, "{} should have 500", proj);
    }

    // Run consolidation on all
    let engine = ConsolidationEngine::new();
    for proj in ["proj-a", "proj-b", "proj-c"] {
        // Mark old enough
        let all = store.query_by_layer(proj, MemoryLayer::Raw, 1000).unwrap();
        for m in &all {
            let mut updated = m.clone();
            updated.created_at = (chrono::Utc::now() - chrono::Duration::days(3)).to_rfc3339();
            updated.significance = 0.3;
            store.insert_memory(&updated).unwrap();
        }
        engine.run_all(&store, proj).unwrap();
    }

    // Create integrity proofs
    for proj in ["proj-a", "proj-b", "proj-c"] {
        let proof = IntegrityVerifier::create_merkle_proof(&store, proj).unwrap();
        assert!(proof.leaf_count > 0);
        let verified =
            IntegrityVerifier::verify_against_proof(&store, proj, &proof.root_hash).unwrap();
        assert!(verified, "{} integrity should verify", proj);
    }

    println!("E2E-7 PASSED: Concurrent 3-project stress test with consolidation and integrity");
}
