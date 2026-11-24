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
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA busy_timeout=5000;",
        )
        .ok();

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
                ssh_key TEXT NOT NULL DEFAULT '',
                profile_id TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .map_err(|e| format!("sqlite migrate: {e}"))?;

        let cipher = Self::load_or_create_cipher(&conn)?;

        // migrate: add profile_id column if missing
        let has_profile_id: bool = conn
            .prepare("SELECT profile_id FROM ssh_tunnels LIMIT 0")
            .is_ok();
        if !has_profile_id {
            conn.execute_batch(
                "ALTER TABLE ssh_tunnels ADD COLUMN profile_id TEXT NOT NULL DEFAULT '';",
            )
            .ok();
        }

        // migrate: add multihop/obfuscation columns if missing
        let has_multihop: bool = conn
            .prepare("SELECT multihop FROM profiles LIMIT 0")
            .is_ok();
        if !has_multihop {
            conn.execute_batch(
                "ALTER TABLE profiles ADD COLUMN multihop INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE profiles ADD COLUMN entry_profile_id TEXT NOT NULL DEFAULT '';
                 ALTER TABLE profiles ADD COLUMN obfuscation TEXT NOT NULL DEFAULT 'off';
                 ALTER TABLE profiles ADD COLUMN obfuscation_port INTEGER NOT NULL DEFAULT 0;",
            )
            .ok();
        }

        // migrate: add mtu column if missing
        let has_mtu: bool = conn.prepare("SELECT mtu FROM profiles LIMIT 0").is_ok();
        if !has_mtu {
            conn.execute_batch("ALTER TABLE profiles ADD COLUMN mtu INTEGER NOT NULL DEFAULT 0;")
                .ok();
        }

        // migrate: add preshared_key and daita columns if missing
        let has_psk: bool = conn
            .prepare("SELECT preshared_key_enc FROM profiles LIMIT 0")
            .is_ok();
        if !has_psk {
            conn.execute_batch(
                "ALTER TABLE profiles ADD COLUMN preshared_key_enc TEXT NOT NULL DEFAULT '';
                 ALTER TABLE profiles ADD COLUMN daita INTEGER NOT NULL DEFAULT 0;",
            )
            .ok();
        }

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
        let combined = B64.decode(enc_b64).map_err(|e| format!("decode: {e}"))?;
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
                    public_key, address, allowed_ips, dns, ovpn_config_path,
                    multihop, entry_profile_id, obfuscation, obfuscation_port, mtu,
                    preshared_key_enc, daita
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
                row.get::<_, i64>(11).unwrap_or(0),
                row.get::<_, String>(12).unwrap_or_default(),
                row.get::<_, String>(13)
                    .unwrap_or_else(|_| "off".to_string()),
                row.get::<_, i64>(14).unwrap_or(0),
                row.get::<_, i64>(15).unwrap_or(0),
                row.get::<_, String>(16).unwrap_or_default(),
                row.get::<_, i64>(17).unwrap_or(0),
            ))
        })
        .ok()
        .map(|rows| {
            rows.filter_map(|r| r.ok())
                .map(
                    |(
                        id,
                        name,
                        vt,
                        endpoint,
                        spk,
                        pk_enc,
                        pubk,
                        addr,
                        aips,
                        dns,
                        ovpn,
                        mh,
                        entry_pid,
                        obfs,
                        obfs_port,
                        mtu_val,
                        psk_enc,
                        daita_val,
                    )| {
                        let private_key = self.decrypt(&pk_enc).unwrap_or_default();
                        let preshared_key = self.decrypt(&psk_enc).unwrap_or_default();
                        Profile {
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
                            multihop: mh != 0,
                            entry_profile_id: entry_pid,
                            obfuscation: obfs,
                            obfuscation_port: obfs_port as u16,
                            mtu: mtu_val as u16,
                            preshared_key,
                            daita: daita_val != 0,
                        }
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
        let psk_enc = self.encrypt(&profile.preshared_key)?;
        let vt = match profile.vpn_type {
            VpnType::Openvpn => "openvpn",
            VpnType::Wireguard => "wireguard",
        };
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO profiles
                 (id, name, vpn_type, endpoint, server_pubkey, private_key_enc,
                  public_key, address, allowed_ips, dns, ovpn_config_path,
                  multihop, entry_profile_id, obfuscation, obfuscation_port, mtu,
                  preshared_key_enc, daita)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
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
                    profile.multihop as i64,
                    profile.entry_profile_id,
                    profile.obfuscation,
                    profile.obfuscation_port as i64,
                    profile.mtu as i64,
                    psk_enc,
                    profile.daita as i64,
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
                    ssh_host, ssh_user, ssh_key, profile_id
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
                row.get::<_, String>(9)?,
            ))
        })
        .ok()
        .map(|rows| {
            rows.filter_map(|r| r.ok())
                .map(|(id, name, tt, lp, rh, rp, sh, su, sk, pid)| SshTunnel {
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
                    profile_id: pid,
                })
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
                  ssh_host, ssh_user, ssh_key, profile_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
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
                    tunnel.profile_id,
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

    #[allow(dead_code)]
    pub fn list_ssh_tunnels_by_profile(&self, profile_id: &str) -> Vec<SshTunnel> {
        let conn = self.conn();
        let mut stmt = match conn.prepare(
            "SELECT id, name, tunnel_type, local_port, remote_host, remote_port,
                    ssh_host, ssh_user, ssh_key, profile_id
             FROM ssh_tunnels WHERE profile_id = ?1 ORDER BY name",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        stmt.query_map(params![profile_id], |row| {
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
                row.get::<_, String>(9)?,
            ))
        })
        .ok()
        .map(|rows| {
            rows.filter_map(|r| r.ok())
                .map(|(id, name, tt, lp, rh, rp, sh, su, sk, pid)| SshTunnel {
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
                    profile_id: pid,
                })
                .collect()
        })
        .unwrap_or_default()
    }

    // ── Settings ────────────────────────────────────────────────────

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.conn()
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .ok()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(|e| format!("save setting: {e}"))?;
        Ok(())
    }

    pub fn set_settings_batch(&self, pairs: &[(&str, &str)]) -> Result<(), String> {
        let conn = self.conn();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("begin tx: {e}"))?;
        for (key, value) in pairs {
            tx.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(|e| format!("save setting {key}: {e}"))?;
        }
        tx.commit().map_err(|e| format!("commit: {e}"))?;
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
