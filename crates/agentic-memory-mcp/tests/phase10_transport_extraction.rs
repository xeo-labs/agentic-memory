use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use tempfile::tempdir;
use uuid::Uuid;

const MAGIC: &[u8; 4] = b"MWAL";
const VERSION: u16 = 1;
const HEADER_BYTES: usize = 64;
const ENTRY_FIXED_BYTES: usize = 45;

fn write_wal_header(path: &Path, session_id: [u8; 16]) {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .expect("open wal");

    let mut header = [0u8; HEADER_BYTES];
    header[0..4].copy_from_slice(MAGIC);
    header[4..6].copy_from_slice(&VERSION.to_le_bytes());
    header[6..22].copy_from_slice(&session_id);
    header[22..30].copy_from_slice(&0i64.to_le_bytes());
    header[30..34].copy_from_slice(&0u32.to_le_bytes());
    file.write_all(&header).expect("write header");
}

fn append_wal_entry(path: &Path, sequence: u64, direction: u8, session_id: [u8; 16], data: &[u8]) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open wal");
    let checksum = crc32fast::hash(data);
    let total_len = (ENTRY_FIXED_BYTES + data.len()) as u32;
    let ts_nanos: i64 = 1_700_000_000_000_000_000i64 + sequence as i64;

    file.write_all(&total_len.to_le_bytes()).expect("len");
    file.write_all(&checksum.to_le_bytes()).expect("checksum");
    file.write_all(&ts_nanos.to_le_bytes()).expect("timestamp");
    file.write_all(&sequence.to_le_bytes()).expect("seq");
    file.write_all(&[direction]).expect("direction");
    file.write_all(&session_id).expect("session");
    file.write_all(&(data.len() as u32).to_le_bytes())
        .expect("data len");
    file.write_all(data).expect("data");
}

fn run(args: &[String]) -> std::process::Output {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root");
    Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg("agentic-memory-mcp")
        .arg("--quiet")
        .arg("--")
        .args(args)
        .current_dir(workspace_root)
        .output()
        .expect("run command")
}

#[test]
fn edge_status_on_missing_wal() {
    let dir = tempdir().expect("temp dir");
    let wal = dir.path().join("missing.wal");

    let out = run(&[
        "status".to_string(),
        "--wal".to_string(),
        wal.display().to_string(),
    ]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parsed: Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(parsed["exists"], false);
    assert_eq!(parsed["entries"], 0);
}

#[test]
fn edge_extract_detects_client_and_tool() {
    let dir = tempdir().expect("temp dir");
    let wal = dir.path().join("transport.wal");
    let session_id = *Uuid::new_v4().as_bytes();
    write_wal_header(&wal, session_id);

    append_wal_entry(
        &wal,
        0,
        0,
        session_id,
        br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"Cursor","version":"1.2.3"}}}"#,
    );
    append_wal_entry(
        &wal,
        1,
        0,
        session_id,
        br#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_add","arguments":{"content":"x"}}}"#,
    );
    append_wal_entry(
        &wal,
        2,
        1,
        session_id,
        br#"{"jsonrpc":"2.0","id":2,"result":{"ok":true}}"#,
    );

    let out = run(&[
        "extract".to_string(),
        "--wal".to_string(),
        wal.display().to_string(),
    ]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parsed: Value = serde_json::from_slice(&out.stdout).expect("json");
    let arr = parsed.as_array().expect("array");
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0]["client_name"], "Cursor");
    assert_eq!(arr[1]["client_family"], "cursor");
    assert_eq!(arr[1]["tool_name"], "memory_add");
    assert_eq!(arr[2]["message_kind"], "response");
}

#[test]
fn edge_extract_handles_invalid_json_payload() {
    let dir = tempdir().expect("temp dir");
    let wal = dir.path().join("transport.wal");
    let session_id = *Uuid::new_v4().as_bytes();
    write_wal_header(&wal, session_id);

    append_wal_entry(&wal, 0, 0, session_id, b"not-json");
    append_wal_entry(
        &wal,
        1,
        1,
        session_id,
        br#"{"jsonrpc":"2.0","error":{"code":-32600,"message":"bad request"}}"#,
    );

    let out = run(&[
        "extract".to_string(),
        "--wal".to_string(),
        wal.display().to_string(),
    ]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parsed: Value = serde_json::from_slice(&out.stdout).expect("json");
    let arr = parsed.as_array().expect("array");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["message_kind"], "invalid_json");
    assert_eq!(arr[1]["message_kind"], "error");
}

#[test]
fn stress_extract_limit_large_wal() {
    let dir = tempdir().expect("temp dir");
    let wal = dir.path().join("transport.wal");
    let session_id = *Uuid::new_v4().as_bytes();
    write_wal_header(&wal, session_id);

    append_wal_entry(
        &wal,
        0,
        0,
        session_id,
        br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"Claude Code","version":"1.0.0"}}}"#,
    );

    for i in 1..=2000u64 {
        let payload = format!(
            r#"{{"jsonrpc":"2.0","id":{},"method":"tools/call","params":{{"name":"memory_query","arguments":{{"query":"item {}"}}}}}}"#,
            i + 1,
            i
        );
        append_wal_entry(&wal, i, (i % 2) as u8, session_id, payload.as_bytes());
    }

    let out = run(&[
        "extract".to_string(),
        "--wal".to_string(),
        wal.display().to_string(),
        "--limit".to_string(),
        "1500".to_string(),
    ]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parsed: Value = serde_json::from_slice(&out.stdout).expect("json");
    let arr = parsed.as_array().expect("array");
    assert_eq!(arr.len(), 1500);
    assert_eq!(
        arr.last()
            .and_then(|v| v.get("sequence"))
            .and_then(Value::as_u64),
        Some(2000)
    );
}

#[test]
fn edge_replay_outputs_sequential_lines() {
    let dir = tempdir().expect("temp dir");
    let wal = dir.path().join("transport.wal");
    let session_id = *Uuid::new_v4().as_bytes();
    write_wal_header(&wal, session_id);

    for i in 0..4u64 {
        let payload = format!(r#"{{"jsonrpc":"2.0","id":{},"method":"ping"}}"#, i);
        append_wal_entry(&wal, i, 0, session_id, payload.as_bytes());
    }

    let out = run(&[
        "replay".to_string(),
        "--wal".to_string(),
        wal.display().to_string(),
        "--limit".to_string(),
        "4".to_string(),
    ]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 4);
    assert!(lines[0].contains("seq=0"));
    assert!(lines[3].contains("seq=3"));
}
