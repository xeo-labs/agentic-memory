use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::tempdir;

fn run_with_home(home: &Path, args: &[&str]) -> std::process::Output {
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
        .env("HOME", home)
        .current_dir(workspace_root)
        .output()
        .expect("run command")
}

#[test]
fn edge_daemon_status_when_not_running() {
    let home = tempdir().expect("temp home");
    let out = run_with_home(home.path(), &["daemon", "status"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("State:             not running"));
}

#[test]
fn edge_daemon_logs_tails_last_lines() {
    let home = tempdir().expect("temp home");
    let log_dir = home.path().join(".agentic").join("memory");
    fs::create_dir_all(&log_dir).expect("create log dir");
    let log_file = log_dir.join("daemon.log");
    fs::write(&log_file, "a\nb\nc\nd\n").expect("write log");

    let out = run_with_home(home.path(), &["daemon", "logs", "-n", "2"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("c"));
    assert!(stdout.contains("d"));
}

#[test]
fn edge_daemon_stop_when_not_running() {
    let home = tempdir().expect("temp home");
    let out = run_with_home(home.path(), &["daemon", "stop"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Daemon is not running"));
}
