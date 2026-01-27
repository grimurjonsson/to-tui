//! Plugin manager for discovering and tracking plugins.
//!
//! The PluginManager scans the plugins directory for plugin.toml manifests
//! and tracks their state (enabled, available, errors).

use crate::config::PluginsConfig;
use crate::plugin::manifest::PluginManifest;
use crate::utils::paths::get_plugins_dir;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Source of plugin installation
#[derive(Debug, Clone, Default)]
pub enum PluginSource {
    /// Unknown source (legacy or manual install)
    #[default]
    Unknown,
    /// Installed from local directory
    Local,
    /// Installed from remote marketplace
    Remote { owner: String, repo: String },
}

impl fmt::Display for PluginSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginSource::Unknown => write!(f, "unknown"),
            PluginSource::Local => write!(f, "local"),
            PluginSource::Remote { owner, repo } => write!(f, "{}/{}", owner, repo),
        }
    }
}

/// Information about a discovered plugin.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Parsed manifest from plugin.toml
    pub manifest: PluginManifest,
    /// Path to the plugin directory
    pub path: PathBuf,
    /// Whether the plugin is enabled by the user
    pub enabled: bool,
    /// Whether the plugin is available (no errors and version compatible)
    pub available: bool,
    /// Reason why plugin is unavailable (version incompatibility, etc.)
    pub availability_reason: Option<String>,
    /// Parse or validation error (None if manifest is valid)
    pub error: Option<String>,
    /// Source of the plugin (local, remote marketplace, unknown)
    pub source: PluginSource,
}

/// Manages plugin discovery and state.
#[derive(Debug, Default)]
pub struct PluginManager {
    plugins: HashMap<String, PluginInfo>,
}

impl PluginManager {
    /// Discover plugins from ~/.local/share/to-tui/plugins/
    ///
    /// Scans the plugins directory for subdirectories containing plugin.toml
    /// files. Each plugin is loaded and validated, with errors captured in
    /// PluginInfo.error rather than failing the entire discovery.
    ///
    /// Returns an empty manager if the plugins directory does not exist.
    pub fn discover() -> Result<Self> {
        let plugins_dir = get_plugins_dir()?;
        let mut manager = Self::default();

        if !plugins_dir.exists() {
            tracing::debug!("Plugins directory does not exist: {:?}", plugins_dir);
            return Ok(manager);
        }

        let entries = fs::read_dir(&plugins_dir)
            .with_context(|| format!("Failed to read plugins directory: {:?}", plugins_dir))?;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };

            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }

            let name = match plugin_dir.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let info = Self::load_plugin_info(&plugin_dir);
            tracing::debug!("Discovered plugin '{}': error={:?}", name, info.error);
            manager.plugins.insert(name.to_lowercase(), info);
        }

        Ok(manager)
    }

    /// Load plugin info from a directory.
    ///
    /// Parses the plugin.toml manifest and validates it. Used both for discovery
    /// and for validating plugins before installation.
    pub fn load_plugin_info(plugin_dir: &Path) -> PluginInfo {
        let manifest_path = plugin_dir.join("plugin.toml");

        // Read source file for tracking (non-fatal if missing)
        let source = Self::read_source_file(plugin_dir);

        // Check manifest exists
        if !manifest_path.exists() {
            return PluginInfo {
                manifest: PluginManifest::default(),
                path: plugin_dir.to_path_buf(),
                enabled: true,
                available: false,
                availability_reason: None,
                error: Some("Missing plugin.toml".to_string()),
                source,
            };
        }

        // Read manifest file
        let content = match fs::read_to_string(&manifest_path) {
            Ok(c) => c,
            Err(e) => {
                return PluginInfo {
                    manifest: PluginManifest::default(),
                    path: plugin_dir.to_path_buf(),
                    enabled: true,
                    available: false,
                    availability_reason: None,
                    error: Some(format!("Failed to read plugin.toml: {}", e)),
                    source,
                };
            }
        };

        // Parse manifest
        let manifest: PluginManifest = match toml::from_str(&content) {
            Ok(m) => m,
            Err(e) => {
                return PluginInfo {
                    manifest: PluginManifest::default(),
                    path: plugin_dir.to_path_buf(),
                    enabled: true,
                    available: false,
                    availability_reason: None,
                    error: Some(format!("Invalid plugin.toml: {}", e)),
                    source,
                };
            }
        };

        // Validate manifest
        if let Err(e) = manifest.validate() {
            return PluginInfo {
                manifest,
                path: plugin_dir.to_path_buf(),
                enabled: true,
                available: false,
                availability_reason: None,
                error: Some(e),
                source,
            };
        }

        // Check interface version compatibility (PLUG-06)
        if let Some(min_ver) = manifest.min_interface_version.clone() {
            use totui_plugin_interface::{is_version_compatible, INTERFACE_VERSION};
            match is_version_compatible(&min_ver, INTERFACE_VERSION) {
                Ok(true) => {
                    // Compatible - continue to return available plugin
                }
                Ok(false) => {
                    return PluginInfo {
                        manifest,
                        path: plugin_dir.to_path_buf(),
                        enabled: true,
                        available: false,
                        availability_reason: Some(format!(
                            "Requires interface version {}, host provides {}",
                            min_ver, INTERFACE_VERSION
                        )),
                        error: None,
                        source,
                    };
                }
                Err(e) => {
                    return PluginInfo {
                        manifest,
                        path: plugin_dir.to_path_buf(),
                        enabled: true,
                        available: false,
                        availability_reason: Some(format!("Version check failed: {}", e)),
                        error: None,
                        source,
                    };
                }
            }
        }

        PluginInfo {
            manifest,
            path: plugin_dir.to_path_buf(),
            enabled: true,
            available: true,
            availability_reason: None,
            error: None,
            source,
        }
    }

    /// Read source tracking file from plugin directory.
    fn read_source_file(plugin_dir: &Path) -> PluginSource {
        let source_file = plugin_dir.join(".source");
        if let Ok(content) = fs::read_to_string(&source_file) {
            let content = content.trim();
            if content == "local" {
                PluginSource::Local
            } else if let Some((owner, repo)) = content.split_once('/') {
                PluginSource::Remote {
                    owner: owner.to_string(),
                    repo: repo.to_string(),
                }
            } else {
                PluginSource::Unknown
            }
        } else {
            PluginSource::Unknown
        }
    }

    /// List all discovered plugins.
    pub fn list(&self) -> Vec<&PluginInfo> {
        self.plugins.values().collect()
    }

    /// Get a plugin by name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&PluginInfo> {
        self.plugins.get(&name.to_lowercase())
    }

    /// Get mutable reference to a plugin by name (case-insensitive).
    pub fn get_mut(&mut self, name: &str) -> Option<&mut PluginInfo> {
        self.plugins.get_mut(&name.to_lowercase())
    }

    /// List plugins that are enabled, available, and have no errors.
    ///
    /// These are the plugins that can actually be loaded and used.
    pub fn enabled_plugins(&self) -> Vec<&PluginInfo> {
        self.plugins
            .values()
            .filter(|p| p.enabled && p.available && p.error.is_none())
            .collect()
    }

    /// List plugins that are available (regardless of enabled state).
    ///
    /// Available means: no errors and version compatible with host.
    pub fn available_plugins(&self) -> Vec<&PluginInfo> {
        self.plugins
            .values()
            .filter(|p| p.available && p.error.is_none())
            .collect()
    }

    /// Get plugins with errors (for status bar warning).
    pub fn plugins_with_errors(&self) -> Vec<(&String, &PluginInfo)> {
        self.plugins
            .iter()
            .filter(|(_, p)| p.error.is_some())
            .collect()
    }

    /// Apply config to update enabled state of plugins.
    ///
    /// Reads the PluginsConfig and sets each plugin's enabled flag
    /// based on whether it appears in the disabled set.
    pub fn apply_config(&mut self, config: &PluginsConfig) {
        for (name, info) in self.plugins.iter_mut() {
            info.enabled = config.is_enabled(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginsConfig;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_plugin(dir: &Path, name: &str, manifest_content: &str) {
        let plugin_dir = dir.join(name);
        fs::create_dir_all(&plugin_dir).unwrap();
        let manifest_path = plugin_dir.join("plugin.toml");
        let mut file = fs::File::create(manifest_path).unwrap();
        file.write_all(manifest_content.as_bytes()).unwrap();
    }

    #[test]
    fn test_load_valid_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let manifest = r#"
name = "test-plugin"
version = "1.0.0"
description = "A test plugin"
"#;
        create_test_plugin(temp_dir.path(), "test-plugin", manifest);

        let info = PluginManager::load_plugin_info(&temp_dir.path().join("test-plugin"));
        assert!(info.error.is_none());
        assert!(info.available);
        assert_eq!(info.manifest.name, "test-plugin");
        assert!(info.enabled);
    }

    #[test]
    fn test_load_missing_manifest() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("no-manifest");
        fs::create_dir_all(&plugin_dir).unwrap();

        let info = PluginManager::load_plugin_info(&plugin_dir);
        assert!(info.error.is_some());
        assert!(!info.available);
        assert!(info.error.unwrap().contains("Missing plugin.toml"));
    }

    #[test]
    fn test_load_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        create_test_plugin(temp_dir.path(), "bad-plugin", "this is not valid toml [[[");

        let info = PluginManager::load_plugin_info(&temp_dir.path().join("bad-plugin"));
        assert!(info.error.is_some());
        assert!(!info.available);
        assert!(info.error.unwrap().contains("Invalid plugin.toml"));
    }

    #[test]
    fn test_load_invalid_version() {
        let temp_dir = TempDir::new().unwrap();
        let manifest = r#"
name = "bad-version"
version = "not-semver"
description = "Has invalid version"
"#;
        create_test_plugin(temp_dir.path(), "bad-version", manifest);

        let info = PluginManager::load_plugin_info(&temp_dir.path().join("bad-version"));
        assert!(info.error.is_some());
        assert!(info.error.unwrap().contains("Invalid version"));
    }

    #[test]
    fn test_incompatible_min_interface_version() {
        // Test PLUG-06: plugin requiring higher interface version than host provides
        let temp_dir = TempDir::new().unwrap();
        let manifest = r#"
name = "future-plugin"
version = "1.0.0"
description = "Requires future interface"
min_interface_version = "99.0.0"
"#;
        create_test_plugin(temp_dir.path(), "future-plugin", manifest);

        let info = PluginManager::load_plugin_info(&temp_dir.path().join("future-plugin"));
        assert!(info.error.is_none()); // Not a parse error
        assert!(!info.available); // But not available
        assert!(info.availability_reason.is_some());
        assert!(info
            .availability_reason
            .unwrap()
            .contains("Requires interface version"));
    }

    #[test]
    fn test_compatible_min_interface_version() {
        // Plugin requiring current or older interface version should be available
        let temp_dir = TempDir::new().unwrap();
        let manifest = r#"
name = "compatible-plugin"
version = "1.0.0"
description = "Compatible interface"
min_interface_version = "0.1.0"
"#;
        create_test_plugin(temp_dir.path(), "compatible-plugin", manifest);

        let info = PluginManager::load_plugin_info(&temp_dir.path().join("compatible-plugin"));
        assert!(info.error.is_none());
        assert!(info.available);
        assert!(info.availability_reason.is_none());
    }

    #[test]
    fn test_case_insensitive_lookup() {
        let mut manager = PluginManager::default();
        manager.plugins.insert(
            "my-plugin".to_string(),
            PluginInfo {
                manifest: PluginManifest::default(),
                path: PathBuf::from("/test"),
                enabled: true,
                available: true,
                availability_reason: None,
                error: None,
                source: PluginSource::Unknown,
            },
        );

        assert!(manager.get("my-plugin").is_some());
        assert!(manager.get("MY-PLUGIN").is_some());
        assert!(manager.get("My-Plugin").is_some());
    }

    #[test]
    fn test_enabled_plugins_filters_errors_and_unavailable() {
        let mut manager = PluginManager::default();

        manager.plugins.insert(
            "good".to_string(),
            PluginInfo {
                manifest: PluginManifest::default(),
                path: PathBuf::from("/good"),
                enabled: true,
                available: true,
                availability_reason: None,
                error: None,
                source: PluginSource::Unknown,
            },
        );

        manager.plugins.insert(
            "errored".to_string(),
            PluginInfo {
                manifest: PluginManifest::default(),
                path: PathBuf::from("/errored"),
                enabled: true,
                available: false,
                availability_reason: None,
                error: Some("Some error".to_string()),
                source: PluginSource::Unknown,
            },
        );

        manager.plugins.insert(
            "unavailable".to_string(),
            PluginInfo {
                manifest: PluginManifest::default(),
                path: PathBuf::from("/unavailable"),
                enabled: true,
                available: false,
                availability_reason: Some("Version mismatch".to_string()),
                error: None,
                source: PluginSource::Unknown,
            },
        );

        manager.plugins.insert(
            "disabled".to_string(),
            PluginInfo {
                manifest: PluginManifest::default(),
                path: PathBuf::from("/disabled"),
                enabled: false,
                available: true,
                availability_reason: None,
                error: None,
                source: PluginSource::Unknown,
            },
        );

        let enabled = manager.enabled_plugins();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].path, PathBuf::from("/good"));
    }

    #[test]
    fn test_available_plugins() {
        let mut manager = PluginManager::default();

        manager.plugins.insert(
            "available-enabled".to_string(),
            PluginInfo {
                manifest: PluginManifest::default(),
                path: PathBuf::from("/available-enabled"),
                enabled: true,
                available: true,
                availability_reason: None,
                error: None,
                source: PluginSource::Unknown,
            },
        );

        manager.plugins.insert(
            "available-disabled".to_string(),
            PluginInfo {
                manifest: PluginManifest::default(),
                path: PathBuf::from("/available-disabled"),
                enabled: false,
                available: true,
                availability_reason: None,
                error: None,
                source: PluginSource::Unknown,
            },
        );

        manager.plugins.insert(
            "unavailable".to_string(),
            PluginInfo {
                manifest: PluginManifest::default(),
                path: PathBuf::from("/unavailable"),
                enabled: true,
                available: false,
                availability_reason: Some("Version mismatch".to_string()),
                error: None,
                source: PluginSource::Unknown,
            },
        );

        let available = manager.available_plugins();
        assert_eq!(available.len(), 2);
    }

    #[test]
    fn test_apply_config() {
        let mut manager = PluginManager::default();
        manager.plugins.insert(
            "plugin-a".to_string(),
            PluginInfo {
                manifest: PluginManifest::default(),
                path: PathBuf::from("/a"),
                enabled: true,
                available: true,
                availability_reason: None,
                error: None,
                source: PluginSource::Unknown,
            },
        );
        manager.plugins.insert(
            "plugin-b".to_string(),
            PluginInfo {
                manifest: PluginManifest::default(),
                path: PathBuf::from("/b"),
                enabled: true,
                available: true,
                availability_reason: None,
                error: None,
                source: PluginSource::Unknown,
            },
        );

        let mut plugins_config = PluginsConfig::default();
        plugins_config.disable("plugin-a");

        manager.apply_config(&plugins_config);

        assert!(!manager.get("plugin-a").unwrap().enabled);
        assert!(manager.get("plugin-b").unwrap().enabled);
    }

    #[test]
    fn test_source_tracking_local() {
        let temp_dir = TempDir::new().unwrap();
        let manifest = r#"
name = "local-plugin"
version = "1.0.0"
description = "A test plugin"
"#;
        create_test_plugin(temp_dir.path(), "local-plugin", manifest);

        // Write local source file
        let source_file = temp_dir.path().join("local-plugin").join(".source");
        fs::write(&source_file, "local").unwrap();

        let info = PluginManager::load_plugin_info(&temp_dir.path().join("local-plugin"));
        assert!(matches!(info.source, PluginSource::Local));
        assert_eq!(info.source.to_string(), "local");
    }

    #[test]
    fn test_source_tracking_remote() {
        let temp_dir = TempDir::new().unwrap();
        let manifest = r#"
name = "remote-plugin"
version = "1.0.0"
description = "A test plugin"
"#;
        create_test_plugin(temp_dir.path(), "remote-plugin", manifest);

        // Write remote source file
        let source_file = temp_dir.path().join("remote-plugin").join(".source");
        fs::write(&source_file, "grimurjonsson/to-tui-plugins").unwrap();

        let info = PluginManager::load_plugin_info(&temp_dir.path().join("remote-plugin"));
        if let PluginSource::Remote { owner, repo } = &info.source {
            assert_eq!(owner, "grimurjonsson");
            assert_eq!(repo, "to-tui-plugins");
        } else {
            panic!("Expected Remote source");
        }
        assert_eq!(info.source.to_string(), "grimurjonsson/to-tui-plugins");
    }

    #[test]
    fn test_source_tracking_unknown_without_file() {
        let temp_dir = TempDir::new().unwrap();
        let manifest = r#"
name = "legacy-plugin"
version = "1.0.0"
description = "A test plugin"
"#;
        create_test_plugin(temp_dir.path(), "legacy-plugin", manifest);

        // No .source file
        let info = PluginManager::load_plugin_info(&temp_dir.path().join("legacy-plugin"));
        assert!(matches!(info.source, PluginSource::Unknown));
        assert_eq!(info.source.to_string(), "unknown");
    }
}
