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

    #[serde(default)]
    pub skipped_version: Option<String>,

    #[serde(default)]
    pub last_used_project: Option<String>,
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
            skipped_version: None,
            last_used_project: None,
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
        let mut config: Config = toml::from_str(&content)?;

        config.keybindings = config.keybindings.merge_with_defaults();

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = get_config_path()?;

        // Ensure config directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;

        Ok(())
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
