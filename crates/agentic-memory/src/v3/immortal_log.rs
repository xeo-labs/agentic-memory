//! The append-only immortal log. Never deletes. Never modifies. Only appends.

use super::block::{Block, BlockContent, BlockHash, BlockType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, Write};
use std::path::PathBuf;

/// The append-only immortal log.
/// Source of truth — everything written here is permanent.
pub struct ImmortalLog {
    /// Path to the log file
    path: PathBuf,

    /// File handle for writing
    file: File,

    /// Current write position
    write_pos: u64,

    /// Block count
    block_count: u64,

    /// Last block hash (for chaining)
    last_hash: BlockHash,

    /// Block offset index (sequence -> file offset)
    offsets: Vec<u64>,

    /// Content-address index (hash -> sequence)
    content_index: HashMap<BlockHash, u64>,
}

impl ImmortalLog {
    /// Create or open an immortal log
    pub fn open(path: PathBuf) -> Result<Self, std::io::Error> {
        let exists = path.exists() && std::fs::metadata(&path)?.len() > 0;

        if exists {
            Self::load_existing(path)
        } else {
            Self::create_new(path)
        }
    }

    fn create_new(path: PathBuf) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        Ok(Self {
            path,
            file,
            write_pos: 0,
            block_count: 0,
            last_hash: BlockHash::zero(),
            offsets: Vec::new(),
            content_index: HashMap::new(),
        })
    }

    fn load_existing(path: PathBuf) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new().read(true).write(true).open(&path)?;

        let mut log = Self {
            path,
            file,
            write_pos: 0,
            block_count: 0,
            last_hash: BlockHash::zero(),
            offsets: Vec::new(),
            content_index: HashMap::new(),
        };

        // Scan and rebuild indexes
        let read_file = OpenOptions::new().read(true).open(&log.path)?;
        let mut reader = BufReader::new(read_file);

        loop {
            let pos = reader.stream_position()?;

            // Read block length prefix (4 bytes)
            let mut len_buf = [0u8; 4];
            match reader.read_exact(&mut len_buf) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
            let block_len = u32::from_le_bytes(len_buf) as usize;

            if block_len == 0 {
                break;
            }

            // Read block data
            let mut block_data = vec![0u8; block_len];
            match reader.read_exact(&mut block_data) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }

            // Deserialize
            if let Ok(block) = serde_json::from_slice::<Block>(&block_data) {
                log.offsets.push(pos);
                log.content_index.insert(block.hash, block.sequence);
                log.last_hash = block.hash;
                log.block_count = block.sequence + 1;
            }

            log.write_pos = reader.stream_position()?;
        }

        Ok(log)
    }

    /// Append a block to the log
    pub fn append(
        &mut self,
        block_type: BlockType,
        content: BlockContent,
    ) -> Result<Block, std::io::Error> {
        let block = Block::new(self.last_hash, self.block_count, block_type, content);

        // Serialize
        let block_data = serde_json::to_vec(&block)?;
        let block_len = block_data.len() as u32;

        // Seek to write position and write length prefix + data
        self.file.seek(std::io::SeekFrom::Start(self.write_pos))?;
        self.file.write_all(&block_len.to_le_bytes())?;
        self.file.write_all(&block_data)?;
        self.file.flush()?;

        // Update indexes
        self.offsets.push(self.write_pos);
        self.content_index.insert(block.hash, block.sequence);
        self.last_hash = block.hash;
        self.block_count += 1;
        self.write_pos += 4 + block_data.len() as u64;

        Ok(block)
    }

    /// Get block by sequence number
    pub fn get(&self, sequence: u64) -> Option<Block> {
        let offset = *self.offsets.get(sequence as usize)?;
        self.read_block_at(offset)
    }

    /// Get block by hash
    pub fn get_by_hash(&self, hash: &BlockHash) -> Option<Block> {
        let sequence = *self.content_index.get(hash)?;
        self.get(sequence)
    }

    /// Read block at file offset
    fn read_block_at(&self, offset: u64) -> Option<Block> {
        let file = OpenOptions::new().read(true).open(&self.path).ok()?;
        let mut reader = BufReader::new(file);
        reader.seek(std::io::SeekFrom::Start(offset)).ok()?;

        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).ok()?;
        let block_len = u32::from_le_bytes(len_buf) as usize;

        if block_len == 0 {
            return None;
        }

        let mut block_data = vec![0u8; block_len];
        reader.read_exact(&mut block_data).ok()?;

        serde_json::from_slice(&block_data).ok()
    }

    /// Iterate over all blocks
    pub fn iter(&self) -> impl Iterator<Item = Block> + '_ {
        (0..self.block_count).filter_map(move |seq| self.get(seq))
    }

    /// Iterate over blocks in sequence range
    pub fn iter_range(&self, start: u64, end: u64) -> impl Iterator<Item = Block> + '_ {
        (start..std::cmp::min(end, self.block_count)).filter_map(move |seq| self.get(seq))
    }

    /// Get block count
    pub fn len(&self) -> u64 {
        self.block_count
    }

    /// Check if log is empty
    pub fn is_empty(&self) -> bool {
        self.block_count == 0
    }

    /// Get last block hash
    pub fn last_hash(&self) -> BlockHash {
        self.last_hash
    }

    /// Verify integrity of entire log
    pub fn verify_integrity(&self) -> IntegrityReport {
        let mut report = IntegrityReport {
            verified: true,
            blocks_checked: 0,
            chain_intact: true,
            missing_blocks: vec![],
            corrupted_blocks: vec![],
        };

        let mut expected_prev = BlockHash::zero();

        for seq in 0..self.block_count {
            match self.get(seq) {
                Some(block) => {
                    report.blocks_checked += 1;

                    if !block.verify() {
                        report.corrupted_blocks.push(seq);
                        report.verified = false;
                    }

                    if block.prev_hash != expected_prev {
                        report.chain_intact = false;
                        report.verified = false;
                    }

                    expected_prev = block.hash;
                }
                None => {
                    report.missing_blocks.push(seq);
                    report.verified = false;
                }
            }
        }

        report
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    pub verified: bool,
    pub blocks_checked: u64,
    pub chain_intact: bool,
    pub missing_blocks: Vec<u64>,
    pub corrupted_blocks: Vec<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_and_append() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.log");

        let mut log = ImmortalLog::open(path).unwrap();
        assert_eq!(log.len(), 0);

        let block = log
            .append(
                BlockType::UserMessage,
                BlockContent::Text {
                    text: "Hello".to_string(),
                    role: Some("user".to_string()),
                    tokens: None,
                },
            )
            .unwrap();

        assert_eq!(log.len(), 1);
        assert!(block.verify());
    }

    #[test]
    fn test_persistence() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.log");

        // Write
        {
            let mut log = ImmortalLog::open(path.clone()).unwrap();
            log.append(
                BlockType::UserMessage,
                BlockContent::Text {
                    text: "First".to_string(),
                    role: None,
                    tokens: None,
                },
            )
            .unwrap();
            log.append(
                BlockType::AssistantMessage,
                BlockContent::Text {
                    text: "Second".to_string(),
                    role: None,
                    tokens: None,
                },
            )
            .unwrap();
        }

        // Read back
        {
            let log = ImmortalLog::open(path).unwrap();
            assert_eq!(log.len(), 2);

            let b0 = log.get(0).unwrap();
            assert_eq!(b0.block_type, BlockType::UserMessage);

            let b1 = log.get(1).unwrap();
            assert_eq!(b1.block_type, BlockType::AssistantMessage);
            assert_eq!(b1.prev_hash, b0.hash);
        }
    }

    #[test]
    fn test_integrity_verification() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.log");

        let mut log = ImmortalLog::open(path).unwrap();
        for i in 0..10 {
            log.append(
                BlockType::UserMessage,
                BlockContent::Text {
                    text: format!("Message {}", i),
                    role: None,
                    tokens: None,
                },
            )
            .unwrap();
        }

        let report = log.verify_integrity();
        assert!(report.verified);
        assert_eq!(report.blocks_checked, 10);
        assert!(report.chain_intact);
    }
}
