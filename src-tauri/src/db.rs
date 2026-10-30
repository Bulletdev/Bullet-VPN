use crate::profiles::{Profile, VpnType};
use crate::ssh::{SshTunnel, TunnelType};
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

pub struct Database {
    conn: Mutex<Connection>,
    cipher: Aes256Gcm,
}

fn db_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bullet-vpn");
    fs::create_dir_all(&dir).ok();
    dir
}

impl Database {
    pub fn open() -> Result<Self, String> {
        let path = db_dir().join("bullet-vpn.db");
        let conn = Connection::open(&path).map_err(|e| format!("sqlite open: {e}"))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS encryption_key (
                id INTEGER PRIMARY KEY,
                key_data TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS profiles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                vpn_type TEXT NOT NULL DEFAULT 'wireguard',
                endpoint TEXT NOT NULL DEFAULT '',
                server_pubkey TEXT NOT NULL DEFAULT '',
                private_key_enc TEXT NOT NULL DEFAULT '',
                public_key TEXT NOT NULL DEFAULT '',
                address TEXT NOT NULL DEFAULT '',
                allowed_ips TEXT NOT NULL DEFAULT '',
                dns TEXT NOT NULL DEFAULT '',
                ovpn_config_path TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS ssh_tunnels (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                tunnel_type TEXT NOT NULL DEFAULT 'local',
                local_port INTEGER NOT NULL DEFAULT 0,
                remote_host TEXT NOT NULL DEFAULT '127.0.0.1',
                remote_port INTEGER NOT NULL DEFAULT 0,
                ssh_host TEXT NOT NULL DEFAULT '',
                ssh_user TEXT NOT NULL DEFAULT 'root',
                ssh_key TEXT NOT NULL DEFAULT ''
            );",
        )
        .map_err(|e| format!("sqlite migrate: {e}"))?;

        let cipher = Self::load_or_create_cipher(&conn)?;

        let db = Database {
            conn: Mutex::new(conn),
            cipher,
        };
        db.auto_migrate_json()?;
        Ok(db)
    }

    fn load_or_create_cipher(conn: &Connection) -> Result<Aes256Gcm, String> {
        let existing: Option<String> = conn
            .query_row(
                "SELECT key_data FROM encryption_key WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .ok();

        let key_bytes: [u8; 32] = if let Some(b64) = existing {
            B64.decode(&b64)
                .map_err(|e| format!("decode master key: {e}"))?
                .try_into()
                .map_err(|_| "master key not 32 bytes".to_string())?
        } else {
            let key = Aes256Gcm::generate_key(OsRng);
            let b64 = B64.encode(key.as_slice());
            conn.execute(
                "INSERT INTO encryption_key (id, key_data) VALUES (1, ?1)",
                params![b64],
            )
            .map_err(|e| format!("store master key: {e}"))?;
            key.into()
        };

        Ok(Aes256Gcm::new_from_slice(&key_bytes).unwrap())
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn encrypt(&self, plaintext: &str) -> Result<String, String> {
        if plaintext.is_empty() {
            return Ok(String::new());
        }
        let nonce_bytes = rand::random::<[u8; 12]>();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| format!("encrypt: {e}"))?;
        let mut combined = nonce_bytes.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(B64.encode(&combined))
    }

    fn decrypt(&self, enc_b64: &str) -> Result<String, String> {
        if enc_b64.is_empty() {
            return Ok(String::new());
        }
        let combined = B64
            .decode(enc_b64)
            .map_err(|e| format!("decode: {e}"))?;
        if combined.len() < 12 {
            return Err("ciphertext too short".into());
        }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("decrypt: {e}"))?;
        String::from_utf8(plaintext).map_err(|e| format!("utf8: {e}"))
    }

    // ── Profiles ────────────────────────────────────────────────────

    pub fn list_profiles(&self) -> Vec<Profile> {
        let conn = self.conn();
        let mut stmt = match conn.prepare(
            "SELECT id, name, vpn_type, endpoint, server_pubkey, private_key_enc,
                    public_key, address, allowed_ips, dns, ovpn_config_path
             FROM profiles ORDER BY name",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
            ))
        })
        .ok()
        .map(|rows| {
            rows.filter_map(|r| r.ok())
                .filter_map(
                    |(id, name, vt, endpoint, spk, pk_enc, pubk, addr, aips, dns, ovpn)| {
                        let private_key = self.decrypt(&pk_enc).unwrap_or_default();
                        Some(Profile {
                            id,
                            name,
                            vpn_type: match vt.as_str() {
                                "openvpn" => VpnType::Openvpn,
                                _ => VpnType::Wireguard,
                            },
                            endpoint,
                            server_pubkey: spk,
                            private_key,
                            public_key: pubk,
                            address: addr,
                            allowed_ips: aips,
                            dns,
                            ovpn_config_path: ovpn,
                        })
                    },
                )
                .collect()
        })
        .unwrap_or_default()
    }

    pub fn get_profile(&self, id: &str) -> Option<Profile> {
        self.list_profiles().into_iter().find(|p| p.id == id)
    }

    pub fn save_profile(&self, profile: &Profile) -> Result<(), String> {
        let pk_enc = self.encrypt(&profile.private_key)?;
        let vt = match profile.vpn_type {
            VpnType::Openvpn => "openvpn",
            VpnType::Wireguard => "wireguard",
        };
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO profiles
                 (id, name, vpn_type, endpoint, server_pubkey, private_key_enc,
                  public_key, address, allowed_ips, dns, ovpn_config_path)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    profile.id,
                    profile.name,
                    vt,
                    profile.endpoint,
                    profile.server_pubkey,
                    pk_enc,
                    profile.public_key,
                    profile.address,
                    profile.allowed_ips,
                    profile.dns,
                    profile.ovpn_config_path,
                ],
            )
            .map_err(|e| format!("save profile: {e}"))?;
        Ok(())
    }

    pub fn remove_profile(&self, id: &str) -> Result<(), String> {
        self.conn()
            .execute("DELETE FROM profiles WHERE id = ?1", params![id])
            .map_err(|e| format!("remove profile: {e}"))?;
        Ok(())
    }

    // ── SSH Tunnels ─────────────────────────────────────────────────

    pub fn list_ssh_tunnels(&self) -> Vec<SshTunnel> {
        let conn = self.conn();
        let mut stmt = match conn.prepare(
            "SELECT id, name, tunnel_type, local_port, remote_host, remote_port,
                    ssh_host, ssh_user, ssh_key
             FROM ssh_tunnels ORDER BY name",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })
        .ok()
        .map(|rows| {
            rows.filter_map(|r| r.ok())
                .map(
                    |(id, name, tt, lp, rh, rp, sh, su, sk)| SshTunnel {
                        id,
                        name,
                        tunnel_type: match tt.as_str() {
                            "remote" => TunnelType::Remote,
                            "dynamic" => TunnelType::Dynamic,
                            _ => TunnelType::Local,
                        },
                        local_port: lp as u16,
                        remote_host: rh,
                        remote_port: rp as u16,
                        ssh_host: sh,
                        ssh_user: su,
                        ssh_key: sk,
                    },
                )
                .collect()
        })
        .unwrap_or_default()
    }

    pub fn get_ssh_tunnel(&self, id: &str) -> Option<SshTunnel> {
        self.list_ssh_tunnels().into_iter().find(|t| t.id == id)
    }

    pub fn save_ssh_tunnel(&self, tunnel: &SshTunnel) -> Result<(), String> {
        let tt = match tunnel.tunnel_type {
            TunnelType::Local => "local",
            TunnelType::Remote => "remote",
            TunnelType::Dynamic => "dynamic",
        };
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO ssh_tunnels
                 (id, name, tunnel_type, local_port, remote_host, remote_port,
                  ssh_host, ssh_user, ssh_key)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    tunnel.id,
                    tunnel.name,
                    tt,
                    tunnel.local_port as i64,
                    tunnel.remote_host,
                    tunnel.remote_port as i64,
                    tunnel.ssh_host,
                    tunnel.ssh_user,
                    tunnel.ssh_key,
                ],
            )
            .map_err(|e| format!("save tunnel: {e}"))?;
        Ok(())
    }

    pub fn remove_ssh_tunnel(&self, id: &str) -> Result<(), String> {
        self.conn()
            .execute("DELETE FROM ssh_tunnels WHERE id = ?1", params![id])
            .map_err(|e| format!("remove tunnel: {e}"))?;
        Ok(())
    }

    // ── Auto-migrate from JSON ──────────────────────────────────────

    fn auto_migrate_json(&self) -> Result<(), String> {
        let dir = db_dir();

        let profiles_json = dir.join("profiles.json");
        if profiles_json.exists() {
            if let Ok(data) = fs::read_to_string(&profiles_json) {
                #[derive(serde::Deserialize)]
                struct LegacyStore {
                    profiles: Vec<Profile>,
                }
                if let Ok(legacy) = serde_json::from_str::<LegacyStore>(&data) {
                    for p in &legacy.profiles {
                        let _ = self.save_profile(p);
                    }
                }
            }
            let _ = fs::rename(&profiles_json, dir.join("profiles.json.bak"));
        }

        let tunnels_json = dir.join("ssh-tunnels.json");
        if tunnels_json.exists() {
            if let Ok(data) = fs::read_to_string(&tunnels_json) {
                #[derive(serde::Deserialize)]
                struct LegacyTunnels {
                    tunnels: Vec<SshTunnel>,
                }
                if let Ok(legacy) = serde_json::from_str::<LegacyTunnels>(&data) {
                    for t in &legacy.tunnels {
                        let _ = self.save_ssh_tunnel(t);
                    }
                }
            }
            let _ = fs::rename(&tunnels_json, dir.join("ssh-tunnels.json.bak"));
        }

        Ok(())
    }
}
