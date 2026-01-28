//! Metadata storage operations for plugin data.
//!
//! This module provides CRUD operations for plugin metadata attached to
//! todo items and projects. Each plugin gets its own namespace (keyed by
//! plugin_name), so plugins can't see or modify each other's metadata.
//!
//! Metadata is stored as JSON strings. Keys starting with '_' are reserved
//! for system use and will be rejected.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

use super::database::get_connection;

// ============================================================================
// JSON Validation
// ============================================================================

/// Validate metadata JSON and check for reserved keys.
///
/// Reserved keys start with '_' (underscore) and are reserved for system use.
fn validate_metadata_json(data: &str) -> Result<()> {
    let value: serde_json::Value =
        serde_json::from_str(data).with_context(|| format!("Invalid JSON: {}", data))?;

    if let serde_json::Value::Object(map) = &value {
        for key in map.keys() {
            if key.starts_with('_') {
                anyhow::bail!("Keys starting with '_' are reserved: {}", key);
            }
        }
    }
    Ok(())
}

// ============================================================================
// Todo Metadata CRUD
// ============================================================================

/// Set metadata for a todo item.
///
/// # Arguments
///
/// * `todo_id` - UUID of the todo item
/// * `plugin_name` - Name of the plugin storing the metadata
/// * `data` - JSON string containing the metadata
/// * `merge` - If true, merge with existing data using json_patch; if false, replace entirely
///
/// # Errors
///
/// Returns an error if:
/// * The JSON is invalid
/// * A key starts with '_' (reserved)
/// * Database operation fails
pub fn set_todo_metadata(todo_id: &Uuid, plugin_name: &str, data: &str, merge: bool) -> Result<()> {
    validate_metadata_json(data)?;

    let conn = get_connection()?;
    let now = Utc::now().to_rfc3339();
    let todo_id_str = todo_id.to_string();

    if merge {
        // Try to merge with existing data
        let existing: Option<String> = conn
            .query_row(
                "SELECT data FROM todo_metadata WHERE todo_id = ?1 AND plugin_name = ?2",
                params![&todo_id_str, plugin_name],
                |row| row.get(0),
            )
            .ok();

        if let Some(existing_data) = existing {
            // Merge JSON objects
            let merged = merge_json(&existing_data, data)?;
            conn.execute(
                "UPDATE todo_metadata SET data = ?1, updated_at = ?2 WHERE todo_id = ?3 AND plugin_name = ?4",
                params![&merged, &now, &todo_id_str, plugin_name],
            )?;
        } else {
            // No existing data, insert new
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO todo_metadata (id, todo_id, plugin_name, data, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![&id, &todo_id_str, plugin_name, data, &now],
            )?;
        }
    } else {
        // Replace entirely using upsert
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO todo_metadata (id, todo_id, plugin_name, data, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(todo_id, plugin_name) DO UPDATE SET data = ?4, updated_at = ?5",
            params![&id, &todo_id_str, plugin_name, data, &now],
        )?;
    }

    Ok(())
}

/// Get metadata for a todo item.
///
/// Returns an empty JSON object "{}" if no metadata exists.
///
/// # Arguments
///
/// * `todo_id` - UUID of the todo item
/// * `plugin_name` - Name of the plugin retrieving the metadata
pub fn get_todo_metadata(todo_id: &Uuid, plugin_name: &str) -> Result<String> {
    let conn = get_connection()?;
    let todo_id_str = todo_id.to_string();

    let result: rusqlite::Result<String> = conn.query_row(
        "SELECT data FROM todo_metadata WHERE todo_id = ?1 AND plugin_name = ?2",
        params![&todo_id_str, plugin_name],
        |row| row.get(0),
    );

    match result {
        Ok(data) => Ok(data),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok("{}".to_string()),
        Err(e) => Err(e.into()),
    }
}

/// Delete metadata for a todo item.
///
/// Returns true if metadata was deleted, false if it didn't exist.
///
/// # Arguments
///
/// * `todo_id` - UUID of the todo item
/// * `plugin_name` - Name of the plugin deleting the metadata
pub fn delete_todo_metadata(todo_id: &Uuid, plugin_name: &str) -> Result<bool> {
    let conn = get_connection()?;
    let todo_id_str = todo_id.to_string();

    let rows_affected = conn.execute(
        "DELETE FROM todo_metadata WHERE todo_id = ?1 AND plugin_name = ?2",
        params![&todo_id_str, plugin_name],
    )?;

    Ok(rows_affected > 0)
}

// ============================================================================
// Project Metadata CRUD
// ============================================================================

/// Set metadata for a project.
///
/// # Arguments
///
/// * `project_name` - Name of the project
/// * `plugin_name` - Name of the plugin storing the metadata
/// * `data` - JSON string containing the metadata
/// * `merge` - If true, merge with existing data using json_patch; if false, replace entirely
///
/// # Errors
///
/// Returns an error if:
/// * The JSON is invalid
/// * A key starts with '_' (reserved)
/// * Database operation fails
pub fn set_project_metadata(
    project_name: &str,
    plugin_name: &str,
    data: &str,
    merge: bool,
) -> Result<()> {
    validate_metadata_json(data)?;

    let conn = get_connection()?;
    let now = Utc::now().to_rfc3339();

    if merge {
        // Try to merge with existing data
        let existing: Option<String> = conn
            .query_row(
                "SELECT data FROM project_metadata WHERE project_name = ?1 AND plugin_name = ?2",
                params![project_name, plugin_name],
                |row| row.get(0),
            )
            .ok();

        if let Some(existing_data) = existing {
            // Merge JSON objects
            let merged = merge_json(&existing_data, data)?;
            conn.execute(
                "UPDATE project_metadata SET data = ?1, updated_at = ?2 WHERE project_name = ?3 AND plugin_name = ?4",
                params![&merged, &now, project_name, plugin_name],
            )?;
        } else {
            // No existing data, insert new
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO project_metadata (id, project_name, plugin_name, data, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![&id, project_name, plugin_name, data, &now],
            )?;
        }
    } else {
        // Replace entirely using upsert
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO project_metadata (id, project_name, plugin_name, data, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(project_name, plugin_name) DO UPDATE SET data = ?4, updated_at = ?5",
            params![&id, project_name, plugin_name, data, &now],
        )?;
    }

    Ok(())
}

/// Get metadata for a project.
///
/// Returns an empty JSON object "{}" if no metadata exists.
///
/// # Arguments
///
/// * `project_name` - Name of the project
/// * `plugin_name` - Name of the plugin retrieving the metadata
pub fn get_project_metadata(project_name: &str, plugin_name: &str) -> Result<String> {
    let conn = get_connection()?;

    let result: rusqlite::Result<String> = conn.query_row(
        "SELECT data FROM project_metadata WHERE project_name = ?1 AND plugin_name = ?2",
        params![project_name, plugin_name],
        |row| row.get(0),
    );

    match result {
        Ok(data) => Ok(data),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok("{}".to_string()),
        Err(e) => Err(e.into()),
    }
}

/// Delete metadata for a project.
///
/// Returns true if metadata was deleted, false if it didn't exist.
///
/// # Arguments
///
/// * `project_name` - Name of the project
/// * `plugin_name` - Name of the plugin deleting the metadata
pub fn delete_project_metadata(project_name: &str, plugin_name: &str) -> Result<bool> {
    let conn = get_connection()?;

    let rows_affected = conn.execute(
        "DELETE FROM project_metadata WHERE project_name = ?1 AND plugin_name = ?2",
        params![project_name, plugin_name],
    )?;

    Ok(rows_affected > 0)
}

// ============================================================================
// External ID Operations
// ============================================================================

/// Set the external ID for a todo item.
///
/// External IDs allow plugins to reference todos by their own stable identifiers
/// (e.g., "claude-tasklist-1-task-2") instead of totui's internal UUIDs.
///
/// # Arguments
///
/// * `todo_id` - UUID of the todo item
/// * `plugin_name` - Name of the plugin setting the external ID
/// * `external_id` - The plugin's stable identifier for this todo
pub fn set_external_id(todo_id: &Uuid, plugin_name: &str, external_id: &str) -> Result<()> {
    let conn = get_connection()?;
    let now = Utc::now().to_rfc3339();
    let todo_id_str = todo_id.to_string();

    // Upsert: create metadata row if not exists, or update external_id
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO todo_metadata (id, todo_id, plugin_name, data, external_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, '{}', ?4, ?5, ?5)
         ON CONFLICT(todo_id, plugin_name) DO UPDATE SET external_id = ?4, updated_at = ?5",
        params![&id, &todo_id_str, plugin_name, external_id, &now],
    )?;

    Ok(())
}

/// Look up a todo's UUID by its external ID.
///
/// # Arguments
///
/// * `plugin_name` - Name of the plugin that set the external ID
/// * `external_id` - The plugin's stable identifier
///
/// # Returns
///
/// * `Ok(Some(uuid))` - The todo's UUID if found
/// * `Ok(None)` - If no todo with this external ID exists
pub fn get_todo_id_by_external_id(plugin_name: &str, external_id: &str) -> Result<Option<Uuid>> {
    let conn = get_connection()?;

    let result: rusqlite::Result<String> = conn.query_row(
        "SELECT todo_id FROM todo_metadata WHERE plugin_name = ?1 AND external_id = ?2",
        params![plugin_name, external_id],
        |row| row.get(0),
    );

    match result {
        Ok(todo_id_str) => {
            let uuid = Uuid::parse_str(&todo_id_str)
                .with_context(|| format!("Invalid UUID in database: {}", todo_id_str))?;
            Ok(Some(uuid))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

// ============================================================================
// JSON Utilities
// ============================================================================

/// Merge two JSON objects, with new_data overwriting existing keys.
fn merge_json(existing: &str, new_data: &str) -> Result<String> {
    let mut existing_value: serde_json::Value = serde_json::from_str(existing)
        .with_context(|| format!("Invalid existing JSON: {}", existing))?;

    let new_value: serde_json::Value = serde_json::from_str(new_data)
        .with_context(|| format!("Invalid new JSON: {}", new_data))?;

    if let (serde_json::Value::Object(existing_map), serde_json::Value::Object(new_map)) =
        (&mut existing_value, new_value)
    {
        for (key, value) in new_map {
            existing_map.insert(key, value);
        }
    }

    serde_json::to_string(&existing_value).with_context(|| "Failed to serialize merged JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::init_database;
    use std::env;
    use tempfile::TempDir;

    fn setup_test_env() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        // Create the .to-tui directory that init_database expects
        let to_tui_dir = temp_dir.path().join(".to-tui");
        std::fs::create_dir_all(&to_tui_dir).unwrap();
        // SAFETY: Tests run single-threaded (cargo test -- --test-threads=1 or serial)
        // and HOME is only modified in test setup before any other code runs.
        unsafe {
            env::set_var("HOME", temp_dir.path());
        }
        init_database().unwrap();
        temp_dir
    }

    #[test]
    fn test_set_and_get_todo_metadata() {
        let _temp = setup_test_env();
        let todo_id = Uuid::new_v4();
        let plugin_name = "test_plugin";
        let data = r#"{"key": "value"}"#;

        set_todo_metadata(&todo_id, plugin_name, data, false).unwrap();
        let result = get_todo_metadata(&todo_id, plugin_name).unwrap();

        assert_eq!(result, data);
    }

    #[test]
    fn test_get_todo_metadata_returns_empty_for_nonexistent() {
        let _temp = setup_test_env();
        let todo_id = Uuid::new_v4();
        let plugin_name = "test_plugin";

        let result = get_todo_metadata(&todo_id, plugin_name).unwrap();

        assert_eq!(result, "{}");
    }

    #[test]
    fn test_set_todo_metadata_merge_true_merges_keys() {
        let _temp = setup_test_env();
        let todo_id = Uuid::new_v4();
        let plugin_name = "test_plugin";

        // Initial data
        set_todo_metadata(&todo_id, plugin_name, r#"{"a": 1, "b": 2}"#, false).unwrap();

        // Merge with new data
        set_todo_metadata(&todo_id, plugin_name, r#"{"b": 3, "c": 4}"#, true).unwrap();

        let result = get_todo_metadata(&todo_id, plugin_name).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        assert_eq!(parsed["a"], 1);
        assert_eq!(parsed["b"], 3); // Overwritten
        assert_eq!(parsed["c"], 4); // New key
    }

    #[test]
    fn test_set_todo_metadata_merge_false_replaces_entirely() {
        let _temp = setup_test_env();
        let todo_id = Uuid::new_v4();
        let plugin_name = "test_plugin";

        // Initial data
        set_todo_metadata(&todo_id, plugin_name, r#"{"a": 1, "b": 2}"#, false).unwrap();

        // Replace entirely
        set_todo_metadata(&todo_id, plugin_name, r#"{"c": 3}"#, false).unwrap();

        let result = get_todo_metadata(&todo_id, plugin_name).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        assert!(parsed.get("a").is_none());
        assert!(parsed.get("b").is_none());
        assert_eq!(parsed["c"], 3);
    }

    #[test]
    fn test_reserved_key_prefix_rejected() {
        let _temp = setup_test_env();
        let todo_id = Uuid::new_v4();
        let plugin_name = "test_plugin";
        let data = r#"{"_reserved": "value"}"#;

        let result = set_todo_metadata(&todo_id, plugin_name, data, false);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Keys starting with '_' are reserved"));
    }

    #[test]
    fn test_invalid_json_rejected() {
        let _temp = setup_test_env();
        let todo_id = Uuid::new_v4();
        let plugin_name = "test_plugin";
        let data = "not valid json";

        let result = set_todo_metadata(&todo_id, plugin_name, data, false);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid JSON"));
    }

    #[test]
    fn test_delete_todo_metadata_returns_true_for_existing() {
        let _temp = setup_test_env();
        let todo_id = Uuid::new_v4();
        let plugin_name = "test_plugin";

        set_todo_metadata(&todo_id, plugin_name, r#"{"key": "value"}"#, false).unwrap();

        let deleted = delete_todo_metadata(&todo_id, plugin_name).unwrap();
        assert!(deleted);

        // Verify it's gone
        let result = get_todo_metadata(&todo_id, plugin_name).unwrap();
        assert_eq!(result, "{}");
    }

    #[test]
    fn test_delete_todo_metadata_returns_false_for_nonexistent() {
        let _temp = setup_test_env();
        let todo_id = Uuid::new_v4();
        let plugin_name = "test_plugin";

        let deleted = delete_todo_metadata(&todo_id, plugin_name).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_set_and_get_project_metadata() {
        let _temp = setup_test_env();
        let project_name = "my_project";
        let plugin_name = "test_plugin";
        let data = r#"{"project_key": "project_value"}"#;

        set_project_metadata(project_name, plugin_name, data, false).unwrap();
        let result = get_project_metadata(project_name, plugin_name).unwrap();

        assert_eq!(result, data);
    }

    #[test]
    fn test_get_project_metadata_returns_empty_for_nonexistent() {
        let _temp = setup_test_env();
        let project_name = "nonexistent_project";
        let plugin_name = "test_plugin";

        let result = get_project_metadata(project_name, plugin_name).unwrap();

        assert_eq!(result, "{}");
    }

    #[test]
    fn test_delete_project_metadata() {
        let _temp = setup_test_env();
        let project_name = "my_project";
        let plugin_name = "test_plugin";

        set_project_metadata(project_name, plugin_name, r#"{"key": "value"}"#, false).unwrap();

        let deleted = delete_project_metadata(project_name, plugin_name).unwrap();
        assert!(deleted);

        // Verify it's gone
        let result = get_project_metadata(project_name, plugin_name).unwrap();
        assert_eq!(result, "{}");
    }

    #[test]
    fn test_different_plugins_have_separate_metadata() {
        let _temp = setup_test_env();
        let todo_id = Uuid::new_v4();

        set_todo_metadata(&todo_id, "plugin_a", r#"{"from": "a"}"#, false).unwrap();
        set_todo_metadata(&todo_id, "plugin_b", r#"{"from": "b"}"#, false).unwrap();

        let result_a = get_todo_metadata(&todo_id, "plugin_a").unwrap();
        let result_b = get_todo_metadata(&todo_id, "plugin_b").unwrap();

        assert_eq!(result_a, r#"{"from": "a"}"#);
        assert_eq!(result_b, r#"{"from": "b"}"#);
    }

    // ========================================================================
    // External ID Tests
    // ========================================================================

    #[test]
    fn test_set_and_get_external_id() {
        let _temp = setup_test_env();
        let todo_id = Uuid::new_v4();
        let plugin_name = "claude-tasks";
        let external_id = "claude-abc123-1";

        set_external_id(&todo_id, plugin_name, external_id).unwrap();

        let result = get_todo_id_by_external_id(plugin_name, external_id).unwrap();
        assert_eq!(result, Some(todo_id));
    }

    #[test]
    fn test_get_external_id_not_found() {
        let _temp = setup_test_env();

        let result = get_todo_id_by_external_id("plugin", "nonexistent").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_external_id_isolated_by_plugin() {
        let _temp = setup_test_env();
        let todo_id_a = Uuid::new_v4();
        let todo_id_b = Uuid::new_v4();
        let external_id = "same-external-id";

        // Same external_id can exist for different plugins
        set_external_id(&todo_id_a, "plugin_a", external_id).unwrap();
        set_external_id(&todo_id_b, "plugin_b", external_id).unwrap();

        let result_a = get_todo_id_by_external_id("plugin_a", external_id).unwrap();
        let result_b = get_todo_id_by_external_id("plugin_b", external_id).unwrap();

        assert_eq!(result_a, Some(todo_id_a));
        assert_eq!(result_b, Some(todo_id_b));
    }

    #[test]
    fn test_external_id_update() {
        let _temp = setup_test_env();
        let todo_id = Uuid::new_v4();
        let plugin_name = "plugin";

        // Set initial external_id
        set_external_id(&todo_id, plugin_name, "old-id").unwrap();
        assert_eq!(
            get_todo_id_by_external_id(plugin_name, "old-id").unwrap(),
            Some(todo_id)
        );

        // Update to new external_id
        set_external_id(&todo_id, plugin_name, "new-id").unwrap();

        // Old ID should no longer resolve
        assert_eq!(
            get_todo_id_by_external_id(plugin_name, "old-id").unwrap(),
            None
        );
        // New ID should resolve
        assert_eq!(
            get_todo_id_by_external_id(plugin_name, "new-id").unwrap(),
            Some(todo_id)
        );
    }

    #[test]
    fn test_external_id_with_metadata() {
        let _temp = setup_test_env();
        let todo_id = Uuid::new_v4();
        let plugin_name = "plugin";
        let external_id = "ext-123";

        // Set external_id first
        set_external_id(&todo_id, plugin_name, external_id).unwrap();

        // Then set metadata - should not clobber external_id
        set_todo_metadata(&todo_id, plugin_name, r#"{"key": "value"}"#, false).unwrap();

        // External ID should still work
        let result = get_todo_id_by_external_id(plugin_name, external_id).unwrap();
        assert_eq!(result, Some(todo_id));

        // Metadata should also work
        let metadata = get_todo_metadata(&todo_id, plugin_name).unwrap();
        assert_eq!(metadata, r#"{"key": "value"}"#);
    }
}
