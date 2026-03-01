//! Transport-level write-ahead capture for raw JSON-RPC bytes.
//!
//! Phase 1 guarantee: capture inbound/outbound payloads before parse/send.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

const MAGIC: &[u8; 4] = b"MWAL";
const VERSION: u16 = 1;
const HEADER_BYTES: usize = 64;
const ENTRY_FIXED_BYTES: usize = 45; // len + checksum + ts + seq + dir + session + data_len
const MAX_ENTRY_BYTES: usize = 16 * 1024 * 1024;

const ENV_CAPTURE_ENABLED: &str = "AMEM_TRANSPORT_CAPTURE";
const ENV_CAPTURE_DIR: &str = "AMEM_TRANSPORT_WAL_DIR";
const ENV_CAPTURE_SYNC: &str = "AMEM_TRANSPORT_CAPTURE_SYNC";
const ENV_CAPTURE_BATCH: &str = "AMEM_TRANSPORT_CAPTURE_BATCH";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Inbound = 0,
    Outbound = 1,
}

impl Direction {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Inbound),
            1 => Some(Self::Outbound),
            _ => None,
        }
    }
}

/// Public direction type for decoded transport WAL entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureDirection {
    Inbound,
    Outbound,
}

impl CaptureDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
        }
    }
}

impl From<Direction> for CaptureDirection {
    fn from(value: Direction) -> Self {
        match value {
            Direction::Inbound => Self::Inbound,
            Direction::Outbound => Self::Outbound,
        }
    }
}

/// Decoded transport WAL entry.
#[derive(Debug, Clone)]
pub struct CapturedTransportEntry {
    pub timestamp_nanos: i64,
    pub sequence: u64,
    pub direction: CaptureDirection,
    pub session_id: [u8; 16],
    pub data: Vec<u8>,
}

impl CapturedTransportEntry {
    pub fn session_id_string(&self) -> String {
        uuid::Uuid::from_bytes(self.session_id).to_string()
    }
}

/// Basic status information for a transport WAL file.
#[derive(Debug, Clone)]
pub struct CaptureWalStatus {
    pub wal_path: PathBuf,
    pub exists: bool,
    pub bytes: u64,
    pub entries: u64,
    pub next_sequence: u64,
    pub session_id: Option<[u8; 16]>,
}

#[derive(Debug, Clone, Copy)]
enum SyncMode {
    EveryMessage,
    Batched(usize),
}

impl SyncMode {
    fn from_env() -> Self {
        let mode = std::env::var(ENV_CAPTURE_SYNC)
            .unwrap_or_else(|_| "every".to_string())
            .to_ascii_lowercase();
        if mode == "batched" {
            let batch = std::env::var(ENV_CAPTURE_BATCH)
                .ok()
                .and_then(|raw| raw.parse::<usize>().ok())
                .filter(|n| *n > 0)
                .unwrap_or(32);
            Self::Batched(batch)
        } else {
            Self::EveryMessage
        }
    }
}

#[derive(Debug)]
struct WalEntry {
    timestamp_nanos: i64,
    sequence: u64,
    direction: Direction,
    session_id: [u8; 16],
    data: Vec<u8>,
}

impl WalEntry {
    fn encode(&self) -> Vec<u8> {
        let checksum = crc32fast::hash(&self.data);
        let total_len = ENTRY_FIXED_BYTES + self.data.len();

        let mut out = Vec::with_capacity(total_len);
        out.extend_from_slice(&(total_len as u32).to_le_bytes());
        out.extend_from_slice(&checksum.to_le_bytes());
        out.extend_from_slice(&self.timestamp_nanos.to_le_bytes());
        out.extend_from_slice(&self.sequence.to_le_bytes());
        out.push(self.direction as u8);
        out.extend_from_slice(&self.session_id);
        out.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.data);
        out
    }
}

#[derive(Debug, Clone, Copy)]
struct RecoverySummary {
    entries: u64,
    next_sequence: u64,
    truncate_to: u64,
}

/// Transport capture wrapper.
///
/// When disabled, methods are no-ops.
pub(crate) enum TransportCapture {
    Disabled,
    Enabled(EnabledCapture),
}

pub(crate) struct EnabledCapture {
    wal_path: PathBuf,
    wal: File,
    session_id: [u8; 16],
    sequence: u64,
    sync_mode: SyncMode,
    since_sync: usize,
}

impl TransportCapture {
    /// Construct capture based on environment:
    /// - `AMEM_TRANSPORT_CAPTURE=false|0|off` disables capture
    /// - `AMEM_TRANSPORT_WAL_DIR` overrides wal directory
    pub fn from_env() -> std::io::Result<Self> {
        if !capture_enabled() {
            tracing::info!("Transport capture disabled via {ENV_CAPTURE_ENABLED}");
            return Ok(Self::Disabled);
        }

        let dir = capture_dir_from_env();
        let sync_mode = SyncMode::from_env();
        Self::open_enabled(dir, sync_mode)
    }

    fn open_enabled(dir: PathBuf, sync_mode: SyncMode) -> std::io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let wal_path = dir.join("transport.wal");

        let mut wal = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&wal_path)?;

        if wal.metadata()?.len() == 0 {
            let session_id = *Uuid::new_v4().as_bytes();
            write_header(&mut wal, session_id)?;
            wal.seek(SeekFrom::End(0))?;
            tracing::info!(
                "Transport capture initialized at {} (new WAL)",
                wal_path.display()
            );
            return Ok(Self::Enabled(EnabledCapture {
                wal_path,
                wal,
                session_id,
                sequence: 0,
                sync_mode,
                since_sync: 0,
            }));
        }

        let header = read_header(&mut wal)?;
        let recovery = recover_entries(&wal_path)?;
        if recovery.truncate_to < wal.metadata()?.len() {
            wal.set_len(recovery.truncate_to)?;
        }
        wal.seek(SeekFrom::End(0))?;

        tracing::info!(
            "Transport capture initialized at {} (recovered {} entries, next_seq={})",
            wal_path.display(),
            recovery.entries,
            recovery.next_sequence
        );

        Ok(Self::Enabled(EnabledCapture {
            wal_path,
            wal,
            session_id: header.session_id,
            sequence: recovery.next_sequence,
            sync_mode,
            since_sync: 0,
        }))
    }

    pub fn capture_inbound(&mut self, raw: &[u8]) -> std::io::Result<()> {
        match self {
            Self::Disabled => Ok(()),
            Self::Enabled(inner) => inner.capture(Direction::Inbound, raw),
        }
    }

    pub fn capture_outbound(&mut self, raw: &[u8]) -> std::io::Result<()> {
        match self {
            Self::Disabled => Ok(()),
            Self::Enabled(inner) => inner.capture(Direction::Outbound, raw),
        }
    }

    pub fn sync(&mut self) -> std::io::Result<()> {
        match self {
            Self::Disabled => Ok(()),
            Self::Enabled(inner) => inner.flush_sync(),
        }
    }

    #[cfg(test)]
    fn for_tests(dir: &Path) -> std::io::Result<Self> {
        Self::open_enabled(dir.to_path_buf(), SyncMode::EveryMessage)
    }
}

impl EnabledCapture {
    fn capture(&mut self, direction: Direction, raw: &[u8]) -> std::io::Result<()> {
        if raw.is_empty() {
            return Ok(());
        }

        let entry = WalEntry {
            timestamp_nanos: now_unix_nanos(),
            sequence: self.sequence,
            direction,
            session_id: self.session_id,
            data: raw.to_vec(),
        };
        self.sequence = self.sequence.saturating_add(1);

        self.wal.seek(SeekFrom::End(0))?;
        self.wal.write_all(&entry.encode())?;

        match self.sync_mode {
            SyncMode::EveryMessage => self.flush_sync()?,
            SyncMode::Batched(batch) => {
                self.since_sync = self.since_sync.saturating_add(1);
                if self.since_sync >= batch {
                    self.flush_sync()?;
                }
            }
        }
        Ok(())
    }

    fn flush_sync(&mut self) -> std::io::Result<()> {
        self.wal.flush()?;
        self.wal.sync_all()?;
        self.since_sync = 0;
        Ok(())
    }
}

impl Drop for EnabledCapture {
    fn drop(&mut self) {
        if let Err(err) = self.flush_sync() {
            tracing::warn!(
                "Transport capture final sync failed ({}): {}",
                self.wal_path.display(),
                err
            );
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct WalHeader {
    session_id: [u8; 16],
}

fn write_header(file: &mut File, session_id: [u8; 16]) -> std::io::Result<()> {
    let mut header = [0u8; HEADER_BYTES];
    header[0..4].copy_from_slice(MAGIC);
    header[4..6].copy_from_slice(&VERSION.to_le_bytes());
    header[6..22].copy_from_slice(&session_id);
    header[22..30].copy_from_slice(&now_unix_secs().to_le_bytes());
    header[30..34].copy_from_slice(&0u32.to_le_bytes()); // flags
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&header)?;
    file.sync_all()?;
    Ok(())
}

fn read_header(file: &mut File) -> std::io::Result<WalHeader> {
    let mut header = [0u8; HEADER_BYTES];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut header)?;

    if &header[0..4] != MAGIC {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid transport WAL magic",
        ));
    }

    let version = u16::from_le_bytes([header[4], header[5]]);
    if version != VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unsupported transport WAL version: {version}"),
        ));
    }

    let mut session_id = [0u8; 16];
    session_id.copy_from_slice(&header[6..22]);
    Ok(WalHeader { session_id })
}

fn recover_entries(path: &Path) -> std::io::Result<RecoverySummary> {
    let file = OpenOptions::new().read(true).open(path)?;
    let file_len = file.metadata()?.len();
    if file_len <= HEADER_BYTES as u64 {
        return Ok(RecoverySummary {
            entries: 0,
            next_sequence: 0,
            truncate_to: HEADER_BYTES as u64,
        });
    }

    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(HEADER_BYTES as u64))?;

    let mut entries = 0u64;
    let mut next_sequence = 0u64;
    let mut last_good_pos = HEADER_BYTES as u64;

    loop {
        let entry_start = reader.stream_position()?;
        if entry_start >= file_len {
            break;
        }

        let mut len_buf = [0u8; 4];
        match reader.read_exact(&mut len_buf) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
        let total_len = u32::from_le_bytes(len_buf) as usize;
        if !(ENTRY_FIXED_BYTES..=MAX_ENTRY_BYTES).contains(&total_len) {
            break;
        }

        let remaining = total_len.saturating_sub(4);
        let mut body = vec![0u8; remaining];
        match reader.read_exact(&mut body) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }

        if body.len() < (ENTRY_FIXED_BYTES - 4) {
            break;
        }

        let checksum = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let sequence = u64::from_le_bytes([
            body[12], body[13], body[14], body[15], body[16], body[17], body[18], body[19],
        ]);
        let direction = body[20];
        let data_len = u32::from_le_bytes([body[37], body[38], body[39], body[40]]) as usize;
        let data_offset = 41usize;
        if body.len() < data_offset + data_len {
            break;
        }
        if Direction::from_u8(direction).is_none() {
            break;
        }

        let data = &body[data_offset..data_offset + data_len];
        if crc32fast::hash(data) != checksum {
            break;
        }

        entries = entries.saturating_add(1);
        next_sequence = sequence.saturating_add(1);
        last_good_pos = entry_start.saturating_add(total_len as u64);
    }

    Ok(RecoverySummary {
        entries,
        next_sequence,
        truncate_to: last_good_pos,
    })
}

fn capture_enabled() -> bool {
    match std::env::var(ENV_CAPTURE_ENABLED) {
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
        Err(_) => true,
    }
}

fn capture_dir_from_env() -> PathBuf {
    if let Ok(raw) = std::env::var(ENV_CAPTURE_DIR) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".agentic")
            .join("memory")
            .join("transport-wal");
    }

    PathBuf::from(".agentic/memory/transport-wal")
}

/// Resolve the default transport capture directory from environment.
pub fn default_capture_dir() -> PathBuf {
    capture_dir_from_env()
}

/// Resolve the default transport WAL path.
pub fn default_wal_path() -> PathBuf {
    default_capture_dir().join("transport.wal")
}

/// Read basic status for a transport WAL file.
pub fn wal_status(path: &Path) -> std::io::Result<CaptureWalStatus> {
    if !path.exists() {
        return Ok(CaptureWalStatus {
            wal_path: path.to_path_buf(),
            exists: false,
            bytes: 0,
            entries: 0,
            next_sequence: 0,
            session_id: None,
        });
    }

    let mut file = OpenOptions::new().read(true).open(path)?;
    let bytes = file.metadata()?.len();
    let session_id = if bytes >= HEADER_BYTES as u64 {
        Some(read_header(&mut file)?.session_id)
    } else {
        None
    };
    let summary = recover_entries(path)?;

    Ok(CaptureWalStatus {
        wal_path: path.to_path_buf(),
        exists: true,
        bytes,
        entries: summary.entries,
        next_sequence: summary.next_sequence,
        session_id,
    })
}

/// Decode transport WAL entries in sequence order.
///
/// Corrupt tails are tolerated by stopping at the last valid entry.
/// If `max_entries` is provided, returns only the most recent `max_entries` entries.
pub fn read_entries(
    path: &Path,
    max_entries: Option<usize>,
) -> std::io::Result<Vec<CapturedTransportEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut file = OpenOptions::new().read(true).open(path)?;
    let file_len = file.metadata()?.len();
    if file_len <= HEADER_BYTES as u64 {
        return Ok(Vec::new());
    }

    let header = read_header(&mut file)?;
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(HEADER_BYTES as u64))?;

    let mut entries = Vec::new();

    loop {
        let pos = reader.stream_position()?;
        if pos >= file_len {
            break;
        }

        let mut len_buf = [0u8; 4];
        match reader.read_exact(&mut len_buf) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
        let total_len = u32::from_le_bytes(len_buf) as usize;
        if !(ENTRY_FIXED_BYTES..=MAX_ENTRY_BYTES).contains(&total_len) {
            break;
        }

        let remaining = total_len.saturating_sub(4);
        let mut body = vec![0u8; remaining];
        match reader.read_exact(&mut body) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
        if body.len() < (ENTRY_FIXED_BYTES - 4) {
            break;
        }

        let checksum = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let timestamp_nanos = i64::from_le_bytes([
            body[4], body[5], body[6], body[7], body[8], body[9], body[10], body[11],
        ]);
        let sequence = u64::from_le_bytes([
            body[12], body[13], body[14], body[15], body[16], body[17], body[18], body[19],
        ]);
        let direction = match Direction::from_u8(body[20]) {
            Some(d) => d,
            None => break,
        };

        let mut entry_session = [0u8; 16];
        entry_session.copy_from_slice(&body[21..37]);
        let data_len = u32::from_le_bytes([body[37], body[38], body[39], body[40]]) as usize;
        let data_offset = 41usize;
        if body.len() < data_offset + data_len {
            break;
        }
        let data = body[data_offset..data_offset + data_len].to_vec();
        if crc32fast::hash(&data) != checksum {
            break;
        }

        entries.push(CapturedTransportEntry {
            timestamp_nanos,
            sequence,
            direction: direction.into(),
            session_id: if entry_session == [0u8; 16] {
                header.session_id
            } else {
                entry_session
            },
            data,
        });
    }

    if let Some(max) = max_entries {
        if max == 0 {
            return Ok(Vec::new());
        }
        if entries.len() > max {
            entries = entries.split_off(entries.len() - max);
        }
    }

    Ok(entries)
}

fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn now_unix_nanos() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| {
            (d.as_secs() as i128)
                .saturating_mul(1_000_000_000i128)
                .saturating_add(d.subsec_nanos() as i128) as i64
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Seek;
    use tempfile::tempdir;

    #[test]
    fn writes_header_and_entries() {
        let dir = tempdir().expect("temp dir");
        let mut capture = TransportCapture::for_tests(dir.path()).expect("capture init");
        capture
            .capture_inbound(br#"{"jsonrpc":"2.0"}"#)
            .expect("in");
        capture.capture_outbound(br#"{"result":{}}"#).expect("out");
        capture.sync().expect("sync");

        let wal_path = dir.path().join("transport.wal");
        assert!(wal_path.exists());
        let summary = recover_entries(&wal_path).expect("recover");
        assert_eq!(summary.entries, 2);
        assert_eq!(summary.next_sequence, 2);
    }

    #[test]
    fn truncates_partial_tail_on_reopen() {
        let dir = tempdir().expect("temp dir");
        let wal_path = dir.path().join("transport.wal");

        let mut capture = TransportCapture::for_tests(dir.path()).expect("capture init");
        capture
            .capture_inbound(br#"{"method":"ping"}"#)
            .expect("in");
        capture.sync().expect("sync");

        {
            let mut wal = OpenOptions::new()
                .append(true)
                .open(&wal_path)
                .expect("open wal");
            wal.write_all(b"\x10\x00\x00\x00garbage")
                .expect("append garbage");
            wal.flush().expect("flush");
        }

        let original_len = std::fs::metadata(&wal_path).expect("metadata").len();
        let mut capture = TransportCapture::for_tests(dir.path()).expect("reopen");
        capture.capture_outbound(br#"{"ok":true}"#).expect("out");
        capture.sync().expect("sync");

        let final_len = std::fs::metadata(&wal_path).expect("metadata").len();
        assert!(final_len < original_len + 128);
        let summary = recover_entries(&wal_path).expect("recover");
        assert_eq!(summary.entries, 2);
    }

    #[test]
    fn disabled_capture_is_noop() {
        let mut capture = TransportCapture::Disabled;
        capture.capture_inbound(b"abc").expect("in");
        capture.capture_outbound(b"def").expect("out");
        capture.sync().expect("sync");
    }

    #[test]
    fn recovers_zero_from_header_only_file() {
        let dir = tempdir().expect("temp dir");
        let wal_path = dir.path().join("transport.wal");
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .truncate(true)
            .open(&wal_path)
            .expect("open wal");
        write_header(&mut file, *Uuid::new_v4().as_bytes()).expect("write header");
        file.seek(SeekFrom::End(0)).expect("seek");
        file.write_all(&[1, 2, 3]).expect("partial bytes");
        file.flush().expect("flush");

        let summary = recover_entries(&wal_path).expect("recover");
        assert_eq!(summary.entries, 0);
        assert_eq!(summary.truncate_to, HEADER_BYTES as u64);
    }

    #[test]
    fn public_read_entries_and_status_work() {
        let dir = tempdir().expect("temp dir");
        let mut capture = TransportCapture::for_tests(dir.path()).expect("capture init");
        capture
            .capture_inbound(br#"{"jsonrpc":"2.0","method":"ping"}"#)
            .expect("in");
        capture
            .capture_outbound(br#"{"jsonrpc":"2.0","result":{}}"#)
            .expect("out");
        capture.sync().expect("sync");

        let wal_path = dir.path().join("transport.wal");
        let status = wal_status(&wal_path).expect("status");
        assert!(status.exists);
        assert_eq!(status.entries, 2);
        assert!(status.session_id.is_some());

        let entries = read_entries(&wal_path, None).expect("read entries");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].direction.as_str(), "inbound");
        assert_eq!(entries[1].direction.as_str(), "outbound");
    }
}
