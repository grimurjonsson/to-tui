use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use uuid::Uuid;

use crate::utils::paths::get_ui_cache_path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiCache {
    /// The ID of the currently selected todo item
    pub selected_todo_id: Option<Uuid>,
}

impl UiCache {
    pub fn load() -> Result<Self> {
        let path = get_ui_cache_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)?;
        let cache: UiCache = serde_json::from_str(&content)?;
        Ok(cache)
    }

    pub fn save(&self) -> Result<()> {
        let path = get_ui_cache_path()?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_cache() {
        let cache = UiCache::default();
        assert!(cache.selected_todo_id.is_none());
    }

    #[test]
    fn test_serialize_deserialize() {
        let todo_id = Uuid::new_v4();
        let cache = UiCache {
            selected_todo_id: Some(todo_id),
        };

        let json = serde_json::to_string(&cache).unwrap();
        let loaded: UiCache = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.selected_todo_id, Some(todo_id));
    }

    #[test]
    fn test_serialize_none() {
        let cache = UiCache {
            selected_todo_id: None,
        };

        let json = serde_json::to_string(&cache).unwrap();
        let loaded: UiCache = serde_json::from_str(&json).unwrap();

        assert!(loaded.selected_todo_id.is_none());
    }
}
