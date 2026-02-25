//! Graph lifecycle management, file I/O, and session tracking.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use agentic_memory::{
    AmemReader, AmemWriter, CognitiveEventBuilder, Edge, EdgeType, EventType, MemoryGraph,
    QueryEngine, WriteEngine,
};
use serde_json::Value;

use crate::types::{McpError, McpResult};

/// Default auto-save interval.
const DEFAULT_AUTO_SAVE_SECS: u64 = 30;
/// Default backup interval.
const DEFAULT_BACKUP_INTERVAL_SECS: u64 = 900;
/// Default number of backups to retain per brain file.
const DEFAULT_BACKUP_RETENTION: usize = 24;
/// Default maintenance sleep-cycle interval.
const DEFAULT_SLEEP_CYCLE_SECS: u64 = 1800;
/// Minimum completed-session size before auto-archive.
const DEFAULT_ARCHIVE_MIN_SESSION_NODES: usize = 25;
/// Default hot-tier threshold (decay score).
const DEFAULT_HOT_MIN_DECAY: f32 = 0.7;
/// Default warm-tier threshold (decay score).
const DEFAULT_WARM_MIN_DECAY: f32 = 0.3;
/// Default sustained mutation rate threshold before throttling heavy maintenance.
const DEFAULT_SLA_MAX_MUTATIONS_PER_MIN: u32 = 240;
/// Default interval for writing health-ledger snapshots.
const DEFAULT_HEALTH_LEDGER_EMIT_SECS: u64 = 30;
/// Default long-horizon storage budget target (2 GiB over 20 years).
const DEFAULT_STORAGE_BUDGET_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Default storage budget projection horizon.
const DEFAULT_STORAGE_BUDGET_HORIZON_YEARS: u32 = 20;
/// Default maximum chars persisted for one auto-captured prompt/feedback item.
const DEFAULT_AUTO_CAPTURE_MAX_CHARS: usize = 2048;
/// Current `.amem` storage version used by this server.
const CURRENT_AMEM_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy)]
enum AutonomicProfile {
    Desktop,
    Cloud,
    Aggressive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StorageMigrationPolicy {
    AutoSafe,
    Strict,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StorageBudgetMode {
    AutoRollup,
    Warn,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoCaptureMode {
    /// Capture prompt-focused events and feedback context.
    Safe,
    /// Capture broader tool input text (except explicit memory_add payload duplication).
    Full,
    /// Disable automatic capture.
    Off,
}

#[derive(Debug, Clone, Copy)]
struct ProfileDefaults {
    auto_save_secs: u64,
    backup_secs: u64,
    backup_retention: usize,
    sleep_cycle_secs: u64,
    sleep_idle_secs: u64,
    archive_min_session_nodes: usize,
    hot_min_decay: f32,
    warm_min_decay: f32,
    sla_max_mutations_per_min: u32,
}

impl AutonomicProfile {
    fn from_env(name: &str) -> Self {
        let raw = read_env_string(name).unwrap_or_else(|| "desktop".to_string());
        match raw.trim().to_ascii_lowercase().as_str() {
            "cloud" => Self::Cloud,
            "aggressive" => Self::Aggressive,
            _ => Self::Desktop,
        }
    }

    fn defaults(self) -> ProfileDefaults {
        match self {
            Self::Desktop => ProfileDefaults {
                auto_save_secs: DEFAULT_AUTO_SAVE_SECS,
                backup_secs: DEFAULT_BACKUP_INTERVAL_SECS,
                backup_retention: DEFAULT_BACKUP_RETENTION,
                sleep_cycle_secs: DEFAULT_SLEEP_CYCLE_SECS,
                sleep_idle_secs: 180,
                archive_min_session_nodes: DEFAULT_ARCHIVE_MIN_SESSION_NODES,
                hot_min_decay: DEFAULT_HOT_MIN_DECAY,
                warm_min_decay: DEFAULT_WARM_MIN_DECAY,
                sla_max_mutations_per_min: DEFAULT_SLA_MAX_MUTATIONS_PER_MIN,
            },
            Self::Cloud => ProfileDefaults {
                auto_save_secs: 15,
                backup_secs: 600,
                backup_retention: 48,
                sleep_cycle_secs: 900,
                sleep_idle_secs: 90,
                archive_min_session_nodes: 50,
                hot_min_decay: 0.75,
                warm_min_decay: 0.4,
                sla_max_mutations_per_min: 600,
            },
            Self::Aggressive => ProfileDefaults {
                auto_save_secs: 10,
                backup_secs: 300,
                backup_retention: 16,
                sleep_cycle_secs: 300,
                sleep_idle_secs: 45,
                archive_min_session_nodes: 15,
                hot_min_decay: 0.8,
                warm_min_decay: 0.5,
                sla_max_mutations_per_min: 900,
            },
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Cloud => "cloud",
            Self::Aggressive => "aggressive",
        }
    }
}

impl StorageMigrationPolicy {
    fn from_env(name: &str) -> Self {
        let raw = read_env_string(name).unwrap_or_else(|| "auto-safe".to_string());
        match raw.trim().to_ascii_lowercase().as_str() {
            "strict" => Self::Strict,
            "off" | "disabled" | "none" => Self::Off,
            _ => Self::AutoSafe,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::AutoSafe => "auto-safe",
            Self::Strict => "strict",
            Self::Off => "off",
        }
    }
}

impl StorageBudgetMode {
    fn from_env(name: &str) -> Self {
        let raw = read_env_string(name).unwrap_or_else(|| "auto-rollup".to_string());
        match raw.trim().to_ascii_lowercase().as_str() {
            "warn" => Self::Warn,
            "off" | "disabled" | "none" => Self::Off,
            _ => Self::AutoRollup,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::AutoRollup => "auto-rollup",
            Self::Warn => "warn",
            Self::Off => "off",
        }
    }
}

impl AutoCaptureMode {
    fn from_env(name: &str) -> Self {
        let raw = read_env_string(name).unwrap_or_else(|| "safe".to_string());
        match raw.trim().to_ascii_lowercase().as_str() {
            "full" => Self::Full,
            "off" | "disabled" | "none" => Self::Off,
            _ => Self::Safe,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Full => "full",
            Self::Off => "off",
        }
    }
}

/// Manages the memory graph lifecycle, file I/O, and session state.
pub struct SessionManager {
    graph: MemoryGraph,
    query_engine: QueryEngine,
    write_engine: WriteEngine,
    file_path: PathBuf,
    current_session: u32,
    profile: AutonomicProfile,
    migration_policy: StorageMigrationPolicy,
    dirty: bool,
    last_save: Instant,
    auto_save_interval: Duration,
    backup_interval: Duration,
    backup_retention: usize,
    backups_dir: PathBuf,
    save_generation: u64,
    last_backup_generation: u64,
    last_backup: Instant,
    sleep_cycle_interval: Duration,
    archive_min_session_nodes: usize,
    hot_min_decay: f32,
    warm_min_decay: f32,
    sla_max_mutations_per_min: u32,
    last_sleep_cycle: Instant,
    sleep_idle_min: Duration,
    last_activity: Instant,
    mutation_window_started: Instant,
    mutation_window_count: u32,
    maintenance_throttle_count: u64,
    last_health_ledger_emit: Instant,
    health_ledger_emit_interval: Duration,
    storage_budget_mode: StorageBudgetMode,
    storage_budget_max_bytes: u64,
    storage_budget_horizon_years: u32,
    storage_budget_target_fraction: f32,
    storage_budget_rollup_count: u64,
    auto_capture_mode: AutoCaptureMode,
    auto_capture_redact: bool,
    auto_capture_max_chars: usize,
    auto_capture_count: u64,
    /// ID of the last node added to the temporal chain in this session.
    /// Used to create TemporalNext edges between consecutive captures.
    last_temporal_node_id: Option<u64>,
    /// Last known file modification time (for detecting external writes).
    last_file_mtime: Option<SystemTime>,
    /// Multi-context workspace manager for cross-memory queries.
    workspace_manager: super::workspace::WorkspaceManager,
}

impl SessionManager {
    /// Open or create a memory file at the given path.
    pub fn open(path: &str) -> McpResult<Self> {
        let file_path = PathBuf::from(path);
        let dimension = agentic_memory::DEFAULT_DIMENSION;
        let file_existed = file_path.exists();
        let profile = AutonomicProfile::from_env("AMEM_AUTONOMIC_PROFILE");
        let defaults = profile.defaults();
        let migration_policy = StorageMigrationPolicy::from_env("AMEM_STORAGE_MIGRATION_POLICY");
        let detected_version = if file_existed {
            read_storage_version(&file_path)
        } else {
            None
        };
        let legacy_version = detected_version.filter(|v| *v < CURRENT_AMEM_VERSION);

        let graph = if file_existed {
            tracing::info!("Opening existing memory file: {}", file_path.display());
            AmemReader::read_from_file(&file_path)
                .map_err(|e| McpError::AgenticMemory(format!("Failed to read memory file: {e}")))?
        } else {
            tracing::info!("Creating new memory file: {}", file_path.display());
            // Ensure parent directory exists
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    McpError::Io(std::io::Error::other(format!(
                        "Failed to create directory {}: {e}",
                        parent.display()
                    )))
                })?;
            }
            MemoryGraph::new(dimension)
        };

        // Determine the next session ID from existing sessions.
        // Incorporate PID to avoid collisions when multiple MCP instances share
        // the same .amem file (e.g. two Claude Code windows on different projects).
        let session_ids = graph.session_index().session_ids();
        let max_existing = session_ids.iter().copied().max().unwrap_or(0);
        let pid_component = std::process::id() % 1000;
        let current_session = max_existing.saturating_add(1).saturating_add(pid_component);

        tracing::info!(
            "Session {} started. Graph has {} nodes, {} edges.",
            current_session,
            graph.node_count(),
            graph.edge_count()
        );
        tracing::info!(
            "Autonomic profile={} migration_policy={}",
            profile.as_str(),
            migration_policy.as_str()
        );

        let auto_save_secs = read_env_u64("AMEM_AUTOSAVE_SECS", defaults.auto_save_secs);
        let backup_secs = read_env_u64("AMEM_AUTO_BACKUP_SECS", defaults.backup_secs).max(30);
        let backup_retention =
            read_env_usize("AMEM_AUTO_BACKUP_RETENTION", defaults.backup_retention).max(1);
        let backups_dir = resolve_backups_dir(&file_path);
        let sleep_cycle_secs =
            read_env_u64("AMEM_SLEEP_CYCLE_SECS", defaults.sleep_cycle_secs).max(60);
        let sleep_idle_secs =
            read_env_u64("AMEM_SLEEP_IDLE_SECS", defaults.sleep_idle_secs).max(30);
        let archive_min_session_nodes = read_env_usize(
            "AMEM_ARCHIVE_MIN_SESSION_NODES",
            defaults.archive_min_session_nodes,
        )
        .max(1);
        let hot_min_decay =
            read_env_f32("AMEM_TIER_HOT_MIN_DECAY", defaults.hot_min_decay).clamp(0.0, 1.0);
        let warm_min_decay = read_env_f32("AMEM_TIER_WARM_MIN_DECAY", defaults.warm_min_decay)
            .clamp(0.0, 1.0)
            .min(hot_min_decay);
        let sla_max_mutations_per_min = read_env_u32(
            "AMEM_SLA_MAX_MUTATIONS_PER_MIN",
            defaults.sla_max_mutations_per_min,
        )
        .max(1);
        let health_ledger_emit_interval = Duration::from_secs(
            read_env_u64(
                "AMEM_HEALTH_LEDGER_EMIT_SECS",
                DEFAULT_HEALTH_LEDGER_EMIT_SECS,
            )
            .max(5),
        );
        let storage_budget_mode = StorageBudgetMode::from_env("AMEM_STORAGE_BUDGET_MODE");
        let storage_budget_max_bytes =
            read_env_u64("AMEM_STORAGE_BUDGET_BYTES", DEFAULT_STORAGE_BUDGET_BYTES).max(1);
        let storage_budget_horizon_years = read_env_u32(
            "AMEM_STORAGE_BUDGET_HORIZON_YEARS",
            DEFAULT_STORAGE_BUDGET_HORIZON_YEARS,
        )
        .max(1);
        let storage_budget_target_fraction =
            read_env_f32("AMEM_STORAGE_BUDGET_TARGET_FRACTION", 0.85).clamp(0.50, 0.99);
        let auto_capture_mode = AutoCaptureMode::from_env("AMEM_AUTO_CAPTURE_MODE");
        let auto_capture_redact = read_env_bool("AMEM_AUTO_CAPTURE_REDACT", true);
        let auto_capture_max_chars = read_env_usize(
            "AMEM_AUTO_CAPTURE_MAX_CHARS",
            DEFAULT_AUTO_CAPTURE_MAX_CHARS,
        )
        .clamp(256, 16384);

        let mut manager = Self {
            graph,
            query_engine: QueryEngine::new(),
            write_engine: WriteEngine::new(dimension),
            file_path,
            current_session,
            profile,
            migration_policy,
            dirty: false,
            last_save: Instant::now(),
            auto_save_interval: Duration::from_secs(auto_save_secs),
            backup_interval: Duration::from_secs(backup_secs),
            backup_retention,
            backups_dir,
            save_generation: if file_existed { 1 } else { 0 },
            last_backup_generation: 0,
            last_backup: Instant::now(),
            sleep_cycle_interval: Duration::from_secs(sleep_cycle_secs),
            archive_min_session_nodes,
            hot_min_decay,
            warm_min_decay,
            sla_max_mutations_per_min,
            last_sleep_cycle: Instant::now(),
            sleep_idle_min: Duration::from_secs(sleep_idle_secs),
            last_activity: Instant::now(),
            mutation_window_started: Instant::now(),
            mutation_window_count: 0,
            maintenance_throttle_count: 0,
            last_health_ledger_emit: Instant::now()
                .checked_sub(health_ledger_emit_interval)
                .unwrap_or_else(Instant::now),
            health_ledger_emit_interval,
            storage_budget_mode,
            storage_budget_max_bytes,
            storage_budget_horizon_years,
            storage_budget_target_fraction,
            storage_budget_rollup_count: 0,
            auto_capture_mode,
            auto_capture_redact,
            auto_capture_max_chars,
            auto_capture_count: 0,
            last_temporal_node_id: None,
            last_file_mtime: if file_existed {
                std::fs::metadata(path).and_then(|m| m.modified()).ok()
            } else {
                None
            },
            workspace_manager: super::workspace::WorkspaceManager::new(),
        };

        if let Some(version) = legacy_version {
            match migration_policy {
                StorageMigrationPolicy::Strict => {
                    return Err(McpError::AgenticMemory(format!(
                        "Legacy .amem version {} blocked by strict migration policy",
                        version
                    )));
                }
                StorageMigrationPolicy::Off => {
                    tracing::warn!(
                        "Legacy storage version detected (v{}), auto-migration disabled by policy",
                        version
                    );
                }
                StorageMigrationPolicy::AutoSafe => {
                    if let Some(checkpoint) = manager.create_migration_checkpoint(version)? {
                        tracing::info!(
                            "Legacy storage version detected (v{}), checkpoint created at {}",
                            version,
                            checkpoint.display()
                        );
                    }
                    manager.dirty = true;
                    manager.save()?;
                    tracing::info!(
                        "Auto-migrated memory storage from v{} to v{} at {}",
                        version,
                        CURRENT_AMEM_VERSION,
                        manager.file_path.display()
                    );
                }
            }
        }

        Ok(manager)
    }

    /// Get an immutable reference to the graph.
    pub fn graph(&self) -> &MemoryGraph {
        &self.graph
    }

    /// Get a mutable reference to the graph and mark as dirty.
    pub fn graph_mut(&mut self) -> &mut MemoryGraph {
        self.dirty = true;
        self.last_activity = Instant::now();
        self.record_mutation();
        &mut self.graph
    }

    /// Get the query engine.
    pub fn query_engine(&self) -> &QueryEngine {
        &self.query_engine
    }

    /// Get the write engine.
    pub fn write_engine(&self) -> &WriteEngine {
        &self.write_engine
    }

    /// Get the workspace manager (immutable).
    pub fn workspace_manager(&self) -> &super::workspace::WorkspaceManager {
        &self.workspace_manager
    }

    /// Get the workspace manager (mutable).
    pub fn workspace_manager_mut(&mut self) -> &mut super::workspace::WorkspaceManager {
        &mut self.workspace_manager
    }

    /// Current session ID.
    pub fn current_session_id(&self) -> u32 {
        self.current_session
    }

    /// Start a new session, optionally with an explicit ID.
    pub fn start_session(&mut self, explicit_id: Option<u32>) -> McpResult<u32> {
        let session_id = explicit_id.unwrap_or_else(|| {
            let ids = self.graph.session_index().session_ids();
            let max_indexed = ids.iter().copied().max().unwrap_or(0);
            // Ensure monotonic: new session must be > current session.
            max_indexed.max(self.current_session).saturating_add(1)
        });

        self.current_session = session_id;
        self.last_temporal_node_id = None;
        self.last_activity = Instant::now();
        tracing::info!("Started session {session_id}");
        Ok(session_id)
    }

    /// End a session and optionally create an episode summary.
    pub fn end_session_with_episode(&mut self, session_id: u32, summary: &str) -> McpResult<u64> {
        let episode_id = self
            .write_engine
            .compress_session(&mut self.graph, session_id, summary)
            .map_err(|e| McpError::AgenticMemory(format!("Failed to compress session: {e}")))?;

        self.dirty = true;
        self.last_activity = Instant::now();
        self.record_mutation();
        self.save()?;

        tracing::info!("Ended session {session_id}, created episode node {episode_id}");

        Ok(episode_id)
    }

    /// Save the graph to file with file-locking for concurrent session safety.
    ///
    /// When multiple MCP instances share the same `.amem` file, this method:
    /// 1. Acquires an exclusive file lock (sidecar `.amem.lock`)
    /// 2. Checks if the file was modified externally (by another instance)
    /// 3. If so, re-reads the disk graph and merges our session's new nodes
    /// 4. Writes the merged graph and releases the lock
    pub fn save(&mut self) -> McpResult<()> {
        if !self.dirty {
            return Ok(());
        }

        let _lock = FileLock::acquire(&self.file_path)?;

        // Detect external modifications from concurrent sessions.
        if self.file_path.exists() {
            let current_mtime = std::fs::metadata(&self.file_path)
                .and_then(|m| m.modified())
                .ok();
            if let (Some(current), Some(last_known)) = (current_mtime, self.last_file_mtime) {
                if current > last_known {
                    tracing::info!("Detected external modification, merging with disk state");
                    self.merge_with_disk()?;
                }
            }
        }

        let writer = AmemWriter::new(self.graph.dimension());
        writer
            .write_to_file(&self.graph, &self.file_path)
            .map_err(|e| McpError::AgenticMemory(format!("Failed to write memory file: {e}")))?;

        // Update our mtime tracking after successful write.
        self.last_file_mtime = std::fs::metadata(&self.file_path)
            .and_then(|m| m.modified())
            .ok();

        self.dirty = false;
        self.last_save = Instant::now();
        self.save_generation = self.save_generation.saturating_add(1);
        tracing::debug!("Saved memory file: {}", self.file_path.display());
        Ok(())
    }

    /// Merge our session's nodes/edges with the latest disk state.
    ///
    /// This handles the case where another MCP instance wrote to the same file
    /// since we last read it. We re-read the disk, then re-add our session's
    /// nodes on top of the latest state.
    fn merge_with_disk(&mut self) -> McpResult<()> {
        let disk_graph = AmemReader::read_from_file(&self.file_path)
            .map_err(|e| McpError::AgenticMemory(format!("Failed to re-read for merge: {e}")))?;

        // Collect our session's nodes (those we created in this process).
        let our_nodes: Vec<_> = self
            .graph
            .nodes()
            .iter()
            .filter(|n| n.session_id == self.current_session)
            .cloned()
            .collect();

        // Collect edges where source belongs to our session.
        let our_node_ids: std::collections::HashSet<u64> = our_nodes.iter().map(|n| n.id).collect();
        let our_edges: Vec<_> = self
            .graph
            .edges()
            .iter()
            .filter(|e| our_node_ids.contains(&e.source_id) || our_node_ids.contains(&e.target_id))
            .cloned()
            .collect();

        // Replace our graph with the latest disk state.
        self.graph = disk_graph;

        // Re-add our session's nodes with fresh IDs from the merged graph.
        let mut id_map: HashMap<u64, u64> = HashMap::new();
        for node in &our_nodes {
            let event = CognitiveEventBuilder::new(node.event_type, node.content.clone())
                .session_id(self.current_session)
                .confidence(node.confidence)
                .build();
            let result = self
                .write_engine
                .ingest(&mut self.graph, vec![event], vec![])
                .map_err(|e| McpError::AgenticMemory(format!("Merge node re-add failed: {e}")))?;
            if let Some(&new_id) = result.new_node_ids.first() {
                id_map.insert(node.id, new_id);
            }
        }

        // Re-add our session's edges with remapped IDs.
        for edge in &our_edges {
            let source = id_map
                .get(&edge.source_id)
                .copied()
                .unwrap_or(edge.source_id);
            let target = id_map
                .get(&edge.target_id)
                .copied()
                .unwrap_or(edge.target_id);
            let new_edge = Edge::new(source, target, edge.edge_type, edge.weight);
            if let Err(e) = self.graph.add_edge(new_edge) {
                tracing::warn!("Merge edge re-add skipped: {e}");
            }
        }

        tracing::info!(
            "Merged {} nodes and {} edges from session {} into disk state",
            our_nodes.len(),
            our_edges.len(),
            self.current_session
        );
        Ok(())
    }

    /// Check if auto-save is needed and save if so.
    pub fn maybe_auto_save(&mut self) -> McpResult<()> {
        if self.dirty && self.last_save.elapsed() >= self.auto_save_interval {
            self.save()?;
        }
        Ok(())
    }

    /// Runs autonomous maintenance: sleep-cycle, auto-save, and periodic backup.
    pub fn run_maintenance_tick(&mut self) -> McpResult<()> {
        if self.should_throttle_maintenance() {
            self.maintenance_throttle_count = self.maintenance_throttle_count.saturating_add(1);
            self.maybe_auto_save()?;
            self.emit_health_ledger("throttled")?;
            tracing::debug!(
                "Maintenance throttled by SLA guard: mutation_rate={} threshold={}",
                self.mutation_rate_per_min(),
                self.sla_max_mutations_per_min
            );
            return Ok(());
        }

        self.maybe_run_sleep_cycle()?;
        self.maybe_auto_save()?;
        self.maybe_enforce_storage_budget()?;
        self.maybe_auto_backup()?;
        self.emit_health_ledger("normal")?;
        Ok(())
    }

    /// Run a periodic sleep-cycle: decay refresh + tier balancing + auto-archive.
    pub fn maybe_run_sleep_cycle(&mut self) -> McpResult<()> {
        if self.last_sleep_cycle.elapsed() < self.sleep_cycle_interval {
            return Ok(());
        }
        if self.last_activity.elapsed() < self.sleep_idle_min {
            return Ok(());
        }

        let now = agentic_memory::now_micros();
        let decay_report = self
            .write_engine
            .run_decay(&mut self.graph, now)
            .map_err(|e| McpError::AgenticMemory(format!("Sleep-cycle decay failed: {e}")))?;
        let archived_sessions = self.auto_archive_completed_sessions()?;

        if decay_report.nodes_decayed > 0 || archived_sessions > 0 {
            self.dirty = true;
            self.save()?;
        }

        let (hot, warm, cold) = self.tier_counts();
        self.last_sleep_cycle = Instant::now();
        tracing::info!(
            "Sleep-cycle complete: decayed={} archived_sessions={} tiers(h/w/c)={}/{}/{}",
            decay_report.nodes_decayed,
            archived_sessions,
            hot,
            warm,
            cold
        );
        Ok(())
    }

    /// Periodic backup of persisted state with retention pruning.
    pub fn maybe_auto_backup(&mut self) -> McpResult<()> {
        if self.last_backup.elapsed() < self.backup_interval {
            return Ok(());
        }
        if self.save_generation <= self.last_backup_generation {
            return Ok(());
        }
        if !self.file_path.exists() {
            return Ok(());
        }

        std::fs::create_dir_all(&self.backups_dir).map_err(McpError::Io)?;
        let backup_path = self.next_backup_path();
        std::fs::copy(&self.file_path, &backup_path).map_err(McpError::Io)?;
        self.last_backup_generation = self.save_generation;
        self.last_backup = Instant::now();
        self.prune_old_backups()?;
        tracing::info!("Auto-backup written: {}", backup_path.display());
        Ok(())
    }

    /// Mark the graph as dirty (needs saving).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.last_activity = Instant::now();
        self.record_mutation();
    }

    /// Get the file path.
    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    /// The ID of the most recent node in the temporal chain for this session.
    pub fn last_temporal_node_id(&self) -> Option<u64> {
        self.last_temporal_node_id
    }

    /// Advance the temporal chain pointer to the given node ID.
    pub fn advance_temporal_chain(&mut self, node_id: u64) {
        self.last_temporal_node_id = Some(node_id);
    }

    /// Create a TemporalNext edge from `prev_id` to `next_id` (forward in time).
    pub fn link_temporal(&mut self, prev_id: u64, next_id: u64) -> McpResult<()> {
        let edge = Edge::new(prev_id, next_id, EdgeType::TemporalNext, 1.0);
        self.graph
            .add_edge(edge)
            .map_err(|e| McpError::AgenticMemory(format!("Failed to add temporal edge: {e}")))?;
        self.dirty = true;
        Ok(())
    }

    /// Background maintenance loop interval.
    pub fn maintenance_interval(&self) -> Duration {
        self.auto_save_interval
            .min(self.backup_interval)
            .min(self.sleep_cycle_interval)
    }

    /// Capture a prompt template invocation (`prompts/get`) into memory.
    pub fn capture_prompt_request(
        &mut self,
        prompt_name: &str,
        arguments: Option<&Value>,
    ) -> McpResult<Option<u64>> {
        if self.auto_capture_mode == AutoCaptureMode::Off {
            return Ok(None);
        }
        match extract_prompt_capture_text(prompt_name, arguments)? {
            Some(text) => self.persist_auto_capture(EventType::Fact, &text, 0.90),
            None => Ok(None),
        }
    }

    /// Capture a tool call input context into memory based on capture mode.
    pub fn capture_tool_call(
        &mut self,
        tool_name: &str,
        arguments: Option<&Value>,
    ) -> McpResult<Option<u64>> {
        if self.auto_capture_mode == AutoCaptureMode::Off {
            return Ok(None);
        }

        let text = match self.auto_capture_mode {
            AutoCaptureMode::Safe => extract_safe_tool_capture_text(tool_name, arguments)?,
            AutoCaptureMode::Full => extract_full_tool_capture_text(tool_name, arguments)?,
            AutoCaptureMode::Off => None,
        };
        match text {
            Some(v) => self.persist_auto_capture(EventType::Inference, &v, 0.82),
            None => Ok(None),
        }
    }

    /// Add a cognitive event to the graph.
    pub fn add_event(
        &mut self,
        event_type: EventType,
        content: &str,
        confidence: f32,
        edges: Vec<(u64, EdgeType, f32)>,
    ) -> McpResult<(u64, usize)> {
        let event = CognitiveEventBuilder::new(event_type, content.to_string())
            .session_id(self.current_session)
            .confidence(confidence)
            .build();

        // First, add the node to get its assigned ID
        let result = self
            .write_engine
            .ingest(&mut self.graph, vec![event], vec![])
            .map_err(|e| McpError::AgenticMemory(format!("Failed to add event: {e}")))?;

        let node_id = result.new_node_ids.first().copied().ok_or_else(|| {
            McpError::InternalError("No node ID returned from ingest".to_string())
        })?;

        // Then add edges with the correct source_id
        let mut edge_count = 0;
        for (target_id, edge_type, weight) in &edges {
            let edge = Edge::new(node_id, *target_id, *edge_type, *weight);
            self.graph
                .add_edge(edge)
                .map_err(|e| McpError::AgenticMemory(format!("Failed to add edge: {e}")))?;
            edge_count += 1;
        }

        self.dirty = true;
        self.last_activity = Instant::now();
        self.record_mutation();
        self.maybe_auto_save()?;

        Ok((node_id, edge_count))
    }

    /// Correct a previous belief.
    pub fn correct_node(&mut self, old_node_id: u64, new_content: &str) -> McpResult<u64> {
        let new_id = self
            .write_engine
            .correct(
                &mut self.graph,
                old_node_id,
                new_content,
                self.current_session,
            )
            .map_err(|e| McpError::AgenticMemory(format!("Failed to correct node: {e}")))?;

        self.dirty = true;
        self.last_activity = Instant::now();
        self.record_mutation();
        self.maybe_auto_save()?;

        Ok(new_id)
    }

    fn record_mutation(&mut self) {
        if self.mutation_window_started.elapsed() >= Duration::from_secs(60) {
            self.mutation_window_started = Instant::now();
            self.mutation_window_count = 0;
        }
        self.mutation_window_count = self.mutation_window_count.saturating_add(1);
    }

    fn mutation_rate_per_min(&self) -> u32 {
        let elapsed = self.mutation_window_started.elapsed().as_secs().max(1);
        let scaled = (self.mutation_window_count as u64)
            .saturating_mul(60)
            .saturating_div(elapsed);
        scaled.min(u32::MAX as u64) as u32
    }

    fn should_throttle_maintenance(&self) -> bool {
        self.mutation_rate_per_min() > self.sla_max_mutations_per_min
    }

    fn emit_health_ledger(&mut self, maintenance_mode: &str) -> McpResult<()> {
        if self.last_health_ledger_emit.elapsed() < self.health_ledger_emit_interval {
            return Ok(());
        }

        let dir = resolve_health_ledger_dir();
        std::fs::create_dir_all(&dir).map_err(McpError::Io)?;
        let path = dir.join("agentic-memory.json");
        let tmp = dir.join("agentic-memory.json.tmp");
        let (hot, warm, cold) = self.tier_counts();
        let current_size_bytes = self.current_file_size_bytes();
        let projected_size_bytes = self.projected_file_size_bytes(current_size_bytes);
        let over_budget = current_size_bytes > self.storage_budget_max_bytes
            || projected_size_bytes
                .map(|v| v > self.storage_budget_max_bytes)
                .unwrap_or(false);
        let payload = serde_json::json!({
            "project": "AgenticMemory",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "status": "ok",
            "autonomic": {
                "profile": self.profile.as_str(),
                "migration_policy": self.migration_policy.as_str(),
                "maintenance_mode": maintenance_mode,
                "throttle_count": self.maintenance_throttle_count,
            },
            "sla": {
                "mutation_rate_per_min": self.mutation_rate_per_min(),
                "max_mutations_per_min": self.sla_max_mutations_per_min
            },
            "storage": {
                "file": self.file_path.display().to_string(),
                "dirty": self.dirty,
                "save_generation": self.save_generation,
                "backup_retention": self.backup_retention,
            },
            "storage_budget": {
                "mode": self.storage_budget_mode.as_str(),
                "max_bytes": self.storage_budget_max_bytes,
                "horizon_years": self.storage_budget_horizon_years,
                "target_fraction": self.storage_budget_target_fraction,
                "current_size_bytes": current_size_bytes,
                "projected_size_bytes": projected_size_bytes,
                "over_budget": over_budget,
                "rollup_count": self.storage_budget_rollup_count,
            },
            "auto_capture": {
                "mode": self.auto_capture_mode.as_str(),
                "redact": self.auto_capture_redact,
                "max_chars": self.auto_capture_max_chars,
                "captured_count": self.auto_capture_count
            },
            "graph": {
                "nodes": self.graph.node_count(),
                "edges": self.graph.edge_count(),
                "tiers": {
                    "hot": hot,
                    "warm": warm,
                    "cold": cold,
                },
            },
        });
        let bytes = serde_json::to_vec_pretty(&payload).map_err(|e| {
            McpError::AgenticMemory(format!("Failed to encode health ledger payload: {e}"))
        })?;
        std::fs::write(&tmp, bytes).map_err(McpError::Io)?;
        std::fs::rename(&tmp, &path).map_err(McpError::Io)?;
        self.last_health_ledger_emit = Instant::now();
        Ok(())
    }
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        if self.dirty {
            if let Err(e) = self.save() {
                tracing::error!("Failed to save on drop: {e}");
            }
        }
        if let Err(e) = self.maybe_auto_backup() {
            tracing::error!("Failed auto-backup on drop: {e}");
        }
    }
}

impl SessionManager {
    fn auto_archive_completed_sessions(&mut self) -> McpResult<usize> {
        self.auto_archive_completed_sessions_with_min(self.archive_min_session_nodes)
    }

    fn auto_archive_completed_sessions_with_min(
        &mut self,
        min_session_nodes: usize,
    ) -> McpResult<usize> {
        let mut session_ids = self.graph.session_index().session_ids();
        session_ids.sort_unstable();

        let mut archived = 0usize;
        for session_id in session_ids {
            if session_id >= self.current_session {
                continue;
            }

            let node_ids = self.graph.session_index().get_session(session_id).to_vec();
            if node_ids.is_empty() {
                continue;
            }

            let mut has_episode = false;
            let mut event_nodes = 0usize;
            let mut hot = 0usize;
            let mut warm = 0usize;
            let mut cold = 0usize;

            for node_id in &node_ids {
                if let Some(node) = self.graph.get_node(*node_id) {
                    if node.event_type == EventType::Episode {
                        has_episode = true;
                        continue;
                    }
                    event_nodes += 1;
                    if node.decay_score >= self.hot_min_decay {
                        hot += 1;
                    } else if node.decay_score >= self.warm_min_decay {
                        warm += 1;
                    } else {
                        cold += 1;
                    }
                }
            }

            if has_episode || event_nodes < min_session_nodes {
                continue;
            }

            let summary = format!(
                "Auto-archive session {}: {} events ({} hot / {} warm / {} cold)",
                session_id, event_nodes, hot, warm, cold
            );
            self.write_engine
                .compress_session(&mut self.graph, session_id, &summary)
                .map_err(|e| {
                    McpError::AgenticMemory(format!(
                        "Auto-archive failed for session {session_id}: {e}"
                    ))
                })?;
            archived = archived.saturating_add(1);
        }

        Ok(archived)
    }

    fn maybe_enforce_storage_budget(&mut self) -> McpResult<()> {
        if self.storage_budget_mode == StorageBudgetMode::Off {
            return Ok(());
        }

        let current_size = self.current_file_size_bytes();
        if current_size == 0 {
            return Ok(());
        }
        let projected = self.projected_file_size_bytes(current_size);
        let over_current = current_size > self.storage_budget_max_bytes;
        let over_projected = projected
            .map(|v| v > self.storage_budget_max_bytes)
            .unwrap_or(false);

        if !over_current && !over_projected {
            return Ok(());
        }

        if self.storage_budget_mode == StorageBudgetMode::Warn {
            tracing::warn!(
                "Storage budget warning: current={} projected={:?} budget={} (mode=warn)",
                current_size,
                projected,
                self.storage_budget_max_bytes
            );
            return Ok(());
        }

        let target_bytes = ((self.storage_budget_max_bytes as f64
            * self.storage_budget_target_fraction as f64)
            .round() as u64)
            .max(1);
        let mut rollup_count = 0usize;
        let mut threshold = self.archive_min_session_nodes.saturating_div(2).max(1);

        for _ in 0..3 {
            let archived = self.auto_archive_completed_sessions_with_min(threshold)?;
            if archived == 0 {
                if threshold > 1 {
                    threshold = 1;
                    continue;
                }
                break;
            }
            rollup_count += archived;
            self.dirty = true;
            self.save()?;
            let new_size = self.current_file_size_bytes();
            if new_size <= target_bytes {
                break;
            }
            threshold = 1;
        }

        if rollup_count > 0 {
            self.storage_budget_rollup_count = self
                .storage_budget_rollup_count
                .saturating_add(rollup_count as u64);
            tracing::info!(
                "Storage budget rollup applied: archived_sessions={} budget={} target={} current={}",
                rollup_count,
                self.storage_budget_max_bytes,
                target_bytes,
                self.current_file_size_bytes()
            );
        } else {
            tracing::warn!(
                "Storage budget exceeded but no completed sessions eligible for rollup (current={} projected={:?} budget={})",
                current_size,
                projected,
                self.storage_budget_max_bytes
            );
        }

        Ok(())
    }

    fn current_file_size_bytes(&self) -> u64 {
        std::fs::metadata(&self.file_path)
            .map(|m| m.len())
            .unwrap_or(0)
    }

    fn persist_auto_capture(
        &mut self,
        event_type: EventType,
        raw_text: &str,
        confidence: f32,
    ) -> McpResult<Option<u64>> {
        let mut text = raw_text.trim().to_string();
        if text.is_empty() {
            return Ok(None);
        }

        if self.auto_capture_redact {
            text = redact_sensitive_tokens(&text);
        }

        if text.len() > self.auto_capture_max_chars {
            text.truncate(self.auto_capture_max_chars);
            text.push_str(" …[truncated]");
        }

        let prev_id = self.last_temporal_node_id;
        let (node_id, _) = self.add_event(event_type, &text, confidence, vec![])?;

        // Chain this capture to the previous node in the session's temporal thread.
        if let Some(prev) = prev_id {
            if let Err(e) = self.link_temporal(prev, node_id) {
                tracing::warn!("Failed to link temporal chain: {e}");
            }
        }
        self.last_temporal_node_id = Some(node_id);

        self.auto_capture_count = self.auto_capture_count.saturating_add(1);
        Ok(Some(node_id))
    }

    fn projected_file_size_bytes(&self, current_size: u64) -> Option<u64> {
        if current_size == 0 || self.graph.node_count() < 2 {
            return None;
        }
        let mut min_ts = u64::MAX;
        let mut max_ts = 0u64;
        for node in self.graph.nodes() {
            min_ts = min_ts.min(node.created_at);
            max_ts = max_ts.max(node.created_at);
        }
        if min_ts == u64::MAX || max_ts <= min_ts {
            return None;
        }

        let span_secs_raw = (max_ts - min_ts) / 1_000_000;
        // Clamp to at least one week to avoid unstable extrapolation on tiny windows.
        let span_secs = span_secs_raw.max(7 * 24 * 3600) as f64;
        let per_sec = current_size as f64 / span_secs;
        let horizon_secs = (self.storage_budget_horizon_years as f64) * 365.25 * 24.0 * 3600.0;
        let projected = (per_sec * horizon_secs).round();
        Some(projected.max(0.0).min(u64::MAX as f64) as u64)
    }

    fn tier_counts(&self) -> (usize, usize, usize) {
        let mut hot = 0usize;
        let mut warm = 0usize;
        let mut cold = 0usize;

        for node in self.graph.nodes() {
            if node.event_type == EventType::Episode {
                continue;
            }
            if node.decay_score >= self.hot_min_decay {
                hot += 1;
            } else if node.decay_score >= self.warm_min_decay {
                warm += 1;
            } else {
                cold += 1;
            }
        }

        (hot, warm, cold)
    }

    fn create_migration_checkpoint(&self, from_version: u32) -> McpResult<Option<PathBuf>> {
        if !self.file_path.exists() {
            return Ok(None);
        }

        let migration_dir = resolve_migration_dir(&self.file_path);
        std::fs::create_dir_all(&migration_dir).map_err(McpError::Io)?;

        let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
        let stem = self
            .file_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("brain");
        let checkpoint = migration_dir.join(format!("{stem}.v{from_version}.{ts}.amem.checkpoint"));
        std::fs::copy(&self.file_path, &checkpoint).map_err(McpError::Io)?;
        Ok(Some(checkpoint))
    }

    fn next_backup_path(&self) -> PathBuf {
        let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
        let stem = self
            .file_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("brain");
        self.backups_dir.join(format!("{stem}.{ts}.amem.bak"))
    }

    fn prune_old_backups(&self) -> McpResult<()> {
        let mut entries = std::fs::read_dir(&self.backups_dir)
            .map_err(McpError::Io)?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(|name| name.ends_with(".amem.bak"))
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();

        if entries.len() <= self.backup_retention {
            return Ok(());
        }

        entries.sort_by_key(|entry| {
            entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });
        let to_remove = entries.len().saturating_sub(self.backup_retention);
        for entry in entries.into_iter().take(to_remove) {
            let _ = std::fs::remove_file(entry.path());
        }
        Ok(())
    }
}

fn resolve_backups_dir(memory_path: &std::path::Path) -> PathBuf {
    if let Ok(custom) = std::env::var("AMEM_AUTO_BACKUP_DIR") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    let parent = memory_path.parent().unwrap_or(std::path::Path::new("."));
    parent.join(".amem-backups")
}

fn resolve_migration_dir(memory_path: &Path) -> PathBuf {
    let parent = memory_path.parent().unwrap_or(std::path::Path::new("."));
    parent.join(".amem-migrations")
}

fn read_storage_version(path: &Path) -> Option<u32> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = [0u8; 8];
    file.read_exact(&mut header).ok()?;
    if &header[0..4] != b"AMEM" {
        return None;
    }
    Some(u32::from_le_bytes([
        header[4], header[5], header[6], header[7],
    ]))
}

fn read_env_u64(name: &str, default_value: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default_value)
}

fn read_env_u32(name: &str, default_value: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(default_value)
}

fn read_env_usize(name: &str, default_value: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default_value)
}

fn read_env_f32(name: &str, default_value: f32) -> f32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(default_value)
}

fn read_env_bool(name: &str, default_value: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default_value)
}

fn read_env_string(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string())
}

fn resolve_health_ledger_dir() -> PathBuf {
    if let Some(custom) = read_env_string("AMEM_HEALTH_LEDGER_DIR") {
        if !custom.is_empty() {
            return PathBuf::from(custom);
        }
    }
    if let Some(custom) = read_env_string("AGENTRA_HEALTH_LEDGER_DIR") {
        if !custom.is_empty() {
            return PathBuf::from(custom);
        }
    }

    let home = std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".agentra").join("health-ledger")
}

fn extract_prompt_capture_text(
    prompt_name: &str,
    arguments: Option<&Value>,
) -> McpResult<Option<String>> {
    let args = arguments.unwrap_or(&Value::Null);
    let fields = collect_text_fields_by_keys(
        args,
        &[
            "information",
            "context",
            "topic",
            "old_belief",
            "new_information",
            "reason",
            "summary",
            "instruction",
            "prompt",
            "query",
        ],
        8,
    );
    if fields.is_empty() {
        return Ok(None);
    }
    let joined = fields.join(" | ");
    Ok(Some(format!(
        "[auto-capture][prompt] template={prompt_name} input={joined}"
    )))
}

fn extract_safe_tool_capture_text(
    tool_name: &str,
    arguments: Option<&Value>,
) -> McpResult<Option<String>> {
    let args = arguments.unwrap_or(&Value::Null);
    let keys = ["feedback", "summary", "note"];
    if tool_name != "session_end" {
        // Keep safe mode low-noise and non-invasive: only capture explicit feedback fields.
        let explicit_feedback = collect_text_fields_by_keys(args, &["feedback", "note"], 4);
        if explicit_feedback.is_empty() {
            return Ok(None);
        }
    }
    let fields = collect_text_fields_by_keys(args, &keys, 6);
    if fields.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "[auto-capture][feedback] tool={tool_name} context={}",
        fields.join(" | ")
    )))
}

fn extract_full_tool_capture_text(
    tool_name: &str,
    arguments: Option<&Value>,
) -> McpResult<Option<String>> {
    if tool_name == "memory_add" {
        return Ok(None);
    }
    let args = arguments.unwrap_or(&Value::Null);
    let preferred = collect_text_fields_by_keys(
        args,
        &[
            "query",
            "content",
            "prompt",
            "new_content",
            "reason",
            "summary",
            "topic",
            "instruction",
            "information",
            "context",
            "feedback",
        ],
        10,
    );

    let fields = if preferred.is_empty() {
        collect_all_string_like_fields(args, 8)
    } else {
        preferred
    };

    if fields.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "[auto-capture][tool] tool={tool_name} input={}",
        fields.join(" | ")
    )))
}

fn collect_text_fields_by_keys(value: &Value, keys: &[&str], limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::<String>::new();

    fn walk(
        value: &Value,
        path: String,
        keys: &[&str],
        out: &mut Vec<String>,
        seen: &mut std::collections::BTreeSet<String>,
        limit: usize,
    ) {
        if out.len() >= limit {
            return;
        }
        match value {
            Value::Object(map) => {
                for (k, v) in map {
                    if out.len() >= limit {
                        break;
                    }
                    let next = if path.is_empty() {
                        k.to_string()
                    } else {
                        format!("{path}.{k}")
                    };
                    let key_match = keys
                        .iter()
                        .any(|needle| k.eq_ignore_ascii_case(needle) || next.ends_with(needle));
                    if key_match {
                        if let Some(s) = value_to_compact_string(v) {
                            let entry = format!("{next}={s}");
                            if seen.insert(entry.clone()) {
                                out.push(entry);
                            }
                        }
                    }
                    walk(v, next, keys, out, seen, limit);
                }
            }
            Value::Array(items) => {
                for (idx, item) in items.iter().enumerate() {
                    if out.len() >= limit {
                        break;
                    }
                    let next = format!("{path}[{idx}]");
                    walk(item, next, keys, out, seen, limit);
                }
            }
            _ => {}
        }
    }

    walk(value, String::new(), keys, &mut out, &mut seen, limit);
    out
}

fn collect_all_string_like_fields(value: &Value, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(value: &Value, path: String, out: &mut Vec<String>, limit: usize) {
        if out.len() >= limit {
            return;
        }
        match value {
            Value::Object(map) => {
                for (k, v) in map {
                    if out.len() >= limit {
                        break;
                    }
                    let next = if path.is_empty() {
                        k.to_string()
                    } else {
                        format!("{path}.{k}")
                    };
                    walk(v, next, out, limit);
                }
            }
            Value::Array(items) => {
                for (idx, item) in items.iter().enumerate() {
                    if out.len() >= limit {
                        break;
                    }
                    walk(item, format!("{path}[{idx}]"), out, limit);
                }
            }
            _ => {
                if let Some(s) = value_to_compact_string(value) {
                    out.push(format!("{path}={s}"));
                }
            }
        }
    }
    walk(value, String::new(), &mut out, limit);
    out
}

fn value_to_compact_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        Value::Array(arr) => {
            if arr.is_empty() {
                None
            } else {
                Some(format!("<array:{}>", arr.len()))
            }
        }
        Value::Object(map) => {
            if map.is_empty() {
                None
            } else {
                Some(format!("<object:{}>", map.len()))
            }
        }
    }
}

fn redact_sensitive_tokens(text: &str) -> String {
    text.split_whitespace()
        .map(redact_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_token(token: &str) -> String {
    let trimmed = token.trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == ';');
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.starts_with("/Users/")
        || trimmed.starts_with("C:\\Users\\")
        || trimmed.contains("/Users/")
        || trimmed.contains("C:\\Users\\")
    {
        return "[REDACTED_PATH]".to_string();
    }
    if trimmed.contains('@') && trimmed.contains('.') {
        return "[REDACTED_EMAIL]".to_string();
    }
    if lower.starts_with("sk-")
        || lower.contains("api_key")
        || lower.contains("access_token")
        || lower.contains("bearer")
        || lower.contains("authorization")
    {
        return "[REDACTED_SECRET]".to_string();
    }
    if looks_like_long_secret(trimmed) {
        return "[REDACTED_SECRET]".to_string();
    }
    token.to_string()
}

fn looks_like_long_secret(token: &str) -> bool {
    if token.len() < 24 {
        return false;
    }
    token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// File-based exclusive lock for concurrent `.amem` access.
///
/// Uses a sidecar `.amem.lock` file with `create_new` (O_EXCL) for atomic
/// creation. Stale locks older than 60 seconds are auto-cleaned. The lock
/// is released on drop.
struct FileLock {
    lock_path: PathBuf,
}

impl FileLock {
    /// Acquire an exclusive lock for the given data file.
    /// Spins with a 50ms backoff until the lock is available.
    fn acquire(data_path: &Path) -> McpResult<Self> {
        let lock_path = data_path.with_extension("amem.lock");
        let stale_threshold = Duration::from_secs(60);
        let max_attempts = 200; // 200 * 50ms = 10 seconds max wait

        for attempt in 0..max_attempts {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(_file) => {
                    if attempt > 0 {
                        tracing::debug!(
                            "Acquired file lock after {} attempts: {}",
                            attempt + 1,
                            lock_path.display()
                        );
                    }
                    return Ok(FileLock { lock_path });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // Check if the lock is stale (owner crashed).
                    if let Ok(meta) = std::fs::metadata(&lock_path) {
                        let is_stale = meta
                            .modified()
                            .ok()
                            .and_then(|m| m.elapsed().ok())
                            .map(|age| age > stale_threshold)
                            .unwrap_or(false);
                        if is_stale {
                            tracing::warn!("Removing stale lock file: {}", lock_path.display());
                            let _ = std::fs::remove_file(&lock_path);
                            continue;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(McpError::Io(e));
                }
            }
        }

        Err(McpError::AgenticMemory(format!(
            "Timed out waiting for file lock: {}",
            lock_path.display()
        )))
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn budget_projection_available_with_timeline() {
        let dir = tempfile::tempdir().expect("tempdir");
        let brain = dir.path().join("projection.amem");
        let mut manager = SessionManager::open(brain.to_str().expect("path")).expect("open");

        let (id_a, _) = manager
            .add_event(EventType::Fact, "old fact", 0.9, vec![])
            .expect("add fact");
        let (_id_b, _) = manager
            .add_event(EventType::Fact, "new fact", 0.9, vec![])
            .expect("add fact");

        {
            let graph = manager.graph_mut();
            let old = graph.get_node_mut(id_a).expect("node");
            old.created_at = old.created_at.saturating_sub(15 * 24 * 3600 * 1_000_000);
        }
        manager.save().expect("save");
        let size = manager.current_file_size_bytes();
        let projected = manager.projected_file_size_bytes(size);
        assert!(size > 0);
        assert!(projected.is_some());
    }

    #[test]
    fn budget_auto_rollup_archives_completed_session() {
        let dir = tempfile::tempdir().expect("tempdir");
        let brain = dir.path().join("rollup.amem");
        let mut manager = SessionManager::open(brain.to_str().expect("path")).expect("open");

        // Build current session with enough content, then advance so it becomes completed.
        let _ = manager
            .add_event(EventType::Fact, "alpha", 0.8, vec![])
            .expect("add");
        let _ = manager
            .add_event(EventType::Decision, "beta", 0.9, vec![])
            .expect("add");
        manager.start_session(None).expect("session");
        manager.save().expect("save");

        // Force tiny budget to trigger rollup.
        manager.storage_budget_mode = StorageBudgetMode::AutoRollup;
        manager.storage_budget_max_bytes = 1;
        manager.storage_budget_target_fraction = 0.5;

        manager
            .maybe_enforce_storage_budget()
            .expect("enforce budget");

        let episode_count = manager
            .graph()
            .nodes()
            .iter()
            .filter(|n| n.event_type == EventType::Episode)
            .count();
        assert!(episode_count >= 1);
        assert!(manager.storage_budget_rollup_count >= 1);
    }

    #[test]
    fn auto_capture_off_noop() {
        let dir = tempfile::tempdir().expect("tempdir");
        let brain = dir.path().join("capture-off.amem");
        let mut manager = SessionManager::open(brain.to_str().expect("path")).expect("open");
        manager.auto_capture_mode = AutoCaptureMode::Off;

        let captured = manager
            .capture_prompt_request(
                "remember",
                Some(&json!({"information":"hello world","context":"ctx"})),
            )
            .expect("capture");
        assert!(captured.is_none());
        assert_eq!(manager.graph().node_count(), 0);
    }

    #[test]
    fn auto_capture_full_records_and_redacts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let brain = dir.path().join("capture-full.amem");
        let mut manager = SessionManager::open(brain.to_str().expect("path")).expect("open");
        manager.auto_capture_mode = AutoCaptureMode::Full;
        manager.auto_capture_redact = true;

        manager
            .capture_tool_call(
                "memory_query",
                Some(&json!({
                    "query":"Find anything about token sk-THISISALONGSECRET123456",
                    "context":"/Users/omoshola/Documents/private.txt",
                    "reason":"email me at test@example.com"
                })),
            )
            .expect("capture");

        assert!(manager.graph().node_count() >= 1);
        let latest = manager
            .graph()
            .nodes()
            .iter()
            .max_by_key(|n| n.id)
            .expect("node");
        assert!(latest.content.contains("[auto-capture][tool]"));
        assert!(latest.content.contains("[REDACTED_SECRET]"));
        assert!(latest.content.contains("[REDACTED_PATH]"));
        assert!(latest.content.contains("[REDACTED_EMAIL]"));
    }

    #[test]
    fn auto_capture_temporal_chain() {
        let dir = tempfile::tempdir().expect("tempdir");
        let brain = dir.path().join("chain.amem");
        let mut manager = SessionManager::open(brain.to_str().expect("path")).expect("open");
        manager.auto_capture_mode = AutoCaptureMode::Full;

        // First capture: no predecessor.
        let id1 = manager
            .capture_tool_call("memory_query", Some(&json!({"query": "first question"})))
            .expect("capture")
            .expect("node_id");

        assert_eq!(manager.last_temporal_node_id(), Some(id1));

        // Second capture: should link to first.
        let id2 = manager
            .capture_tool_call(
                "memory_similar",
                Some(&json!({"query_text": "second question"})),
            )
            .expect("capture")
            .expect("node_id");

        assert_eq!(manager.last_temporal_node_id(), Some(id2));

        // Verify the TemporalNext edge exists: id1 -> id2.
        let has_temporal = manager.graph().edges().iter().any(|e| {
            e.source_id == id1 && e.target_id == id2 && e.edge_type == EdgeType::TemporalNext
        });
        assert!(has_temporal, "Expected TemporalNext edge from id1 to id2");
    }

    #[test]
    fn temporal_chain_resets_on_new_session() {
        let dir = tempfile::tempdir().expect("tempdir");
        let brain = dir.path().join("reset.amem");
        let mut manager = SessionManager::open(brain.to_str().expect("path")).expect("open");
        manager.auto_capture_mode = AutoCaptureMode::Full;

        let _id1 = manager
            .capture_tool_call("memory_query", Some(&json!({"query": "first"})))
            .expect("capture");

        assert!(manager.last_temporal_node_id().is_some());

        // Starting a new session should reset the chain.
        manager.start_session(None).expect("new session");
        assert!(manager.last_temporal_node_id().is_none());
    }

    #[test]
    fn memory_add_joins_temporal_chain() {
        let dir = tempfile::tempdir().expect("tempdir");
        let brain = dir.path().join("splice.amem");
        let mut manager = SessionManager::open(brain.to_str().expect("path")).expect("open");
        manager.auto_capture_mode = AutoCaptureMode::Full;

        // Create a chain head via auto-capture.
        let id1 = manager
            .capture_tool_call("memory_query", Some(&json!({"query": "something"})))
            .expect("capture")
            .expect("node_id");

        // Simulate what memory_add tool does: add_event + link_temporal + advance.
        let (id2, _) = manager
            .add_event(EventType::Fact, "User prefers dark mode", 0.9, vec![])
            .expect("add_event");
        manager.link_temporal(id1, id2).expect("link");
        manager.advance_temporal_chain(id2);

        assert_eq!(manager.last_temporal_node_id(), Some(id2));

        let has_edge = manager.graph().edges().iter().any(|e| {
            e.source_id == id1 && e.target_id == id2 && e.edge_type == EdgeType::TemporalNext
        });
        assert!(has_edge, "memory_add node should be linked into chain");
    }
}
