use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;

use crate::keybindings::KeybindingsConfig;
use crate::plugin::marketplace::DEFAULT_MARKETPLACE;
use crate::utils::paths::get_config_path;

/// Plugin enable/disable configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginsConfig {
    /// Explicitly disabled plugins (enabled by default)
    #[serde(default)]
    pub disabled: HashSet<String>,
}

impl PluginsConfig {
    /// Check if a plugin is enabled (not in disabled set)
    pub fn is_enabled(&self, name: &str) -> bool {
        !self.disabled.contains(&name.to_lowercase())
    }

    /// Enable a plugin by removing from disabled set
    pub fn enable(&mut self, name: &str) {
        self.disabled.remove(&name.to_lowercase());
    }

    /// Disable a plugin by adding to disabled set
    pub fn disable(&mut self, name: &str) {
        self.disabled.insert(name.to_lowercase());
    }
}

/// Marketplace configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplacesConfig {
    /// Default marketplace for short plugin names (owner/repo format)
    #[serde(default = "default_marketplace")]
    pub default: String,
}

fn default_marketplace() -> String {
    DEFAULT_MARKETPLACE.to_string()
}

impl Default for MarketplacesConfig {
    fn default() -> Self {
        Self {
            default: default_marketplace(),
        }
    }
}

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

    #[serde(default)]
    pub plugins: PluginsConfig,

    #[serde(default)]
    pub marketplaces: MarketplacesConfig,
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
            plugins: PluginsConfig::default(),
            marketplaces: MarketplacesConfig::default(),
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

    #[test]
    fn test_plugins_config_default_enabled() {
        let config = PluginsConfig::default();
        assert!(config.is_enabled("any-plugin"));
    }

    #[test]
    fn test_plugins_config_disable_enable() {
        let mut config = PluginsConfig::default();
        config.disable("my-plugin");
        assert!(!config.is_enabled("my-plugin"));
        assert!(!config.is_enabled("MY-PLUGIN")); // case insensitive

        config.enable("MY-PLUGIN");
        assert!(config.is_enabled("my-plugin"));
    }

    #[test]
    fn test_config_with_plugins_serialization_roundtrip() {
        // Verify Config with plugins field serializes/deserializes correctly
        let mut config = Config::default();
        config.plugins.disable("test-plugin");

        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("[plugins]"));
        assert!(toml_str.contains("test-plugin"));

        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert!(!parsed.plugins.is_enabled("test-plugin"));
    }

    #[test]
    fn test_marketplaces_config_default() {
        let config = MarketplacesConfig::default();
        assert_eq!(config.default, "grimurjonsson/to-tui-plugins");
    }

    #[test]
    fn test_marketplaces_config_deserialization() {
        let toml_str = r#"
        [marketplaces]
        default = "myorg/my-plugins"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.marketplaces.default, "myorg/my-plugins");
    }

    #[test]
    fn test_marketplaces_config_uses_default_when_missing() {
        let toml_str = r#"
        theme = "dark"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.marketplaces.default, "grimurjonsson/to-tui-plugins");
    }
}
