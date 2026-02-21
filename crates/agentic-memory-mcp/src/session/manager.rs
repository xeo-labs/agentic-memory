//! Graph lifecycle management, file I/O, and session tracking.

use std::ffi::OsStr;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agentic_memory::{
    AmemReader, AmemWriter, CognitiveEventBuilder, Edge, EdgeType, EventType, MemoryGraph,
    QueryEngine, WriteEngine,
};

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

        // Determine the next session ID from existing sessions
        let session_ids = graph.session_index().session_ids();
        let current_session = session_ids.iter().copied().max().unwrap_or(0) + 1;

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

    /// Current session ID.
    pub fn current_session_id(&self) -> u32 {
        self.current_session
    }

    /// Start a new session, optionally with an explicit ID.
    pub fn start_session(&mut self, explicit_id: Option<u32>) -> McpResult<u32> {
        let session_id = explicit_id.unwrap_or_else(|| {
            let ids = self.graph.session_index().session_ids();
            ids.iter().copied().max().unwrap_or(0) + 1
        });

        self.current_session = session_id;
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

    /// Save the graph to file.
    pub fn save(&mut self) -> McpResult<()> {
        if !self.dirty {
            return Ok(());
        }

        let writer = AmemWriter::new(self.graph.dimension());
        writer
            .write_to_file(&self.graph, &self.file_path)
            .map_err(|e| McpError::AgenticMemory(format!("Failed to write memory file: {e}")))?;

        self.dirty = false;
        self.last_save = Instant::now();
        self.save_generation = self.save_generation.saturating_add(1);
        tracing::debug!("Saved memory file: {}", self.file_path.display());
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

    /// Background maintenance loop interval.
    pub fn maintenance_interval(&self) -> Duration {
        self.auto_save_interval
            .min(self.backup_interval)
            .min(self.sleep_cycle_interval)
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

            if has_episode || event_nodes < self.archive_min_session_nodes {
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
