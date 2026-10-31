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

fn run(priv_cmd: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(priv_cmd)
        .args(args)
        .output()
        .map_err(|e| format!("{} falhou: {}", priv_cmd, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{}: {}", args[0], stderr.trim()));
    }
    Ok(())
}

pub fn setup(iface: &str) -> Result<(), String> {
    let p = permissions::priv_cmd();

    let _ = run(p, &["modprobe", "xt_cgroup"]);

    let (gw, dev) = detect_physical_route(iface).ok_or(
        "Não foi possível detectar o gateway físico (não-VPN). Verifique sua conexão de rede.",
    )?;

    run(p, &["mkdir", "-p", CGROUP_PATH])?;

    let classid_path = format!("{}/net_cls.classid", CGROUP_PATH);
    let classid_str = CLASSID.to_string();
    run(
        p,
        &[
            "sh",
            "-c",
            &format!("echo {} > {}", classid_str, classid_path),
        ],
    )?;

    let _ = run(p, &["iptables", "-t", "mangle", "-N", CHAIN]);
    let _ = run(p, &["iptables", "-t", "mangle", "-F", CHAIN]);

    run(
        p,
        &[
            "iptables",
            "-t",
            "mangle",
            "-A",
            CHAIN,
            "-m",
            "cgroup",
            "--cgroup",
            &format!("{}", CLASSID),
            "-j",
            "MARK",
            "--set-mark",
            FWMARK,
        ],
    )?;

    let _ = run(
        p,
        &["iptables", "-t", "mangle", "-D", "OUTPUT", "-j", CHAIN],
    );
    run(
        p,
        &["iptables", "-t", "mangle", "-A", "OUTPUT", "-j", CHAIN],
    )?;

    let _ = run(p, &["ip", "rule", "del", "fwmark", FWMARK, "table", TABLE]);
    run(p, &["ip", "rule", "add", "fwmark", FWMARK, "table", TABLE])?;

    let _ = run(p, &["ip", "route", "flush", "table", TABLE]);

    run(
        p,
        &[
            "ip", "route", "add", "default", "via", &gw, "dev", &dev, "table", TABLE,
        ],
    )?;

    fs::write(marker_path(), iface).ok();
    Ok(())
}

pub fn teardown() -> Result<(), String> {
    let p = permissions::priv_cmd();

    let _ = run(
        p,
        &["iptables", "-t", "mangle", "-D", "OUTPUT", "-j", CHAIN],
    );
    let _ = run(p, &["iptables", "-t", "mangle", "-F", CHAIN]);
    let _ = run(p, &["iptables", "-t", "mangle", "-X", CHAIN]);

    let _ = run(p, &["ip", "rule", "del", "fwmark", FWMARK, "table", TABLE]);
    let _ = run(p, &["ip", "route", "flush", "table", TABLE]);

    let tasks = list_pids();
    if !tasks.is_empty() {
        let init_cgroup = "/sys/fs/cgroup/net_cls/tasks";
        for pid in tasks {
            let _ = run(p, &["sh", "-c", &format!("echo {} > {}", pid, init_cgroup)]);
        }
    }

    let _ = run(p, &["rmdir", CGROUP_PATH]);

    fs::remove_file(marker_path()).ok();
    Ok(())
}

pub fn add_pid(pid: u32) -> Result<(), String> {
    let p = permissions::priv_cmd();
    let tasks_path = format!("{}/tasks", CGROUP_PATH);
    run(p, &["sh", "-c", &format!("echo {} > {}", pid, tasks_path)])
}

pub fn remove_pid(pid: u32) -> Result<(), String> {
    let p = permissions::priv_cmd();
    let init_tasks = "/sys/fs/cgroup/net_cls/tasks";
    run(p, &["sh", "-c", &format!("echo {} > {}", pid, init_tasks)])
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
            if d == vpn_iface || d.starts_with("bvpn-") {
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
            if d == vpn_iface || d.starts_with("bvpn-") {
                continue;
            }
            return Some((parts[2].to_string(), d.to_string()));
        }
    }

    None
}
