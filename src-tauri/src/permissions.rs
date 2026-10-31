use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn marker_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bullet-vpn")
        .join(".sudoers-installed")
}

pub fn is_installed() -> bool {
    marker_path().exists()
}

pub fn priv_cmd() -> &'static str {
    if is_installed() {
        "sudo"
    } else {
        "pkexec"
    }
}

pub fn install() -> Result<(), String> {
    let user = std::env::var("USER").map_err(|_| "Variavel USER nao definida".to_string())?;

    let wg = find_bin("wg-quick").unwrap_or("/usr/bin/wg-quick".into());
    let wg_bin = find_bin("wg").unwrap_or("/usr/bin/wg".into());
    let ovpn = find_bin("openvpn").unwrap_or("/usr/sbin/openvpn".into());
    let ip = find_bin("ip").unwrap_or("/usr/sbin/ip".into());
    let killall = find_bin("killall").unwrap_or("/usr/bin/killall".into());
    let iptables = find_bin("iptables").unwrap_or("/usr/sbin/iptables".into());
    let modprobe = find_bin("modprobe").unwrap_or("/usr/sbin/modprobe".into());
    let dpkg = find_bin("dpkg").unwrap_or("/usr/bin/dpkg".into());
    let sh = find_bin("sh").unwrap_or("/bin/sh".into());
    let mkdir = find_bin("mkdir").unwrap_or("/bin/mkdir".into());
    let rmdir = find_bin("rmdir").unwrap_or("/bin/rmdir".into());

    let content = format!(
        "# Bullet VPN - passwordless VPN management\n\
         {user} ALL=(root) NOPASSWD: {wg}, {wg_bin}, {ovpn}, {ip}, /usr/bin/kill, \
         {killall}, {iptables}, {modprobe}, {dpkg}, {sh}, {mkdir}, {rmdir}\n"
    );

    let tmp = std::env::temp_dir().join("bullet-vpn-sudoers");
    fs::write(&tmp, &content).map_err(|e| e.to_string())?;

    let script = format!(
        "install -m 0440 '{}' /etc/sudoers.d/bullet-vpn && \
         visudo -cf /etc/sudoers.d/bullet-vpn || \
         {{ rm -f /etc/sudoers.d/bullet-vpn; exit 1; }}",
        tmp.display()
    );

    let output = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .output()
        .map_err(|e| format!("pkexec falhou: {}", e))?;

    fs::remove_file(&tmp).ok();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Instalacao falhou: {}", stderr.trim()));
    }

    fs::write(marker_path(), "installed").map_err(|e| e.to_string())?;
    Ok(())
}

pub fn uninstall() -> Result<(), String> {
    let output = Command::new("pkexec")
        .args(["rm", "-f", "/etc/sudoers.d/bullet-vpn"])
        .output()
        .map_err(|e| format!("pkexec falhou: {}", e))?;

    if !output.status.success() {
        return Err("Falha ao remover sudoers".to_string());
    }

    fs::remove_file(marker_path()).ok();
    Ok(())
}

fn find_bin(name: &str) -> Result<String, String> {
    let output = Command::new("which")
        .arg(name)
        .output()
        .map_err(|e| format!("which {} falhou: {}", name, e))?;

    if !output.status.success() {
        return Err(format!("{} nao encontrado", name));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
