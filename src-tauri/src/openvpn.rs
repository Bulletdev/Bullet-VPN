use crate::permissions;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::Command;

const MAX_LOG_BYTES: u64 = 1_000_000;

fn truncate_log(path: &PathBuf) {
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() > MAX_LOG_BYTES {
            fs::remove_file(path).ok();
        }
    }
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bullet-vpn")
        .join("openvpn")
}

fn pid_path(profile_id: &str) -> PathBuf {
    config_dir().join(format!("{}.pid", profile_id))
}

pub fn import_config(profile_id: &str, ovpn_content: &str) -> Result<PathBuf, String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let path = dir.join(format!("{}.ovpn", profile_id));
    fs::write(&path, ovpn_content).map_err(|e| e.to_string())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }

    Ok(path)
}

pub fn up(profile_id: &str) -> Result<(), String> {
    let conf = config_dir().join(format!("{}.ovpn", profile_id));
    if !conf.exists() {
        return Err("Arquivo .ovpn nao encontrado".to_string());
    }

    let pid = pid_path(profile_id);

    let log_path = config_dir().join(format!("{}.log", profile_id));
    truncate_log(&log_path);

    let output = Command::new(permissions::priv_cmd())
        .args([
            "openvpn",
            "--config",
            &conf.to_string_lossy(),
            "--daemon",
            "--writepid",
            &pid.to_string_lossy(),
            "--log",
            &log_path.to_string_lossy(),
            "--verb",
            "1",
        ])
        .output()
        .map_err(|e| format!("Falha ao executar pkexec: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("openvpn falhou: {}", stderr.trim()));
    }
    Ok(())
}

pub fn down(profile_id: &str) -> Result<(), String> {
    let pid_file = pid_path(profile_id);
    if !pid_file.exists() {
        let output = Command::new(permissions::priv_cmd())
            .args(["killall", "openvpn"])
            .output();
        match output {
            Ok(o) if o.status.success() => return Ok(()),
            _ => return Err("Nao foi possivel parar o OpenVPN".to_string()),
        }
    }

    let pid = fs::read_to_string(&pid_file)
        .map_err(|e| e.to_string())?
        .trim()
        .to_string();

    let output = Command::new(permissions::priv_cmd())
        .args(["kill", &pid])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        fs::remove_file(&pid_file).ok();
        let log_file = config_dir().join(format!("{}.log", profile_id));
        fs::remove_file(&log_file).ok();
    }
    Ok(())
}

pub fn is_running(profile_id: &str) -> bool {
    let pid_file = pid_path(profile_id);
    if !pid_file.exists() {
        return PathBuf::from("/sys/class/net/tun0").exists();
    }

    if let Ok(pid) = fs::read_to_string(&pid_file) {
        let pid = pid.trim();
        PathBuf::from(format!("/proc/{}", pid)).exists()
    } else {
        false
    }
}

pub fn read_log(profile_id: &str) -> String {
    let log = config_dir().join(format!("{}.log", profile_id));
    let mut file = match fs::File::open(&log) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let meta = match file.metadata() {
        Ok(m) => m,
        Err(_) => return String::new(),
    };
    const TAIL_BYTES: u64 = 8192;
    if meta.len() > TAIL_BYTES {
        file.seek(SeekFrom::End(-(TAIL_BYTES as i64))).ok();
    }
    let mut buf = String::new();
    file.read_to_string(&mut buf).ok();
    let lines: Vec<&str> = buf.lines().collect();
    let start = lines.len().saturating_sub(20);
    lines[start..].join("\n")
}
