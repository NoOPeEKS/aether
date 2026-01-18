use std::{collections::HashMap, fs::OpenOptions, io::Write, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct AetherConfig {
    pub profiles: HashMap<String, BrokerProfile>,
    pub active: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct BrokerProfile {
    pub broker_ip: String,
    pub broker_api_port: usize,
    pub token: Option<String>,
}

impl AetherConfig {
    const AETHER_REL_PATH: &str = ".aether/config.json";

    pub fn aether_path() -> anyhow::Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;

        Ok(home.join(Self::AETHER_REL_PATH))
    }

    pub fn get() -> anyhow::Result<Self> {
        let path = Self::aether_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if path.exists() {
            let file = std::fs::File::open(&path)?;
            Ok(serde_json::from_reader(file)?)
        } else {
            let default_cfg = AetherConfig {
                profiles: HashMap::new(),
                active: None,
            };

            let mut file = std::fs::File::create(&path)?;
            file.write_all(serde_json::to_string_pretty(&default_cfg)?.as_bytes())?;

            Ok(default_cfg)
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::aether_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&path)?;

        file.write_all(serde_json::to_string_pretty(self)?.as_bytes())?;

        Ok(())
    }
}

impl BrokerProfile {
    pub fn new(broker_ip: &str, broker_api_port: usize, token: &str) -> Self {
        Self {
            broker_ip: broker_ip.into(),
            broker_api_port,
            token: Some(token.into()),
        }
    }

    pub fn resolve(
        broker_ip: Option<String>,
        broker_api_port: Option<usize>,
        token: Option<String>,
    ) -> anyhow::Result<Self> {
        if let Some(broker_ip) = broker_ip
            && let Some(broker_api_port) = broker_api_port
            && let Some(token) = token
        {
            Ok(Self {
                broker_ip,
                broker_api_port,
                token: Some(token),
            })
        } else {
            let cfg = AetherConfig::get()?;
            if let Some(active) = cfg.active {
                if let Some(prf) = cfg.profiles.get(&active).cloned() {
                    return Ok(prf);
                }
                anyhow::bail!("Could not get active profile for unexpected reasons");
            } else {
                anyhow::bail!("There's no active default profile.");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_file_creation() {
        let path = AetherConfig::aether_path().unwrap();
        let cfg = AetherConfig::get().unwrap();
        assert!(path.exists());
        assert_eq!(
            cfg,
            AetherConfig {
                profiles: HashMap::new(),
                active: None
            }
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_config_file_save() {
        let path = AetherConfig::aether_path().unwrap();
        let profile = BrokerProfile::new("127.0.0.1", 8080, "fake-jwt");
        let mut cfg = AetherConfig {
            profiles: HashMap::new(),
            active: None,
        };
        cfg.profiles.insert("sample_server".into(), profile);
        _ = std::fs::remove_file(path);
        cfg.save().unwrap();
        let fs_cfg = AetherConfig::get().unwrap();
        assert_eq!(cfg, fs_cfg)
    }
}
