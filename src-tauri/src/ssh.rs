use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TunnelType {
    Local,
    Remote,
    Dynamic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshTunnel {
    pub id: String,
    pub name: String,
    pub tunnel_type: TunnelType,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub ssh_host: String,
    pub ssh_user: String,
    #[serde(default)]
    pub ssh_key: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TunnelStore {
    tunnels: Vec<SshTunnel>,
}

fn store_path() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bullet-vpn");
    fs::create_dir_all(&dir).ok();
    dir.join("ssh-tunnels.json")
}

fn pid_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bullet-vpn")
        .join("ssh-pids");
    fs::create_dir_all(&dir).ok();
    dir
}

impl TunnelStore {
    pub fn load() -> Self {
        let path = store_path();
        if !path.exists() {
            return Self::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(store_path(), data).map_err(|e| e.to_string())
    }

    pub fn list(&self) -> Vec<SshTunnel> {
        self.tunnels.clone()
    }

    pub fn add(&mut self, tunnel: SshTunnel) {
        self.tunnels.retain(|t| t.id != tunnel.id);
        self.tunnels.push(tunnel);
    }

    pub fn remove(&mut self, id: &str) {
        self.tunnels.retain(|t| t.id != id);
    }

    pub fn get(&self, id: &str) -> Option<SshTunnel> {
        self.tunnels.iter().find(|t| t.id == id).cloned()
    }
}

pub fn start(tunnel: &SshTunnel) -> Result<u32, String> {
    let forward_arg = match tunnel.tunnel_type {
        TunnelType::Local => format!(
            "-L{}:{}:{}",
            tunnel.local_port, tunnel.remote_host, tunnel.remote_port
        ),
        TunnelType::Remote => format!(
            "-R{}:{}:{}",
            tunnel.remote_port, tunnel.remote_host, tunnel.local_port
        ),
        TunnelType::Dynamic => format!("-D{}", tunnel.local_port),
    };

    let mut args = vec![
        "-N".to_string(),
        "-o".to_string(), "ServerAliveInterval=30".to_string(),
        "-o".to_string(), "ServerAliveCountMax=3".to_string(),
        "-o".to_string(), "ExitOnForwardFailure=yes".to_string(),
        "-o".to_string(), "StrictHostKeyChecking=accept-new".to_string(),
        forward_arg,
    ];

    if !tunnel.ssh_key.is_empty() {
        args.push("-i".to_string());
        args.push(expand_home(&tunnel.ssh_key));
    }

    args.push(format!("{}@{}", tunnel.ssh_user, tunnel.ssh_host));

    let child = Command::new("ssh")
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("ssh falhou: {}", e))?;

    let pid = child.id();
    let pid_file = pid_dir().join(format!("{}.pid", tunnel.id));
    fs::write(&pid_file, pid.to_string()).map_err(|e| e.to_string())?;

    Ok(pid)
}

pub fn stop(tunnel_id: &str) -> Result<(), String> {
    let pid_file = pid_dir().join(format!("{}.pid", tunnel_id));
    if !pid_file.exists() {
        return Ok(());
    }

    let pid = fs::read_to_string(&pid_file)
        .map_err(|e| e.to_string())?
        .trim()
        .to_string();

    let _ = Command::new("kill")
        .arg(&pid)
        .output();

    fs::remove_file(&pid_file).ok();
    Ok(())
}

pub fn is_running(tunnel_id: &str) -> bool {
    let pid_file = pid_dir().join(format!("{}.pid", tunnel_id));
    if !pid_file.exists() {
        return false;
    }

    if let Ok(pid_str) = fs::read_to_string(&pid_file) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            return PathBuf::from(format!("/proc/{}", pid)).exists();
        }
    }
    false
}

fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    path.to_string()
}
