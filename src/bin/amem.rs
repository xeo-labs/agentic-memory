//! CLI entry point for the `amem` command-line tool.

use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

use agentic_memory::cli::commands;
use agentic_memory::engine::PatternSort;
use agentic_memory::graph::TraversalDirection;
use agentic_memory::types::{EdgeType, EventType};

#[derive(Parser)]
#[command(
    name = "amem",
    about = "AgenticMemory CLI — binary graph-based memory for AI agents"
)]
struct Cli {
    /// Output format: "text" (default) or "json"
    #[arg(long, default_value = "text")]
    format: String,

    /// Enable debug logging
    #[arg(long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new empty .amem file
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
}

fn main() {
    let cli = Cli::parse();
    let json = cli.format == "json";

    if cli.verbose {
        // env_logger is only available in dev/test builds
        eprintln!("Verbose mode enabled");
    }

    let result = match cli.command {
        Commands::Create { file, dimension } => commands::cmd_create(&file, dimension),
        Commands::Info { file } => commands::cmd_info(&file, json),
        Commands::Add {
            file,
            event_type,
            content,
            session,
            confidence,
            supersedes,
        } => {
            let et = match EventType::from_name(&event_type) {
                Some(et) => et,
                None => {
                    eprintln!("Invalid event type: {}", event_type);
                    process::exit(3);
                }
            };
            commands::cmd_add(&file, et, &content, session, confidence, supersedes, json)
        }
        Commands::Link {
            file,
            source_id,
            target_id,
            edge_type,
            weight,
        } => {
            let et = match EdgeType::from_name(&edge_type) {
                Some(et) => et,
                None => {
                    eprintln!("Invalid edge type: {}", edge_type);
                    process::exit(3);
                }
            };
            commands::cmd_link(&file, source_id, target_id, et, weight, json)
        }
        Commands::Get { file, node_id } => commands::cmd_get(&file, node_id, json),
        Commands::Traverse {
            file,
            start_id,
            edge_types,
            direction,
            max_depth,
            max_results,
            min_confidence,
        } => {
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
        Commands::Search {
            file,
            event_types,
            session,
            min_confidence,
            max_confidence,
            after,
            before,
            sort,
            limit,
        } => {
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
        Commands::Impact {
            file,
            node_id,
            max_depth,
        } => commands::cmd_impact(&file, node_id, max_depth, json),
        Commands::Resolve { file, node_id } => commands::cmd_resolve(&file, node_id, json),
        Commands::Sessions { file, limit } => commands::cmd_sessions(&file, limit, json),
        Commands::Export {
            file,
            nodes_only,
            session,
            pretty,
        } => commands::cmd_export(&file, nodes_only, session, pretty),
        Commands::Import { file, json_file } => commands::cmd_import(&file, &json_file),
        Commands::Decay { file, threshold } => commands::cmd_decay(&file, threshold, json),
        Commands::Stats { file } => commands::cmd_stats(&file, json),
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
