use crate::permissions;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

static GUARD_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn is_active() -> bool {
    GUARD_ACTIVE.load(Ordering::Relaxed)
}

pub enum VpnGuard {
    Wireguard {
        iface_name: String,
        config_path: String,
    },
    Openvpn {
        profile_id: String,
    },
}

pub fn start(guard: VpnGuard) {
    if GUARD_ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    GUARD_ACTIVE.store(true, Ordering::Relaxed);

    thread::spawn(move || {
        while GUARD_ACTIVE.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(5));

            if !GUARD_ACTIVE.load(Ordering::Relaxed) {
                break;
            }

            match &guard {
                VpnGuard::Wireguard {
                    iface_name,
                    config_path,
                } => {
                    let iface_path = format!("/sys/class/net/{}", iface_name);
                    if !std::path::Path::new(&iface_path).exists() {
                        let _ = Command::new(permissions::priv_cmd())
                            .args(["wg-quick", "up", config_path])
                            .output();
                    }
                }
                VpnGuard::Openvpn { profile_id } => {
                    let pid_file = dirs::config_dir()
                        .unwrap_or_default()
                        .join("bullet-vpn")
                        .join("openvpn")
                        .join(format!("{}.pid", profile_id));
                    let running = if pid_file.exists() {
                        std::fs::read_to_string(&pid_file)
                            .ok()
                            .and_then(|p| p.trim().parse::<u32>().ok())
                            .map(|pid| PathBuf::from(format!("/proc/{}", pid)).exists())
                            .unwrap_or(false)
                    } else {
                        PathBuf::from("/sys/class/net/tun0").exists()
                    };
                    if !running {
                        let conf = dirs::config_dir()
                            .unwrap_or_default()
                            .join("bullet-vpn")
                            .join("openvpn")
                            .join(format!("{}.ovpn", profile_id));
                        let log = dirs::config_dir()
                            .unwrap_or_default()
                            .join("bullet-vpn")
                            .join("openvpn")
                            .join(format!("{}.log", profile_id));
                        if let Ok(m) = std::fs::metadata(&log) {
                            if m.len() > 1_000_000 {
                                let _ = std::fs::remove_file(&log);
                            }
                        }
                        let _ = Command::new(permissions::priv_cmd())
                            .args([
                                "openvpn",
                                "--config",
                                &conf.to_string_lossy(),
                                "--daemon",
                                "--writepid",
                                &pid_file.to_string_lossy(),
                                "--log",
                                &log.to_string_lossy(),
                                "--verb",
                                "1",
                            ])
                            .output();
                    }
                }
            }
        }
    });
}

pub fn stop() {
    GUARD_ACTIVE.store(false, Ordering::Relaxed);
}
