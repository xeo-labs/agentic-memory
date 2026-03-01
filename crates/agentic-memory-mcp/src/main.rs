//! AgenticMemory MCP Server — entry point.

use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use agentic_memory_mcp::config::resolve_memory_path;
use agentic_memory_mcp::protocol::ProtocolHandler;
use agentic_memory_mcp::session::autosave::spawn_maintenance;
use agentic_memory_mcp::session::SessionManager;
use agentic_memory_mcp::tools::ToolRegistry;
use agentic_memory_mcp::transport::capture::{
    self, CaptureDirection, CaptureWalStatus, CapturedTransportEntry,
};
use agentic_memory_mcp::transport::StdioTransport;
use agentic_memory_mcp::types::MemoryMode;

mod daemon;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ClientIdentity {
    name: String,
    version: String,
    family: String,
}

#[derive(Debug, Clone, Serialize)]
struct Layer2Record {
    timestamp: String,
    timestamp_nanos: i64,
    sequence: u64,
    direction: String,
    wal_session_id: String,
    client_name: Option<String>,
    client_version: Option<String>,
    client_family: Option<String>,
    message_kind: String,
    method: Option<String>,
    request_id: Option<Value>,
    tool_name: Option<String>,
    prompt_name: Option<String>,
    uri: Option<String>,
    payload_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DaemonCheckpoint {
    last_sequence: Option<u64>,
    processed_records: u64,
    updated_at: String,
}

fn infer_client_family(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.contains("claude") {
        "claude".to_string()
    } else if lower.contains("cursor") {
        "cursor".to_string()
    } else if lower.contains("windsurf") || lower.contains("codeium") {
        "windsurf".to_string()
    } else if lower.contains("cody") || lower.contains("sourcegraph") {
        "cody".to_string()
    } else {
        "unknown".to_string()
    }
}

fn ts_nanos_to_string(nanos: i64) -> String {
    let secs = nanos.div_euclid(1_000_000_000);
    let sub = nanos.rem_euclid(1_000_000_000) as u32;
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, sub)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

fn parse_json(data: &[u8]) -> Option<Value> {
    serde_json::from_slice::<Value>(data).ok()
}

fn parse_initialize_client(value: &Value) -> Option<ClientIdentity> {
    let method = value.get("method").and_then(Value::as_str)?;
    if method != "initialize" {
        return None;
    }
    let info = value.get("params")?.get("clientInfo")?;
    let name = info.get("name").and_then(Value::as_str)?.to_string();
    let version = info
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    Some(ClientIdentity {
        family: infer_client_family(&name),
        name,
        version,
    })
}

fn extract_layer2_records(
    entries: &[CapturedTransportEntry],
    include_raw: bool,
) -> Vec<Layer2Record> {
    let mut current_client: Option<ClientIdentity> = None;
    let mut out = Vec::with_capacity(entries.len());

    for entry in entries {
        let parsed = parse_json(&entry.data);
        if let Some(v) = parsed.as_ref().and_then(parse_initialize_client) {
            current_client = Some(v);
        }

        let method = parsed
            .as_ref()
            .and_then(|v| v.get("method"))
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let request_id = parsed.as_ref().and_then(|v| v.get("id")).cloned();

        let mut message_kind = "unknown".to_string();
        let mut tool_name = None;
        let mut prompt_name = None;
        let mut uri = None;

        if let Some(v) = parsed.as_ref() {
            if let Some(m) = method.as_deref() {
                message_kind = "request".to_string();
                match m {
                    "initialize" => message_kind = "initialize".to_string(),
                    "tools/call" => {
                        message_kind = "tool_call".to_string();
                        tool_name = v
                            .get("params")
                            .and_then(|p| p.get("name"))
                            .and_then(Value::as_str)
                            .map(ToString::to_string);
                    }
                    "prompts/get" => {
                        message_kind = "prompt_get".to_string();
                        prompt_name = v
                            .get("params")
                            .and_then(|p| p.get("name"))
                            .and_then(Value::as_str)
                            .map(ToString::to_string);
                    }
                    "resources/read" => {
                        message_kind = "resource_read".to_string();
                        uri = v
                            .get("params")
                            .and_then(|p| p.get("uri"))
                            .and_then(Value::as_str)
                            .map(ToString::to_string);
                    }
                    "initialized" | "$/cancelRequest" | "notifications/cancelled" => {
                        message_kind = "notification".to_string();
                    }
                    _ => {}
                }
            } else if v.get("result").is_some() {
                message_kind = "response".to_string();
            } else if v.get("error").is_some() {
                message_kind = "error".to_string();
            }
        } else {
            message_kind = "invalid_json".to_string();
        }

        let client = current_client.clone();
        out.push(Layer2Record {
            timestamp: ts_nanos_to_string(entry.timestamp_nanos),
            timestamp_nanos: entry.timestamp_nanos,
            sequence: entry.sequence,
            direction: match entry.direction {
                CaptureDirection::Inbound => "inbound".to_string(),
                CaptureDirection::Outbound => "outbound".to_string(),
            },
            wal_session_id: entry.session_id_string(),
            client_name: client.as_ref().map(|c| c.name.clone()),
            client_version: client.as_ref().map(|c| c.version.clone()),
            client_family: client.as_ref().map(|c| c.family.clone()),
            message_kind,
            method,
            request_id,
            tool_name,
            prompt_name,
            uri,
            payload_bytes: entry.data.len(),
            raw: if include_raw { parsed } else { None },
        });
    }

    out
}

fn print_replay_line(record: &Layer2Record) {
    let actor = record
        .client_family
        .as_ref()
        .or(record.client_name.as_ref())
        .map(String::as_str)
        .unwrap_or("unknown");
    let target = record
        .tool_name
        .as_ref()
        .or(record.prompt_name.as_ref())
        .or(record.uri.as_ref())
        .or(record.method.as_ref())
        .map(String::as_str)
        .unwrap_or("-");
    println!(
        "[{}] seq={} {} {} {} {}",
        record.timestamp, record.sequence, record.direction, actor, record.message_kind, target
    );
}

fn append_jsonl(path: &Path, records: &[Layer2Record]) -> anyhow::Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut writer = std::io::BufWriter::new(file);
    for record in records {
        serde_json::to_writer(&mut writer, record)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn load_checkpoint(path: &Path) -> anyhow::Result<DaemonCheckpoint> {
    if !path.exists() {
        return Ok(DaemonCheckpoint::default());
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str::<DaemonCheckpoint>(&raw).unwrap_or_default())
}

fn save_checkpoint(path: &Path, state: &DaemonCheckpoint) -> anyhow::Result<()> {
    let tmp = path.with_extension("tmp");
    let payload = serde_json::to_vec_pretty(state)?;
    std::fs::write(&tmp, payload)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

#[derive(Debug, Clone, Args, Default)]
struct DaemonRunArgs {
    /// Path to transport.wal (defaults to AMEM transport wal path).
    #[arg(long)]
    wal: Option<PathBuf>,
    /// Output JSONL file path.
    #[arg(long)]
    out: Option<PathBuf>,
    /// State file path for resume checkpoints.
    #[arg(long)]
    state: Option<PathBuf>,
    /// Poll interval in seconds.
    #[arg(long, default_value = "2")]
    poll_secs: u64,
    /// Include raw JSON payload in extracted records.
    #[arg(long)]
    include_raw: bool,
}

#[derive(Debug, Clone, Subcommand)]
enum DaemonSubcommand {
    /// Start daemon process (background by default).
    Start {
        /// Run in foreground.
        #[arg(long)]
        foreground: bool,
        #[command(flatten)]
        args: DaemonRunArgs,
    },
    /// Stop daemon process.
    Stop,
    /// Show daemon status.
    Status,
    /// Restart daemon process.
    Restart {
        /// Run in foreground after restart.
        #[arg(long)]
        foreground: bool,
        #[command(flatten)]
        args: DaemonRunArgs,
    },
    /// Show daemon logs.
    Logs {
        /// Number of lines to show.
        #[arg(short = 'n', long, default_value = "50")]
        lines: usize,
        /// Follow log output.
        #[arg(short = 'f', long)]
        follow: bool,
    },
    /// Install daemon as system service.
    Install,
    /// Uninstall daemon system service.
    Uninstall,
    /// Internal command used by background spawn and services.
    Run {
        #[command(flatten)]
        args: DaemonRunArgs,
    },
}

fn resolve_daemon_paths(args: &DaemonRunArgs) -> (PathBuf, PathBuf, PathBuf, daemon::DaemonPaths) {
    let paths = daemon::DaemonPaths::default();
    let wal_path = args.wal.clone().unwrap_or_else(capture::default_wal_path);
    let out_path = args.out.clone().unwrap_or_else(|| {
        wal_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("layer2-extraction.jsonl")
    });
    let state_path = args
        .state
        .clone()
        .unwrap_or_else(|| out_path.with_extension("state.json"));
    (wal_path, out_path, state_path, paths)
}

fn daemon_state_label(state: daemon::DaemonRunState) -> &'static str {
    match state {
        daemon::DaemonRunState::Starting => "starting",
        daemon::DaemonRunState::Running => "running",
        daemon::DaemonRunState::Extracting => "extracting",
        daemon::DaemonRunState::Idle => "idle",
        daemon::DaemonRunState::Stopping => "stopping",
        daemon::DaemonRunState::Error => "error",
    }
}

fn append_daemon_log(path: &Path, message: impl AsRef<str>) {
    let line = format!(
        "{} {}",
        chrono::Utc::now().to_rfc3339(),
        message.as_ref().trim_end()
    );
    let _ = daemon::append_log_line(path, &line);
}

fn stop_daemon(paths: &daemon::DaemonPaths) -> anyhow::Result<bool> {
    let pid = daemon::read_pid_file(&paths.pid_file).or_else(|| {
        daemon::DaemonStatus::load(&paths.status_file)
            .ok()
            .flatten()
            .map(|s| s.pid)
    });
    let Some(pid) = pid else {
        return Ok(false);
    };

    if let Ok(Some(mut status)) = daemon::DaemonStatus::load(&paths.status_file) {
        status.state = daemon::DaemonRunState::Stopping;
        status.touch();
        let _ = status.save(&paths.status_file);
    }

    if !daemon::process_alive(pid) {
        let _ = daemon::remove_pid_file(&paths.pid_file);
        return Ok(false);
    }

    let stopped = daemon::stop_process(pid)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if !daemon::process_alive(pid) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let _ = daemon::remove_pid_file(&paths.pid_file);
    Ok(stopped)
}

fn show_daemon_status(paths: &daemon::DaemonPaths) -> anyhow::Result<()> {
    let status = daemon::DaemonStatus::load(&paths.status_file)?;
    println!("Daemon Status");
    println!("═════════════");
    match status {
        Some(status) => {
            let pid_alive = daemon::process_alive(status.pid);
            let activity_alive = status.is_alive();
            let running = pid_alive && activity_alive;
            let state = if running {
                daemon_state_label(status.state)
            } else {
                "dead"
            };
            println!("State:             {state}");
            println!("PID:               {}", status.pid);
            println!("Uptime:            {}s", status.uptime_secs());
            println!("WAL files:         {}", status.wal_files_count);
            println!("Entries extracted: {}", status.entries_extracted);
            println!("Memories written:  {}", status.memories_written);
            if let Some(last) = status.last_sequence {
                println!("Last sequence:     {last}");
            }
            if let Some(err) = status.last_error {
                println!("Last error:        {err}");
            }
            if !running {
                println!("Activity:          stale");
            }
        }
        None => {
            println!("State:             not running");
        }
    }

    println!();
    println!("Service Installation");
    println!("════════════════════");
    println!(
        "installed: {}",
        if daemon::is_service_installed() {
            "yes"
        } else {
            "no"
        }
    );
    println!("status file: {}", paths.status_file.display());
    println!("pid file:    {}", paths.pid_file.display());
    println!("log file:    {}", paths.log_file.display());
    Ok(())
}

fn show_daemon_logs(paths: &daemon::DaemonPaths, lines: usize, follow: bool) -> anyhow::Result<()> {
    let tail = daemon::read_last_lines(&paths.log_file, lines)?;
    for line in tail {
        println!("{line}");
    }
    if !follow {
        return Ok(());
    }

    let mut cursor = std::fs::metadata(&paths.log_file)
        .map(|m| m.len())
        .unwrap_or(0);
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let len = std::fs::metadata(&paths.log_file)
            .map(|m| m.len())
            .unwrap_or(0);
        if len < cursor {
            cursor = 0;
        }
        if len == cursor {
            continue;
        }
        let mut file = match std::fs::File::open(&paths.log_file) {
            Ok(file) => file,
            Err(_) => continue,
        };
        file.seek(SeekFrom::Start(cursor))?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        if !buf.is_empty() {
            print!("{buf}");
            std::io::stdout().flush()?;
        }
        cursor = len;
    }
}

fn push_daemon_run_args(cmd: &mut std::process::Command, args: &DaemonRunArgs) {
    if let Some(wal) = &args.wal {
        cmd.arg("--wal").arg(wal);
    }
    if let Some(out) = &args.out {
        cmd.arg("--out").arg(out);
    }
    if let Some(state) = &args.state {
        cmd.arg("--state").arg(state);
    }
    if args.poll_secs != 2 {
        cmd.arg("--poll-secs").arg(args.poll_secs.to_string());
    }
    if args.include_raw {
        cmd.arg("--include-raw");
    }
}

fn start_daemon_background(args: &DaemonRunArgs) -> anyhow::Result<()> {
    let (_, _, _, paths) = resolve_daemon_paths(args);
    paths.ensure_dirs()?;
    if let Some(pid) = daemon::read_pid_file(&paths.pid_file) {
        if daemon::process_alive(pid) {
            println!("Daemon already running (pid={pid})");
            return Ok(());
        }
    }

    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("daemon").arg("run");
    push_daemon_run_args(&mut cmd, args);
    cmd.env("AMEM_DAEMON_MODE", "1")
        .stdin(std::process::Stdio::null());

    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)?;
    let stderr = stdout.try_clone()?;
    cmd.stdout(stdout).stderr(stderr);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let child = cmd.spawn()?;
    println!("Daemon started (pid={})", child.id());
    Ok(())
}

async fn run_daemon_loop(args: DaemonRunArgs) -> anyhow::Result<()> {
    let (wal_path, out_path, state_path, paths) = resolve_daemon_paths(&args);
    paths.ensure_dirs()?;

    if let Some(pid) = daemon::read_pid_file(&paths.pid_file) {
        if pid != std::process::id() && daemon::process_alive(pid) {
            anyhow::bail!("daemon already running (pid={pid})");
        }
    }

    let mut checkpoint = load_checkpoint(&state_path)?;
    let mut status = daemon::DaemonStatus::new();
    status.state = daemon::DaemonRunState::Running;
    status.last_sequence = checkpoint.last_sequence;
    status.wal_files_count = usize::from(wal_path.exists());
    status.save(&paths.status_file)?;
    daemon::write_pid_file(&paths.pid_file, std::process::id())?;

    append_daemon_log(
        &paths.log_file,
        format!(
            "daemon started (wal={}, out={})",
            wal_path.display(),
            out_path.display()
        ),
    );

    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    #[cfg(unix)]
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;

    let mut interval = tokio::time::interval(Duration::from_secs(args.poll_secs.max(1)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let process_tick = |status: &mut daemon::DaemonStatus,
                        checkpoint: &mut DaemonCheckpoint|
     -> anyhow::Result<()> {
        status.state = daemon::DaemonRunState::Extracting;
        status.wal_files_count = usize::from(wal_path.exists());
        status.touch();

        let entries = capture::read_entries(&wal_path, None)?;
        let records = extract_layer2_records(&entries, args.include_raw);
        let new_records: Vec<Layer2Record> = records
            .into_iter()
            .filter(|r| {
                checkpoint
                    .last_sequence
                    .map(|seq| r.sequence > seq)
                    .unwrap_or(true)
            })
            .collect();

        if !new_records.is_empty() {
            append_jsonl(&out_path, &new_records)?;
            checkpoint.last_sequence = new_records.last().map(|r| r.sequence);
            checkpoint.processed_records = checkpoint
                .processed_records
                .saturating_add(new_records.len() as u64);
            checkpoint.updated_at = chrono::Utc::now().to_rfc3339();
            save_checkpoint(&state_path, checkpoint)?;

            status.record_extraction(new_records.len() as u64, new_records.len() as u64);
            status.last_sequence = checkpoint.last_sequence;
            status.clear_error();
            append_daemon_log(
                &paths.log_file,
                format!(
                    "extracted {} records (last_seq={})",
                    new_records.len(),
                    status.last_sequence.unwrap_or(0)
                ),
            );
        } else {
            status.state = daemon::DaemonRunState::Idle;
            status.touch();
        }
        status.save(&paths.status_file)?;
        Ok(())
    };

    process_tick(&mut status, &mut checkpoint)?;
    loop {
        #[cfg(unix)]
        {
            tokio::select! {
                _ = sigterm.recv() => {
                    append_daemon_log(&paths.log_file, "received SIGTERM");
                    break;
                }
                _ = sigint.recv() => {
                    append_daemon_log(&paths.log_file, "received SIGINT");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(err) = process_tick(&mut status, &mut checkpoint) {
                        status.set_error(err.to_string());
                        let _ = status.save(&paths.status_file);
                        append_daemon_log(&paths.log_file, format!("error: {err}"));
                    }
                }
            }
        }
        #[cfg(not(unix))]
        {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    append_daemon_log(&paths.log_file, "received ctrl-c");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(err) = process_tick(&mut status, &mut checkpoint) {
                        status.set_error(err.to_string());
                        let _ = status.save(&paths.status_file);
                        append_daemon_log(&paths.log_file, format!("error: {err}"));
                    }
                }
            }
        }
    }

    status.state = daemon::DaemonRunState::Stopping;
    status.touch();
    let _ = status.save(&paths.status_file);
    let _ = daemon::remove_pid_file(&paths.pid_file);
    append_daemon_log(&paths.log_file, "daemon stopped");
    Ok(())
}

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

    /// Show transport WAL extraction status and detected client mix.
    Status {
        /// Path to transport.wal (defaults to AMEM transport wal path).
        #[arg(long)]
        wal: Option<PathBuf>,
    },

    /// Layer 2 extraction from transport WAL into structured records.
    Extract {
        /// Path to transport.wal (defaults to AMEM transport wal path).
        #[arg(long)]
        wal: Option<PathBuf>,
        /// Optional file path to write JSONL output.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Include raw JSON payload in extracted records.
        #[arg(long)]
        include_raw: bool,
        /// Limit decoded WAL entries to the most recent N.
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Replay extracted transport events in time order.
    Replay {
        /// Path to transport.wal (defaults to AMEM transport wal path).
        #[arg(long)]
        wal: Option<PathBuf>,
        /// Keep watching the WAL for new events.
        #[arg(long)]
        follow: bool,
        /// Poll interval (seconds) when --follow is enabled.
        #[arg(long, default_value = "2")]
        poll_secs: u64,
        /// Limit decoded WAL entries to the most recent N.
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Long-running extraction daemon and service controls.
    Daemon {
        #[command(subcommand)]
        command: Option<DaemonSubcommand>,
        #[command(flatten)]
        args: DaemonRunArgs,
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

            // V3 Ghost Writer: background sync to Claude, Cursor, Windsurf, Cody
            #[cfg(feature = "v3")]
            let _ghost_writer_task = {
                tracing::info!("V3 feature enabled — starting Ghost Writer");
                agentic_memory_mcp::ghost_bridge::spawn_ghost_writer(session.clone())
            };

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

        Commands::Status { wal } => {
            let wal_path = wal.unwrap_or_else(capture::default_wal_path);
            let status: CaptureWalStatus = capture::wal_status(&wal_path)?;
            let extracted = capture::read_entries(&wal_path, Some(20_000))?;
            let records = extract_layer2_records(&extracted, false);
            let mut clients = BTreeSet::new();
            for rec in &records {
                if let Some(name) = rec.client_name.as_ref() {
                    clients.insert(name.clone());
                } else if let Some(family) = rec.client_family.as_ref() {
                    clients.insert(family.clone());
                }
            }

            let tool_count = ToolRegistry::list_tools().len();
            let latest = records.last().map(|r| r.timestamp.clone());
            let session = status
                .session_id
                .map(|id| uuid::Uuid::from_bytes(id).to_string());
            let payload = serde_json::json!({
                "wal_path": status.wal_path,
                "exists": status.exists,
                "bytes": status.bytes,
                "entries": status.entries,
                "next_sequence": status.next_sequence,
                "wal_session_id": session,
                "detected_clients": clients.into_iter().collect::<Vec<_>>(),
                "latest_event_at": latest,
                "mcp_tool_count": tool_count
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }

        Commands::Extract {
            wal,
            out,
            include_raw,
            limit,
        } => {
            let wal_path = wal.unwrap_or_else(capture::default_wal_path);
            let entries = capture::read_entries(&wal_path, limit)?;
            let records = extract_layer2_records(&entries, include_raw);

            if let Some(out_path) = out {
                append_jsonl(&out_path, &records)?;
                let summary = serde_json::json!({
                    "status": "ok",
                    "wal_path": wal_path,
                    "out": out_path,
                    "records_written": records.len(),
                    "mcp_tool_count_unchanged": ToolRegistry::list_tools().len()
                });
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!("{}", serde_json::to_string_pretty(&records)?);
            }
        }

        Commands::Replay {
            wal,
            follow,
            poll_secs,
            limit,
        } => {
            let wal_path = wal.unwrap_or_else(capture::default_wal_path);
            let mut last_seq: Option<u64> = None;
            loop {
                let entries = capture::read_entries(&wal_path, limit)?;
                let records = extract_layer2_records(&entries, false);
                for record in records {
                    if last_seq.map(|s| record.sequence <= s).unwrap_or(false) {
                        continue;
                    }
                    print_replay_line(&record);
                    last_seq = Some(record.sequence);
                }
                if !follow {
                    break;
                }
                std::thread::sleep(Duration::from_secs(poll_secs.max(1)));
            }
        }

        Commands::Daemon { command, args } => {
            let subcommand = command.unwrap_or(DaemonSubcommand::Run { args });
            match subcommand {
                DaemonSubcommand::Start { foreground, args } => {
                    if foreground {
                        run_daemon_loop(args).await?;
                    } else {
                        start_daemon_background(&args)?;
                    }
                }
                DaemonSubcommand::Stop => {
                    let paths = daemon::DaemonPaths::default();
                    if stop_daemon(&paths)? {
                        println!("Daemon stopped");
                    } else {
                        println!("Daemon is not running");
                    }
                }
                DaemonSubcommand::Status => {
                    let paths = daemon::DaemonPaths::default();
                    show_daemon_status(&paths)?;
                }
                DaemonSubcommand::Restart { foreground, args } => {
                    let paths = daemon::DaemonPaths::default();
                    let _ = stop_daemon(&paths)?;
                    std::thread::sleep(Duration::from_secs(1));
                    if foreground {
                        run_daemon_loop(args).await?;
                    } else {
                        start_daemon_background(&args)?;
                    }
                }
                DaemonSubcommand::Logs { lines, follow } => {
                    let paths = daemon::DaemonPaths::default();
                    show_daemon_logs(&paths, lines, follow)?;
                }
                DaemonSubcommand::Install => {
                    let paths = daemon::DaemonPaths::default();
                    paths.ensure_dirs()?;
                    let binary = std::env::current_exe()?;
                    daemon::install_service(&binary, &paths.log_file)?;
                    println!("Daemon service installed");
                }
                DaemonSubcommand::Uninstall => {
                    daemon::uninstall_service()?;
                    println!("Daemon service uninstalled");
                }
                DaemonSubcommand::Run { args } => {
                    run_daemon_loop(args).await?;
                }
            }
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
