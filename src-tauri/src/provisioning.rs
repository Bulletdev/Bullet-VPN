use std::process::Command;

fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

pub fn list_ssh_keys() -> Vec<String> {
    let ssh_dir = match dirs::home_dir() {
        Some(h) => h.join(".ssh"),
        None => return vec![],
    };

    let entries = match std::fs::read_dir(&ssh_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let mut keys = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if !name.ends_with(".pub") {
            continue;
        }

        let private_name = name.trim_end_matches(".pub");
        let private_path = ssh_dir.join(private_name);
        if private_path.exists() {
            keys.push(format!("~/.ssh/{}", private_name));
        }
    }

    keys.sort();
    keys
}

pub fn register_peer(
    ssh_host: &str,
    ssh_user: &str,
    ssh_key: &str,
    server_iface: &str,
    client_pubkey: &str,
    client_allowed_ips: &str,
) -> Result<String, String> {
    let remote_cmd = format!(
        "sudo wg set {} peer {} allowed-ips {} && sudo wg-quick save {}",
        shell_escape(server_iface),
        shell_escape(client_pubkey),
        shell_escape(client_allowed_ips),
        shell_escape(server_iface),
    );

    let mut args = vec![
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "ConnectTimeout=10".to_string(),
        "-o".to_string(),
        "BatchMode=yes".to_string(),
    ];

    if !ssh_key.is_empty() {
        args.push("-i".to_string());
        args.push(expand_home(ssh_key));
    }

    args.push(format!("{}@{}", ssh_user, ssh_host));
    args.push(remote_cmd);

    let output = Command::new("ssh")
        .args(&args)
        .output()
        .map_err(|e| format!("ssh falhou: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!(
            "Registro falhou (exit {}): {}{}",
            output.status.code().unwrap_or(-1),
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!("\n{}", stdout.trim())
            }
        ));
    }

    Ok(format!(
        "Peer registrado com sucesso no servidor.{}",
        if stdout.trim().is_empty() {
            String::new()
        } else {
            format!("\n{}", stdout.trim())
        }
    ))
}

pub fn check_server_connectivity(host: &str, port: u16) -> Result<String, String> {
    let output = Command::new("nc")
        .args(["-uzw3", host, &port.to_string()])
        .output()
        .or_else(|_| {
            Command::new("bash")
                .args([
                    "-c",
                    &format!(
                        "echo -n '' | timeout 3 socat - UDP:{}:{},connect-timeout=3 2>&1 || echo unreachable",
                        host, port
                    ),
                ])
                .output()
        })
        .map_err(|e| format!("Teste falhou: {}", e))?;

    if output.status.success() {
        Ok(format!("{}:{} acessivel via UDP", host, port))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "{}:{} nao acessivel: {}",
            host,
            port,
            stderr.trim()
        ))
    }
}

fn shell_escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len() + 2);
    escaped.push('\'');
    for c in s.chars() {
        if c == '\'' {
            escaped.push_str("'\\''");
        } else {
            escaped.push(c);
        }
    }
    escaped.push('\'');
    escaped
}
