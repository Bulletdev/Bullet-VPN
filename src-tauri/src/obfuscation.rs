use std::process::Command;
use std::sync::Mutex;

static OBFUSCATION_PROCESS: Mutex<Option<u32>> = Mutex::new(None);

pub fn start_udp2tcp(local_port: u16, remote_endpoint: &str) -> Result<u16, String> {
    let listen_port = local_port;
    if listen_port == 0 {
        return Err("obfuscation_port must be set".into());
    }

    let parts: Vec<&str> = remote_endpoint.split(':').collect();
    if parts.len() != 2 {
        return Err("Invalid endpoint format, expected host:port".into());
    }

    let child = Command::new("udp2raw")
        .args([
            "--client",
            "-l",
            &format!("127.0.0.1:{}", listen_port),
            "-r",
            remote_endpoint,
            "--raw-mode",
            "faketcp",
            "-a",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("udp2raw failed to start: {e}"))?;

    let pid = child.id();
    *OBFUSCATION_PROCESS.lock().unwrap_or_else(|e| e.into_inner()) = Some(pid);

    Ok(listen_port)
}

pub fn start_shadowsocks(
    local_port: u16,
    remote_host: &str,
    remote_port: u16,
    password: &str,
) -> Result<u16, String> {
    let listen_port = local_port;
    if listen_port == 0 {
        return Err("obfuscation_port must be set".into());
    }

    let child = Command::new("ss-local")
        .args([
            "-s",
            remote_host,
            "-p",
            &remote_port.to_string(),
            "-l",
            &listen_port.to_string(),
            "-k",
            password,
            "-m",
            "aes-256-gcm",
            "-U",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("ss-local failed to start: {e}"))?;

    let pid = child.id();
    *OBFUSCATION_PROCESS.lock().unwrap_or_else(|e| e.into_inner()) = Some(pid);

    Ok(listen_port)
}

pub fn stop() {
    let mut proc = OBFUSCATION_PROCESS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(pid) = proc.take() {
        let _ = Command::new("kill").arg(pid.to_string()).output();
    }
}

pub fn start_obfs4(
    local_port: u16,
    bridge_addr: &str,
    cert: &str,
    iat_mode: u8,
) -> Result<u16, String> {
    if local_port == 0 {
        return Err("obfuscation_port must be set".into());
    }

    let state_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("bullet-vpn")
        .join("obfs4-state");
    std::fs::create_dir_all(&state_dir).ok();

    let child = Command::new("obfs4proxy")
        .env("TOR_PT_MANAGED_TRANSPORT_VER", "1")
        .env("TOR_PT_CLIENT_TRANSPORTS", "obfs4")
        .env("TOR_PT_STATE_LOCATION", state_dir.to_string_lossy().as_ref())
        .env(
            "TOR_PT_EXIT_ON_STDIN_CLOSE",
            "0",
        )
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("obfs4proxy failed to start: {e}"))?;

    let pid = child.id();
    *OBFUSCATION_PROCESS.lock().unwrap_or_else(|e| e.into_inner()) = Some(pid);

    // obfs4proxy speaks the PT spec on stdout - in practice WireGuard
    // connects to bridge_addr through the SOCKS port obfs4proxy opens.
    // Store bridge params for the connect flow.
    let params_path = state_dir.join("bridge-line");
    let bridge_line = format!(
        "Bridge obfs4 {} cert={} iat-mode={}",
        bridge_addr, cert, iat_mode
    );
    let _ = std::fs::write(params_path, &bridge_line);

    Ok(local_port)
}

pub fn is_running() -> bool {
    let proc = OBFUSCATION_PROCESS.lock().unwrap_or_else(|e| e.into_inner());
    match *proc {
        Some(pid) => std::path::Path::new(&format!("/proc/{}", pid)).exists(),
        None => false,
    }
}
