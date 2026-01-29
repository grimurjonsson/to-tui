//! Marketplace manifest parsing for plugin registries.
//!
//! A marketplace is a GitHub repository containing a marketplace.toml
//! at its root, listing available plugins with metadata.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

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
    /// Platform-specific download URLs (populated by CI)
    #[serde(default)]
    pub downloads: std::collections::HashMap<String, String>,
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

    debug!(owner = %owner, repo = %repo, url = %url, "Fetching marketplace manifest");

    let client = reqwest::blocking::Client::builder()
        .user_agent("to-tui")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    debug!("HTTP client created, sending request...");

    let response = match client.get(&url).send() {
        Ok(r) => {
            debug!(status = %r.status(), "Received HTTP response");
            r
        }
        Err(e) => {
            warn!(error = %e, url = %url, "Failed to fetch marketplace manifest");
            return Err(e.into());
        }
    };

    if !response.status().is_success() {
        warn!(
            status = %response.status(),
            owner = %owner,
            repo = %repo,
            "Marketplace fetch returned non-success status"
        );
        anyhow::bail!(
            "Failed to fetch marketplace manifest from {}/{}: HTTP {}",
            owner,
            repo,
            response.status()
        );
    }

    let content = response.text()?;
    debug!(content_length = content.len(), "Received marketplace manifest content");

    match MarketplaceManifest::parse(&content) {
        Ok(manifest) => {
            debug!(
                marketplace_name = %manifest.marketplace.name,
                plugin_count = manifest.plugins.len(),
                plugins = ?manifest.plugins.iter().map(|p| &p.name).collect::<Vec<_>>(),
                "Successfully parsed marketplace manifest"
            );
            Ok(manifest)
        }
        Err(e) => {
            warn!(error = %e, "Failed to parse marketplace.toml");
            // Log first 500 chars of content for debugging
            let preview: String = content.chars().take(500).collect();
            debug!(content_preview = %preview, "Raw content (first 500 chars)");
            Err(e)
        }
    }
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

[plugins.downloads]
x86_64-unknown-linux-gnu = "https://example.com/github-linux.tar.gz"
"#;
        let manifest = MarketplaceManifest::parse(toml).unwrap();
        assert_eq!(manifest.marketplace.name, "to-tui-plugins");
        assert_eq!(manifest.plugins.len(), 2);
        assert_eq!(manifest.plugins[0].name, "jira");
        assert_eq!(manifest.plugins[0].version, "1.0.0");
        assert!(manifest.plugins[0].repository.is_none());
        assert!(manifest.plugins[0].downloads.is_empty());
        assert!(manifest.plugins[1].repository.is_some());
        assert_eq!(manifest.plugins[1].downloads.len(), 1);
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

    /// Integration test that fetches the real marketplace manifest
    /// Run with: cargo test test_fetch_real_marketplace -- --ignored --nocapture
    #[test]
    #[ignore] // Requires network access
    fn test_fetch_real_marketplace() {
        // Initialize tracing for debug output
        let _ = tracing_subscriber::fmt()
            .with_env_filter("debug")
            .with_test_writer()
            .try_init();

        let result = fetch_marketplace("grimurjonsson", "to-tui-plugins");
        
        match result {
            Ok(manifest) => {
                println!("✅ Marketplace fetch successful!");
                println!("   Name: {}", manifest.marketplace.name);
                println!("   Plugins: {}", manifest.plugins.len());
                for plugin in &manifest.plugins {
                    println!("   - {} v{}: {}", plugin.name, plugin.version, plugin.description);
                }
                assert!(!manifest.plugins.is_empty(), "Expected at least one plugin");
            }
            Err(e) => {
                println!("❌ Marketplace fetch failed: {}", e);
                println!("   Caused by: {:?}", e.source());
                panic!("Marketplace fetch should succeed");
            }
        }
    }
}
