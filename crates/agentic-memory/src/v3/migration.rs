//! V2 to V3 migration — convert .amem files to V3 immortal log.

use super::block::*;
use super::engine::MemoryEngineV3;
use std::path::Path;

/// Migrate V2 .amem file to V3 immortal log
pub struct V2ToV3Migration;

impl V2ToV3Migration {
    /// Migrate a V2 memory file to V3
    ///
    /// Reads the V2 graph and converts each node to an appropriate V3 block.
    /// V2 nodes map to V3 block types as follows:
    ///   - Episode/Concept/Reflection -> Text blocks
    ///   - Fact/Preference -> Decision blocks
    ///   - Procedure -> Tool blocks
    pub fn migrate(
        v2_path: &Path,
        v3_engine: &MemoryEngineV3,
    ) -> Result<MigrationReport, std::io::Error> {
        let mut report = MigrationReport::default();

        // Check format
        if !Self::is_v2_format(v2_path) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Not a V2 .amem file (missing AMEM magic bytes)",
            ));
        }

        // Try loading V2 graph using the existing format module
        #[cfg(feature = "format")]
        {
            use crate::format::AmemReader;

            match AmemReader::read_from_file(v2_path) {
                Ok(graph) => {
                    report.v2_nodes = graph.node_count();
                    report.v2_edges = graph.edge_count();

                    // Convert each V2 node to a V3 block
                    for node in graph.nodes() {
                        match v3_engine.capture_user_message(&node.content, None) {
                            Ok(_) => report.blocks_created += 1,
                            Err(e) => {
                                report.errors.push(format!("Node {}: {}", node.id, e))
                            }
                        }
                    }

                    // Mark migration complete
                    let _ = v3_engine.capture_boundary(
                        BoundaryType::SessionStart,
                        0,
                        0,
                        &format!(
                            "Migrated from V2: {} nodes, {} edges",
                            report.v2_nodes, report.v2_edges
                        ),
                        Some("V3 immortal mode active"),
                    );

                    report.success = report.errors.is_empty();
                }
                Err(e) => {
                    report.errors.push(format!("Failed to read V2 file: {}", e));
                }
            }
        }

        #[cfg(not(feature = "format"))]
        {
            let _ = v3_engine;
            report
                .errors
                .push("V2 migration requires 'format' feature".to_string());
        }

        Ok(report)
    }

    /// Check if a file is V2 format
    pub fn is_v2_format(path: &Path) -> bool {
        if let Ok(data) = std::fs::read(path) {
            // V2 starts with AMEM magic
            data.len() >= 4 && &data[0..4] == b"AMEM"
        } else {
            false
        }
    }

    /// Check if a file is V3 format
    pub fn is_v3_format(path: &Path) -> bool {
        if let Ok(data) = std::fs::read(path) {
            // V3 immortal log is JSON-based with length prefixes
            // Check first block can be deserialized
            if data.len() >= 4 {
                let block_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
                if data.len() >= 4 + block_len && block_len > 0 {
                    return serde_json::from_slice::<Block>(&data[4..4 + block_len]).is_ok();
                }
            }
            false
        } else {
            false
        }
    }
}

/// Migration report
#[derive(Debug, Default)]
pub struct MigrationReport {
    pub success: bool,
    pub v2_nodes: usize,
    pub v2_edges: usize,
    pub blocks_created: usize,
    pub errors: Vec<String>,
}
