use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "macos")]
const SERVICE_LABEL_MACOS: &str = "com.agentic.memory";
#[cfg(target_os = "linux")]
const SERVICE_LABEL_LINUX: &str = "agentic-memory";
#[cfg(target_os = "windows")]
const SERVICE_LABEL_WINDOWS: &str = "AgenticMemory";

#[derive(Debug, Clone)]
pub struct DaemonPaths {
    pub base_dir: PathBuf,
    pub status_file: PathBuf,
    pub pid_file: PathBuf,
    pub log_file: PathBuf,
}

impl Default for DaemonPaths {
    fn default() -> Self {
        let base = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".agentic")
            .join("memory");
        Self {
            status_file: base.join("daemon.status"),
            pid_file: base.join("daemon.pid"),
            log_file: base.join("daemon.log"),
            base_dir: base,
        }
    }
}

impl DaemonPaths {
    pub fn ensure_dirs(&self) -> io::Result<()> {
        fs::create_dir_all(&self.base_dir)?;
        if let Some(parent) = self.status_file.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = self.log_file.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonRunState {
    Starting,
    Running,
    Extracting,
    Idle,
    Stopping,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub pid: u32,
    pub started_at: u64,
    pub last_active: u64,
    pub state: DaemonRunState,
    pub wal_files_count: usize,
    pub entries_extracted: u64,
    pub memories_written: u64,
    pub last_error: Option<String>,
    pub last_sequence: Option<u64>,
}

impl DaemonStatus {
    pub fn new() -> Self {
        Self {
            pid: std::process::id(),
            started_at: now_unix(),
            last_active: now_unix(),
            state: DaemonRunState::Starting,
            wal_files_count: 0,
            entries_extracted: 0,
            memories_written: 0,
            last_error: None,
            last_sequence: None,
        }
    }

    pub fn touch(&mut self) {
        self.last_active = now_unix();
    }

    pub fn record_extraction(&mut self, entries: u64, memories: u64) {
        self.entries_extracted = self.entries_extracted.saturating_add(entries);
        self.memories_written = self.memories_written.saturating_add(memories);
        self.touch();
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.last_error = Some(message.into());
        self.state = DaemonRunState::Error;
        self.touch();
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
        if self.state == DaemonRunState::Error {
            self.state = DaemonRunState::Running;
        }
    }

    pub fn is_alive(&self) -> bool {
        now_unix().saturating_sub(self.last_active) < 30
    }

    pub fn uptime_secs(&self) -> u64 {
        now_unix().saturating_sub(self.started_at)
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        let data = serde_json::to_vec_pretty(self)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        fs::write(&tmp, data)?;
        fs::rename(tmp, path)?;
        Ok(())
    }

    pub fn load(path: &Path) -> io::Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(path)?;
        let parsed: Self = serde_json::from_str(&raw)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        Ok(Some(parsed))
    }
}

pub fn write_pid_file(path: &Path, pid: u32) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, pid.to_string())
}

pub fn remove_pid_file(path: &Path) -> io::Result<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn read_pid_file(path: &Path) -> Option<u32> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
}

pub fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}")])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
}

pub fn stop_process(pid: u32) -> io::Result<bool> {
    #[cfg(unix)]
    {
        let output = Command::new("kill").args([&pid.to_string()]).output()?;
        Ok(output.status.success())
    }
    #[cfg(windows)]
    {
        let output = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output()?;
        Ok(output.status.success())
    }
}

#[cfg(target_os = "macos")]
pub fn launchd_plist_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{SERVICE_LABEL_MACOS}.plist"))
}

#[cfg(target_os = "linux")]
pub fn systemd_service_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".config")
        .join("systemd")
        .join("user")
        .join(format!("{SERVICE_LABEL_LINUX}.service"))
}

#[cfg(target_os = "macos")]
fn render_launchd_plist(binary_path: &Path, log_path: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{SERVICE_LABEL_MACOS}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>daemon</string>
        <string>run</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>AMEM_DAEMON_MODE</key>
        <string>1</string>
    </dict>
    <key>ProcessType</key>
    <string>Background</string>
</dict>
</plist>
"#,
        binary_path.display(),
        log_path.display(),
        log_path.display()
    )
}

#[cfg(target_os = "linux")]
fn render_systemd_service(binary_path: &Path, log_path: &Path) -> String {
    format!(
        r#"[Unit]
Description=Agentic Memory Daemon
After=network.target

[Service]
Type=simple
ExecStart={} daemon run
Restart=on-failure
RestartSec=5
Environment=AMEM_DAEMON_MODE=1
Nice=10
IOSchedulingClass=idle
StandardOutput=append:{}
StandardError=append:{}

[Install]
WantedBy=default.target
"#,
        binary_path.display(),
        log_path.display(),
        log_path.display()
    )
}

pub fn install_service(binary_path: &Path, log_path: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let plist = launchd_plist_path();
        if let Some(parent) = plist.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&plist, render_launchd_plist(binary_path, log_path))?;
        let _ = Command::new("launchctl")
            .args(["unload", plist.to_string_lossy().as_ref()])
            .output();
        let _ = Command::new("launchctl")
            .args(["load", plist.to_string_lossy().as_ref()])
            .output()?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        let svc = systemd_service_path();
        if let Some(parent) = svc.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&svc, render_systemd_service(binary_path, log_path))?;
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output()?;
        let _ = Command::new("systemctl")
            .args(["--user", "enable", SERVICE_LABEL_LINUX])
            .output()?;
        let _ = Command::new("systemctl")
            .args(["--user", "start", SERVICE_LABEL_LINUX])
            .output()?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        let command = format!("\"{}\" daemon run", binary_path.display());
        let _ = Command::new("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                SERVICE_LABEL_WINDOWS,
                "/t",
                "REG_SZ",
                "/d",
                &command,
                "/f",
            ])
            .output()?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Unsupported platform for service install",
    ))
}

pub fn uninstall_service() -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let plist = launchd_plist_path();
        if plist.exists() {
            let _ = Command::new("launchctl")
                .args(["unload", plist.to_string_lossy().as_ref()])
                .output();
            fs::remove_file(&plist)?;
        }
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        let svc = systemd_service_path();
        let _ = Command::new("systemctl")
            .args(["--user", "stop", SERVICE_LABEL_LINUX])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "disable", SERVICE_LABEL_LINUX])
            .output();
        if svc.exists() {
            fs::remove_file(&svc)?;
        }
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("reg")
            .args([
                "delete",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                SERVICE_LABEL_WINDOWS,
                "/f",
            ])
            .output()?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Unsupported platform for service uninstall",
    ))
}

pub fn is_service_installed() -> bool {
    #[cfg(target_os = "macos")]
    {
        return launchd_plist_path().exists();
    }
    #[cfg(target_os = "linux")]
    {
        return systemd_service_path().exists();
    }
    #[cfg(target_os = "windows")]
    {
        return Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                SERVICE_LABEL_WINDOWS,
            ])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    }
    #[allow(unreachable_code)]
    false
}

pub fn read_last_lines(path: &Path, count: usize) -> io::Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)?;
    let mut lines: Vec<String> = raw.lines().map(ToString::to_string).collect();
    if lines.len() > count {
        lines = lines.split_off(lines.len() - count);
    }
    Ok(lines)
}

pub fn append_log_line(path: &Path, line: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("daemon.status");
        let mut status = DaemonStatus::new();
        status.state = DaemonRunState::Running;
        status.entries_extracted = 3;
        status.save(&path).expect("save");
        let loaded = DaemonStatus::load(&path).expect("load").expect("exists");
        assert_eq!(loaded.state, DaemonRunState::Running);
        assert_eq!(loaded.entries_extracted, 3);
    }

    #[test]
    fn tail_lines_returns_last_n() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("daemon.log");
        fs::write(&path, "a\nb\nc\nd\n").expect("write");
        let lines = read_last_lines(&path, 2).expect("tail");
        assert_eq!(lines, vec!["c".to_string(), "d".to_string()]);
    }
}
