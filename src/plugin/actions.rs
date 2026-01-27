//! Plugin action registry for managing keybinding-triggered actions.

use crate::keybindings::{KeyBinding, KeySequence, KeybindingCache};
use crate::plugin::manifest::PluginManifest;
use std::collections::HashMap;

/// A registered plugin action with resolved keybinding
#[derive(Debug, Clone)]
pub struct PluginAction {
    /// Plugin that owns this action
    pub plugin_name: String,
    /// Action name (e.g., "fetch")
    pub action_name: String,
    /// Description for help panel
    pub description: String,
    /// Resolved keybinding (None if conflict or disabled)
    pub keybinding: Option<KeySequence>,
    /// Full namespace for internal routing: "plugin:jira:fetch"
    pub namespace: String,
}

/// Registry for plugin actions with conflict-aware keybinding resolution
#[derive(Debug, Default)]
pub struct PluginActionRegistry {
    /// All registered actions
    actions: Vec<PluginAction>,
    /// Lookup: single key -> action index
    /// For single-key bindings, maps directly to action
    single_key_map: HashMap<KeyBinding, usize>,
    /// Lookup: namespace -> action index
    namespace_map: HashMap<String, usize>,
    /// Warnings generated during registration (conflicts, etc.)
    warnings: Vec<String>,
}

impl PluginActionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register actions from a plugin manifest.
    /// Returns warnings for any conflicts detected.
    #[allow(unused_variables)]
    pub fn register_plugin(
        &mut self,
        manifest: &PluginManifest,
        overrides: &HashMap<String, String>,
        host_keybindings: &KeybindingCache,
    ) -> Vec<String> {
        let mut warnings = Vec::new();

        for (action_name, action_def) in &manifest.actions {
            let namespace = format!("plugin:{}:{}", manifest.name, action_name);

            // Determine the keybinding to use (override > default > none)
            let keybinding_str = overrides
                .get(action_name)
                .or(action_def.default_keybinding.as_ref());

            // Check if explicitly disabled
            let keybinding = if let Some(kb_str) = keybinding_str {
                if kb_str.is_empty() || kb_str.to_lowercase() == "none" {
                    None // Explicitly disabled
                } else {
                    match kb_str.parse::<KeySequence>() {
                        Ok(seq) => {
                            // Check for host conflict (host wins)
                            if self.conflicts_with_host(&seq, host_keybindings) {
                                warnings.push(format!(
                                    "Plugin '{}' action '{}': keybinding '{}' conflicts with host, action has no binding",
                                    manifest.name, action_name, kb_str
                                ));
                                None
                            }
                            // Check for plugin-to-plugin conflict (first wins)
                            else if let Some(existing) = self.find_conflict(&seq) {
                                warnings.push(format!(
                                    "Plugin '{}' action '{}': keybinding '{}' conflicts with '{}', action has no binding",
                                    manifest.name, action_name, kb_str, existing
                                ));
                                None
                            } else {
                                Some(seq)
                            }
                        }
                        Err(_) => {
                            warnings.push(format!(
                                "Plugin '{}' action '{}': invalid keybinding '{}'",
                                manifest.name, action_name, kb_str
                            ));
                            None
                        }
                    }
                }
            } else {
                None // No default keybinding
            };

            let action = PluginAction {
                plugin_name: manifest.name.clone(),
                action_name: action_name.clone(),
                description: action_def.description.clone(),
                keybinding: keybinding.clone(),
                namespace: namespace.clone(),
            };

            let idx = self.actions.len();
            self.namespace_map.insert(namespace, idx);

            // Register keybinding lookup (only single-key for now)
            if let Some(ref seq) = keybinding
                && seq.is_single()
            {
                self.single_key_map.insert(seq.0[0], idx);
            }
            // Note: Multi-key sequences would need additional handling
            // Kept simple for initial implementation

            self.actions.push(action);
        }

        self.warnings.extend(warnings.clone());
        warnings
    }

    /// Check if keybinding conflicts with host keybindings
    #[allow(unused_variables)]
    fn conflicts_with_host(&self, seq: &KeySequence, host: &KeybindingCache) -> bool {
        // Check if the first key of the sequence is bound in host navigate mode
        // This is a simplified check - full implementation would check all modes
        // For now, return false and let execution-time routing handle it
        // (host checks first, then plugin)
        false
    }

    /// Find existing plugin action that conflicts with this keybinding
    fn find_conflict(&self, seq: &KeySequence) -> Option<String> {
        if seq.is_single()
            && let Some(&idx) = self.single_key_map.get(&seq.0[0])
        {
            return Some(self.actions[idx].namespace.clone());
        }
        None
    }

    /// Lookup action by single keybinding
    pub fn lookup(&self, binding: &KeyBinding) -> Option<&PluginAction> {
        self.single_key_map
            .get(binding)
            .map(|&idx| &self.actions[idx])
    }

    /// Lookup action by namespace
    pub fn lookup_by_namespace(&self, namespace: &str) -> Option<&PluginAction> {
        self.namespace_map
            .get(namespace)
            .map(|&idx| &self.actions[idx])
    }

    /// Get all actions grouped by plugin name for help display
    pub fn actions_by_plugin(&self) -> HashMap<String, Vec<&PluginAction>> {
        let mut grouped: HashMap<String, Vec<&PluginAction>> = HashMap::new();
        for action in &self.actions {
            grouped
                .entry(action.plugin_name.clone())
                .or_default()
                .push(action);
        }
        grouped
    }

    /// Get all warnings generated during registration
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Check if registry has any registered actions
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::ActionDefinition;

    fn make_manifest(name: &str, actions: HashMap<String, ActionDefinition>) -> PluginManifest {
        PluginManifest {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin".to_string(),
            actions,
            ..Default::default()
        }
    }

    #[test]
    fn test_register_plugin_actions() {
        let mut registry = PluginActionRegistry::new();
        let mut actions = HashMap::new();
        actions.insert(
            "fetch".to_string(),
            ActionDefinition {
                description: "Fetch from Jira".to_string(),
                default_keybinding: Some("<C-j>".to_string()),
            },
        );
        let manifest = make_manifest("jira", actions);
        let host = KeybindingCache::default();

        let warnings = registry.register_plugin(&manifest, &HashMap::new(), &host);
        assert!(warnings.is_empty());

        let action = registry.lookup_by_namespace("plugin:jira:fetch");
        assert!(action.is_some());
        assert_eq!(action.unwrap().description, "Fetch from Jira");
    }

    #[test]
    fn test_override_keybinding() {
        use crossterm::event::KeyModifiers;

        let mut registry = PluginActionRegistry::new();
        let mut actions = HashMap::new();
        actions.insert(
            "fetch".to_string(),
            ActionDefinition {
                description: "Fetch".to_string(),
                default_keybinding: Some("<C-j>".to_string()),
            },
        );
        let manifest = make_manifest("jira", actions);
        let host = KeybindingCache::default();

        let mut overrides = HashMap::new();
        overrides.insert("fetch".to_string(), "<C-f>".to_string());

        registry.register_plugin(&manifest, &overrides, &host);

        let action = registry.lookup_by_namespace("plugin:jira:fetch").unwrap();
        let kb = action.keybinding.as_ref().unwrap();
        assert!(kb.is_single());
        // Verify it's Ctrl-F not Ctrl-J
        assert!(kb.0[0].modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_disable_action() {
        let mut registry = PluginActionRegistry::new();
        let mut actions = HashMap::new();
        actions.insert(
            "fetch".to_string(),
            ActionDefinition {
                description: "Fetch".to_string(),
                default_keybinding: Some("<C-j>".to_string()),
            },
        );
        let manifest = make_manifest("jira", actions);
        let host = KeybindingCache::default();

        let mut overrides = HashMap::new();
        overrides.insert("fetch".to_string(), "none".to_string());

        registry.register_plugin(&manifest, &overrides, &host);

        let action = registry.lookup_by_namespace("plugin:jira:fetch").unwrap();
        assert!(action.keybinding.is_none());
    }

    #[test]
    fn test_plugin_to_plugin_conflict() {
        let mut registry = PluginActionRegistry::new();
        let host = KeybindingCache::default();

        // First plugin
        let mut actions1 = HashMap::new();
        actions1.insert(
            "action1".to_string(),
            ActionDefinition {
                description: "Action 1".to_string(),
                default_keybinding: Some("<C-x>".to_string()),
            },
        );
        let manifest1 = make_manifest("plugin1", actions1);
        registry.register_plugin(&manifest1, &HashMap::new(), &host);

        // Second plugin with same keybinding
        let mut actions2 = HashMap::new();
        actions2.insert(
            "action2".to_string(),
            ActionDefinition {
                description: "Action 2".to_string(),
                default_keybinding: Some("<C-x>".to_string()),
            },
        );
        let manifest2 = make_manifest("plugin2", actions2);
        let warnings = registry.register_plugin(&manifest2, &HashMap::new(), &host);

        assert!(!warnings.is_empty());
        assert!(warnings[0].contains("conflicts"));

        // First plugin keeps the binding
        let action1 = registry
            .lookup_by_namespace("plugin:plugin1:action1")
            .unwrap();
        assert!(action1.keybinding.is_some());

        // Second plugin has no binding
        let action2 = registry
            .lookup_by_namespace("plugin:plugin2:action2")
            .unwrap();
        assert!(action2.keybinding.is_none());
    }

    #[test]
    fn test_actions_by_plugin() {
        let mut registry = PluginActionRegistry::new();
        let host = KeybindingCache::default();

        let mut actions = HashMap::new();
        actions.insert(
            "fetch".to_string(),
            ActionDefinition {
                description: "Fetch".to_string(),
                default_keybinding: None,
            },
        );
        actions.insert(
            "sync".to_string(),
            ActionDefinition {
                description: "Sync".to_string(),
                default_keybinding: None,
            },
        );
        let manifest = make_manifest("jira", actions);
        registry.register_plugin(&manifest, &HashMap::new(), &host);

        let grouped = registry.actions_by_plugin();
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped.get("jira").unwrap().len(), 2);
    }

    #[test]
    fn test_lookup_by_keybinding() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut registry = PluginActionRegistry::new();
        let mut actions = HashMap::new();
        actions.insert(
            "fetch".to_string(),
            ActionDefinition {
                description: "Fetch".to_string(),
                default_keybinding: Some("<C-j>".to_string()),
            },
        );
        let manifest = make_manifest("jira", actions);
        let host = KeybindingCache::default();
        registry.register_plugin(&manifest, &HashMap::new(), &host);

        // Lookup with correct keybinding
        let binding = KeyBinding::new(KeyCode::Char('j'), KeyModifiers::CONTROL);
        let action = registry.lookup(&binding);
        assert!(action.is_some());
        assert_eq!(action.unwrap().action_name, "fetch");

        // Lookup with wrong keybinding
        let wrong_binding = KeyBinding::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
        let no_action = registry.lookup(&wrong_binding);
        assert!(no_action.is_none());
    }

    #[test]
    fn test_is_empty() {
        let registry = PluginActionRegistry::new();
        assert!(registry.is_empty());

        let mut registry = PluginActionRegistry::new();
        let mut actions = HashMap::new();
        actions.insert(
            "fetch".to_string(),
            ActionDefinition {
                description: "Fetch".to_string(),
                default_keybinding: None,
            },
        );
        let manifest = make_manifest("jira", actions);
        let host = KeybindingCache::default();
        registry.register_plugin(&manifest, &HashMap::new(), &host);

        assert!(!registry.is_empty());
    }

    #[test]
    fn test_warnings_stored() {
        let mut registry = PluginActionRegistry::new();
        let host = KeybindingCache::default();

        // Invalid keybinding
        let mut actions = HashMap::new();
        actions.insert(
            "fetch".to_string(),
            ActionDefinition {
                description: "Fetch".to_string(),
                default_keybinding: Some("<Invalid>".to_string()),
            },
        );
        let manifest = make_manifest("jira", actions);
        registry.register_plugin(&manifest, &HashMap::new(), &host);

        assert!(!registry.warnings().is_empty());
        assert!(registry.warnings()[0].contains("invalid keybinding"));
    }
}
