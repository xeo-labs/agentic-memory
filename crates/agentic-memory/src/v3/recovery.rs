//! Crash recovery and write-ahead logging.

use super::block::Block;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Write-ahead log for crash recovery
pub struct WriteAheadLog {
    path: PathBuf,
    file: File,
    sequence: u64,
}

impl WriteAheadLog {
    /// Open or create WAL
    pub fn open(dir: &Path) -> Result<Self, std::io::Error> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("memory.wal");

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;

        let mut wal = Self {
            path,
            file,
            sequence: 0,
        };

        // Recover sequence number
        wal.recover_sequence()?;

        Ok(wal)
    }

    /// Write entry to WAL (before main log)
    pub fn write(&mut self, block: &Block) -> Result<(), std::io::Error> {
        let data = serde_json::to_vec(block)?;

        // Write: sequence (8) + length (4) + data + checksum (4)
        let checksum = crc32fast::hash(&data);

        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(&self.sequence.to_le_bytes())?;
        self.file.write_all(&(data.len() as u32).to_le_bytes())?;
        self.file.write_all(&data)?;
        self.file.write_all(&checksum.to_le_bytes())?;
        self.file.sync_all()?;

        self.sequence += 1;
        Ok(())
    }

    /// Mark entry as committed (can be garbage collected)
    pub fn commit(&mut self, _sequence: u64) -> Result<(), std::io::Error> {
        // In a full implementation, we'd track committed entries
        // and periodically truncate the WAL
        Ok(())
    }

    /// Recover uncommitted entries after crash (handles WAL corruption gracefully)
    pub fn recover(&self) -> Result<Vec<Block>, std::io::Error> {
        let mut entries = Vec::new();
        let mut skipped = 0u32;

        let file = match OpenOptions::new().read(true).open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e),
        };

        let file_len = file.metadata()?.len();
        if file_len == 0 {
            return Ok(vec![]);
        }

        let mut reader = BufReader::new(file);

        loop {
            let pos = reader.stream_position().unwrap_or(file_len);
            if pos >= file_len {
                break;
            }

            // Read sequence
            let mut seq_buf = [0u8; 8];
            match reader.read_exact(&mut seq_buf) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }

            // Read length
            let mut len_buf = [0u8; 4];
            match reader.read_exact(&mut len_buf) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
            let len = u32::from_le_bytes(len_buf) as usize;

            // Sanity check length (max 100MB per entry)
            if len > 100 * 1024 * 1024 {
                log::warn!("WAL entry at position {} has unreasonable length {}, skipping", pos, len);
                skipped += 1;
                // Try to find next valid entry by scanning forward
                if self.try_skip_to_next_entry(&mut reader, file_len).is_err() {
                    break;
                }
                continue;
            }

            // Read data
            let mut data = vec![0u8; len];
            match reader.read_exact(&mut data) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }

            // Read and verify checksum
            let mut checksum_buf = [0u8; 4];
            match reader.read_exact(&mut checksum_buf) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
            let stored_checksum = u32::from_le_bytes(checksum_buf);
            let computed_checksum = crc32fast::hash(&data);

            if stored_checksum == computed_checksum {
                if let Ok(block) = serde_json::from_slice::<Block>(&data) {
                    if block.verify() {
                        entries.push(block);
                    } else {
                        log::warn!("WAL entry at position {} failed block verification, skipping", pos);
                        skipped += 1;
                    }
                } else {
                    log::warn!("WAL entry at position {} failed deserialization, skipping", pos);
                    skipped += 1;
                }
            } else {
                log::warn!(
                    "WAL checksum mismatch at position {} (stored={:#x}, computed={:#x}), skipping",
                    pos, stored_checksum, computed_checksum
                );
                skipped += 1;
            }
        }

        if skipped > 0 {
            log::warn!("WAL recovery skipped {} corrupt entries, recovered {}", skipped, entries.len());
        }

        Ok(entries)
    }

    /// Try to find next valid WAL entry after corruption
    fn try_skip_to_next_entry(&self, reader: &mut BufReader<File>, file_len: u64) -> Result<(), std::io::Error> {
        // Scan byte-by-byte looking for a reasonable sequence number
        let mut byte = [0u8; 1];
        let scan_limit = 1024; // Don't scan more than 1KB ahead
        let mut scanned = 0;

        while scanned < scan_limit {
            let pos = reader.stream_position()?;
            if pos + 16 >= file_len {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "End of WAL"));
            }

            match reader.read_exact(&mut byte) {
                Ok(_) => scanned += 1,
                Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "End of WAL")),
            }

            // Try reading a sequence number at current position
            let current_pos = reader.stream_position()?;
            if current_pos + 12 < file_len {
                // Peek at potential entry
                let mut peek_seq = [0u8; 8];
                let mut peek_len = [0u8; 4];
                if reader.read_exact(&mut peek_seq).is_ok() && reader.read_exact(&mut peek_len).is_ok() {
                    let seq = u64::from_le_bytes(peek_seq);
                    let len = u32::from_le_bytes(peek_len) as usize;
                    // Reasonable sequence number and length?
                    if seq < 1_000_000_000 && len > 0 && len < 100 * 1024 * 1024 {
                        // Seek back to start of this potential entry
                        // (current_pos is always >= 1 here since we read a byte,
                        // but use saturating_sub as safety)
                        reader.seek(SeekFrom::Start(current_pos.saturating_sub(1)))?;
                        return Ok(());
                    }
                }
                // Seek back to continue scanning
                reader.seek(SeekFrom::Start(current_pos))?;
            }
        }

        Err(std::io::Error::new(std::io::ErrorKind::Other, "Could not find next valid WAL entry"))
    }

    /// Clear WAL after successful checkpoint
    pub fn clear(&mut self) -> Result<(), std::io::Error> {
        self.file.set_len(0)?;
        self.file.seek(SeekFrom::Start(0))?;
        self.sequence = 0;
        Ok(())
    }

    fn recover_sequence(&mut self) -> Result<(), std::io::Error> {
        let metadata = self.file.metadata()?;
        if metadata.len() == 0 {
            return Ok(());
        }

        let file = OpenOptions::new().read(true).open(&self.path)?;
        let mut reader = BufReader::new(file);
        let mut max_seq = 0u64;

        loop {
            let mut seq_buf = [0u8; 8];
            match reader.read_exact(&mut seq_buf) {
                Ok(_) => {
                    let seq = u64::from_le_bytes(seq_buf);
                    // Sanity check: reject unreasonable sequence numbers
                    if seq > 1_000_000_000 {
                        break;
                    }
                    max_seq = max_seq.max(seq);

                    // Skip rest of entry
                    let mut len_buf = [0u8; 4];
                    if reader.read_exact(&mut len_buf).is_err() {
                        break;
                    }
                    let len = u32::from_le_bytes(len_buf) as usize;

                    // Sanity check: reject unreasonable lengths (max 100MB per entry)
                    if len > 100 * 1024 * 1024 {
                        break;
                    }

                    let mut skip = vec![0u8; len + 4]; // data + checksum
                    if reader.read_exact(&mut skip).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        self.sequence = if metadata.len() > 0 {
            max_seq.saturating_add(1)
        } else {
            0
        };
        Ok(())
    }
}

/// Recovery manager wrapping WAL
pub struct RecoveryManager {
    wal: WriteAheadLog,
}

impl RecoveryManager {
    pub fn new(data_dir: &Path) -> Result<Self, std::io::Error> {
        Ok(Self {
            wal: WriteAheadLog::open(data_dir)?,
        })
    }

    /// Call before writing to main log
    pub fn pre_write(&mut self, block: &Block) -> Result<(), std::io::Error> {
        self.wal.write(block)
    }

    /// Call after successful write to main log
    pub fn post_write(&mut self, sequence: u64) -> Result<(), std::io::Error> {
        self.wal.commit(sequence)
    }

    /// Recover any uncommitted writes
    pub fn recover(&self) -> Result<Vec<Block>, std::io::Error> {
        self.wal.recover()
    }

    /// Checkpoint: clear WAL
    pub fn checkpoint(&mut self) -> Result<(), std::io::Error> {
        self.wal.clear()
    }
}
