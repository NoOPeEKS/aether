use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::CliError;

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

    pub fn aether_path() -> Result<PathBuf, CliError> {
        let home = dirs::home_dir().ok_or(CliError::InvalidHomeDir)?;

        Ok(home.join(Self::AETHER_REL_PATH))
    }

    pub fn get() -> Result<Self, CliError> {
        let path = Self::aether_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| {
                CliError::GetConfigError("Could not create config parent directories.".into())
            })?;
        }

        if path.exists() {
            let file = std::fs::File::open(&path)
                .map_err(|_| CliError::GetConfigError("Could not open file descriptor.".into()))?;
            Ok(serde_json::from_reader(file).map_err(|_| CliError::DeserializeSerdeError)?)
        } else {
            let default_cfg = AetherConfig {
                profiles: HashMap::new(),
                active: None,
            };

            let mut file = std::fs::File::create(&path).map_err(|_| {
                CliError::GetConfigError("Could not create ~/.aether/config.json".into())
            })?;
            file.write_all(
                serde_json::to_string_pretty(&default_cfg)
                    .map_err(|_| CliError::SerializeSerdeError)?
                    .as_bytes(),
            )
            .map_err(|_| CliError::GetConfigError("Could not write to file descriptor.".into()))?;

            Ok(default_cfg)
        }
    }

    pub fn save(&self) -> Result<(), CliError> {
        let path = Self::aether_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| {
                CliError::SaveConfigError("Could not create config parent directories.".into())
            })?;
        }

        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&path)
            .map_err(|_| {
                CliError::SaveConfigError("Could not create or open file descriptor.".into())
            })?;

        file.write_all(
            serde_json::to_string_pretty(self)
                .map_err(|_| CliError::SerializeSerdeError)?
                .as_bytes(),
        )
        .map_err(|_| CliError::SaveConfigError("Could not write to file descriptor.".into()))?;

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
    ) -> Result<Self, CliError> {
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
                Err(CliError::BrokerProfileResolveError(
                    "Could not get active profile for unexpected reasons.".into(),
                ))
            } else {
                Err(CliError::BrokerProfileResolveError(
                    "There's no active default profile.".into(),
                ))
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
