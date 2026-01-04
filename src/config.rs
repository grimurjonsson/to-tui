use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::keybindings::KeybindingsConfig;
use crate::utils::paths::get_config_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_theme")]
    pub theme: String,

    #[serde(default = "default_timeoutlen")]
    pub timeoutlen: u64,

    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

fn default_theme() -> String {
    "default".to_string()
}

fn default_timeoutlen() -> u64 {
    1000
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            timeoutlen: default_timeoutlen(),
            keybindings: KeybindingsConfig::default(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = get_config_path()?;

        if !config_path.exists() {
            return Ok(Config::default());
        }

        let content = fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content)?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.theme, "default");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("theme"));
    }

    #[test]
    fn test_config_deserialization() {
        let toml_str = r#"
        theme = "dark"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.theme, "dark");
    }
}
