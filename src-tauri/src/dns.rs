use crate::permissions;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn marker_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("bullet-vpn")
        .join(".dns-protected")
}

pub fn is_active() -> bool {
    marker_path().exists()
}

pub fn protect(dns_server: &str) -> Result<(), String> {
    let p = permissions::priv_cmd();
    let dst = format!("{}:53", dns_server);

    run(p, &["iptables", "-t", "nat", "-A", "OUTPUT",
        "-p", "udp", "--dport", "53", "!", "-d", dns_server,
        "-j", "DNAT", "--to-destination", &dst])?;

    run(p, &["iptables", "-t", "nat", "-A", "OUTPUT",
        "-p", "tcp", "--dport", "53", "!", "-d", dns_server,
        "-j", "DNAT", "--to-destination", &dst])?;

    fs::write(marker_path(), dns_server).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn restore() -> Result<(), String> {
    let p = permissions::priv_cmd();

    let server = fs::read_to_string(marker_path()).unwrap_or_default();
    let server = server.trim();

    if !server.is_empty() {
        let dst = format!("{}:53", server);

        let _ = run(p, &["iptables", "-t", "nat", "-D", "OUTPUT",
            "-p", "udp", "--dport", "53", "!", "-d", server,
            "-j", "DNAT", "--to-destination", &dst]);

        let _ = run(p, &["iptables", "-t", "nat", "-D", "OUTPUT",
            "-p", "tcp", "--dport", "53", "!", "-d", server,
            "-j", "DNAT", "--to-destination", &dst]);
    }

    fs::remove_file(marker_path()).ok();
    Ok(())
}

pub fn check_leak() -> Result<Vec<String>, String> {
    let content = fs::read_to_string("/etc/resolv.conf")
        .map_err(|e| e.to_string())?;

    let servers: Vec<String> = content
        .lines()
        .filter_map(|l| l.strip_prefix("nameserver "))
        .map(|s| s.trim().to_string())
        .collect();

    Ok(servers)
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
