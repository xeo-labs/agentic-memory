//! Storage budget management and projection.
//!
//! Default budget: 10 GB per project, allocated across layers.
//! Alerts at 80% (warning), 95% (critical).

use super::hierarchy::{HierarchyStats, MemoryLayer};
use super::store::{LongevityError, LongevityStore};
use serde::{Deserialize, Serialize};

/// Budget allocation for a single layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerBudget {
    pub layer: String,
    pub allocated_bytes: u64,
    pub used_bytes: u64,
    pub used_percent: f64,
    pub status: BudgetStatus,
}

/// Budget alert level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetAlert {
    Healthy,
    Warning,
    Critical,
}

/// Budget status string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub alert: BudgetAlert,
    pub message: String,
}

/// Storage projection for future growth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageProjection {
    pub current_bytes: u64,
    pub daily_growth_bytes: u64,
    pub projected_1_year: u64,
    pub projected_5_year: u64,
    pub projected_10_year: u64,
    pub projected_20_year: u64,
    pub budget_exceeded_in_days: Option<u64>,
}

/// Default layer budget allocation percentages.
const LAYER_ALLOCATIONS: [(MemoryLayer, f64); 6] = [
    (MemoryLayer::Raw, 0.15),
    (MemoryLayer::Episode, 0.25),
    (MemoryLayer::Summary, 0.25),
    (MemoryLayer::Pattern, 0.20),
    (MemoryLayer::Trait, 0.10),
    (MemoryLayer::Identity, 0.05),
];

/// The storage budget manager.
pub struct StorageBudget {
    /// Total budget in bytes (default: 10 GB)
    pub total_budget_bytes: u64,
}

impl StorageBudget {
    /// Create with default budget (10 GB).
    pub fn new() -> Self {
        Self {
            total_budget_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
        }
    }

    /// Create with custom budget.
    pub fn with_budget(budget_bytes: u64) -> Self {
        Self {
            total_budget_bytes: budget_bytes,
        }
    }

    /// Get budget allocation and usage for each layer.
    pub fn layer_budgets(
        &self,
        stats: &HierarchyStats,
    ) -> Vec<LayerBudget> {
        LAYER_ALLOCATIONS
            .iter()
            .map(|(layer, fraction)| {
                let allocated = (self.total_budget_bytes as f64 * fraction) as u64;
                let used = stats.bytes_for_layer(*layer);
                let used_percent = if allocated > 0 {
                    (used as f64 / allocated as f64) * 100.0
                } else {
                    0.0
                };

                let alert = if used_percent >= 95.0 {
                    BudgetAlert::Critical
                } else if used_percent >= 80.0 {
                    BudgetAlert::Warning
                } else {
                    BudgetAlert::Healthy
                };

                let message = match alert {
                    BudgetAlert::Critical => format!(
                        "CRITICAL: {} layer at {:.1}% — emergency compression needed",
                        layer, used_percent
                    ),
                    BudgetAlert::Warning => format!(
                        "WARNING: {} layer at {:.1}% — accelerate consolidation",
                        layer, used_percent
                    ),
                    BudgetAlert::Healthy => format!(
                        "{} layer at {:.1}% — healthy",
                        layer, used_percent
                    ),
                };

                LayerBudget {
                    layer: layer.to_string(),
                    allocated_bytes: allocated,
                    used_bytes: used,
                    used_percent,
                    status: BudgetStatus { alert, message },
                }
            })
            .collect()
    }

    /// Get the overall budget status.
    pub fn overall_status(&self, stats: &HierarchyStats) -> BudgetStatus {
        let used_percent =
            (stats.total_bytes as f64 / self.total_budget_bytes as f64) * 100.0;

        let alert = if used_percent >= 95.0 {
            BudgetAlert::Critical
        } else if used_percent >= 80.0 {
            BudgetAlert::Warning
        } else {
            BudgetAlert::Healthy
        };

        let message = match alert {
            BudgetAlert::Critical => format!(
                "CRITICAL: Total storage at {:.1}% ({} / {})",
                used_percent,
                format_bytes(stats.total_bytes),
                format_bytes(self.total_budget_bytes)
            ),
            BudgetAlert::Warning => format!(
                "WARNING: Total storage at {:.1}% ({} / {})",
                used_percent,
                format_bytes(stats.total_bytes),
                format_bytes(self.total_budget_bytes)
            ),
            BudgetAlert::Healthy => format!(
                "Healthy: Total storage at {:.1}% ({} / {})",
                used_percent,
                format_bytes(stats.total_bytes),
                format_bytes(self.total_budget_bytes)
            ),
        };

        BudgetStatus { alert, message }
    }

    /// Project storage growth over time.
    pub fn project_growth(
        &self,
        store: &LongevityStore,
        project_id: &str,
    ) -> Result<StorageProjection, LongevityError> {
        let stats = store.hierarchy_stats(project_id)?;
        let db_size = store.database_size_bytes()?;

        // Estimate daily growth from current data
        // Use total memories / age in days as rough estimate
        let total_count = stats.total_count;
        let daily_growth_bytes = if total_count > 0 {
            // Assume ~1 KB per memory average, estimate from current data
            let avg_bytes_per_memory = if stats.total_bytes > 0 {
                stats.total_bytes / total_count
            } else {
                1024
            };
            // Rough estimate: 50 memories per day for active use
            avg_bytes_per_memory * 50
        } else {
            // Default estimate: ~50 KB/day for active developer
            50 * 1024
        };

        // Apply compression ratios for projections
        let year_1 = db_size + daily_growth_bytes * 365;
        let year_5 = year_1 + (daily_growth_bytes * 365 * 4) / 5; // Episodes compress 5:1
        let year_10 = year_5 + (daily_growth_bytes * 365 * 5) / 10; // Summaries 10:1
        let year_20 = year_10 + (daily_growth_bytes * 365 * 10) / 20; // Patterns 20:1

        let budget_exceeded_in_days = if daily_growth_bytes > 0 && db_size < self.total_budget_bytes
        {
            Some((self.total_budget_bytes - db_size) / daily_growth_bytes)
        } else if db_size >= self.total_budget_bytes {
            Some(0)
        } else {
            None
        };

        Ok(StorageProjection {
            current_bytes: db_size,
            daily_growth_bytes,
            projected_1_year: year_1,
            projected_5_year: year_5,
            projected_10_year: year_10,
            projected_20_year: year_20,
            budget_exceeded_in_days,
        })
    }
}

impl Default for StorageBudget {
    fn default() -> Self {
        Self::new()
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
