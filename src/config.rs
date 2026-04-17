use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub ssh_usernames: HashMap<String, String>,
    /// Credentials keyed by "service:dns_name" (e.g. "rdp:myhost.tail1234.ts.net")
    #[serde(default)]
    pub credentials: HashMap<String, Credentials>,
}

impl AppConfig {
    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("cosmic-tailscale")
            .join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            if let Err(e) = std::fs::write(&path, json) {
                eprintln!("Failed to save config: {e}");
            }
        }
    }

    pub fn get_creds(&self, service: &str, dns_name: &str) -> Option<&Credentials> {
        let key = format!("{service}:{dns_name}");
        self.credentials.get(&key)
    }

    pub fn save_creds(&mut self, service: &str, dns_name: &str, creds: Credentials) {
        let key = format!("{service}:{dns_name}");
        self.credentials.insert(key, creds);
        self.save();
    }
}
