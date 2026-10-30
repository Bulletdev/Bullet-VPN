use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VpnType {
    Wireguard,
    Openvpn,
}

impl Default for VpnType {
    fn default() -> Self {
        VpnType::Wireguard
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub vpn_type: VpnType,
    pub endpoint: String,
    #[serde(default)]
    pub server_pubkey: String,
    #[serde(default)]
    pub private_key: String,
    #[serde(default)]
    pub public_key: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub allowed_ips: String,
    #[serde(default)]
    pub dns: String,
    #[serde(default)]
    pub ovpn_config_path: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProfileStore {
    profiles: Vec<Profile>,
}

impl ProfileStore {
    fn config_path() -> PathBuf {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("bullet-vpn");
        fs::create_dir_all(&dir).ok();
        dir.join("profiles.json")
    }

    pub fn load() -> Result<Self, io::Error> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = fs::read_to_string(&path)?;
        serde_json::from_str(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self) -> Result<(), io::Error> {
        let path = Self::config_path();
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)
    }

    pub fn list(&self) -> Vec<Profile> {
        self.profiles.clone()
    }

    pub fn get(&self, id: &str) -> Option<Profile> {
        self.profiles.iter().find(|p| p.id == id).cloned()
    }

    pub fn add(&mut self, profile: Profile) {
        self.profiles.retain(|p| p.id != profile.id);
        self.profiles.push(profile);
    }

    pub fn remove(&mut self, id: &str) {
        self.profiles.retain(|p| p.id != id);
    }
}
