//! Sister integration bridge traits for AgenticMemory.
//!
//! Each bridge defines the interface for integrating with another Agentra sister.
//! Default implementations are no-ops, allowing gradual adoption.
//! Trait-based design ensures Hydra compatibility — swap implementors without refactoring.

/// Bridge to agentic-identity for cryptographic signing of memory operations.
pub trait IdentityBridge: Send + Sync {
    /// Sign a memory node to prove authorship
    fn sign_node(&self, node_id: u64, content_hash: &str) -> Result<String, String> {
        let _ = (node_id, content_hash);
        Err("Identity bridge not connected".to_string())
    }

    /// Verify that a memory node was signed by a specific agent
    fn verify_node_signature(&self, node_id: u64, agent_id: &str, signature: &str) -> bool {
        let _ = (node_id, agent_id, signature);
        true // Default: trust all
    }

    /// Get the identity anchor for attribution
    fn resolve_identity(&self, agent_id: &str) -> Option<String> {
        let _ = agent_id;
        None
    }

    /// Anchor a receipt for a memory operation (add, correct, delete)
    fn anchor_receipt(&self, action: &str, node_id: u64) -> Result<String, String> {
        let _ = (action, node_id);
        Err("Identity bridge not connected".to_string())
    }
}

/// Bridge to agentic-vision for linking visual captures to memory nodes.
pub trait VisionBridge: Send + Sync {
    /// Link a visual capture to a memory node
    fn link_capture(&self, capture_id: u64, node_id: u64, relationship: &str) -> Result<(), String> {
        let _ = (capture_id, node_id, relationship);
        Err("Vision bridge not connected".to_string())
    }

    /// Query visual captures related to a memory topic
    fn query_visual_context(&self, topic: &str, max_results: usize) -> Vec<String> {
        let _ = (topic, max_results);
        Vec::new()
    }

    /// Capture current visual state and link to a memory episode
    fn capture_and_link(&self, description: &str, node_id: u64) -> Result<u64, String> {
        let _ = (description, node_id);
        Err("Vision bridge not connected".to_string())
    }
}

/// Bridge to agentic-time for temporal context of memories.
pub trait TimeBridge: Send + Sync {
    /// Associate a deadline with a memory node
    fn link_deadline(&self, node_id: u64, deadline_id: &str) -> Result<(), String> {
        let _ = (node_id, deadline_id);
        Err("Time bridge not connected".to_string())
    }

    /// Get temporal context (deadlines, schedules) relevant to a memory topic
    fn temporal_context(&self, topic: &str) -> Vec<String> {
        let _ = topic;
        Vec::new()
    }

    /// Check if a memory's associated deadline has passed
    fn is_deadline_past(&self, deadline_id: &str) -> Option<bool> {
        let _ = deadline_id;
        None
    }

    /// Schedule a memory decay check at a future time
    fn schedule_decay_check(&self, node_id: u64, check_at: u64) -> Result<String, String> {
        let _ = (node_id, check_at);
        Err("Time bridge not connected".to_string())
    }
}

/// Bridge to agentic-contract for policy enforcement on memory operations.
pub trait ContractBridge: Send + Sync {
    /// Check if a memory operation is allowed by current policies
    fn check_policy(&self, operation: &str, context: &str) -> Result<bool, String> {
        let _ = (operation, context);
        Ok(true) // Default: allow all
    }

    /// Record a memory operation for audit trail
    fn record_operation(&self, operation: &str, node_id: u64) -> Result<(), String> {
        let _ = (operation, node_id);
        Err("Contract bridge not connected".to_string())
    }

    /// Validate that memory retention complies with obligations
    fn check_retention_policy(&self, node_id: u64, age_seconds: u64) -> Result<bool, String> {
        let _ = (node_id, age_seconds);
        Ok(true) // Default: keep all
    }
}

/// Bridge to agentic-codebase for code-aware memory operations.
pub trait CodebaseBridge: Send + Sync {
    /// Link a memory node to a code symbol
    fn link_symbol(&self, node_id: u64, symbol_name: &str) -> Result<(), String> {
        let _ = (node_id, symbol_name);
        Err("Codebase bridge not connected".to_string())
    }

    /// Find code symbols related to a memory topic
    fn find_related_code(&self, topic: &str, max_results: usize) -> Vec<String> {
        let _ = (topic, max_results);
        Vec::new()
    }

    /// Get code context for enriching a memory node
    fn code_context(&self, symbol_name: &str) -> Option<String> {
        let _ = symbol_name;
        None
    }
}

/// Bridge to agentic-comm for message-linked memories.
pub trait CommBridge: Send + Sync {
    /// Store a conversation episode from a comm channel
    fn store_from_channel(&self, channel_id: u64, summary: &str) -> Result<u64, String> {
        let _ = (channel_id, summary);
        Err("Comm bridge not connected".to_string())
    }

    /// Notify comm of a memory event (for broadcast/pub-sub)
    fn notify_memory_event(&self, event_type: &str, node_id: u64) -> Result<(), String> {
        let _ = (event_type, node_id);
        Err("Comm bridge not connected".to_string())
    }
}

/// No-op implementation of all bridges for standalone use.
#[derive(Debug, Clone, Default)]
pub struct NoOpBridges;

impl IdentityBridge for NoOpBridges {}
impl VisionBridge for NoOpBridges {}
impl TimeBridge for NoOpBridges {}
impl ContractBridge for NoOpBridges {}
impl CodebaseBridge for NoOpBridges {}
impl CommBridge for NoOpBridges {}

/// Configuration for which bridges are active.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    pub identity_enabled: bool,
    pub vision_enabled: bool,
    pub time_enabled: bool,
    pub contract_enabled: bool,
    pub codebase_enabled: bool,
    pub comm_enabled: bool,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            identity_enabled: false,
            vision_enabled: false,
            time_enabled: false,
            contract_enabled: false,
            codebase_enabled: false,
            comm_enabled: false,
        }
    }
}

/// Hydra adapter trait — future orchestrator discovery interface.
/// Each sister implements this so Hydra can discover and route through it.
pub trait HydraAdapter: Send + Sync {
    /// Unique adapter identifier for this sister instance
    fn adapter_id(&self) -> &str;

    /// List capabilities this sister exposes to Hydra
    fn capabilities(&self) -> Vec<String>;

    /// Handle an adapter request from Hydra
    fn handle_request(&self, method: &str, params: &str) -> Result<String, String>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_bridges_implements_all_traits() {
        let b = NoOpBridges;
        let _: &dyn IdentityBridge = &b;
        let _: &dyn VisionBridge = &b;
        let _: &dyn TimeBridge = &b;
        let _: &dyn ContractBridge = &b;
        let _: &dyn CodebaseBridge = &b;
        let _: &dyn CommBridge = &b;
    }

    #[test]
    fn identity_bridge_defaults() {
        let b = NoOpBridges;
        assert!(b.sign_node(1, "hash").is_err());
        assert!(b.verify_node_signature(1, "agent-1", "sig"));
        assert!(b.resolve_identity("agent-1").is_none());
        assert!(b.anchor_receipt("add", 1).is_err());
    }

    #[test]
    fn vision_bridge_defaults() {
        let b = NoOpBridges;
        assert!(b.link_capture(1, 2, "observed_during").is_err());
        assert!(b.query_visual_context("ui", 5).is_empty());
        assert!(b.capture_and_link("screenshot", 1).is_err());
    }

    #[test]
    fn time_bridge_defaults() {
        let b = NoOpBridges;
        assert!(b.link_deadline(1, "dl-1").is_err());
        assert!(b.temporal_context("topic").is_empty());
        assert!(b.is_deadline_past("dl-1").is_none());
        assert!(b.schedule_decay_check(1, 1000).is_err());
    }

    #[test]
    fn contract_bridge_defaults() {
        let b = NoOpBridges;
        assert!(b.check_policy("add", "ctx").unwrap());
        assert!(b.record_operation("add", 1).is_err());
        assert!(b.check_retention_policy(1, 86400).unwrap());
    }

    #[test]
    fn codebase_bridge_defaults() {
        let b = NoOpBridges;
        assert!(b.link_symbol(1, "my_func").is_err());
        assert!(b.find_related_code("topic", 5).is_empty());
        assert!(b.code_context("my_func").is_none());
    }

    #[test]
    fn comm_bridge_defaults() {
        let b = NoOpBridges;
        assert!(b.store_from_channel(1, "summary").is_err());
        assert!(b.notify_memory_event("add", 1).is_err());
    }

    #[test]
    fn bridge_config_defaults_all_false() {
        let cfg = BridgeConfig::default();
        assert!(!cfg.identity_enabled);
        assert!(!cfg.vision_enabled);
        assert!(!cfg.time_enabled);
        assert!(!cfg.contract_enabled);
        assert!(!cfg.codebase_enabled);
        assert!(!cfg.comm_enabled);
    }

    #[test]
    fn noop_bridges_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NoOpBridges>();
    }

    #[test]
    fn noop_bridges_default_and_clone() {
        let b = NoOpBridges::default();
        let _b2 = b.clone();
    }
}
