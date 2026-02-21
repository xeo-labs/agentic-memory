//! Slash command dispatch for the amem REPL.

use std::path::PathBuf;

use crate::cli::commands;
use crate::cli::repl_complete::COMMANDS;
use crate::engine::PatternSort;
use crate::graph::TraversalDirection;
use crate::types::EventType;

/// Session state.
pub struct ReplState {
    /// Path to the currently loaded .amem file.
    pub file_path: Option<PathBuf>,
}

impl Default for ReplState {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplState {
    pub fn new() -> Self {
        Self { file_path: None }
    }

    fn require_file(&self) -> Option<&PathBuf> {
        if let Some(ref p) = self.file_path {
            Some(p)
        } else {
            eprintln!("  No .amem file loaded. Use /load <file.amem> or /create <file.amem>");
            None
        }
    }
}

/// Execute a slash command. Returns `true` if REPL should exit.
pub fn execute(input: &str, state: &mut ReplState) -> Result<bool, Box<dyn std::error::Error>> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(false);
    }

    let input = input.strip_prefix('/').unwrap_or(input);
    if input.is_empty() {
        cmd_help();
        return Ok(false);
    }

    let mut parts = input.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("");
    let args = parts.next().unwrap_or("").trim();

    match cmd {
        "exit" | "quit" => return Ok(true),
        "help" | "h" | "?" => cmd_help(),
        "clear" | "cls" => eprint!("\x1b[2J\x1b[H"),
        "create" => cmd_create(args, state)?,
        "load" => cmd_load(args, state)?,
        "info" => cmd_info(state)?,
        "add" => cmd_add(args, state)?,
        "get" => cmd_get(args, state)?,
        "search" => cmd_search(args, state)?,
        "text-search" | "ts" => cmd_text_search(args, state)?,
        "traverse" => cmd_traverse(args, state)?,
        "impact" => cmd_impact(args, state)?,
        "centrality" => cmd_centrality(args, state)?,
        "path" => cmd_path(args, state)?,
        "gaps" => cmd_gaps(args, state)?,
        "stats" => cmd_stats(state)?,
        "sessions" => cmd_sessions(state)?,
        _ => {
            if let Some(suggestion) = crate::cli::repl_complete::suggest_command(cmd) {
                eprintln!("  Unknown command '/{cmd}'. Did you mean {suggestion}?");
            } else {
                eprintln!("  Unknown command '/{cmd}'. Type /help for commands.");
            }
        }
    }

    Ok(false)
}

fn cmd_help() {
    eprintln!();
    eprintln!("  Commands:");
    eprintln!();
    for (cmd, desc) in COMMANDS {
        eprintln!("    {cmd:<20} {desc}");
    }
    eprintln!();
    eprintln!("  Tip: Tab completion works for commands, event types, and .amem files.");
    eprintln!();
}

fn cmd_create(args: &str, state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("  Usage: /create <file.amem> [--dimension N]");
        return Ok(());
    }
    let tokens: Vec<&str> = args.split_whitespace().collect();
    let file = PathBuf::from(tokens[0]);
    let dim: usize = tokens
        .iter()
        .position(|&t| t == "--dimension")
        .and_then(|i| tokens.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(128);

    commands::cmd_create(&file, dim)?;
    state.file_path = Some(file.clone());
    eprintln!("  Created and loaded: {}", file.display());
    Ok(())
}

fn cmd_load(args: &str, state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("  Usage: /load <file.amem>");
        return Ok(());
    }
    let file = PathBuf::from(args.split_whitespace().next().unwrap_or(args));
    if !file.exists() {
        eprintln!("  File not found: {}", file.display());
        return Ok(());
    }
    // Verify it's readable
    commands::cmd_info(&file, false)?;
    state.file_path = Some(file);
    Ok(())
}

fn cmd_info(state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    let file = match state.require_file() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    commands::cmd_info(&file, false)?;
    Ok(())
}

fn cmd_add(args: &str, state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    let file = match state.require_file() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    let tokens: Vec<&str> = args.splitn(2, ' ').collect();
    if tokens.len() < 2 {
        eprintln!("  Usage: /add <type> <content>");
        eprintln!("  Types: fact, decision, inference, correction, skill, episode");
        return Ok(());
    }
    let et = match EventType::from_name(tokens[0]) {
        Some(et) => et,
        None => {
            eprintln!("  Invalid event type: {}", tokens[0]);
            return Ok(());
        }
    };
    commands::cmd_add(&file, et, tokens[1], 0, 1.0, None, false)?;
    Ok(())
}

fn cmd_get(args: &str, state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    let file = match state.require_file() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    let node_id: u64 = match args.split_whitespace().next().and_then(|s| s.parse().ok()) {
        Some(id) => id,
        None => {
            eprintln!("  Usage: /get <node-id>");
            return Ok(());
        }
    };
    commands::cmd_get(&file, node_id, false)?;
    Ok(())
}

fn cmd_search(args: &str, state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    let file = match state.require_file() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    // Parse simple flags
    let tokens: Vec<&str> = args.split_whitespace().collect();
    let mut event_types = Vec::new();
    let mut limit: usize = 20;
    let mut sort = PatternSort::MostRecent;
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "--type" if i + 1 < tokens.len() => {
                for t in tokens[i + 1].split(',') {
                    if let Some(et) = EventType::from_name(t.trim()) {
                        event_types.push(et);
                    }
                }
                i += 2;
            }
            "--limit" if i + 1 < tokens.len() => {
                limit = tokens[i + 1].parse().unwrap_or(20);
                i += 2;
            }
            "--sort" if i + 1 < tokens.len() => {
                sort = match tokens[i + 1] {
                    "confidence" => PatternSort::HighestConfidence,
                    "accessed" => PatternSort::MostAccessed,
                    "importance" => PatternSort::MostImportant,
                    _ => PatternSort::MostRecent,
                };
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }
    commands::cmd_search(
        &file,
        event_types,
        vec![],
        None,
        None,
        None,
        None,
        sort,
        limit,
        false,
    )?;
    Ok(())
}

fn cmd_text_search(args: &str, state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    let file = match state.require_file() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    if args.is_empty() {
        eprintln!("  Usage: /text-search <query>");
        return Ok(());
    }
    let query = args.to_string();
    commands::cmd_text_search(&file, &query, vec![], vec![], 20, 0.0, false)?;
    Ok(())
}

fn cmd_traverse(args: &str, state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    let file = match state.require_file() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    let start_id: u64 = match args.split_whitespace().next().and_then(|s| s.parse().ok()) {
        Some(id) => id,
        None => {
            eprintln!("  Usage: /traverse <start-node-id> [--depth N]");
            return Ok(());
        }
    };
    let tokens: Vec<&str> = args.split_whitespace().collect();
    let depth: u32 = tokens
        .iter()
        .position(|&t| t == "--depth")
        .and_then(|i| tokens.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    commands::cmd_traverse(
        &file,
        start_id,
        vec![],
        TraversalDirection::Both,
        depth,
        50,
        0.0,
        false,
    )?;
    Ok(())
}

fn cmd_impact(args: &str, state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    let file = match state.require_file() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    let node_id: u64 = match args.split_whitespace().next().and_then(|s| s.parse().ok()) {
        Some(id) => id,
        None => {
            eprintln!("  Usage: /impact <node-id>");
            return Ok(());
        }
    };
    commands::cmd_impact(&file, node_id, 10, false)?;
    Ok(())
}

fn cmd_centrality(args: &str, state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    let file = match state.require_file() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    let algo = args.split_whitespace().next().unwrap_or("pagerank");
    commands::cmd_centrality(&file, algo, 0.85, vec![], vec![], 20, 100, false)?;
    Ok(())
}

fn cmd_path(args: &str, state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    let file = match state.require_file() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    let tokens: Vec<&str> = args.split_whitespace().collect();
    if tokens.len() < 2 {
        eprintln!("  Usage: /path <source-id> <target-id>");
        return Ok(());
    }
    let source: u64 = match tokens[0].parse() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("  Invalid source ID");
            return Ok(());
        }
    };
    let target: u64 = match tokens[1].parse() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("  Invalid target ID");
            return Ok(());
        }
    };
    commands::cmd_path(
        &file,
        source,
        target,
        vec![],
        TraversalDirection::Both,
        20,
        false,
        false,
    )?;
    Ok(())
}

fn cmd_gaps(args: &str, state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    let file = match state.require_file() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    let limit: usize = args
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    commands::cmd_gaps(&file, 0.5, 1, limit, "dangerous", None, false)?;
    Ok(())
}

fn cmd_stats(state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    let file = match state.require_file() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    commands::cmd_stats(&file, false)?;
    Ok(())
}

fn cmd_sessions(state: &mut ReplState) -> Result<(), Box<dyn std::error::Error>> {
    let file = match state.require_file() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    commands::cmd_sessions(&file, 20, false)?;
    Ok(())
}
