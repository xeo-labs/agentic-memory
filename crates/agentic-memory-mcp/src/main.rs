//! AgenticMemory MCP Server — entry point.

use std::io::Write;
use std::sync::Arc;
use tokio::sync::Mutex;

use clap::{Parser, Subcommand};

use agentic_memory_mcp::config::resolve_memory_path;
use agentic_memory_mcp::protocol::ProtocolHandler;
use agentic_memory_mcp::session::autosave::spawn_maintenance;
use agentic_memory_mcp::session::SessionManager;
use agentic_memory_mcp::tools::ToolRegistry;
use agentic_memory_mcp::transport::StdioTransport;
use agentic_memory_mcp::types::MemoryMode;

#[derive(Parser)]
#[command(
    name = "agentic-memory-mcp",
    about = "MCP server for AgenticMemory — universal LLM access to persistent graph memory",
    version
)]
struct Cli {
    /// Path to .amem memory file.
    #[arg(short, long)]
    memory: Option<String>,

    /// Configuration file path.
    #[arg(short, long)]
    config: Option<String>,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long, default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start MCP server over stdio (default).
    Serve {
        /// Path to .amem memory file.
        #[arg(short, long)]
        memory: Option<String>,

        /// Configuration file path.
        #[arg(short, long)]
        config: Option<String>,

        /// Log level (trace, debug, info, warn, error).
        #[arg(long)]
        log_level: Option<String>,

        /// Memory mode: minimal (explicit only), smart (auto-save facts+decisions),
        /// full (save everything). Default: smart.
        #[arg(long, default_value = "smart")]
        mode: String,
    },

    /// Start MCP server over HTTP.
    #[cfg(feature = "sse")]
    ServeHttp {
        /// Listen address (host:port).
        #[arg(long, default_value = "127.0.0.1:3000")]
        addr: String,

        /// Path to .amem memory file (single-user mode).
        #[arg(short, long)]
        memory: Option<String>,

        /// Configuration file path.
        #[arg(short, long)]
        config: Option<String>,

        /// Log level (trace, debug, info, warn, error).
        #[arg(long)]
        log_level: Option<String>,

        /// Memory mode: minimal, smart, full. Default: smart.
        #[arg(long, default_value = "smart")]
        mode: String,

        /// Bearer token for authentication.
        /// Also reads from AGENTIC_TOKEN env var.
        #[arg(long)]
        token: Option<String>,

        /// Enable multi-tenant mode (per-user brain files).
        #[arg(long)]
        multi_tenant: bool,

        /// Data directory for multi-tenant brain files.
        /// Each user gets {data-dir}/{user-id}.amem.
        #[arg(long)]
        data_dir: Option<String>,
    },

    /// Validate a memory file.
    Validate,

    /// Print server capabilities as JSON.
    Info,

    /// Delete a specific memory node by ID.
    Delete {
        /// Node ID to delete.
        #[arg(long)]
        node_id: u64,

        /// Skip confirmation prompt.
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Export all memories to stdout.
    Export {
        /// Output format: json or csv.
        #[arg(long, default_value = "json")]
        format: String,
    },

    /// Remove low-scoring nodes (compaction).
    Compact {
        /// Keep nodes with decay_score above this threshold.
        #[arg(long)]
        keep_above: f32,

        /// Skip confirmation prompt.
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Print graph statistics.
    Stats,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cli.log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    match cli.command.unwrap_or(Commands::Serve {
        memory: None,
        config: None,
        log_level: None,
        mode: "smart".to_string(),
    }) {
        Commands::Serve {
            memory,
            config: _,
            log_level: _,
            mode,
        } => {
            let effective_memory = memory.or(cli.memory);
            let memory_path = resolve_memory_path(effective_memory.as_deref());
            let memory_mode = MemoryMode::parse(&mode).unwrap_or_else(|| {
                tracing::warn!("Unknown mode '{mode}', falling back to 'smart'");
                MemoryMode::Smart
            });
            tracing::info!("AgenticMemory MCP server");
            tracing::info!("Brain: {memory_path}");
            tracing::info!("Mode: {mode}");
            let session = SessionManager::open(&memory_path)?;
            let maintenance_interval = session.maintenance_interval();
            let session = Arc::new(Mutex::new(session));
            let _maintenance_task = spawn_maintenance(session.clone(), maintenance_interval);
            let handler = ProtocolHandler::with_mode(session, memory_mode);
            let transport = StdioTransport::new(handler);
            transport.run().await?;
        }

        #[cfg(feature = "sse")]
        Commands::ServeHttp {
            addr,
            memory,
            config: _,
            log_level: _,
            mode,
            token,
            multi_tenant,
            data_dir,
        } => {
            use agentic_memory_mcp::session::tenant::TenantRegistry;
            use agentic_memory_mcp::transport::sse::{ServerMode, SseTransport};

            let memory_mode = MemoryMode::parse(&mode).unwrap_or_else(|| {
                tracing::warn!("Unknown mode '{mode}', falling back to 'smart'");
                MemoryMode::Smart
            });

            // Resolve token: CLI flag > env var
            let effective_token = token.or_else(|| std::env::var("AGENTIC_TOKEN").ok());

            let server_mode = if multi_tenant {
                let dir = data_dir.unwrap_or_else(|| {
                    eprintln!("Error: --data-dir is required when using --multi-tenant");
                    std::process::exit(1);
                });
                let dir = std::path::PathBuf::from(&dir);
                tracing::info!("AgenticMemory MCP server (multi-tenant)");
                tracing::info!("Data dir: {}", dir.display());
                tracing::info!("Mode: {mode}");
                ServerMode::MultiTenant {
                    data_dir: dir.clone(),
                    registry: Arc::new(Mutex::new(TenantRegistry::new(&dir))),
                    memory_mode,
                }
            } else {
                let effective_memory = memory.or(cli.memory);
                let memory_path = resolve_memory_path(effective_memory.as_deref());
                tracing::info!("AgenticMemory MCP server");
                tracing::info!("Brain: {memory_path}");
                tracing::info!("Mode: {mode}");
                let session = SessionManager::open(&memory_path)?;
                let maintenance_interval = session.maintenance_interval();
                let session = Arc::new(Mutex::new(session));
                let _maintenance_task = spawn_maintenance(session.clone(), maintenance_interval);
                let handler = ProtocolHandler::with_mode(session, memory_mode);
                ServerMode::Single(Arc::new(handler))
            };

            if effective_token.is_some() {
                tracing::info!("Auth: bearer token required");
            }

            let transport = SseTransport::with_config(effective_token, server_mode);
            transport.run(&addr).await?;
        }

        Commands::Validate => {
            let memory_path = resolve_memory_path(cli.memory.as_deref());
            match SessionManager::open(&memory_path) {
                Ok(session) => {
                    let graph = session.graph();
                    println!("Valid memory file: {memory_path}");
                    println!("  Nodes: {}", graph.node_count());
                    println!("  Edges: {}", graph.edge_count());
                    println!("  Dimension: {}", graph.dimension());
                    println!("  Sessions: {}", graph.session_index().session_count());
                }
                Err(e) => {
                    eprintln!("Invalid memory file: {e}");
                    std::process::exit(1);
                }
            }
        }

        Commands::Info => {
            let capabilities = agentic_memory_mcp::types::InitializeResult::default_result();
            let tools = ToolRegistry::list_tools();
            let info = serde_json::json!({
                "server": capabilities.server_info,
                "protocol_version": capabilities.protocol_version,
                "capabilities": capabilities.capabilities,
                "tools": tools.iter().map(|t| &t.name).collect::<Vec<_>>(),
                "tool_count": tools.len(),
            });
            println!("{}", serde_json::to_string_pretty(&info)?);
        }

        Commands::Delete { node_id, yes } => {
            let memory_path = resolve_memory_path(cli.memory.as_deref());
            let mut session = match SessionManager::open(&memory_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };

            // Verify node exists before prompting
            let node_info = match session.graph().get_node(node_id) {
                Some(n) => format!(
                    "Node {} ({}, session {}, confidence {:.2}): {:?}",
                    n.id,
                    n.event_type.name(),
                    n.session_id,
                    n.confidence,
                    if n.content.len() > 80 {
                        format!("{}...", &n.content[..80])
                    } else {
                        n.content.clone()
                    }
                ),
                None => {
                    eprintln!("Error: node {node_id} not found");
                    std::process::exit(1);
                }
            };

            if !yes {
                eprint!("Delete {node_info}? [y/N] ");
                std::io::stderr().flush().ok();
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if !answer.trim().eq_ignore_ascii_case("y") {
                    eprintln!("Aborted.");
                    std::process::exit(0);
                }
            }

            match session.graph_mut().remove_node(node_id) {
                Ok(_removed) => {
                    if let Err(e) = session.save() {
                        eprintln!("Error saving: {e}");
                        std::process::exit(1);
                    }
                    println!("Deleted node {node_id}");
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }

        Commands::Export { format } => {
            let memory_path = resolve_memory_path(cli.memory.as_deref());
            let session = match SessionManager::open(&memory_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };

            let graph = session.graph();

            match format.as_str() {
                "json" => {
                    let nodes_json: Vec<serde_json::Value> = graph
                        .nodes()
                        .iter()
                        .map(|n| {
                            serde_json::json!({
                                "id": n.id,
                                "event_type": n.event_type.name(),
                                "created_at": n.created_at,
                                "session_id": n.session_id,
                                "confidence": n.confidence,
                                "access_count": n.access_count,
                                "last_accessed": n.last_accessed,
                                "decay_score": n.decay_score,
                                "content": n.content,
                            })
                        })
                        .collect();

                    let edges_json: Vec<serde_json::Value> = graph
                        .edges()
                        .iter()
                        .map(|e| {
                            serde_json::json!({
                                "source_id": e.source_id,
                                "target_id": e.target_id,
                                "edge_type": e.edge_type.name(),
                                "weight": e.weight,
                                "created_at": e.created_at,
                            })
                        })
                        .collect();

                    let output = serde_json::json!({
                        "nodes": nodes_json,
                        "edges": edges_json,
                    });
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&output).unwrap_or_default()
                    );
                }
                "csv" => {
                    println!("id,event_type,created_at,session_id,confidence,access_count,last_accessed,decay_score,content");
                    for n in graph.nodes() {
                        // Escape content for CSV: double-quote, escape inner quotes
                        let escaped = n.content.replace('"', "\"\"");
                        println!(
                            "{},{},{},{},{:.4},{},{},{:.4},\"{}\"",
                            n.id,
                            n.event_type.name(),
                            n.created_at,
                            n.session_id,
                            n.confidence,
                            n.access_count,
                            n.last_accessed,
                            n.decay_score,
                            escaped
                        );
                    }
                }
                _ => {
                    eprintln!("Error: unknown format '{format}'. Use 'json' or 'csv'.");
                    std::process::exit(1);
                }
            }
        }

        Commands::Compact { keep_above, yes } => {
            let memory_path = resolve_memory_path(cli.memory.as_deref());
            let mut session = match SessionManager::open(&memory_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };

            // First run decay to ensure scores are fresh
            let current_time = agentic_memory::now_micros();
            let write_engine = agentic_memory::WriteEngine::new(session.graph().dimension());
            if let Err(e) = write_engine.run_decay(session.graph_mut(), current_time) {
                eprintln!("Error running decay: {e}");
                std::process::exit(1);
            }

            // Find nodes below threshold
            let to_remove: Vec<(u64, f32, String)> = session
                .graph()
                .nodes()
                .iter()
                .filter(|n| n.decay_score < keep_above)
                .map(|n| {
                    let preview = if n.content.len() > 60 {
                        format!("{}...", &n.content[..60])
                    } else {
                        n.content.clone()
                    };
                    (n.id, n.decay_score, preview)
                })
                .collect();

            if to_remove.is_empty() {
                println!("No nodes below threshold {keep_above}. Nothing to compact.");
                return Ok(());
            }

            if !yes {
                eprintln!(
                    "Will remove {} nodes with decay_score < {keep_above}:",
                    to_remove.len()
                );
                for (id, score, preview) in &to_remove {
                    eprintln!("  Node {id} (score: {score:.4}): {preview}");
                }
                eprint!("Proceed? [y/N] ");
                std::io::stderr().flush().ok();
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if !answer.trim().eq_ignore_ascii_case("y") {
                    eprintln!("Aborted.");
                    std::process::exit(0);
                }
            }

            let mut removed_count = 0;
            for (id, _, _) in &to_remove {
                match session.graph_mut().remove_node(*id) {
                    Ok(_) => removed_count += 1,
                    Err(e) => eprintln!("Warning: failed to remove node {id}: {e}"),
                }
            }

            if let Err(e) = session.save() {
                eprintln!("Error saving: {e}");
                std::process::exit(1);
            }

            println!("Compacted: removed {removed_count} nodes below threshold {keep_above}");
        }

        Commands::Stats => {
            let memory_path = resolve_memory_path(cli.memory.as_deref());
            let session = match SessionManager::open(&memory_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };

            let graph = session.graph();
            let file_size = std::fs::metadata(&memory_path)
                .map(|m| m.len())
                .unwrap_or(0);

            let node_count = graph.node_count();
            let edge_count = graph.edge_count();
            let session_count = graph.session_index().session_count();

            let type_counts = [
                ("facts", agentic_memory::EventType::Fact),
                ("decisions", agentic_memory::EventType::Decision),
                ("inferences", agentic_memory::EventType::Inference),
                ("corrections", agentic_memory::EventType::Correction),
                ("skills", agentic_memory::EventType::Skill),
                ("episodes", agentic_memory::EventType::Episode),
            ];

            let file_size_str = if file_size < 1024 {
                format!("{} B", file_size)
            } else if file_size < 1024 * 1024 {
                format!("{:.1} KB", file_size as f64 / 1024.0)
            } else {
                format!("{:.1} MB", file_size as f64 / (1024.0 * 1024.0))
            };

            println!("Brain: {memory_path}");
            println!("  Nodes:    {node_count}");
            println!("  Edges:    {edge_count}");
            println!("  Sessions: {session_count}");
            println!("  File:     {file_size_str}");

            // Type breakdown
            let mut has_types = false;
            for (label, et) in &type_counts {
                let count = graph.type_index().count(*et);
                if count > 0 {
                    if !has_types {
                        println!("  Types:");
                        has_types = true;
                    }
                    println!("    {label}: {count}");
                }
            }
        }
    }

    Ok(())
}
