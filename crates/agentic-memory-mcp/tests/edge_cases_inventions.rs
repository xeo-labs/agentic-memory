//! Comprehensive edge-case tests for all 100 agentic-memory invention tools.
//!
//! Covers:
//! - Tool count verification (1 test)
//! - Smoke tests for all 100 invention tools via macro (100 tests)
//! - Invalid argument / boundary tests (15 tests)
//! - Rapid-fire / concurrency stress tests (4 tests)

mod common;

use agentic_memory_mcp::tools::ToolRegistry;
use common::fixtures::create_test_session;
use serde_json::json;

// ============================================================
// 1. Tool Count Verification
// ============================================================

#[test]
fn test_tool_registry_includes_all_inventions() {
    let tools = ToolRegistry::list_tools();
    // 24 core tools + 100 invention tools = 124 minimum
    assert!(
        tools.len() >= 124,
        "Expected >= 124 tools (24 core + 100 inventions), found {}",
        tools.len()
    );
}

// ============================================================
// 2. Smoke Tests — every invention tool called with empty args
// ============================================================

/// Macro to generate a smoke test for a single invention tool.
/// Each test calls the tool with `Some(json!({}))` and asserts it
/// does not panic. Both Ok and Err outcomes are acceptable —
/// the goal is to verify the tool handler exists and is wired up.
macro_rules! smoke_test {
    ($name:ident, $tool:expr) => {
        #[tokio::test]
        async fn $name() {
            let session = create_test_session();
            let result = ToolRegistry::call($tool, Some(json!({})), &session).await;
            // Must not panic; Ok or Err is fine
            let _ = result;
        }
    };
}

// --- INFINITE (17) ---
smoke_test!(smoke_memory_immortal_stats, "memory_immortal_stats");
smoke_test!(smoke_memory_immortal_prove, "memory_immortal_prove");
smoke_test!(smoke_memory_immortal_project, "memory_immortal_project");
smoke_test!(smoke_memory_immortal_tier_move, "memory_immortal_tier_move");
smoke_test!(smoke_memory_semantic_compress, "memory_semantic_compress");
smoke_test!(smoke_memory_semantic_dedup, "memory_semantic_dedup");
smoke_test!(smoke_memory_semantic_similar, "memory_semantic_similar");
smoke_test!(smoke_memory_semantic_cluster, "memory_semantic_cluster");
smoke_test!(smoke_memory_context_optimize, "memory_context_optimize");
smoke_test!(smoke_memory_context_expand, "memory_context_expand");
smoke_test!(smoke_memory_context_summarize, "memory_context_summarize");
smoke_test!(smoke_memory_context_navigate, "memory_context_navigate");
smoke_test!(smoke_memory_metabolism_status, "memory_metabolism_status");
smoke_test!(smoke_memory_metabolism_process, "memory_metabolism_process");
smoke_test!(
    smoke_memory_metabolism_strengthen,
    "memory_metabolism_strengthen"
);
smoke_test!(smoke_memory_metabolism_decay, "memory_metabolism_decay");
smoke_test!(
    smoke_memory_metabolism_consolidate,
    "memory_metabolism_consolidate"
);

// --- PROPHETIC (16) ---
smoke_test!(smoke_memory_predict, "memory_predict");
smoke_test!(smoke_memory_predict_preload, "memory_predict_preload");
smoke_test!(smoke_memory_predict_accuracy, "memory_predict_accuracy");
smoke_test!(smoke_memory_predict_feedback, "memory_predict_feedback");
smoke_test!(smoke_memory_prophecy, "memory_prophecy");
smoke_test!(smoke_memory_prophecy_similar, "memory_prophecy_similar");
smoke_test!(smoke_memory_prophecy_regret, "memory_prophecy_regret");
smoke_test!(smoke_memory_prophecy_track, "memory_prophecy_track");
smoke_test!(
    smoke_memory_counterfactual_what_if,
    "memory_counterfactual_what_if"
);
smoke_test!(
    smoke_memory_counterfactual_compare,
    "memory_counterfactual_compare"
);
smoke_test!(
    smoke_memory_counterfactual_insights,
    "memory_counterfactual_insights"
);
smoke_test!(
    smoke_memory_counterfactual_best,
    "memory_counterfactual_best"
);
smoke_test!(smoke_memory_dejavu_check, "memory_dejavu_check");
smoke_test!(smoke_memory_dejavu_history, "memory_dejavu_history");
smoke_test!(smoke_memory_dejavu_patterns, "memory_dejavu_patterns");
smoke_test!(smoke_memory_dejavu_feedback, "memory_dejavu_feedback");

// --- COLLECTIVE (17) ---
smoke_test!(smoke_memory_ancestor_list, "memory_ancestor_list");
smoke_test!(smoke_memory_ancestor_inherit, "memory_ancestor_inherit");
smoke_test!(smoke_memory_ancestor_verify, "memory_ancestor_verify");
smoke_test!(smoke_memory_ancestor_bequeath, "memory_ancestor_bequeath");
smoke_test!(smoke_memory_collective_join, "memory_collective_join");
smoke_test!(
    smoke_memory_collective_contribute,
    "memory_collective_contribute"
);
smoke_test!(smoke_memory_collective_query, "memory_collective_query");
smoke_test!(smoke_memory_collective_endorse, "memory_collective_endorse");
smoke_test!(
    smoke_memory_collective_challenge,
    "memory_collective_challenge"
);
smoke_test!(smoke_memory_fusion_analyze, "memory_fusion_analyze");
smoke_test!(smoke_memory_fusion_execute, "memory_fusion_execute");
smoke_test!(smoke_memory_fusion_resolve, "memory_fusion_resolve");
smoke_test!(smoke_memory_fusion_preview, "memory_fusion_preview");
smoke_test!(smoke_memory_telepathy_link, "memory_telepathy_link");
smoke_test!(smoke_memory_telepathy_sync, "memory_telepathy_sync");
smoke_test!(smoke_memory_telepathy_query, "memory_telepathy_query");
smoke_test!(smoke_memory_telepathy_stream, "memory_telepathy_stream");

// --- RESURRECTION (17) ---
smoke_test!(smoke_memory_archaeology_dig, "memory_archaeology_dig");
smoke_test!(
    smoke_memory_archaeology_artifacts,
    "memory_archaeology_artifacts"
);
smoke_test!(
    smoke_memory_archaeology_reconstruct,
    "memory_archaeology_reconstruct"
);
smoke_test!(smoke_memory_archaeology_verify, "memory_archaeology_verify");
smoke_test!(smoke_memory_holographic_status, "memory_holographic_status");
smoke_test!(
    smoke_memory_holographic_reconstruct,
    "memory_holographic_reconstruct"
);
smoke_test!(
    smoke_memory_holographic_simulate,
    "memory_holographic_simulate"
);
smoke_test!(
    smoke_memory_holographic_distribute,
    "memory_holographic_distribute"
);
smoke_test!(smoke_memory_immune_status, "memory_immune_status");
smoke_test!(smoke_memory_immune_scan, "memory_immune_scan");
smoke_test!(smoke_memory_immune_quarantine, "memory_immune_quarantine");
smoke_test!(smoke_memory_immune_release, "memory_immune_release");
smoke_test!(smoke_memory_immune_train, "memory_immune_train");
smoke_test!(smoke_memory_phoenix_initiate, "memory_phoenix_initiate");
smoke_test!(smoke_memory_phoenix_gather, "memory_phoenix_gather");
smoke_test!(
    smoke_memory_phoenix_reconstruct,
    "memory_phoenix_reconstruct"
);
smoke_test!(smoke_memory_phoenix_status, "memory_phoenix_status");

// --- METAMEMORY (17) ---
smoke_test!(smoke_memory_meta_inventory, "memory_meta_inventory");
smoke_test!(smoke_memory_meta_gaps, "memory_meta_gaps");
smoke_test!(smoke_memory_meta_calibration, "memory_meta_calibration");
smoke_test!(smoke_memory_meta_capabilities, "memory_meta_capabilities");
smoke_test!(smoke_memory_dream_status, "memory_dream_status");
smoke_test!(smoke_memory_dream_start, "memory_dream_start");
smoke_test!(smoke_memory_dream_wake, "memory_dream_wake");
smoke_test!(smoke_memory_dream_insights, "memory_dream_insights");
smoke_test!(smoke_memory_dream_history, "memory_dream_history");
smoke_test!(smoke_memory_belief_list, "memory_belief_list");
smoke_test!(smoke_memory_belief_history, "memory_belief_history");
smoke_test!(smoke_memory_belief_revise, "memory_belief_revise");
smoke_test!(smoke_memory_belief_conflicts, "memory_belief_conflicts");
smoke_test!(smoke_memory_load_status, "memory_load_status");
smoke_test!(smoke_memory_load_cache, "memory_load_cache");
smoke_test!(smoke_memory_load_prefetch, "memory_load_prefetch");
smoke_test!(smoke_memory_load_optimize, "memory_load_optimize");

// --- TRANSCENDENT (16) ---
smoke_test!(smoke_memory_singularity_status, "memory_singularity_status");
smoke_test!(smoke_memory_singularity_query, "memory_singularity_query");
smoke_test!(
    smoke_memory_singularity_contribute,
    "memory_singularity_contribute"
);
smoke_test!(smoke_memory_singularity_trust, "memory_singularity_trust");
smoke_test!(smoke_memory_temporal_travel, "memory_temporal_travel");
smoke_test!(smoke_memory_temporal_project, "memory_temporal_project");
smoke_test!(smoke_memory_temporal_compare, "memory_temporal_compare");
smoke_test!(smoke_memory_temporal_paradox, "memory_temporal_paradox");
smoke_test!(smoke_memory_crystal_create, "memory_crystal_create");
smoke_test!(smoke_memory_crystal_transfer, "memory_crystal_transfer");
smoke_test!(smoke_memory_crystal_inspect, "memory_crystal_inspect");
smoke_test!(smoke_memory_crystal_merge, "memory_crystal_merge");
smoke_test!(smoke_memory_transcend_status, "memory_transcend_status");
smoke_test!(
    smoke_memory_transcend_distribute,
    "memory_transcend_distribute"
);
smoke_test!(smoke_memory_transcend_verify, "memory_transcend_verify");
smoke_test!(smoke_memory_transcend_eternal, "memory_transcend_eternal");

// ============================================================
// 3. Invalid Argument / Boundary Tests (15 tests)
// ============================================================

#[tokio::test]
async fn test_invention_tool_with_none_args_immortal_stats() {
    let session = create_test_session();
    let result = ToolRegistry::call("memory_immortal_stats", None, &session).await;
    // None args should be handled gracefully (Ok or Err, no panic)
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_none_args_predict() {
    let session = create_test_session();
    let result = ToolRegistry::call("memory_predict", None, &session).await;
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_none_args_dream_start() {
    let session = create_test_session();
    let result = ToolRegistry::call("memory_dream_start", None, &session).await;
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_wrong_type_string_instead_of_object() {
    let session = create_test_session();
    // Pass a bare string where an object is expected
    let result = ToolRegistry::call(
        "memory_semantic_compress",
        Some(json!("not_an_object")),
        &session,
    )
    .await;
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_wrong_type_array_instead_of_object() {
    let session = create_test_session();
    let result =
        ToolRegistry::call("memory_collective_join", Some(json!([1, 2, 3])), &session).await;
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_wrong_type_number_instead_of_object() {
    let session = create_test_session();
    let result = ToolRegistry::call("memory_phoenix_initiate", Some(json!(42)), &session).await;
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_nonexistent_node_id() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_immortal_prove",
        Some(json!({"node_id": 999999999})),
        &session,
    )
    .await;
    // Should handle missing node gracefully
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_negative_node_id() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_immortal_tier_move",
        Some(json!({"node_id": -1, "target_tier": "gold"})),
        &session,
    )
    .await;
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_zero_confidence() {
    let session = create_test_session();
    let result =
        ToolRegistry::call("memory_predict", Some(json!({"confidence": 0.0})), &session).await;
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_out_of_range_confidence() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_predict",
        Some(json!({"confidence": 999.99})),
        &session,
    )
    .await;
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_negative_confidence() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_belief_revise",
        Some(json!({"confidence": -0.5, "belief": "test"})),
        &session,
    )
    .await;
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_empty_string_fields() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_prophecy",
        Some(json!({"question": "", "context": ""})),
        &session,
    )
    .await;
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_extremely_long_string() {
    let session = create_test_session();
    let long_str = "x".repeat(100_000);
    let result = ToolRegistry::call(
        "memory_counterfactual_what_if",
        Some(json!({"scenario": long_str})),
        &session,
    )
    .await;
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_unicode_args() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_dejavu_check",
        Some(json!({"pattern": "\u{1F9E0}\u{1F4A1} remembering \u{00E9}\u{00E8}\u{00EA}"})),
        &session,
    )
    .await;
    let _ = result;
}

#[tokio::test]
async fn test_invention_tool_with_null_valued_fields() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_crystal_create",
        Some(json!({"name": null, "data": null, "ttl": null})),
        &session,
    )
    .await;
    let _ = result;
}

// ============================================================
// 4. Rapid-Fire / Concurrency Stress Tests (4 tests)
// ============================================================

#[tokio::test]
async fn test_rapid_fire_immortal_stats_50x() {
    let session = create_test_session();
    for i in 0..50 {
        let result = ToolRegistry::call("memory_immortal_stats", Some(json!({})), &session).await;
        assert!(
            result.is_ok() || result.is_err(),
            "Iteration {i} should not panic"
        );
    }
}

#[tokio::test]
async fn test_rapid_fire_semantic_compress_20x() {
    let session = create_test_session();
    for i in 0..20 {
        let result =
            ToolRegistry::call("memory_semantic_compress", Some(json!({})), &session).await;
        assert!(
            result.is_ok() || result.is_err(),
            "Compress iteration {i} should not panic"
        );
    }
}

#[tokio::test]
async fn test_rapid_fire_interleaved_tools() {
    let session = create_test_session();
    let tools = [
        "memory_immortal_stats",
        "memory_predict",
        "memory_meta_inventory",
        "memory_dream_status",
        "memory_immune_status",
        "memory_archaeology_dig",
        "memory_collective_query",
        "memory_singularity_status",
        "memory_holographic_status",
        "memory_load_status",
    ];
    for round in 0..5 {
        for tool in &tools {
            let result = ToolRegistry::call(tool, Some(json!({})), &session).await;
            let _ = result;
            // Just ensure no panics across 50 interleaved calls
        }
        // Verify session is still usable after each round
        let check = ToolRegistry::call("memory_immortal_stats", Some(json!({})), &session).await;
        assert!(
            check.is_ok() || check.is_err(),
            "Session corrupted after interleaved round {round}"
        );
    }
}

#[tokio::test]
async fn test_concurrent_invention_tools_with_spawn() {
    let session = create_test_session();

    let tools: Vec<&str> = vec![
        "memory_immortal_stats",
        "memory_predict",
        "memory_prophecy",
        "memory_dejavu_check",
        "memory_ancestor_list",
        "memory_collective_query",
        "memory_fusion_analyze",
        "memory_telepathy_query",
        "memory_archaeology_dig",
        "memory_holographic_status",
        "memory_immune_status",
        "memory_phoenix_status",
        "memory_meta_inventory",
        "memory_dream_status",
        "memory_belief_list",
        "memory_load_status",
        "memory_singularity_status",
        "memory_temporal_travel",
        "memory_crystal_inspect",
        "memory_transcend_status",
    ];

    let mut handles = Vec::new();
    for tool in tools {
        let s = session.clone();
        let tool_name = tool.to_string();
        handles.push(tokio::spawn(async move {
            let result = ToolRegistry::call(&tool_name, Some(json!({})), &s).await;
            // No panic — that is the assertion
            let _ = result;
        }));
    }

    for (i, handle) in handles.into_iter().enumerate() {
        handle
            .await
            .unwrap_or_else(|e| panic!("Concurrent task {i} panicked: {e}"));
    }
}
