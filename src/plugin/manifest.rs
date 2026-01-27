//! Plugin manifest parsing and validation.
//!
//! Defines the PluginManifest struct for parsing plugin.toml files.

use crate::keybindings::KeySequence;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Definition of a plugin action from the manifest
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActionDefinition {
    /// Human-readable description (required for help panel)
    pub description: String,

    /// Default keybinding in bracket notation (optional)
    /// e.g., "<C-j>", "<A-f>", "g g"
    #[serde(default)]
    pub default_keybinding: Option<String>,
}

/// Check if an action name is a valid identifier.
///
/// Valid identifiers contain only ASCII alphanumeric characters and underscores,
/// and must not be empty.
fn is_valid_action_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Plugin manifest from plugin.toml file.
///
/// Required fields: name, version, description
/// Optional fields have serde defaults for forward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin name (required, must not be empty)
    pub name: String,

    /// Plugin version in semver format (required)
    pub version: String,

    /// Plugin description (required, must not be empty)
    pub description: String,

    /// Plugin author
    #[serde(default)]
    pub author: Option<String>,

    /// License identifier (e.g., "MIT", "Apache-2.0")
    #[serde(default)]
    pub license: Option<String>,

    /// Homepage URL
    #[serde(default)]
    pub homepage: Option<String>,

    /// Repository URL
    #[serde(default)]
    pub repository: Option<String>,

    /// Minimum interface version required by this plugin
    #[serde(default)]
    pub min_interface_version: Option<String>,

    /// Actions this plugin provides with their metadata
    /// Keys are action names (valid identifiers: alphanumeric + underscore)
    #[serde(default)]
    pub actions: HashMap<String, ActionDefinition>,

    /// Timeout for hook execution in seconds (default: 5).
    /// Hooks that exceed this timeout will be terminated and counted as failures.
    #[serde(default = "default_hook_timeout")]
    pub hook_timeout_secs: u64,
}

fn default_hook_timeout() -> u64 {
    5
}

impl Default for PluginManifest {
    fn default() -> Self {
        Self {
            name: "<unknown>".to_string(),
            version: "0.0.0".to_string(),
            description: "<no description>".to_string(),
            author: None,
            license: None,
            homepage: None,
            repository: None,
            min_interface_version: None,
            actions: HashMap::new(),
            hook_timeout_secs: default_hook_timeout(),
        }
    }
}

impl PluginManifest {
    /// Validate the manifest fields.
    ///
    /// Checks:
    /// - name is not empty
    /// - version is valid semver
    /// - description is not empty
    /// - min_interface_version is valid semver if present
    pub fn validate(&self) -> Result<(), String> {
        // Name cannot be empty
        if self.name.is_empty() {
            return Err("Plugin name cannot be empty".to_string());
        }

        // Version must be valid semver
        if semver::Version::parse(&self.version).is_err() {
            return Err(format!(
                "Invalid version '{}': must be valid semver",
                self.version
            ));
        }

        // Description cannot be empty
        if self.description.is_empty() {
            return Err("Plugin description cannot be empty".to_string());
        }

        // min_interface_version must be valid semver if present
        if let Some(ref min_ver) = self.min_interface_version
            && semver::Version::parse(min_ver).is_err()
        {
            return Err(format!("Invalid min_interface_version '{}'", min_ver));
        }

        // Validate actions
        for (action_name, action_def) in &self.actions {
            // Action name must be a valid identifier
            if !is_valid_action_name(action_name) {
                return Err(format!(
                    "Invalid action name '{}': must be non-empty and contain only alphanumeric characters and underscores",
                    action_name
                ));
            }

            // Description must not be empty
            if action_def.description.is_empty() {
                return Err(format!(
                    "Action '{}' has empty description",
                    action_name
                ));
            }

            // If default_keybinding is present, it must parse as valid KeySequence
            if let Some(ref keybinding) = action_def.default_keybinding
                && keybinding.parse::<KeySequence>().is_err()
            {
                return Err(format!(
                    "Action '{}' has invalid keybinding '{}': must be valid key sequence",
                    action_name, keybinding
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_manifest() {
        let toml = r#"
name = "my-plugin"
version = "1.0.0"
description = "A test plugin"
author = "Test Author"
license = "MIT"
homepage = "https://example.com"
repository = "https://github.com/example/plugin"
min_interface_version = "0.1.0"
"#;
        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.name, "my-plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.description, "A test plugin");
        assert_eq!(manifest.author, Some("Test Author".to_string()));
        assert_eq!(manifest.license, Some("MIT".to_string()));
        assert_eq!(manifest.homepage, Some("https://example.com".to_string()));
        assert_eq!(
            manifest.repository,
            Some("https://github.com/example/plugin".to_string())
        );
        assert_eq!(manifest.min_interface_version, Some("0.1.0".to_string()));
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_parse_minimal_manifest() {
        let toml = r#"
name = "minimal-plugin"
version = "0.1.0"
description = "Minimal plugin"
"#;
        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.name, "minimal-plugin");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.description, "Minimal plugin");
        assert!(manifest.author.is_none());
        assert!(manifest.license.is_none());
        assert!(manifest.homepage.is_none());
        assert!(manifest.repository.is_none());
        assert!(manifest.min_interface_version.is_none());
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_parse_with_unknown_fields() {
        // Unknown fields should be silently ignored for forward compatibility
        let toml = r#"
name = "forward-compat-plugin"
version = "1.0.0"
description = "Plugin with unknown fields"
future_field = "should be ignored"
another_unknown = 42
"#;
        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.name, "forward-compat-plugin");
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_name() {
        let manifest = PluginManifest {
            name: "".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            ..Default::default()
        };
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("name cannot be empty"));
    }

    #[test]
    fn test_validate_invalid_version() {
        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            version: "not-semver".to_string(),
            description: "Test".to_string(),
            ..Default::default()
        };
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid version"));
    }

    #[test]
    fn test_validate_empty_description() {
        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "".to_string(),
            ..Default::default()
        };
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("description cannot be empty"));
    }

    #[test]
    fn test_validate_invalid_min_interface_version() {
        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            min_interface_version: Some("bad-version".to_string()),
            ..Default::default()
        };
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid min_interface_version"));
    }

    #[test]
    fn test_parse_manifest_with_actions() {
        let toml = r#"
name = "action-plugin"
version = "1.0.0"
description = "Plugin with actions"

[actions.fetch]
description = "Fetch items from remote"
default_keybinding = "<C-j>"

[actions.sync]
description = "Synchronize with remote"
"#;
        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.name, "action-plugin");
        assert_eq!(manifest.actions.len(), 2);

        let fetch = manifest.actions.get("fetch").unwrap();
        assert_eq!(fetch.description, "Fetch items from remote");
        assert_eq!(fetch.default_keybinding, Some("<C-j>".to_string()));

        let sync = manifest.actions.get("sync").unwrap();
        assert_eq!(sync.description, "Synchronize with remote");
        assert!(sync.default_keybinding.is_none());

        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_action_name_empty() {
        let mut actions = HashMap::new();
        actions.insert(
            "".to_string(),
            super::ActionDefinition {
                description: "Test action".to_string(),
                default_keybinding: None,
            },
        );
        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            actions,
            ..Default::default()
        };
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid action name"));
    }

    #[test]
    fn test_validate_invalid_action_name_special_chars() {
        let mut actions = HashMap::new();
        actions.insert(
            "my-action".to_string(), // hyphens not allowed
            super::ActionDefinition {
                description: "Test action".to_string(),
                default_keybinding: None,
            },
        );
        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            actions,
            ..Default::default()
        };
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid action name"));
    }

    #[test]
    fn test_validate_action_empty_description() {
        let mut actions = HashMap::new();
        actions.insert(
            "fetch".to_string(),
            super::ActionDefinition {
                description: "".to_string(),
                default_keybinding: None,
            },
        );
        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            actions,
            ..Default::default()
        };
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty description"));
    }

    #[test]
    fn test_validate_action_invalid_keybinding() {
        let mut actions = HashMap::new();
        actions.insert(
            "fetch".to_string(),
            super::ActionDefinition {
                description: "Fetch items".to_string(),
                default_keybinding: Some("<Invalid-Key>".to_string()),
            },
        );
        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            actions,
            ..Default::default()
        };
        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid keybinding"));
    }

    #[test]
    fn test_validate_action_valid_keybindings() {
        let mut actions = HashMap::new();
        actions.insert(
            "action1".to_string(),
            super::ActionDefinition {
                description: "Action with Ctrl key".to_string(),
                default_keybinding: Some("<C-j>".to_string()),
            },
        );
        actions.insert(
            "action2".to_string(),
            super::ActionDefinition {
                description: "Action with Alt key".to_string(),
                default_keybinding: Some("<A-f>".to_string()),
            },
        );
        actions.insert(
            "action3".to_string(),
            super::ActionDefinition {
                description: "Action with sequence".to_string(),
                default_keybinding: Some("g g".to_string()),
            },
        );
        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            actions,
            ..Default::default()
        };
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_is_valid_action_name() {
        use super::is_valid_action_name;

        assert!(is_valid_action_name("fetch"));
        assert!(is_valid_action_name("fetch_items"));
        assert!(is_valid_action_name("fetch123"));
        assert!(is_valid_action_name("action_1"));
        assert!(is_valid_action_name("Action"));
        assert!(is_valid_action_name("_private"));

        assert!(!is_valid_action_name(""));
        assert!(!is_valid_action_name("my-action"));
        assert!(!is_valid_action_name("my.action"));
        assert!(!is_valid_action_name("my action"));
        assert!(!is_valid_action_name("action:name"));
    }

    #[test]
    fn test_hook_timeout_default() {
        // Test that hook_timeout_secs defaults to 5 when not specified
        let toml = r#"
name = "timeout-test"
version = "1.0.0"
description = "Test hook timeout default"
"#;
        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.hook_timeout_secs, 5);
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_hook_timeout_custom() {
        // Test that hook_timeout_secs can be set to a custom value
        let toml = r#"
name = "timeout-test"
version = "1.0.0"
description = "Test hook timeout custom"
hook_timeout_secs = 10
"#;
        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.hook_timeout_secs, 10);
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_default_hook_timeout_fn() {
        assert_eq!(super::default_hook_timeout(), 5);
    }
}
