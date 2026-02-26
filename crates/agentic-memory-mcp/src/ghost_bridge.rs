//! Ghost Writer Bridge — Syncs V2 graph context to AI coding assistants.
//!
//! Bridges the V2 SessionManager graph to the V3 GhostWriter's multi-client
//! output format. Detects Claude Code, Cursor, Windsurf, and Cody, then
//! periodically writes a context summary to each client's memory directory.
//!
//! Gated behind `#[cfg(feature = "v3")]`.

use std::sync::Arc;
use tokio::sync::Mutex;

use agentic_memory::v3::edge_cases;
use agentic_memory::v3::engine::SessionResumeResult;
use agentic_memory::v3::ghost_writer::{DetectedClient, GhostWriter};
use agentic_memory::{EventType, PatternParams, PatternSort};

use crate::session::SessionManager;

/// Spawn a background tokio task that periodically syncs V2 graph context
/// to all detected AI coding assistant memory directories.
///
/// Returns `None` if no AI clients are detected (memory still works via MCP tools).
pub fn spawn_ghost_writer(
    session: Arc<Mutex<SessionManager>>,
) -> Option<tokio::task::JoinHandle<()>> {
    let clients = GhostWriter::detect_all_memory_dirs();
    if clients.is_empty() {
        tracing::info!(
            "Ghost Writer: no AI coding assistants detected. Sync disabled. \
             Memory still works via MCP tools."
        );
        return None;
    }

    for c in &clients {
        tracing::info!(
            "Ghost Writer: {} detected at {:?}",
            c.client_type.display_name(),
            c.memory_dir
        );
    }

    // Do an immediate first sync before entering the loop
    let session_clone = session.clone();
    let clients_clone = clients.clone();
    let handle = tokio::spawn(async move {
        // First sync immediately
        sync_once(&session_clone, &clients_clone).await;

        // Then sync every 5 seconds
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        interval.tick().await; // consume the first (immediate) tick
        loop {
            interval.tick().await;
            sync_once(&session_clone, &clients_clone).await;
        }
    });

    tracing::info!(
        "Ghost Writer: background sync started ({} clients, 5s interval)",
        clients.len()
    );
    Some(handle)
}

/// Perform one sync cycle — build context from V2 graph, write to all clients.
async fn sync_once(session: &Arc<Mutex<SessionManager>>, clients: &[DetectedClient]) {
    let context = match build_context_from_v2(session).await {
        Some(ctx) => ctx,
        None => return,
    };

    for client in clients {
        let filename = client.client_type.memory_filename();
        let target = client.memory_dir.join(filename);
        let markdown = GhostWriter::format_for_client(&context, client.client_type);

        if let Err(e) = edge_cases::safe_write_to_claude(&target, &markdown) {
            tracing::warn!("Ghost Writer: failed to sync to {:?}: {}", target, e);
        }
    }
}

/// Build a `SessionResumeResult` from the V2 SessionManager graph.
///
/// Extracts recent decisions, facts, episodes, skills, and corrections
/// to populate the context that GhostWriter formats for each AI client.
async fn build_context_from_v2(
    session: &Arc<Mutex<SessionManager>>,
) -> Option<SessionResumeResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let query = session.query_engine();

    // Recent decisions (most valuable for context)
    let decisions = query
        .pattern(
            graph,
            PatternParams {
                event_types: vec![EventType::Decision],
                min_confidence: Some(0.5),
                max_confidence: None,
                session_ids: vec![],
                created_after: None,
                created_before: None,
                min_decay_score: None,
                max_results: 10,
                sort_by: PatternSort::MostRecent,
            },
        )
        .unwrap_or_default();

    // Recent facts
    let facts = query
        .pattern(
            graph,
            PatternParams {
                event_types: vec![EventType::Fact],
                min_confidence: Some(0.7),
                max_confidence: None,
                session_ids: vec![],
                created_after: None,
                created_before: None,
                min_decay_score: None,
                max_results: 10,
                sort_by: PatternSort::MostRecent,
            },
        )
        .unwrap_or_default();

    // Last episode (session summary)
    let episodes = query
        .pattern(
            graph,
            PatternParams {
                event_types: vec![EventType::Episode],
                min_confidence: None,
                max_confidence: None,
                session_ids: vec![],
                created_after: None,
                created_before: None,
                min_decay_score: None,
                max_results: 1,
                sort_by: PatternSort::MostRecent,
            },
        )
        .unwrap_or_default();

    // Build recent_messages from episodes + facts (gives context)
    let mut recent_messages: Vec<(String, String)> = Vec::new();
    for ep in &episodes {
        let preview = if ep.content.len() > 200 {
            format!("{}...", &ep.content[..200])
        } else {
            ep.content.clone()
        };
        recent_messages.push(("session_summary".to_string(), preview));
    }

    // Add recent facts as context
    for fact in &facts {
        let preview = if fact.content.len() > 200 {
            format!("{}...", &fact.content[..200])
        } else {
            fact.content.clone()
        };
        recent_messages.push(("fact".to_string(), preview));
    }

    Some(SessionResumeResult {
        session_id: format!("{}", session.current_session_id()),
        block_count: graph.node_count(),
        recent_messages,
        files_touched: vec![], // V2 doesn't track file operations
        decisions: {
            let mut seen = std::collections::HashSet::new();
            decisions
                .iter()
                .filter(|d| seen.insert(d.content.clone()))
                .map(|d| d.content.clone())
                .collect()
        },
        errors_resolved: vec![], // V2 doesn't track error resolution
        all_known_files: vec![], // V2 doesn't track files
    })
}
