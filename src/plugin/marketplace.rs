//! Marketplace manifest parsing for plugin registries.
//!
//! A marketplace is a GitHub repository containing a marketplace.toml
//! at its root, listing available plugins with metadata.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Default marketplace repository
pub const DEFAULT_MARKETPLACE: &str = "grimurjonsson/to-tui-plugins";

/// A plugin entry in the marketplace manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    /// Plugin name (matches directory name in plugins/)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Latest available version
    pub version: String,
    /// Repository URL (defaults to marketplace repo)
    #[serde(default)]
    pub repository: Option<String>,
}

/// Marketplace manifest (marketplace.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceManifest {
    /// Marketplace metadata
    pub marketplace: MarketplaceInfo,
    /// Available plugins
    #[serde(default)]
    pub plugins: Vec<PluginEntry>,
}

/// Marketplace metadata section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceInfo {
    /// Marketplace name
    pub name: String,
    /// Description
    pub description: String,
    /// Repository URL
    #[serde(default)]
    pub url: Option<String>,
}

impl MarketplaceManifest {
    /// Parse marketplace manifest from TOML string
    pub fn parse(content: &str) -> Result<Self> {
        toml::from_str(content).context("Failed to parse marketplace.toml")
    }

    /// Find a plugin by name
    pub fn find_plugin(&self, name: &str) -> Option<&PluginEntry> {
        self.plugins
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }
}

/// Fetch marketplace manifest from GitHub (raw content URL)
pub fn fetch_marketplace(owner: &str, repo: &str) -> Result<MarketplaceManifest> {
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/main/marketplace.toml",
        owner, repo
    );

    let client = reqwest::blocking::Client::builder()
        .user_agent("to-tui")
        .build()?;

    let response = client.get(&url).send()?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to fetch marketplace manifest from {}/{}: HTTP {}",
            owner,
            repo,
            response.status()
        );
    }

    let content = response.text()?;
    MarketplaceManifest::parse(&content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_marketplace_manifest() {
        let toml = r#"
[marketplace]
name = "to-tui-plugins"
description = "Official plugin registry"
url = "https://github.com/grimurjonsson/to-tui-plugins"

[[plugins]]
name = "jira"
description = "Fetch Jira tickets as todos"
version = "1.0.0"

[[plugins]]
name = "github"
description = "GitHub issues integration"
version = "0.2.0"
repository = "https://github.com/other/repo"
"#;
        let manifest = MarketplaceManifest::parse(toml).unwrap();
        assert_eq!(manifest.marketplace.name, "to-tui-plugins");
        assert_eq!(manifest.plugins.len(), 2);
        assert_eq!(manifest.plugins[0].name, "jira");
        assert_eq!(manifest.plugins[0].version, "1.0.0");
        assert!(manifest.plugins[0].repository.is_none());
        assert!(manifest.plugins[1].repository.is_some());
    }

    #[test]
    fn test_find_plugin_case_insensitive() {
        let toml = r#"
[marketplace]
name = "test"
description = "test"

[[plugins]]
name = "Jira"
description = "Test"
version = "1.0.0"
"#;
        let manifest = MarketplaceManifest::parse(toml).unwrap();
        assert!(manifest.find_plugin("jira").is_some());
        assert!(manifest.find_plugin("JIRA").is_some());
        assert!(manifest.find_plugin("Jira").is_some());
    }
}
