use crate::config::Config;
use crate::plugin::manager::PluginSource;
use crate::plugin::marketplace::{fetch_marketplace, DEFAULT_MARKETPLACE};
use crate::plugin::PluginManager;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const GITHUB_REPO: &str = "grimurjonsson/to-tui";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CHECK_INTERVAL_SECS: u64 = 600; // Check every 10 minutes

/// Information about a plugin that has an update available
#[derive(Debug, Clone)]
pub struct PluginUpdateInfo {
    /// Plugin name
    pub plugin_name: String,
    /// Currently installed version
    pub current_version: String,
    /// Latest version available in marketplace
    pub latest_version: String,
    /// Source of the plugin (owner/repo for download URL)
    pub source: PluginSource,
}

/// App update information
#[derive(Debug, Clone)]
pub struct AppUpdateInfo {
    /// Latest version available
    pub latest_version: String,
    /// Whether the latest version is newer than current
    pub is_newer: bool,
}

/// Combined version check result for app and plugins
#[derive(Debug, Clone)]
pub struct VersionCheckResult {
    /// App update info (if available)
    pub app_update: Option<AppUpdateInfo>,
    /// List of plugins with available updates
    pub plugin_updates: Vec<PluginUpdateInfo>,
}

impl VersionCheckResult {
    /// Check if there are any updates available
    pub fn has_updates(&self) -> bool {
        self.app_update.as_ref().is_some_and(|u| u.is_newer) || !self.plugin_updates.is_empty()
    }

    /// Get the app's latest version if newer
    pub fn app_latest_version(&self) -> Option<&str> {
        self.app_update
            .as_ref()
            .filter(|u| u.is_newer)
            .map(|u| u.latest_version.as_str())
    }
}

/// Spawns a background thread that periodically checks for new versions
/// (both app and plugins) and sends results through the provided channel.
pub fn spawn_version_checker() -> mpsc::Receiver<VersionCheckResult> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        // Initial check after a short delay (don't slow down startup)
        thread::sleep(Duration::from_secs(5));

        loop {
            let result = check_all_updates();
            if result.has_updates() {
                // Only send if there are updates available
                let _ = tx.send(result);
            }

            // Wait before next check
            thread::sleep(Duration::from_secs(CHECK_INTERVAL_SECS));
        }
    });

    rx
}

/// Check for all updates: app version and plugin versions
fn check_all_updates() -> VersionCheckResult {
    // Check app version
    let app_update = check_latest_app_version();

    // Check plugin updates
    let plugin_updates = check_plugin_updates();

    VersionCheckResult {
        app_update,
        plugin_updates,
    }
}

/// Check for plugin updates by comparing installed versions against marketplace
fn check_plugin_updates() -> Vec<PluginUpdateInfo> {
    let mut updates = Vec::new();

    // Discover installed plugins
    let manager = match PluginManager::discover() {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!("Failed to discover plugins for update check: {}", e);
            return updates;
        }
    };

    // Get marketplace config
    let marketplace_ref = Config::load()
        .map(|c| c.marketplaces.default.clone())
        .unwrap_or_else(|_| DEFAULT_MARKETPLACE.to_string());

    // Parse owner/repo
    let (owner, repo) = match marketplace_ref.split_once('/') {
        Some((o, r)) => (o.to_string(), r.to_string()),
        None => {
            tracing::debug!("Invalid marketplace format: {}", marketplace_ref);
            return updates;
        }
    };

    // Fetch marketplace manifest
    let manifest = match fetch_marketplace(&owner, &repo) {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!("Failed to fetch marketplace for plugin updates: {}", e);
            return updates;
        }
    };

    // Compare installed plugins against marketplace
    for plugin_info in manager.list() {
        let plugin_name = &plugin_info.manifest.name;
        let current_version = &plugin_info.manifest.version;

        // Find this plugin in the marketplace
        if let Some(marketplace_entry) = manifest.find_plugin(plugin_name) {
            let latest_version = &marketplace_entry.version;

            // Check if marketplace has a newer version
            if is_version_newer(latest_version, current_version) {
                updates.push(PluginUpdateInfo {
                    plugin_name: plugin_name.clone(),
                    current_version: current_version.clone(),
                    latest_version: latest_version.clone(),
                    source: plugin_info.source.clone(),
                });
            }
        }
    }

    updates
}

/// Checks GitHub releases API for the latest app version
fn check_latest_app_version() -> Option<AppUpdateInfo> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );

    // Use blocking reqwest since we're in a background thread
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("to-tui")
        .build()
        .ok()?;

    let response: serde_json::Value = client.get(&url).send().ok()?.json().ok()?;

    let tag_name = response.get("tag_name")?.as_str()?;

    // Strip leading 'v' if present
    let latest_version = tag_name.strip_prefix('v').unwrap_or(tag_name);

    let is_newer = is_version_newer(latest_version, CURRENT_VERSION);

    Some(AppUpdateInfo {
        latest_version: latest_version.to_string(),
        is_newer,
    })
}

/// Compares two semver version strings, returns true if `latest` is newer than `current`
fn is_version_newer(latest: &str, current: &str) -> bool {
    let parse_version = |v: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() >= 3 {
            Some((
                parts[0].parse().ok()?,
                parts[1].parse().ok()?,
                parts[2].parse().ok()?,
            ))
        } else if parts.len() == 2 {
            Some((parts[0].parse().ok()?, parts[1].parse().ok()?, 0))
        } else {
            None
        }
    };

    match (parse_version(latest), parse_version(current)) {
        (Some((l_maj, l_min, l_patch)), Some((c_maj, c_min, c_patch))) => {
            (l_maj, l_min, l_patch) > (c_maj, c_min, c_patch)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_version_newer("1.0.0", "0.9.0"));
        assert!(is_version_newer("0.10.0", "0.9.0"));
        assert!(is_version_newer("0.9.1", "0.9.0"));
        assert!(is_version_newer("2.0.0", "1.9.9"));

        assert!(!is_version_newer("0.9.0", "0.9.0"));
        assert!(!is_version_newer("0.8.0", "0.9.0"));
        assert!(!is_version_newer("0.9.0", "1.0.0"));
    }
}
