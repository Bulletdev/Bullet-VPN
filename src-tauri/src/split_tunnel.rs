use crate::permissions;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const CGROUP_PATH: &str = "/sys/fs/cgroup/net_cls/bullet_vpn";
const CLASSID: u32 = 0x00110011;
const FWMARK: &str = "0x6d6f6c65";
const TABLE: &str = "100";
const CHAIN: &str = "BULLET_SPLIT";

fn marker_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("bullet-vpn")
        .join(".split-tunnel-active")
}

fn run_script(script: &str) -> Result<(), String> {
    let p = permissions::priv_cmd();
    let output = Command::new(p)
        .args(["sh", "-c", script])
        .output()
        .map_err(|e| format!("{} falhou: {}", p, e))?;

    if output.status.success() {
        return Ok(());
    }

    if p == "sudo" {
        let output = Command::new("pkexec")
            .args(["sh", "-c", script])
            .output()
            .map_err(|e| format!("pkexec falhou: {}", e))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("split tunnel: {}", stderr.trim()));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!("split tunnel: {}", stderr.trim()))
}

pub fn setup(iface: &str) -> Result<(), String> {
    let (gw, dev) = detect_physical_route(iface).ok_or(
        "Não foi possível detectar o gateway físico (não-VPN). Verifique sua conexão de rede.",
    )?;

    let classid_str = CLASSID.to_string();

    let script = format!(
        "modprobe xt_cgroup 2>/dev/null; \
         mkdir -p {cgroup} && \
         echo {classid} > {cgroup}/net_cls.classid && \
         iptables -t mangle -N {chain} 2>/dev/null; \
         iptables -t mangle -F {chain} && \
         iptables -t mangle -A {chain} -m cgroup --cgroup {classid} -j MARK --set-mark {fwmark} && \
         iptables -t mangle -D OUTPUT -j {chain} 2>/dev/null; \
         iptables -t mangle -A OUTPUT -j {chain} && \
         ip rule del fwmark {fwmark} table {table} 2>/dev/null; \
         ip rule add fwmark {fwmark} table {table} && \
         ip route flush table {table}; \
         ip route add default via {gw} dev {dev} table {table} && \
         chmod 666 {cgroup}/tasks /sys/fs/cgroup/net_cls/tasks",
        cgroup = CGROUP_PATH,
        classid = classid_str,
        chain = CHAIN,
        fwmark = FWMARK,
        table = TABLE,
        gw = gw,
        dev = dev,
    );

    run_script(&script)?;

    fs::write(marker_path(), iface).ok();
    Ok(())
}

pub fn teardown() -> Result<(), String> {
    let pids = list_pids();
    let mut move_pids = String::new();
    for pid in &pids {
        move_pids.push_str(&format!(
            "echo {} > /sys/fs/cgroup/net_cls/tasks 2>/dev/null; ",
            pid
        ));
    }

    let script = format!(
        "iptables -t mangle -D OUTPUT -j {chain} 2>/dev/null; \
         iptables -t mangle -F {chain} 2>/dev/null; \
         iptables -t mangle -X {chain} 2>/dev/null; \
         ip rule del fwmark {fwmark} table {table} 2>/dev/null; \
         ip route flush table {table} 2>/dev/null; \
         {move_pids}\
         rmdir {cgroup} 2>/dev/null; \
         true",
        chain = CHAIN,
        fwmark = FWMARK,
        table = TABLE,
        cgroup = CGROUP_PATH,
        move_pids = move_pids,
    );

    run_script(&script)?;

    fs::remove_file(marker_path()).ok();
    Ok(())
}

pub fn add_pid(pid: u32) -> Result<(), String> {
    let tasks_path = format!("{}/tasks", CGROUP_PATH);
    fs::write(&tasks_path, pid.to_string()).map_err(|e| format!("add_pid: {}", e))
}

pub fn remove_pid(pid: u32) -> Result<(), String> {
    fs::write("/sys/fs/cgroup/net_cls/tasks", pid.to_string())
        .map_err(|e| format!("remove_pid: {}", e))
}

pub fn list_pids() -> Vec<u32> {
    let tasks_path = format!("{}/tasks", CGROUP_PATH);
    fs::read_to_string(tasks_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect()
}

pub fn is_active() -> bool {
    marker_path().exists()
}

fn is_vpn_device(dev: &str, vpn_iface: &str) -> bool {
    dev == vpn_iface || dev.starts_with("bvpn-") || dev.starts_with("tun") || dev.starts_with("tap")
}

fn detect_physical_route(vpn_iface: &str) -> Option<(String, String)> {
    let output = Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 || parts[0] != "default" || parts[1] != "via" {
            continue;
        }
        let dev = parts
            .iter()
            .position(|&p| p == "dev")
            .and_then(|i| parts.get(i + 1).copied());
        if let Some(d) = dev {
            if is_vpn_device(d, vpn_iface) {
                continue;
            }
            return Some((parts[2].to_string(), d.to_string()));
        }
    }

    let output = Command::new("ip")
        .args(["route", "show", "table", "main"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 || parts[0] != "default" || parts[1] != "via" {
            continue;
        }
        let dev = parts
            .iter()
            .position(|&p| p == "dev")
            .and_then(|i| parts.get(i + 1).copied());
        if let Some(d) = dev {
            if is_vpn_device(d, vpn_iface) {
                continue;
            }
            return Some((parts[2].to_string(), d.to_string()));
        }
    }

    None
}
