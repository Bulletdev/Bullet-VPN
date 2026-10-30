use crate::permissions;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const CHAIN: &str = "BULLET_VPN_KS";

fn marker_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("bullet-vpn")
        .join(".killswitch-active")
}

pub fn is_active() -> bool {
    marker_path().exists()
}

pub fn enable(iface: &str, endpoint_ip: &str) -> Result<(), String> {
    let p = permissions::priv_cmd();

    let _ = run(p, &["iptables", "-N", CHAIN]);
    run(p, &["iptables", "-F", CHAIN])?;

    run(p, &["iptables", "-A", CHAIN, "-o", "lo", "-j", "ACCEPT"])?;
    run(p, &["iptables", "-A", CHAIN, "-o", iface, "-j", "ACCEPT"])?;
    run(
        p,
        &["iptables", "-A", CHAIN, "-d", endpoint_ip, "-j", "ACCEPT"],
    )?;

    for net in &["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"] {
        run(p, &["iptables", "-A", CHAIN, "-d", net, "-j", "ACCEPT"])?;
    }

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
    run(
        p,
        &[
            "iptables", "-A", CHAIN, "-p", "udp", "--dport", "67:68", "-j", "ACCEPT",
        ],
    )?;
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

    let _ = run(p, &["iptables", "-D", "OUTPUT", "-j", CHAIN]);
    run(p, &["iptables", "-I", "OUTPUT", "1", "-j", CHAIN])?;

    fs::write(marker_path(), format!("{}\n{}", iface, endpoint_ip)).ok();
    Ok(())
}

pub fn disable() -> Result<(), String> {
    let p = permissions::priv_cmd();

    let _ = run(p, &["iptables", "-D", "OUTPUT", "-j", CHAIN]);
    let _ = run(p, &["iptables", "-F", CHAIN]);
    let _ = run(p, &["iptables", "-X", CHAIN]);

    fs::remove_file(marker_path()).ok();
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
