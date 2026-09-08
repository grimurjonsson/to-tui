use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
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

/// User preference for what happens at midnight crossover.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutoRolloverPref {
    /// Show the rollover modal and ask the user (default).
    #[default]
    Ask,
    /// Automatically rollover incomplete items at midnight.
    AutoYes,
    /// Never auto-rollover and never prompt; user can still trigger manually with R.
    AutoNo,
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

    #[serde(default)]
    pub auto_rollover: AutoRolloverPref,

    /// Folder key (git repo root or directory path) -> project name,
    /// learned automatically as projects are used per folder
    #[serde(default)]
    pub folder_projects: BTreeMap<String, String>,
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
            auto_rollover: AutoRolloverPref::default(),
            folder_projects: BTreeMap::new(),
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

    /// Point all folder bindings for a renamed project at its new name
    pub fn rebind_project_name(&mut self, old_name: &str, new_name: &str) {
        for project in self.folder_projects.values_mut() {
            if project == old_name {
                *project = new_name.to_string();
            }
        }
    }

    /// Remove all folder bindings for a deleted project
    pub fn unbind_project(&mut self, name: &str) {
        self.folder_projects.retain(|_, project| project != name);
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

    #[test]
    fn test_auto_rollover_default_is_ask() {
        let config = Config::default();
        assert_eq!(config.auto_rollover, AutoRolloverPref::Ask);
    }

    #[test]
    fn test_auto_rollover_serialises_snake_case() {
        let config = Config {
            auto_rollover: AutoRolloverPref::AutoYes,
            ..Config::default()
        };
        let toml_str = toml::to_string(&config).unwrap();
        assert!(
            toml_str.contains("auto_rollover = \"auto_yes\""),
            "expected snake_case serialisation, got: {toml_str}"
        );
    }

    #[test]
    fn test_auto_rollover_deserialises_all_variants() {
        for (input, expected) in [
            ("ask", AutoRolloverPref::Ask),
            ("auto_yes", AutoRolloverPref::AutoYes),
            ("auto_no", AutoRolloverPref::AutoNo),
        ] {
            let toml_str = format!("auto_rollover = \"{input}\"\n");
            let config: Config = toml::from_str(&toml_str).unwrap();
            assert_eq!(config.auto_rollover, expected, "input was {input}");
        }
    }

    #[test]
    fn test_folder_projects_missing_field_defaults_to_empty() {
        let toml_str = "theme = \"dark\"\n";
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.folder_projects.is_empty());
    }

    #[test]
    fn test_folder_projects_serialization_roundtrip() {
        let mut config = Config::default();
        config
            .folder_projects
            .insert("/Users/me/repo".to_string(), "proj-a".to_string());

        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("[folder_projects]"));

        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            parsed.folder_projects.get("/Users/me/repo"),
            Some(&"proj-a".to_string())
        );
    }

    #[test]
    fn test_folder_projects_deserialization() {
        let toml_str = r#"
        [folder_projects]
        "/Users/me/repo" = "proj-a"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.folder_projects.get("/Users/me/repo"),
            Some(&"proj-a".to_string())
        );
    }

    #[test]
    fn test_rebind_project_name_updates_all_folders() {
        let mut config = Config::default();
        config
            .folder_projects
            .insert("/repo/a".to_string(), "old".to_string());
        config
            .folder_projects
            .insert("/repo/b".to_string(), "old".to_string());
        config
            .folder_projects
            .insert("/repo/c".to_string(), "other".to_string());

        config.rebind_project_name("old", "new");

        assert_eq!(config.folder_projects["/repo/a"], "new");
        assert_eq!(config.folder_projects["/repo/b"], "new");
        assert_eq!(config.folder_projects["/repo/c"], "other");
    }

    #[test]
    fn test_unbind_project_removes_its_folders() {
        let mut config = Config::default();
        config
            .folder_projects
            .insert("/repo/a".to_string(), "doomed".to_string());
        config
            .folder_projects
            .insert("/repo/b".to_string(), "kept".to_string());

        config.unbind_project("doomed");

        assert!(!config.folder_projects.contains_key("/repo/a"));
        assert_eq!(config.folder_projects["/repo/b"], "kept");
    }

    #[test]
    fn test_auto_rollover_missing_field_defaults_to_ask() {
        let toml_str = "theme = \"dark\"\n";
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.auto_rollover, AutoRolloverPref::Ask);
    }
}
