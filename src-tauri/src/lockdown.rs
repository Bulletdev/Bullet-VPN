use crate::permissions;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const CHAIN: &str = "BULLET_VPN_LD";

fn marker_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("bullet-vpn")
        .join(".lockdown-active")
}

fn rules_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("bullet-vpn")
        .join("lockdown.rules")
}

pub fn is_active() -> bool {
    marker_path().exists()
}

pub fn enable(allow_lan: bool) -> Result<(), String> {
    let p = permissions::priv_cmd();

    let _ = run(p, &["iptables", "-N", CHAIN]);
    run(p, &["iptables", "-F", CHAIN])?;

    // Allow loopback
    run(p, &["iptables", "-A", CHAIN, "-o", "lo", "-j", "ACCEPT"])?;

    // Allow WireGuard interfaces (bvpn-*)
    run(
        p,
        &["iptables", "-A", CHAIN, "-o", "bvpn-+", "-j", "ACCEPT"],
    )?;

    // Allow OpenVPN interfaces (tun*)
    run(p, &["iptables", "-A", CHAIN, "-o", "tun+", "-j", "ACCEPT"])?;

    // Allow established/related connections
    run(
        p,
        &[
            "iptables",
            "-A",
            CHAIN,
            "-m",
            "conntrack",
            "--ctstate",
            "ESTABLISHED,RELATED",
            "-j",
            "ACCEPT",
        ],
    )?;

    // Allow DHCP
    run(
        p,
        &[
            "iptables", "-A", CHAIN, "-p", "udp", "--dport", "67:68", "-j", "ACCEPT",
        ],
    )?;

    // Allow LAN traffic if requested
    if allow_lan {
        for net in &[
            "10.0.0.0/8",
            "172.16.0.0/12",
            "192.168.0.0/16",
            "169.254.0.0/16",
        ] {
            run(p, &["iptables", "-A", CHAIN, "-d", net, "-j", "ACCEPT"])?;
        }
    }

    // Reject everything else
    run(
        p,
        &[
            "iptables",
            "-A",
            CHAIN,
            "-j",
            "REJECT",
            "--reject-with",
            "icmp-net-unreachable",
        ],
    )?;

    // Insert chain into OUTPUT (remove first if already present)
    let _ = run(p, &["iptables", "-D", "OUTPUT", "-j", CHAIN]);
    run(p, &["iptables", "-I", "OUTPUT", "1", "-j", CHAIN])?;

    // Save rules to file for restore on boot
    save_rules(p)?;

    // Write marker file
    let dir = marker_path()
        .parent()
        .unwrap_or(&PathBuf::from("."))
        .to_path_buf();
    fs::create_dir_all(&dir).ok();
    fs::write(marker_path(), "active").map_err(|e| format!("marker write: {}", e))?;

    Ok(())
}

pub fn disable() -> Result<(), String> {
    let p = permissions::priv_cmd();

    let _ = run(p, &["iptables", "-D", "OUTPUT", "-j", CHAIN]);
    let _ = run(p, &["iptables", "-F", CHAIN]);
    let _ = run(p, &["iptables", "-X", CHAIN]);

    fs::remove_file(rules_path()).ok();
    fs::remove_file(marker_path()).ok();

    Ok(())
}

pub fn restore_on_boot() -> Result<(), String> {
    if !marker_path().exists() {
        return Ok(());
    }

    let rp = rules_path();
    if !rp.exists() {
        return Ok(());
    }

    let p = permissions::priv_cmd();
    let output = Command::new(p)
        .args(["iptables-restore", &rp.to_string_lossy()])
        .output()
        .map_err(|e| format!("{} falhou: {}", p, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("iptables-restore: {}", stderr.trim()));
    }

    Ok(())
}

fn save_rules(priv_cmd: &str) -> Result<(), String> {
    let output = Command::new(priv_cmd)
        .args(["iptables-save"])
        .output()
        .map_err(|e| format!("{} falhou: {}", priv_cmd, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("iptables-save: {}", stderr.trim()));
    }

    let dir = rules_path()
        .parent()
        .unwrap_or(&PathBuf::from("."))
        .to_path_buf();
    fs::create_dir_all(&dir).ok();
    fs::write(rules_path(), &output.stdout).map_err(|e| format!("rules write: {}", e))?;

    Ok(())
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
