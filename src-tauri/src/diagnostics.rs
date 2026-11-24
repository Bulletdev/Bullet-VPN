use crate::permissions;
use std::fs;
use std::process::Command;

/// Generate a full diagnostic report collecting system, network,
/// VPN, DNS, firewall and SSH tunnel information.
/// Sensitive values (PrivateKey, PresharedKey) are redacted.
pub fn generate_report() -> String {
    let mut report = String::new();

    report.push_str(&section("Bullet VPN Diagnostic Report"));

    // -- System Info --
    report.push_str(&section("System Info"));
    report.push_str(&format!("App version: {}\n", env!("CARGO_PKG_VERSION")));
    report.push_str(&format!("OS: {}\n", read_os_info()));
    report.push_str(&format!("Kernel: {}\n", run_cmd("uname", &["-r"]).trim()));

    // -- Network Interfaces --
    report.push_str(&section("Network Interfaces"));
    report.push_str(&run_cmd("ip", &["addr", "show"]));

    // -- WireGuard Status --
    report.push_str(&section("WireGuard Status"));
    let priv_cmd = permissions::priv_cmd();
    let wg_output = run_cmd(priv_cmd, &["wg", "show"]);
    if wg_output.trim().is_empty() || wg_output.contains("failed to run") {
        report.push_str("No WireGuard interface active\n");
    } else {
        report.push_str(&wg_output);
    }

    // -- Routes --
    report.push_str(&section("Routes"));
    report.push_str(&run_cmd("ip", &["route"]));

    // -- DNS Configuration --
    report.push_str(&section("DNS Configuration"));
    match fs::read_to_string("/etc/resolv.conf") {
        Ok(content) => report.push_str(&content),
        Err(e) => report.push_str(&format!("(could not read /etc/resolv.conf: {})\n", e)),
    }

    // -- Firewall Rules --
    report.push_str(&section("Firewall Rules"));
    report.push_str("-- filter table --\n");
    report.push_str(&run_cmd(
        priv_cmd,
        &["iptables", "-L", "-n", "--line-numbers"],
    ));
    report.push_str("\n-- nat table --\n");
    report.push_str(&run_cmd(priv_cmd, &["iptables", "-t", "nat", "-L", "-n"]));

    // -- Active SSH Tunnels --
    report.push_str(&section("Active SSH Tunnels"));
    report.push_str(&list_ssh_tunnels());

    sanitize(&report)
}

/// Ping a host (or host:port endpoint) and return the round-trip
/// time in milliseconds.
pub fn ping_host(host: &str) -> Result<u32, String> {
    let ip = host.split(':').next().unwrap_or(host);

    let output = Command::new("ping")
        .args(["-c", "1", "-W", "2", ip])
        .output()
        .map_err(|e| format!("ping failed: {}", e))?;

    if !output.status.success() {
        return Err("Host unreachable".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(pos) = line.find("time=") {
            let time_str = &line[pos + 5..];
            if let Some(end) = time_str.find(' ') {
                if let Ok(ms) = time_str[..end].parse::<f64>() {
                    return Ok(ms as u32);
                }
            }
        }
    }
    Ok(0)
}

// -- internal helpers ------------------------------------------------

fn section(title: &str) -> String {
    format!("\n=== {} ===\n", title)
}

fn run_cmd(cmd: &str, args: &[&str]) -> String {
    match Command::new(cmd).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.success() {
                stdout.to_string()
            } else {
                format!("(error: {})\n", stderr.trim())
            }
        }
        Err(e) => format!("(failed to run: {})\n", e),
    }
}

fn read_os_info() -> String {
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(pretty) = line.strip_prefix("PRETTY_NAME=") {
                return pretty.trim_matches('"').to_string();
            }
        }
    }
    run_cmd("uname", &["-a"]).trim().to_string()
}

fn list_ssh_tunnels() -> String {
    let pid_dir = dirs::config_dir()
        .unwrap_or_default()
        .join("bullet-vpn")
        .join("ssh-pids");

    let entries = match fs::read_dir(&pid_dir) {
        Ok(e) => e,
        Err(_) => return "No SSH PID directory found\n".to_string(),
    };

    let mut found = false;
    let mut out = String::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("pid") {
            continue;
        }
        let tunnel_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let pid_str = fs::read_to_string(&path).unwrap_or_default();
        let pid_str = pid_str.trim();

        let alive = if let Ok(pid) = pid_str.parse::<u32>() {
            std::path::Path::new(&format!("/proc/{}", pid)).exists()
        } else {
            false
        };

        let status = if alive { "RUNNING" } else { "DEAD" };
        out.push_str(&format!(
            "Tunnel {} - PID {} - {}\n",
            tunnel_id, pid_str, status
        ));
        found = true;
    }

    if !found {
        out.push_str("No active SSH tunnels\n");
    }
    out
}

fn sanitize(report: &str) -> String {
    report
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("PrivateKey") || trimmed.starts_with("PresharedKey") {
                if let Some(pos) = line.find('=') {
                    return format!("{}= [REDACTED]", &line[..=pos]);
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
