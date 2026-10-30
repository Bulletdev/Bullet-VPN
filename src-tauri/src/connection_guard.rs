use crate::permissions;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

static GUARD_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn is_active() -> bool {
    GUARD_ACTIVE.load(Ordering::Relaxed)
}

pub fn start(iface_name: String, config_path: String) {
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

            let iface_path = format!("/sys/class/net/{}", iface_name);
            if !std::path::Path::new(&iface_path).exists() {
                let _ = Command::new(permissions::priv_cmd())
                    .args(["wg-quick", "up", &config_path])
                    .output();
            }
        }
    });
}

pub fn stop() {
    GUARD_ACTIVE.store(false, Ordering::Relaxed);
}
