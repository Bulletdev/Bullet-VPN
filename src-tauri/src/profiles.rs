use crate::db::Database;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

pub struct ProfileStore {
    db: Arc<Database>,
}

impl ProfileStore {
    pub fn new(db: Arc<Database>) -> Self {
        ProfileStore { db }
    }

    pub fn list(&self) -> Vec<Profile> {
        self.db.list_profiles()
    }

    pub fn get(&self, id: &str) -> Option<Profile> {
        self.db.get_profile(id)
    }

    pub fn add(&mut self, profile: Profile) {
        let _ = self.db.save_profile(&profile);
    }

    pub fn remove(&mut self, id: &str) {
        let _ = self.db.remove_profile(id);
    }

    pub fn save(&self) -> Result<(), String> {
        Ok(())
    }
}
