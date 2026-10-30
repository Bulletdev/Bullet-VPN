use crate::permissions;
use crate::profiles::Profile;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bullet-vpn")
        .join("interfaces")
}

pub fn write_config(iface_name: &str, profile: &Profile) -> Result<PathBuf, String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let config = format!(
"[Interface]
PrivateKey = {}
Address = {}
DNS = {}

[Peer]
PublicKey = {}
Endpoint = {}
AllowedIPs = {}
PersistentKeepalive = 25
",
        profile.private_key,
        profile.address,
        profile.dns,
        profile.server_pubkey,
        profile.endpoint,
        profile.allowed_ips,
    );

    let path = dir.join(format!("{}.conf", iface_name));
    fs::write(&path, &config).map_err(|e| e.to_string())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }

    Ok(path)
}

fn find_conflicting_wg_interfaces() -> Vec<String> {
    let mut conflicts = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/net") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let type_path = entry.path().join("type");
            if let Ok(t) = fs::read_to_string(&type_path) {
                if t.trim() == "65534" && !name.starts_with("bvpn-") {
                    conflicts.push(name);
                }
            }
        }
    }
    conflicts
}

pub fn teardown_conflicts() -> Result<Vec<String>, String> {
    let conflicts = find_conflicting_wg_interfaces();
    let mut removed = Vec::new();

    for iface in &conflicts {
        let conf_path = format!("/etc/wireguard/{}.conf", iface);
        let success = if std::path::Path::new(&conf_path).exists() {
            Command::new(permissions::priv_cmd())
                .args(["wg-quick", "down", iface])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        } else {
            Command::new(permissions::priv_cmd())
                .args(["ip", "link", "delete", "dev", iface])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };

        if success {
            removed.push(iface.clone());
        }
    }

    Ok(removed)
}

pub fn up(config_path: &PathBuf) -> Result<(), String> {
    let output = Command::new(permissions::priv_cmd())
        .args(["wg-quick", "up", &config_path.to_string_lossy()])
        .output()
        .map_err(|e| format!("Falha ao executar pkexec: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "wg-quick up falhou (exit {}): {} {}",
            output.status.code().unwrap_or(-1),
            stderr.trim(),
            stdout.trim()
        ));
    }
    Ok(())
}

pub fn down(iface_name: &str) -> Result<(), String> {
    let path = config_dir().join(format!("{}.conf", iface_name));

    let output = Command::new(permissions::priv_cmd())
        .args(["wg-quick", "down", &path.to_string_lossy()])
        .output()
        .map_err(|e| format!("Falha ao executar pkexec: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("wg-quick down falhou: {}", stderr.trim()));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct WgStatus {
    pub connected: bool,
    pub interface_name: String,
    pub handshake_ago: String,
    pub transfer_rx: String,
    pub transfer_tx: String,
    pub endpoint: String,
}

fn interface_exists(iface_name: &str) -> bool {
    PathBuf::from(format!("/sys/class/net/{}", iface_name)).exists()
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.2} GiB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.2} MiB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.2} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn read_transfer(iface_name: &str) -> (String, String) {
    let content = match fs::read_to_string("/proc/net/dev") {
        Ok(c) => c,
        Err(_) => return ("0 B".to_string(), "0 B".to_string()),
    };

    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(&format!("{}:", iface_name)) {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 9 {
                let rx: u64 = parts[0].parse().unwrap_or(0);
                let tx: u64 = parts[8].parse().unwrap_or(0);
                return (format_bytes(rx), format_bytes(tx));
            }
        }
    }

    ("0 B".to_string(), "0 B".to_string())
}

pub fn status(iface_name: &str) -> Option<WgStatus> {
    if !interface_exists(iface_name) {
        return None;
    }

    let (rx, tx) = read_transfer(iface_name);

    Some(WgStatus {
        connected: true,
        interface_name: iface_name.to_string(),
        handshake_ago: "ativo".to_string(),
        transfer_rx: rx,
        transfer_tx: tx,
        endpoint: String::new(),
    })
}

pub fn detailed_status(iface_name: &str) -> Option<WgStatus> {
    let output = Command::new(permissions::priv_cmd())
        .args(["wg", "show", iface_name])
        .output()
        .ok()?;

    if !output.status.success() {
        return status(iface_name);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut endpoint = String::new();
    let mut handshake = String::new();

    for line in stdout.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("endpoint:") {
            endpoint = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("latest handshake:") {
            handshake = val.trim().to_string();
        }
    }

    let (rx, tx) = read_transfer(iface_name);

    Some(WgStatus {
        connected: true,
        interface_name: iface_name.to_string(),
        handshake_ago: if handshake.is_empty() {
            "sem handshake".to_string()
        } else {
            handshake
        },
        transfer_rx: rx,
        transfer_tx: tx,
        endpoint,
    })
}
