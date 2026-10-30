mod db;
mod dns;
mod killswitch;
mod openvpn;
mod permissions;
mod profiles;
mod ssh;
mod wireguard;

use profiles::{Profile, ProfileStore, VpnType};
use ssh::{SshTunnel, TunnelStore};
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, State};

struct AppState {
    store: Mutex<ProfileStore>,
    tunnel_store: Mutex<TunnelStore>,
    active_interface: Mutex<Option<String>>,
    active_ovpn: Mutex<Option<String>>,
}

// ── Profile commands ────────────────────────────────────────────────

#[tauri::command]
fn list_profiles(state: State<AppState>) -> Result<Vec<Profile>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(store.list())
}

#[tauri::command]
fn add_profile(state: State<AppState>, profile: Profile) -> Result<(), String> {
    let mut store = state.store.lock().map_err(|e| e.to_string())?;
    store.add(profile);
    store.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_profile(state: State<AppState>, profile_id: String) -> Result<(), String> {
    let mut store = state.store.lock().map_err(|e| e.to_string())?;
    store.remove(&profile_id);
    store.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn import_ovpn(state: State<AppState>, profile_id: String, content: String) -> Result<(), String> {
    openvpn::import_config(&profile_id, &content)?;

    let mut store = state.store.lock().map_err(|e| e.to_string())?;
    if let Some(mut profile) = store.get(&profile_id) {
        let dir = dirs::config_dir()
            .unwrap_or_default()
            .join("bullet-vpn")
            .join("openvpn")
            .join(format!("{}.ovpn", profile_id));
        profile.ovpn_config_path = dir.to_string_lossy().to_string();
        store.add(profile);
        store.save().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Key generation commands ─────────────────────────────────────────

#[derive(serde::Serialize)]
struct Keypair {
    private_key: String,
    public_key: String,
}

#[tauri::command]
fn generate_keypair() -> Keypair {
    use base64::Engine;
    use x25519_dalek::{PublicKey, StaticSecret};

    let secret = StaticSecret::random_from_rng(rand::rngs::OsRng);
    let public = PublicKey::from(&secret);

    let encoder = base64::engine::general_purpose::STANDARD;
    Keypair {
        private_key: encoder.encode(secret.to_bytes()),
        public_key: encoder.encode(public.as_bytes()),
    }
}

#[tauri::command]
fn derive_public_key(private_key: String) -> Result<String, String> {
    use base64::Engine;
    use x25519_dalek::{PublicKey, StaticSecret};

    let encoder = base64::engine::general_purpose::STANDARD;
    let bytes: [u8; 32] = encoder
        .decode(&private_key)
        .map_err(|e| format!("Base64 invalido: {}", e))?
        .try_into()
        .map_err(|_| "Chave deve ter 32 bytes".to_string())?;

    let secret = StaticSecret::from(bytes);
    let public = PublicKey::from(&secret);
    Ok(encoder.encode(public.as_bytes()))
}

// ── VPN commands ────────────────────────────────────────────────────

#[tauri::command]
fn connect(state: State<AppState>, profile_id: String) -> Result<(), String> {
    let profile = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store
            .get(&profile_id)
            .ok_or_else(|| "Perfil nao encontrado".to_string())?
    };

    match profile.vpn_type {
        VpnType::Wireguard => {
            let iface_name = format!("bvpn-{}", &profile.id[..8]);
            let config_path = wireguard::write_config(&iface_name, &profile)?;
            wireguard::up(&config_path)?;
            let mut active = state.active_interface.lock().map_err(|e| e.to_string())?;
            *active = Some(iface_name);
        }
        VpnType::Openvpn => {
            openvpn::up(&profile.id)?;
            let mut active = state.active_ovpn.lock().map_err(|e| e.to_string())?;
            *active = Some(profile.id.clone());
        }
    }
    Ok(())
}

#[tauri::command]
fn disconnect(state: State<AppState>) -> Result<(), String> {
    if killswitch::is_active() {
        let _ = killswitch::disable();
    }
    if dns::is_active() {
        let _ = dns::restore();
    }

    let wg_iface = {
        let mut active = state.active_interface.lock().map_err(|e| e.to_string())?;
        active.take()
    };
    if let Some(iface) = wg_iface {
        wireguard::down(&iface)?;
    }

    let ovpn_id = {
        let mut active = state.active_ovpn.lock().map_err(|e| e.to_string())?;
        active.take()
    };
    if let Some(id) = ovpn_id {
        openvpn::down(&id)?;
    }

    Ok(())
}

#[tauri::command]
fn teardown_conflicts() -> Result<Vec<String>, String> {
    wireguard::teardown_conflicts()
}

#[derive(serde::Serialize)]
struct ConnectionStatus {
    connected: bool,
    vpn_type: String,
    interface_name: String,
    handshake_ago: String,
    transfer_rx: String,
    transfer_tx: String,
    endpoint: String,
}

#[tauri::command]
fn get_status(state: State<AppState>) -> Result<Option<ConnectionStatus>, String> {
    let wg_active = state.active_interface.lock().map_err(|e| e.to_string())?;
    if let Some(iface) = wg_active.as_deref() {
        if let Some(st) = wireguard::detailed_status(iface) {
            return Ok(Some(ConnectionStatus {
                connected: st.connected,
                vpn_type: "wireguard".to_string(),
                interface_name: st.interface_name,
                handshake_ago: st.handshake_ago,
                transfer_rx: st.transfer_rx,
                transfer_tx: st.transfer_tx,
                endpoint: st.endpoint,
            }));
        }
    }

    let ovpn_active = state.active_ovpn.lock().map_err(|e| e.to_string())?;
    if let Some(id) = ovpn_active.as_deref() {
        if openvpn::is_running(id) {
            let (rx, tx) = read_tun_transfer();
            return Ok(Some(ConnectionStatus {
                connected: true,
                vpn_type: "openvpn".to_string(),
                interface_name: "tun0".to_string(),
                handshake_ago: "ativo".to_string(),
                transfer_rx: rx,
                transfer_tx: tx,
                endpoint: String::new(),
            }));
        }
    }

    Ok(None)
}

// ── Permissions commands ────────────────────────────────────────────

#[tauri::command]
fn check_permissions() -> bool {
    permissions::is_installed()
}

#[tauri::command]
fn install_permissions() -> Result<(), String> {
    permissions::install()
}

#[tauri::command]
fn uninstall_permissions() -> Result<(), String> {
    permissions::uninstall()
}

// ── Kill switch commands ────────────────────────────────────────────

#[tauri::command]
fn enable_killswitch(iface: String, endpoint_ip: String) -> Result<(), String> {
    killswitch::enable(&iface, &endpoint_ip)
}

#[tauri::command]
fn disable_killswitch() -> Result<(), String> {
    killswitch::disable()
}

#[tauri::command]
fn killswitch_status() -> bool {
    killswitch::is_active()
}

// ── DNS protection commands ─────────────────────────────────────────

#[tauri::command]
fn enable_dns_protection(dns_server: String) -> Result<(), String> {
    dns::protect(&dns_server)
}

#[tauri::command]
fn disable_dns_protection() -> Result<(), String> {
    dns::restore()
}

#[tauri::command]
fn dns_protection_status() -> bool {
    dns::is_active()
}

#[tauri::command]
fn check_dns_leak() -> Result<Vec<String>, String> {
    dns::check_leak()
}

// ── OpenVPN log command ─────────────────────────────────────────────

#[tauri::command]
fn read_ovpn_log(state: State<AppState>) -> String {
    let active = state.active_ovpn.lock().unwrap_or_else(|e| e.into_inner());
    match active.as_deref() {
        Some(id) => openvpn::read_log(id),
        None => String::new(),
    }
}

// ── SSH tunnel commands ─────────────────────────────────────────────

#[tauri::command]
fn list_ssh_tunnels(state: State<AppState>) -> Result<Vec<SshTunnel>, String> {
    let store = state.tunnel_store.lock().map_err(|e| e.to_string())?;
    Ok(store.list())
}

#[tauri::command]
fn add_ssh_tunnel(state: State<AppState>, tunnel: SshTunnel) -> Result<(), String> {
    let mut store = state.tunnel_store.lock().map_err(|e| e.to_string())?;
    store.add(tunnel);
    store.save()
}

#[tauri::command]
fn remove_ssh_tunnel(state: State<AppState>, tunnel_id: String) -> Result<(), String> {
    if ssh::is_running(&tunnel_id) {
        let _ = ssh::stop(&tunnel_id);
    }
    let mut store = state.tunnel_store.lock().map_err(|e| e.to_string())?;
    store.remove(&tunnel_id);
    store.save()
}

#[tauri::command]
fn start_ssh_tunnel(state: State<AppState>, tunnel_id: String) -> Result<u32, String> {
    let store = state.tunnel_store.lock().map_err(|e| e.to_string())?;
    let tunnel = store.get(&tunnel_id).ok_or("Tunnel nao encontrado")?;
    drop(store);
    ssh::start(&tunnel)
}

#[tauri::command]
fn stop_ssh_tunnel(tunnel_id: String) -> Result<(), String> {
    ssh::stop(&tunnel_id)
}

#[tauri::command]
fn check_ssh_tunnel(tunnel_id: String) -> bool {
    ssh::is_running(&tunnel_id)
}

// ── Helpers ─────────────────────────────────────────────────────────

fn read_tun_transfer() -> (String, String) {
    let content = match std::fs::read_to_string("/proc/net/dev") {
        Ok(c) => c,
        Err(_) => return ("0 B".to_string(), "0 B".to_string()),
    };

    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("tun0:") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 9 {
                let rx: u64 = parts[0].parse().unwrap_or(0);
                let tx: u64 = parts[8].parse().unwrap_or(0);
                return (format_bytes(rx), format_bytes(tx));
            }
        }
    }
    ("0 B".to_string(), "0 B".to_string())
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.2} GiB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.2} MiB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.2} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

// ── Entry point ─────────────────────────────────────────────────────

pub fn run() {
    if killswitch::is_active() {
        let _ = killswitch::disable();
    }
    if dns::is_active() {
        let _ = dns::restore();
    }

    let database = Arc::new(db::Database::open().expect("failed to open database"));
    let store = ProfileStore::new(database.clone());
    let tunnel_store = TunnelStore::new(database);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            store: Mutex::new(store),
            tunnel_store: Mutex::new(tunnel_store),
            active_interface: Mutex::new(None),
            active_ovpn: Mutex::new(None),
        })
        .setup(|app| {
            let label = MenuItem::with_id(app, "label", "Bullet VPN v2.1.0", false, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "Mostrar", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&label, &show, &quit])?;

            TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { .. } = event {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            add_profile,
            remove_profile,
            import_ovpn,
            generate_keypair,
            derive_public_key,
            connect,
            disconnect,
            get_status,
            teardown_conflicts,
            check_permissions,
            install_permissions,
            uninstall_permissions,
            enable_killswitch,
            disable_killswitch,
            killswitch_status,
            enable_dns_protection,
            disable_dns_protection,
            dns_protection_status,
            check_dns_leak,
            list_ssh_tunnels,
            add_ssh_tunnel,
            remove_ssh_tunnel,
            start_ssh_tunnel,
            stop_ssh_tunnel,
            check_ssh_tunnel,
            read_ovpn_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
