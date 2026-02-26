//! Tiered storage: Hot -> Warm -> Cold -> Frozen.

use super::block::Block;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Tiered storage configuration
#[derive(Clone, Debug)]
pub struct TierConfig {
    /// Hot tier: blocks younger than this
    pub hot_threshold: Duration,
    /// Warm tier: blocks younger than this (but older than hot)
    pub warm_threshold: Duration,
    /// Cold tier: blocks younger than this (but older than warm)
    pub cold_threshold: Duration,
    /// Maximum hot tier size in bytes
    pub hot_max_bytes: usize,
    /// Maximum warm tier size in bytes
    pub warm_max_bytes: usize,
}

impl Default for TierConfig {
    fn default() -> Self {
        Self {
            hot_threshold: Duration::from_secs(24 * 60 * 60), // 24 hours
            warm_threshold: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            cold_threshold: Duration::from_secs(365 * 24 * 60 * 60), // 1 year
            hot_max_bytes: 10 * 1024 * 1024,                  // 10 MB
            warm_max_bytes: 100 * 1024 * 1024,                // 100 MB
        }
    }
}

/// Tiered storage: Hot -> Warm -> Cold -> Frozen
pub struct TieredStorage {
    hot: HashMap<u64, Block>,
    hot_bytes: usize,
    warm: HashMap<u64, Block>,
    warm_bytes: usize,
    cold: HashMap<u64, Vec<u8>>,
    cold_bytes: usize,
    frozen: HashMap<u64, Vec<u8>>,
    config: TierConfig,
}

impl TieredStorage {
    pub fn new(config: TierConfig) -> Self {
        Self {
            hot: HashMap::new(),
            hot_bytes: 0,
            warm: HashMap::new(),
            warm_bytes: 0,
            cold: HashMap::new(),
            cold_bytes: 0,
            frozen: HashMap::new(),
            config,
        }
    }

    /// Store a block (goes to hot tier)
    pub fn store(&mut self, block: Block) {
        self.hot_bytes += block.size_bytes as usize;
        self.hot.insert(block.sequence, block);
        self.maybe_demote();
    }

    /// Retrieve a block from any tier
    pub fn get(&self, sequence: u64) -> Option<Block> {
        self.hot
            .get(&sequence)
            .cloned()
            .or_else(|| self.warm.get(&sequence).cloned())
            .or_else(|| {
                self.cold
                    .get(&sequence)
                    .and_then(|data| serde_json::from_slice(data).ok())
            })
            .or_else(|| {
                self.frozen
                    .get(&sequence)
                    .and_then(|data| serde_json::from_slice(data).ok())
            })
    }

    /// Check if we need to demote blocks to lower tiers
    fn maybe_demote(&mut self) {
        let now = Utc::now();

        // Demote from hot to warm
        if self.hot_bytes > self.config.hot_max_bytes {
            let to_demote: Vec<u64> = self
                .hot
                .iter()
                .filter(|(_, b)| {
                    let age = now.signed_duration_since(b.timestamp);
                    age.num_seconds() > self.config.hot_threshold.as_secs() as i64
                })
                .map(|(&seq, _)| seq)
                .collect();

            for seq in to_demote {
                if let Some(block) = self.hot.remove(&seq) {
                    self.hot_bytes -= block.size_bytes as usize;
                    self.warm_bytes += block.size_bytes as usize;
                    self.warm.insert(seq, block);
                }
            }
        }

        // Demote from warm to cold
        if self.warm_bytes > self.config.warm_max_bytes {
            let to_demote: Vec<u64> = self
                .warm
                .iter()
                .filter(|(_, b)| {
                    let age = now.signed_duration_since(b.timestamp);
                    age.num_seconds() > self.config.warm_threshold.as_secs() as i64
                })
                .map(|(&seq, _)| seq)
                .collect();

            for seq in to_demote {
                if let Some(block) = self.warm.remove(&seq) {
                    self.warm_bytes -= block.size_bytes as usize;
                    let data = serde_json::to_vec(&block).unwrap_or_default();
                    self.cold_bytes += data.len();
                    self.cold.insert(seq, data);
                }
            }
        }
    }

    /// Force archive old blocks to frozen tier
    pub fn archive_old(&mut self, older_than: DateTime<Utc>) {
        let cold_to_freeze: Vec<u64> = self.cold.keys().cloned().collect();

        for seq in cold_to_freeze {
            if let Some(data) = self.cold.get(&seq) {
                if let Ok(block) = serde_json::from_slice::<Block>(data) {
                    if block.timestamp < older_than {
                        let data = self.cold.remove(&seq).unwrap();
                        self.cold_bytes -= data.len();
                        self.frozen.insert(seq, data);
                    }
                }
            }
        }
    }

    /// Get storage statistics
    pub fn stats(&self) -> TierStats {
        TierStats {
            hot_blocks: self.hot.len(),
            hot_bytes: self.hot_bytes,
            warm_blocks: self.warm.len(),
            warm_bytes: self.warm_bytes,
            cold_blocks: self.cold.len(),
            cold_bytes: self.cold_bytes,
            frozen_blocks: self.frozen.len(),
        }
    }

    /// Total bytes across all tiers
    pub fn total_bytes(&self) -> usize {
        self.hot_bytes + self.warm_bytes + self.cold_bytes
    }

    /// Check memory pressure and evict if needed
    pub fn check_memory_pressure(&mut self, max_memory_bytes: usize) {
        let total = self.total_bytes();
        let pressure = total as f64 / max_memory_bytes.max(1) as f64;

        if pressure > 0.9 {
            log::warn!(
                "Memory pressure at {:.1}%, forcing eviction",
                pressure * 100.0
            );
            self.force_eviction(max_memory_bytes, 0.7);
        } else if pressure > 0.8 {
            self.maybe_demote();
        }
    }

    /// Force eviction to reduce memory to target ratio
    pub fn force_eviction(&mut self, max_memory_bytes: usize, target_ratio: f64) {
        let target = (max_memory_bytes as f64 * target_ratio) as usize;

        // Evict oldest from hot to warm
        while self.hot_bytes > target / 3 && !self.hot.is_empty() {
            // Find oldest block in hot tier
            if let Some((&oldest_seq, _)) = self.hot.iter().min_by_key(|(_, b)| b.timestamp) {
                if let Some(block) = self.hot.remove(&oldest_seq) {
                    self.hot_bytes -= block.size_bytes as usize;
                    self.warm_bytes += block.size_bytes as usize;
                    self.warm.insert(oldest_seq, block);
                }
            } else {
                break;
            }
        }

        // Evict oldest from warm to cold
        while self.warm_bytes > target / 3 && !self.warm.is_empty() {
            if let Some((&oldest_seq, _)) = self.warm.iter().min_by_key(|(_, b)| b.timestamp) {
                if let Some(block) = self.warm.remove(&oldest_seq) {
                    self.warm_bytes -= block.size_bytes as usize;
                    let data = serde_json::to_vec(&block).unwrap_or_default();
                    self.cold_bytes += data.len();
                    self.cold.insert(oldest_seq, data);
                }
            } else {
                break;
            }
        }

        log::info!(
            "After eviction: hot={}KB, warm={}KB, cold={}KB",
            self.hot_bytes / 1024,
            self.warm_bytes / 1024,
            self.cold_bytes / 1024
        );
    }

    /// Get count of total blocks across all tiers
    pub fn total_blocks(&self) -> usize {
        self.hot.len() + self.warm.len() + self.cold.len() + self.frozen.len()
    }
}

/// Storage tier statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierStats {
    pub hot_blocks: usize,
    pub hot_bytes: usize,
    pub warm_blocks: usize,
    pub warm_bytes: usize,
    pub cold_blocks: usize,
    pub cold_bytes: usize,
    pub frozen_blocks: usize,
}
