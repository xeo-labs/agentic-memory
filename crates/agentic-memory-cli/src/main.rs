//! CLI entry point for the `amem` command-line tool.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use serde::{Deserialize, Serialize};

use agentic_memory::cli::commands;
use agentic_memory::engine::{PatternSort, QueryEngine, TextSearchParams};
use agentic_memory::format::AmemReader;
use agentic_memory::graph::TraversalDirection;
use agentic_memory::types::{EdgeType, EventType};
use agentic_memory_mcp::session::workspace::{ContextRole, WorkspaceManager};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceContext {
    path: String,
    role: String,
    label: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct WorkspaceState {
    workspaces: HashMap<String, Vec<WorkspaceContext>>,
}

fn state_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".agentic")
        .join("memory")
        .join("workspaces.json")
}

fn load_state() -> agentic_memory::AmemResult<WorkspaceState> {
    let path = state_path();
    if !path.exists() {
        return Ok(WorkspaceState::default());
    }
    let raw = std::fs::read_to_string(path)?;
    serde_json::from_str(&raw).map_err(|e| {
        agentic_memory::AmemError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    })
}

fn save_state(state: &WorkspaceState) -> agentic_memory::AmemResult<()> {
    let path = state_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let raw = serde_json::to_string_pretty(state).map_err(|e| {
        agentic_memory::AmemError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    })?;
    std::fs::write(path, raw)?;
    Ok(())
}

fn load_workspace_manager(
    state: &WorkspaceState,
    workspace: &str,
) -> agentic_memory::AmemResult<(WorkspaceManager, String)> {
    let contexts = state.workspaces.get(workspace).ok_or_else(|| {
        agentic_memory::AmemError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("workspace '{}' not found", workspace),
        ))
    })?;

    let mut manager = WorkspaceManager::new();
    let ws_id = manager.create(workspace);

    for ctx in contexts {
        let role = ContextRole::from_str(&ctx.role).unwrap_or(ContextRole::Primary);
        manager
            .add_context(&ws_id, &ctx.path, role, ctx.label.clone())
            .map_err(|e| {
                agentic_memory::AmemError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    e.to_string(),
                ))
            })?;
    }

    Ok((manager, ws_id))
}

fn suggest_snippets(path: &Path, query: &str, limit: usize) -> agentic_memory::AmemResult<Vec<String>> {
    let graph = AmemReader::read_from_file(path)?;
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    let mut matches: Vec<(f32, String)> = Vec::new();

    for node in graph.nodes() {
        let content_lower = node.content.to_lowercase();
        let overlap = query_words
            .iter()
            .filter(|w| content_lower.contains(**w))
            .count();
        if overlap == 0 {
            continue;
        }
        let score = overlap as f32 / query_words.len().max(1) as f32;
        let snippet = if node.content.len() > 100 {
            format!("{}...", &node.content[..100])
        } else {
            node.content.clone()
        };
        matches.push((score, snippet));
    }

    matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    matches.truncate(limit);
    Ok(matches.into_iter().map(|(_, s)| s).collect())
}

#[derive(Parser)]
#[command(
    name = "amem",
    about = "AgenticMemory CLI — binary graph-based memory for AI agents",
    version
)]
struct Cli {
    /// Output format: "text" (default) or "json"
    #[arg(long, default_value = "text")]
    format: String,

    /// Enable debug logging
    #[arg(long)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new empty .amem file
    #[command(name = "init", alias = "create")]
    Create {
        /// Path to the .amem file to create
        file: PathBuf,
        /// Feature vector dimension
        #[arg(long, default_value = "128")]
        dimension: usize,
    },
    /// Display information about an .amem file
    Info {
        /// Path to the .amem file
        file: PathBuf,
    },
    /// Add a cognitive event to the graph
    Add {
        /// Path to the .amem file
        file: PathBuf,
        /// Event type: fact, decision, inference, correction, skill, episode
        #[arg(name = "type")]
        event_type: String,
        /// The content text
        content: String,
        /// Session ID
        #[arg(long, default_value = "0")]
        session: u32,
        /// Confidence 0.0-1.0
        #[arg(long, default_value = "1.0")]
        confidence: f32,
        /// For corrections: the node ID being corrected
        #[arg(long)]
        supersedes: Option<u64>,
    },
    /// Add an edge between two nodes
    Link {
        /// Path to the .amem file
        file: PathBuf,
        /// Source node ID
        source_id: u64,
        /// Target node ID
        target_id: u64,
        /// Edge type
        edge_type: String,
        /// Edge weight 0.0-1.0
        #[arg(long, default_value = "1.0")]
        weight: f32,
    },
    /// Get a specific node by ID
    Get {
        /// Path to the .amem file
        file: PathBuf,
        /// Node ID
        node_id: u64,
    },
    /// Run a traversal query from a starting node
    Traverse {
        /// Path to the .amem file
        file: PathBuf,
        /// Starting node ID
        start_id: u64,
        /// Comma-separated edge types to follow
        #[arg(long)]
        edge_types: Option<String>,
        /// Direction: forward, backward, or both
        #[arg(long, default_value = "backward")]
        direction: String,
        /// Maximum traversal depth
        #[arg(long, default_value = "5")]
        max_depth: u32,
        /// Maximum nodes to return
        #[arg(long, default_value = "50")]
        max_results: usize,
        /// Minimum confidence filter
        #[arg(long, default_value = "0.0")]
        min_confidence: f32,
    },
    /// Pattern query — find nodes matching conditions
    #[command(name = "query", alias = "search")]
    Search {
        /// Path to the .amem file
        file: PathBuf,
        /// Comma-separated event types to filter
        #[arg(long, name = "type")]
        event_types: Option<String>,
        /// Comma-separated session IDs
        #[arg(long)]
        session: Option<String>,
        /// Minimum confidence
        #[arg(long)]
        min_confidence: Option<f32>,
        /// Maximum confidence
        #[arg(long)]
        max_confidence: Option<f32>,
        /// Created after (Unix microseconds)
        #[arg(long)]
        after: Option<u64>,
        /// Created before (Unix microseconds)
        #[arg(long)]
        before: Option<u64>,
        /// Sort: recent, confidence, accessed, importance
        #[arg(long, default_value = "recent")]
        sort: String,
        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Run causal impact analysis on a node
    Impact {
        /// Path to the .amem file
        file: PathBuf,
        /// Node ID to analyze
        node_id: u64,
        /// Maximum depth
        #[arg(long, default_value = "10")]
        max_depth: u32,
    },
    /// Follow SUPERSEDES chain to find the latest version of a node
    Resolve {
        /// Path to the .amem file
        file: PathBuf,
        /// Node ID to resolve
        node_id: u64,
    },
    /// List all sessions in the file
    Sessions {
        /// Path to the .amem file
        file: PathBuf,
        /// Maximum sessions to show
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Export the graph as JSON
    Export {
        /// Path to the .amem file
        file: PathBuf,
        /// Export only nodes, no edges
        #[arg(long)]
        nodes_only: bool,
        /// Export only nodes from a specific session
        #[arg(long)]
        session: Option<u32>,
        /// Pretty-print JSON
        #[arg(long)]
        pretty: bool,
    },
    /// Verify a claim has memory backing
    Ground {
        /// Path to the .amem file
        file: PathBuf,
        /// Claim text to validate
        claim: String,
        /// Minimum BM25 score for evidence
        #[arg(long, default_value = "0.3")]
        threshold: f32,
    },
    /// Return supporting evidence for a query
    Evidence {
        /// Path to the .amem file
        file: PathBuf,
        /// Query text
        query: String,
        /// Maximum matches
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Suggest similar memories for a phrase
    Suggest {
        /// Path to the .amem file
        file: PathBuf,
        /// Query text
        query: String,
        /// Maximum suggestions
        #[arg(long, default_value = "5")]
        limit: usize,
    },
    /// Workspace operations across multiple memory files
    Workspace {
        #[command(subcommand)]
        subcommand: WorkspaceCommands,
    },
    /// Import nodes and edges from JSON
    Import {
        /// Path to the .amem file
        file: PathBuf,
        /// Path to the JSON file
        json_file: PathBuf,
    },
    /// Run decay calculations
    Decay {
        /// Path to the .amem file
        file: PathBuf,
        /// Report nodes below this decay score
        #[arg(long, default_value = "0.1")]
        threshold: f32,
    },
    /// Detailed statistics about the graph
    Stats {
        /// Path to the .amem file
        file: PathBuf,
    },
    /// Graph health and memory quality report
    Quality {
        /// Path to the .amem file
        file: PathBuf,
        /// Confidence threshold below which nodes are considered weak
        #[arg(long, default_value = "0.45")]
        low_confidence: f32,
        /// Decay threshold below which nodes are considered stale
        #[arg(long, default_value = "0.20")]
        stale_decay: f32,
        /// Max example IDs shown per category
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Scan workspace artifacts (.amem/.acb/.avis) and optionally write an episode snapshot
    RuntimeSync {
        /// Path to the .amem file
        file: PathBuf,
        /// Workspace root to scan
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        /// Maximum directory depth for scan
        #[arg(long, default_value = "4")]
        max_depth: u32,
        /// Session ID for episode write (0 = latest session)
        #[arg(long, default_value = "0")]
        session: u32,
        /// Persist a sync snapshot as an Episode node
        #[arg(long)]
        write_episode: bool,
    },
    /// Estimate long-horizon storage usage against a fixed budget
    Budget {
        /// Path to the .amem file
        file: PathBuf,
        /// Max allowed bytes over the horizon
        #[arg(long, default_value = "2147483648")]
        max_bytes: u64,
        /// Projection horizon in years
        #[arg(long, default_value = "20")]
        horizon_years: u32,
    },
    /// BM25 text search over node contents
    TextSearch {
        /// Path to the .amem file
        file: PathBuf,
        /// Search query text
        query: String,
        /// Comma-separated event types to filter
        #[arg(long, name = "type")]
        event_types: Option<String>,
        /// Comma-separated session IDs
        #[arg(long)]
        session: Option<String>,
        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Minimum BM25 score
        #[arg(long, default_value = "0.0")]
        min_score: f32,
    },
    /// Combined BM25 + vector search with RRF fusion
    HybridSearch {
        /// Path to the .amem file
        file: PathBuf,
        /// Search query text
        query: String,
        /// BM25 weight 0.0-1.0
        #[arg(long, default_value = "0.5")]
        text_weight: f32,
        /// Vector weight 0.0-1.0
        #[arg(long, default_value = "0.5")]
        vec_weight: f32,
        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Comma-separated event types to filter
        #[arg(long, name = "type")]
        event_types: Option<String>,
    },
    /// Compute node importance scores
    Centrality {
        /// Path to the .amem file
        file: PathBuf,
        /// Algorithm: pagerank, degree, or betweenness
        #[arg(long, default_value = "pagerank")]
        algorithm: String,
        /// PageRank damping factor
        #[arg(long, default_value = "0.85")]
        damping: f32,
        /// Comma-separated edge types to consider
        #[arg(long)]
        edge_types: Option<String>,
        /// Comma-separated event types to filter
        #[arg(long, name = "type")]
        event_types: Option<String>,
        /// Top N results
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Max iterations for PageRank
        #[arg(long, default_value = "100")]
        iterations: u32,
    },
    /// Find shortest path between two nodes
    Path {
        /// Path to the .amem file
        file: PathBuf,
        /// Source node ID
        source_id: u64,
        /// Target node ID
        target_id: u64,
        /// Comma-separated edge types to traverse
        #[arg(long)]
        edge_types: Option<String>,
        /// Direction: forward, backward, or both
        #[arg(long, default_value = "both")]
        direction: String,
        /// Maximum path length
        #[arg(long, default_value = "20")]
        max_depth: u32,
        /// Use edge weights for path cost
        #[arg(long)]
        weighted: bool,
    },
    /// Belief revision — counterfactual analysis
    Revise {
        /// Path to the .amem file
        file: PathBuf,
        /// The hypothetical new fact to test
        hypothesis: String,
        /// Contradiction detection threshold
        #[arg(long, default_value = "0.6")]
        threshold: f32,
        /// Propagation depth
        #[arg(long, default_value = "10")]
        max_depth: u32,
        /// Confidence of hypothesis
        #[arg(long, default_value = "0.9")]
        confidence: f32,
    },
    /// Reasoning gap detection
    Gaps {
        /// Path to the .amem file
        file: PathBuf,
        /// Confidence threshold
        #[arg(long, default_value = "0.5")]
        threshold: f32,
        /// Min support edges for decisions
        #[arg(long, default_value = "1")]
        min_support: u32,
        /// Maximum gaps to report
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Sort: dangerous, recent, or confidence
        #[arg(long, default_value = "dangerous")]
        sort: String,
        /// Session range "start:end"
        #[arg(long)]
        session: Option<String>,
    },
    /// Find structurally similar past situations
    Analogy {
        /// Path to the .amem file
        file: PathBuf,
        /// Text describing the current situation
        description: String,
        /// Maximum analogies
        #[arg(long, default_value = "5")]
        limit: usize,
        /// Minimum structural match
        #[arg(long, default_value = "0.3")]
        min_similarity: f32,
        /// Comma-separated sessions to exclude
        #[arg(long)]
        exclude_session: Option<String>,
        /// Context depth
        #[arg(long, default_value = "2")]
        depth: u32,
    },
    /// Brain maintenance — consolidation
    Consolidate {
        /// Path to the .amem file
        file: PathBuf,
        /// Merge near-duplicate facts
        #[arg(long)]
        deduplicate: bool,
        /// Detect and link unlinked contradictions
        #[arg(long)]
        link_contradictions: bool,
        /// Upgrade stable inferences to facts
        #[arg(long)]
        promote_inferences: bool,
        /// Report orphaned nodes (dry-run only)
        #[arg(long)]
        prune: bool,
        /// Report episode compression candidates (dry-run only)
        #[arg(long)]
        compress_episodes: bool,
        /// Run all operations
        #[arg(long)]
        all: bool,
        /// Similarity threshold for dedup
        #[arg(long, default_value = "0.95")]
        threshold: f32,
        /// Actually apply changes (default: dry-run)
        #[arg(long)]
        confirm: bool,
        /// Backup path
        #[arg(long)]
        backup: Option<PathBuf>,
    },
    /// Track how beliefs about a topic evolved
    Drift {
        /// Path to the .amem file
        file: PathBuf,
        /// Topic to track
        topic: String,
        /// Maximum timelines
        #[arg(long, default_value = "5")]
        limit: usize,
        /// Minimum relevance
        #[arg(long, default_value = "0.5")]
        min_relevance: f32,
    },
    /// Generate shell completion scripts
    ///
    /// Examples:
    ///   amem completions bash > ~/.local/share/bash-completion/completions/amem
    ///   amem completions zsh > ~/.zfunc/_amem
    ///   amem completions fish > ~/.config/fish/completions/amem.fish
    Completions {
        /// Shell type (bash, zsh, fish, powershell, elvish)
        shell: Shell,
    },
}

#[derive(Subcommand)]
enum WorkspaceCommands {
    /// Create a workspace
    Create { name: String },
    /// Add a memory file to a workspace
    Add {
        workspace: String,
        file: PathBuf,
        #[arg(long, default_value = "primary")]
        role: String,
        #[arg(long)]
        label: Option<String>,
    },
    /// List files in a workspace
    List { workspace: String },
    /// Query across all files in a workspace
    Query {
        workspace: String,
        query: String,
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Compare an item across workspace contexts
    Compare {
        workspace: String,
        item: String,
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Cross-reference an item across workspace contexts
    Xref { workspace: String, item: String },
}

fn main() {
    let cli = Cli::parse();
    let json = cli.format == "json";

    if cli.verbose {
        // env_logger is only available in dev/test builds
        eprintln!("Verbose mode enabled");
    }

    let result = match cli.command {
        // No subcommand → launch interactive REPL
        None => match agentic_memory::cli::repl::run() {
            Ok(()) => return,
            Err(e) => {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        },

        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "amem", &mut std::io::stdout());
            Ok(())
        }

        Some(Commands::Create { file, dimension }) => commands::cmd_create(&file, dimension),
        Some(Commands::Info { file }) => commands::cmd_info(&file, json),
        Some(Commands::Add {
            file,
            event_type,
            content,
            session,
            confidence,
            supersedes,
        }) => {
            let et = match EventType::from_name(&event_type) {
                Some(et) => et,
                None => {
                    eprintln!("Invalid event type: {}", event_type);
                    process::exit(3);
                }
            };
            commands::cmd_add(&file, et, &content, session, confidence, supersedes, json)
        }
        Some(Commands::Link {
            file,
            source_id,
            target_id,
            edge_type,
            weight,
        }) => {
            let et = match EdgeType::from_name(&edge_type) {
                Some(et) => et,
                None => {
                    eprintln!("Invalid edge type: {}", edge_type);
                    process::exit(3);
                }
            };
            commands::cmd_link(&file, source_id, target_id, et, weight, json)
        }
        Some(Commands::Get { file, node_id }) => commands::cmd_get(&file, node_id, json),
        Some(Commands::Traverse {
            file,
            start_id,
            edge_types,
            direction,
            max_depth,
            max_results,
            min_confidence,
        }) => {
            let ets: Vec<EdgeType> = edge_types
                .map(|s| {
                    s.split(',')
                        .filter_map(|t| EdgeType::from_name(t.trim()))
                        .collect()
                })
                .unwrap_or_default();
            let dir = match direction.as_str() {
                "forward" => TraversalDirection::Forward,
                "backward" => TraversalDirection::Backward,
                _ => TraversalDirection::Both,
            };
            commands::cmd_traverse(
                &file,
                start_id,
                ets,
                dir,
                max_depth,
                max_results,
                min_confidence,
                json,
            )
        }
        Some(Commands::Search {
            file,
            event_types,
            session,
            min_confidence,
            max_confidence,
            after,
            before,
            sort,
            limit,
        }) => {
            let ets: Vec<EventType> = event_types
                .map(|s| {
                    s.split(',')
                        .filter_map(|t| EventType::from_name(t.trim()))
                        .collect()
                })
                .unwrap_or_default();
            let sids: Vec<u32> = session
                .map(|s| s.split(',').filter_map(|t| t.trim().parse().ok()).collect())
                .unwrap_or_default();
            let sort_by = match sort.as_str() {
                "confidence" => PatternSort::HighestConfidence,
                "accessed" => PatternSort::MostAccessed,
                "importance" => PatternSort::MostImportant,
                _ => PatternSort::MostRecent,
            };
            commands::cmd_search(
                &file,
                ets,
                sids,
                min_confidence,
                max_confidence,
                after,
                before,
                sort_by,
                limit,
                json,
            )
        }
        Some(Commands::Impact {
            file,
            node_id,
            max_depth,
        }) => commands::cmd_impact(&file, node_id, max_depth, json),
        Some(Commands::Resolve { file, node_id }) => commands::cmd_resolve(&file, node_id, json),
        Some(Commands::Sessions { file, limit }) => commands::cmd_sessions(&file, limit, json),
        Some(Commands::Export {
            file,
            nodes_only,
            session,
            pretty,
        }) => commands::cmd_export(&file, nodes_only, session, pretty),
        Some(Commands::Ground {
            file,
            claim,
            threshold,
        }) => (|| -> agentic_memory::AmemResult<()> {
            let graph = AmemReader::read_from_file(&file)?;
            let engine = QueryEngine::new();
            let matches = engine.text_search(
                &graph,
                graph.term_index.as_ref(),
                graph.doc_lengths.as_ref(),
                TextSearchParams {
                    query: claim.clone(),
                    max_results: 10,
                    event_types: Vec::new(),
                    session_ids: Vec::new(),
                    min_score: 0.0,
                },
            )?;
            let evidence: Vec<_> = matches.iter().filter(|m| m.score >= threshold).collect();

            if evidence.is_empty() {
                let suggestions = suggest_snippets(&file, &claim, 5)?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "status": "ungrounded",
                            "claim": claim,
                            "suggestions": suggestions
                        }))
                        .unwrap_or_default()
                    );
                } else {
                    println!("Status: ungrounded");
                    println!("Claim: {}", claim);
                    if suggestions.is_empty() {
                        println!("Suggestions: none");
                    } else {
                        println!("Suggestions:");
                        for s in suggestions {
                            println!("  - {}", s);
                        }
                    }
                }
            } else {
                let rows: Vec<_> = evidence
                    .iter()
                    .filter_map(|m| {
                        graph.get_node(m.node_id).map(|node| {
                            serde_json::json!({
                                "node_id": node.id,
                                "event_type": node.event_type.name(),
                                "content": node.content,
                                "confidence": node.confidence,
                                "score": m.score
                            })
                        })
                    })
                    .collect();
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "status": "verified",
                            "claim": claim,
                            "evidence_count": rows.len(),
                            "evidence": rows
                        }))
                        .unwrap_or_default()
                    );
                } else {
                    println!("Status: verified");
                    println!("Claim: {}", claim);
                    println!("Evidence: {}", rows.len());
                    for row in rows {
                        let id = row
                            .get("node_id")
                            .and_then(|v| v.as_u64())
                            .unwrap_or_default();
                        let score = row.get("score").and_then(|v| v.as_f64()).unwrap_or_default();
                        let content = row.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        println!("  - [{}] score={:.3} {}", id, score, content);
                    }
                }
            }
            Ok(())
        })(),
        Some(Commands::Evidence { file, query, limit }) => (|| -> agentic_memory::AmemResult<()> {
            let graph = AmemReader::read_from_file(&file)?;
            let engine = QueryEngine::new();
            let matches = engine.text_search(
                &graph,
                graph.term_index.as_ref(),
                graph.doc_lengths.as_ref(),
                TextSearchParams {
                    query: query.clone(),
                    max_results: limit,
                    event_types: Vec::new(),
                    session_ids: Vec::new(),
                    min_score: 0.0,
                },
            )?;
            let rows: Vec<_> = matches
                .iter()
                .filter_map(|m| {
                    graph.get_node(m.node_id).map(|node| {
                        serde_json::json!({
                            "node_id": node.id,
                            "event_type": node.event_type.name(),
                            "content": node.content,
                            "confidence": node.confidence,
                            "score": m.score
                        })
                    })
                })
                .collect();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "query": query,
                        "count": rows.len(),
                        "evidence": rows
                    }))
                    .unwrap_or_default()
                );
            } else if rows.is_empty() {
                println!("No evidence found.");
            } else {
                println!("Evidence for {:?}:", query);
                for row in rows {
                    let id = row
                        .get("node_id")
                        .and_then(|v| v.as_u64())
                        .unwrap_or_default();
                    let score = row.get("score").and_then(|v| v.as_f64()).unwrap_or_default();
                    let content = row.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    println!("  - [{}] score={:.3} {}", id, score, content);
                }
            }
            Ok(())
        })(),
        Some(Commands::Suggest { file, query, limit }) => (|| -> agentic_memory::AmemResult<()> {
            let suggestions = suggest_snippets(&file, &query, limit)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "query": query,
                        "suggestions": suggestions
                    }))
                    .unwrap_or_default()
                );
            } else if suggestions.is_empty() {
                println!("No suggestions found.");
            } else {
                println!("Suggestions:");
                for s in suggestions {
                    println!("  - {}", s);
                }
            }
            Ok(())
        })(),
        Some(Commands::Workspace { subcommand }) => (|| -> agentic_memory::AmemResult<()> {
            match subcommand {
            WorkspaceCommands::Create { name } => {
                let mut state = load_state()?;
                state.workspaces.entry(name.clone()).or_default();
                save_state(&state)?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "workspace": name,
                            "created": true
                        }))
                        .unwrap_or_default()
                    );
                } else {
                    println!("Created workspace '{}'", name);
                }
                Ok(())
            }
            WorkspaceCommands::Add {
                workspace,
                file,
                role,
                label,
            } => {
                let mut state = load_state()?;
                let contexts = state.workspaces.entry(workspace.clone()).or_default();
                let file_path = file.to_string_lossy().to_string();
                if !contexts.iter().any(|ctx| ctx.path == file_path) {
                    contexts.push(WorkspaceContext {
                        path: file_path.clone(),
                        role: role.to_ascii_lowercase(),
                        label,
                    });
                    save_state(&state)?;
                }
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "workspace": workspace,
                            "path": file_path,
                            "added": true
                        }))
                        .unwrap_or_default()
                    );
                } else {
                    println!("Added {} to workspace '{}'", file.display(), workspace);
                }
                Ok(())
            }
            WorkspaceCommands::List { workspace } => {
                let state = load_state()?;
                let contexts = state.workspaces.get(&workspace).ok_or_else(|| {
                    agentic_memory::AmemError::Io(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("workspace '{}' not found", workspace),
                    ))
                })?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "workspace": workspace,
                            "contexts": contexts
                        }))
                        .unwrap_or_default()
                    );
                } else {
                    println!("Workspace '{}':", workspace);
                    for ctx in contexts {
                        println!(
                            "  - {} (role={}, label={})",
                            ctx.path,
                            ctx.role,
                            ctx.label.clone().unwrap_or_else(|| "-".to_string())
                        );
                    }
                }
                Ok(())
            }
            WorkspaceCommands::Query {
                workspace,
                query,
                limit,
            } => {
                let state = load_state()?;
                let (manager, ws_id) = load_workspace_manager(&state, &workspace)?;
                let results = manager.query_all(&ws_id, &query, limit).map_err(|e| {
                    agentic_memory::AmemError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        e.to_string(),
                    ))
                })?;
                if json {
                    let rows: Vec<_> = results
                        .iter()
                        .map(|r| {
                            serde_json::json!({
                                "context_id": r.context_id,
                                "role": r.context_role.label(),
                                "matches": r.matches.iter().map(|m| serde_json::json!({
                                    "node_id": m.node_id,
                                    "event_type": m.event_type,
                                    "score": m.score,
                                    "confidence": m.confidence,
                                    "content": m.content,
                                })).collect::<Vec<_>>()
                            })
                        })
                        .collect();
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "workspace": workspace,
                            "query": query,
                            "results": rows
                        }))
                        .unwrap_or_default()
                    );
                } else {
                    println!("Workspace query '{}':", query);
                    for r in results {
                        println!("  Context {} ({})", r.context_id, r.context_role.label());
                        for m in r.matches {
                            println!(
                                "    - [{}] score={:.3} {}",
                                m.node_id, m.score, m.content
                            );
                        }
                    }
                }
                Ok(())
            }
            WorkspaceCommands::Compare {
                workspace,
                item,
                limit,
            } => {
                let state = load_state()?;
                let (manager, ws_id) = load_workspace_manager(&state, &workspace)?;
                let comparison = manager.compare(&ws_id, &item, limit).map_err(|e| {
                    agentic_memory::AmemError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        e.to_string(),
                    ))
                })?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "workspace": workspace,
                            "item": comparison.item,
                            "found_in": comparison.found_in,
                            "missing_from": comparison.missing_from,
                            "matches": comparison.matches_per_context.iter().map(|(ctx, matches)| serde_json::json!({
                                "context": ctx,
                                "matches": matches.iter().map(|m| serde_json::json!({
                                    "node_id": m.node_id,
                                    "score": m.score,
                                    "content": m.content,
                                })).collect::<Vec<_>>()
                            })).collect::<Vec<_>>()
                        }))
                        .unwrap_or_default()
                    );
                } else {
                    println!("Found in: {:?}", comparison.found_in);
                    println!("Missing from: {:?}", comparison.missing_from);
                }
                Ok(())
            }
            WorkspaceCommands::Xref { workspace, item } => {
                let state = load_state()?;
                let (manager, ws_id) = load_workspace_manager(&state, &workspace)?;
                let xref = manager.cross_reference(&ws_id, &item).map_err(|e| {
                    agentic_memory::AmemError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        e.to_string(),
                    ))
                })?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "workspace": workspace,
                            "item": xref.item,
                            "present_in": xref.present_in,
                            "absent_from": xref.absent_from
                        }))
                        .unwrap_or_default()
                    );
                } else {
                    println!("Present in: {:?}", xref.present_in);
                    println!("Absent from: {:?}", xref.absent_from);
                }
                Ok(())
            }
            }
        })(),
        Some(Commands::Import { file, json_file }) => commands::cmd_import(&file, &json_file),
        Some(Commands::Decay { file, threshold }) => commands::cmd_decay(&file, threshold, json),
        Some(Commands::Stats { file }) => commands::cmd_stats(&file, json),
        Some(Commands::Quality {
            file,
            low_confidence,
            stale_decay,
            limit,
        }) => commands::cmd_quality(&file, low_confidence, stale_decay, limit, json),
        Some(Commands::RuntimeSync {
            file,
            workspace,
            max_depth,
            session,
            write_episode,
        }) => {
            commands::cmd_runtime_sync(&file, &workspace, max_depth, session, write_episode, json)
        }
        Some(Commands::Budget {
            file,
            max_bytes,
            horizon_years,
        }) => commands::cmd_budget(&file, max_bytes, horizon_years, json),
        Some(Commands::TextSearch {
            file,
            query,
            event_types,
            session,
            limit,
            min_score,
        }) => {
            let ets: Vec<EventType> = event_types
                .map(|s| {
                    s.split(',')
                        .filter_map(|t| EventType::from_name(t.trim()))
                        .collect()
                })
                .unwrap_or_default();
            let sids: Vec<u32> = session
                .map(|s| s.split(',').filter_map(|t| t.trim().parse().ok()).collect())
                .unwrap_or_default();
            commands::cmd_text_search(&file, &query, ets, sids, limit, min_score, json)
        }
        Some(Commands::HybridSearch {
            file,
            query,
            text_weight,
            vec_weight,
            limit,
            event_types,
        }) => {
            let ets: Vec<EventType> = event_types
                .map(|s| {
                    s.split(',')
                        .filter_map(|t| EventType::from_name(t.trim()))
                        .collect()
                })
                .unwrap_or_default();
            commands::cmd_hybrid_search(&file, &query, text_weight, vec_weight, limit, ets, json)
        }
        Some(Commands::Centrality {
            file,
            algorithm,
            damping,
            edge_types,
            event_types,
            limit,
            iterations,
        }) => {
            let ets: Vec<EventType> = event_types
                .map(|s| {
                    s.split(',')
                        .filter_map(|t| EventType::from_name(t.trim()))
                        .collect()
                })
                .unwrap_or_default();
            let edts: Vec<EdgeType> = edge_types
                .map(|s| {
                    s.split(',')
                        .filter_map(|t| EdgeType::from_name(t.trim()))
                        .collect()
                })
                .unwrap_or_default();
            commands::cmd_centrality(
                &file, &algorithm, damping, edts, ets, limit, iterations, json,
            )
        }
        Some(Commands::Path {
            file,
            source_id,
            target_id,
            edge_types,
            direction,
            max_depth,
            weighted,
        }) => {
            let edts: Vec<EdgeType> = edge_types
                .map(|s| {
                    s.split(',')
                        .filter_map(|t| EdgeType::from_name(t.trim()))
                        .collect()
                })
                .unwrap_or_default();
            let dir = match direction.as_str() {
                "forward" => TraversalDirection::Forward,
                "backward" => TraversalDirection::Backward,
                _ => TraversalDirection::Both,
            };
            commands::cmd_path(
                &file, source_id, target_id, edts, dir, max_depth, weighted, json,
            )
        }
        Some(Commands::Revise {
            file,
            hypothesis,
            threshold,
            max_depth,
            confidence,
        }) => commands::cmd_revise(&file, &hypothesis, threshold, max_depth, confidence, json),
        Some(Commands::Gaps {
            file,
            threshold,
            min_support,
            limit,
            sort,
            session,
        }) => {
            let session_range = session.and_then(|s| {
                let parts: Vec<&str> = s.split(':').collect();
                if parts.len() == 2 {
                    let start: u32 = parts[0].trim().parse().ok()?;
                    let end: u32 = parts[1].trim().parse().ok()?;
                    Some((start, end))
                } else {
                    None
                }
            });
            commands::cmd_gaps(
                &file,
                threshold,
                min_support,
                limit,
                &sort,
                session_range,
                json,
            )
        }
        Some(Commands::Analogy {
            file,
            description,
            limit,
            min_similarity,
            exclude_session,
            depth,
        }) => {
            let exclude: Vec<u32> = exclude_session
                .map(|s| s.split(',').filter_map(|t| t.trim().parse().ok()).collect())
                .unwrap_or_default();
            commands::cmd_analogy(
                &file,
                &description,
                limit,
                min_similarity,
                exclude,
                depth,
                json,
            )
        }
        Some(Commands::Consolidate {
            file,
            deduplicate,
            link_contradictions,
            promote_inferences,
            prune,
            compress_episodes,
            all,
            threshold,
            confirm,
            backup,
        }) => commands::cmd_consolidate(
            &file,
            deduplicate,
            link_contradictions,
            promote_inferences,
            prune,
            compress_episodes,
            all,
            threshold,
            confirm,
            backup,
            json,
        ),
        Some(Commands::Drift {
            file,
            topic,
            limit,
            min_relevance,
        }) => commands::cmd_drift(&file, &topic, limit, min_relevance, json),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        let code = match &e {
            agentic_memory::AmemError::Io(_) => 1,
            agentic_memory::AmemError::InvalidMagic
            | agentic_memory::AmemError::UnsupportedVersion(_)
            | agentic_memory::AmemError::Truncated
            | agentic_memory::AmemError::Corrupt(_) => 2,
            agentic_memory::AmemError::NodeNotFound(_)
            | agentic_memory::AmemError::InvalidEdgeTarget(_) => 4,
            _ => 5,
        };
        process::exit(code);
    }
}
