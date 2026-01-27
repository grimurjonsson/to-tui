//! Command executor for plugin mutations.
//!
//! This module provides the CommandExecutor that processes plugin commands
//! (FfiCommand) and applies them to the todo list with proper temp ID resolution.

use anyhow::{anyhow, Result};
use chrono::{NaiveDate, Utc};
use std::collections::HashMap;
use totui_plugin_interface::{FfiCommand, FfiMovePosition, FfiPriority, FfiTodoState};
use uuid::Uuid;

use crate::storage::metadata;
use crate::todo::{Priority, TodoItem, TodoList, TodoState};

/// Executes plugin commands with undo/redo integration.
///
/// Commands are executed as a batch with a single undo snapshot,
/// enabling atomic rollback of all plugin mutations.
pub struct CommandExecutor {
    /// Mapping from temp IDs to real UUIDs created during batch
    temp_id_map: HashMap<String, Uuid>,
    /// Name of the plugin executing commands (for metadata namespace isolation)
    plugin_name: String,
}

impl CommandExecutor {
    /// Create a new CommandExecutor.
    ///
    /// # Arguments
    /// * `plugin_name` - Name of the plugin executing commands (for metadata namespace)
    pub fn new(plugin_name: String) -> Self {
        Self {
            temp_id_map: HashMap::new(),
            plugin_name,
        }
    }

    /// Execute a batch of commands against the todo list.
    ///
    /// Returns the UUIDs of any created items.
    ///
    /// # Arguments
    ///
    /// * `commands` - The commands to execute
    /// * `todo_list` - The todo list to mutate
    ///
    /// # Returns
    ///
    /// * `Ok(created_ids)` - UUIDs of newly created items
    /// * `Err(e)` - If any command fails (e.g., item not found)
    pub fn execute_batch(
        &mut self,
        commands: Vec<FfiCommand>,
        todo_list: &mut TodoList,
    ) -> Result<Vec<Uuid>> {
        // Clear temp ID map at start of each batch
        self.temp_id_map.clear();

        let mut created_ids = Vec::new();

        for command in commands {
            match command {
                FfiCommand::CreateTodo {
                    content,
                    parent_id,
                    temp_id,
                    state,
                    priority,
                    indent_level,
                } => {
                    let parent_id_opt: Option<String> = parent_id.into_option().map(|s| s.into());
                    let temp_id_opt: Option<String> = temp_id.into_option().map(|s| s.into());
                    let id = self.handle_create(
                        content.as_str(),
                        parent_id_opt.as_deref(),
                        temp_id_opt.as_deref(),
                        state,
                        priority.into_option(),
                        indent_level,
                        todo_list,
                    )?;
                    created_ids.push(id);
                }
                FfiCommand::UpdateTodo {
                    id,
                    content,
                    state,
                    priority,
                    due_date,
                    description,
                } => {
                    let content_opt: Option<String> = content.into_option().map(|s| s.into());
                    let due_date_opt: Option<String> = due_date.into_option().map(|s| s.into());
                    let description_opt: Option<String> =
                        description.into_option().map(|s| s.into());
                    self.handle_update(
                        id.as_str(),
                        content_opt.as_deref(),
                        state.into_option(),
                        priority.into_option(),
                        due_date_opt.as_deref(),
                        description_opt.as_deref(),
                        todo_list,
                    )?;
                }
                FfiCommand::DeleteTodo { id } => {
                    self.handle_delete(id.as_str(), todo_list)?;
                }
                FfiCommand::MoveTodo { id, position } => {
                    self.handle_move(id.as_str(), position, todo_list)?;
                }
                FfiCommand::SetTodoMetadata {
                    todo_id,
                    data,
                    merge,
                } => {
                    let uuid = self.resolve_id(todo_id.as_str())?;
                    metadata::set_todo_metadata(&uuid, &self.plugin_name, data.as_str(), merge)?;
                }
                FfiCommand::SetProjectMetadata {
                    project_name,
                    data,
                    merge,
                } => {
                    metadata::set_project_metadata(
                        project_name.as_str(),
                        &self.plugin_name,
                        data.as_str(),
                        merge,
                    )?;
                }
                FfiCommand::DeleteTodoMetadata { todo_id } => {
                    let uuid = self.resolve_id(todo_id.as_str())?;
                    metadata::delete_todo_metadata(&uuid, &self.plugin_name)?;
                }
                FfiCommand::DeleteProjectMetadata { project_name } => {
                    metadata::delete_project_metadata(project_name.as_str(), &self.plugin_name)?;
                }
            }
        }

        // Recalculate parent IDs after all mutations
        todo_list.recalculate_parent_ids();

        Ok(created_ids)
    }

    /// Handle a CreateTodo command.
    #[allow(clippy::too_many_arguments)]
    fn handle_create(
        &mut self,
        content: &str,
        parent_id: Option<&str>,
        temp_id: Option<&str>,
        state: FfiTodoState,
        priority: Option<FfiPriority>,
        indent_level: u32,
        todo_list: &mut TodoList,
    ) -> Result<Uuid> {
        // Create the new item
        let mut item = TodoItem::new(content.to_string(), indent_level as usize);

        // Set state
        item.state = convert_ffi_state(state);

        // Set priority if provided
        if let Some(p) = priority {
            item.priority = Some(convert_ffi_priority(p));
        }

        // Store temp ID mapping if provided
        if let Some(tid) = temp_id {
            self.temp_id_map.insert(tid.to_string(), item.id);
        }

        // Resolve parent_id if provided
        if let Some(pid_str) = parent_id {
            let parent_uuid = self.resolve_id(pid_str)?;
            item.parent_id = Some(parent_uuid);

            // Find insert position after parent's children
            if let Some((_, insert_at)) = todo_list.find_insert_position_for_child(parent_uuid) {
                todo_list.items.insert(insert_at, item.clone());
            } else {
                // Parent not found in list, append to end
                todo_list.items.push(item.clone());
            }
        } else {
            // No parent, append to end
            todo_list.items.push(item.clone());
        }

        Ok(item.id)
    }

    /// Handle an UpdateTodo command.
    #[allow(clippy::too_many_arguments)]
    fn handle_update(
        &self,
        id: &str,
        content: Option<&str>,
        state: Option<FfiTodoState>,
        priority: Option<FfiPriority>,
        due_date: Option<&str>,
        description: Option<&str>,
        todo_list: &mut TodoList,
    ) -> Result<()> {
        let uuid = self.resolve_id(id)?;

        let item = todo_list
            .items
            .iter_mut()
            .find(|i| i.id == uuid)
            .ok_or_else(|| anyhow!("Todo not found: {}", id))?;

        // Update fields that are provided
        if let Some(c) = content {
            item.content = c.to_string();
        }

        if let Some(s) = state {
            item.state = convert_ffi_state(s);
        }

        if let Some(p) = priority {
            item.priority = Some(convert_ffi_priority(p));
        }

        if let Some(dd) = due_date {
            // Parse YYYY-MM-DD format
            item.due_date = NaiveDate::parse_from_str(dd, "%Y-%m-%d").ok();
        }

        if let Some(desc) = description {
            item.description = Some(desc.to_string());
        }

        item.modified_at = Utc::now();

        Ok(())
    }

    /// Handle a DeleteTodo command (soft delete).
    fn handle_delete(&self, id: &str, todo_list: &mut TodoList) -> Result<()> {
        let uuid = self.resolve_id(id)?;

        let item = todo_list
            .items
            .iter_mut()
            .find(|i| i.id == uuid)
            .ok_or_else(|| anyhow!("Todo not found: {}", id))?;

        // Soft delete per codebase convention
        item.deleted_at = Some(Utc::now());

        Ok(())
    }

    /// Handle a MoveTodo command.
    fn handle_move(
        &self,
        id: &str,
        position: FfiMovePosition,
        todo_list: &mut TodoList,
    ) -> Result<()> {
        let uuid = self.resolve_id(id)?;

        // Find current index
        let current_idx = todo_list
            .items
            .iter()
            .position(|i| i.id == uuid)
            .ok_or_else(|| anyhow!("Todo not found: {}", id))?;

        // Remove item from current position
        let item = todo_list.items.remove(current_idx);

        // Calculate new position
        let new_idx = match position {
            FfiMovePosition::Before { target_id } => {
                let target_uuid = self.resolve_id(target_id.as_str())?;
                todo_list
                    .items
                    .iter()
                    .position(|i| i.id == target_uuid)
                    .ok_or_else(|| anyhow!("Target not found: {}", target_id))?
            }
            FfiMovePosition::After { target_id } => {
                let target_uuid = self.resolve_id(target_id.as_str())?;
                let target_idx = todo_list
                    .items
                    .iter()
                    .position(|i| i.id == target_uuid)
                    .ok_or_else(|| anyhow!("Target not found: {}", target_id))?;
                // Insert after target
                target_idx + 1
            }
            FfiMovePosition::AtIndex { index } => {
                // Clamp to valid range
                (index as usize).min(todo_list.items.len())
            }
        };

        // Insert at new position
        todo_list.items.insert(new_idx, item);

        Ok(())
    }

    /// Resolve an ID string to a UUID.
    ///
    /// First checks the temp ID map, then tries to parse as UUID.
    fn resolve_id(&self, id: &str) -> Result<Uuid> {
        // Check temp ID map first
        if let Some(uuid) = self.temp_id_map.get(id) {
            return Ok(*uuid);
        }

        // Try to parse as UUID
        Uuid::parse_str(id).map_err(|_| anyhow!("Invalid UUID: {}", id))
    }
}

impl Default for CommandExecutor {
    fn default() -> Self {
        Self::new(String::new())
    }
}

/// Convert FfiTodoState to TodoState.
fn convert_ffi_state(state: FfiTodoState) -> TodoState {
    match state {
        FfiTodoState::Empty => TodoState::Empty,
        FfiTodoState::Checked => TodoState::Checked,
        FfiTodoState::Question => TodoState::Question,
        FfiTodoState::Exclamation => TodoState::Exclamation,
        FfiTodoState::InProgress => TodoState::InProgress,
        FfiTodoState::Cancelled => TodoState::Cancelled,
    }
}

/// Convert FfiPriority to Priority.
fn convert_ffi_priority(priority: FfiPriority) -> Priority {
    match priority {
        FfiPriority::P0 => Priority::P0,
        FfiPriority::P1 => Priority::P1,
        FfiPriority::P2 => Priority::P2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::std_types::ROption;
    use chrono::Local;
    use std::path::PathBuf;
    use totui_plugin_interface::FfiTodoState;

    fn create_test_list() -> TodoList {
        let date = Local::now().date_naive();
        TodoList {
            date,
            items: vec![TodoItem::new("Existing".to_string(), 0)],
            file_path: PathBuf::from("/tmp/test.md"),
        }
    }

    #[test]
    fn test_execute_create_todo() {
        let mut list = create_test_list();
        let mut executor = CommandExecutor::default();

        let commands = vec![FfiCommand::CreateTodo {
            content: "New task".into(),
            parent_id: ROption::RNone,
            temp_id: ROption::RNone,
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            indent_level: 0,
        }];

        let created = executor.execute_batch(commands, &mut list).unwrap();

        assert_eq!(created.len(), 1);
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[1].content, "New task");
    }

    #[test]
    fn test_execute_update_todo() {
        let mut list = create_test_list();
        let id = list.items[0].id.to_string();
        let mut executor = CommandExecutor::default();

        let commands = vec![FfiCommand::UpdateTodo {
            id: id.into(),
            content: ROption::RSome("Updated".into()),
            state: ROption::RNone,
            priority: ROption::RNone,
            due_date: ROption::RNone,
            description: ROption::RNone,
        }];

        executor.execute_batch(commands, &mut list).unwrap();

        assert_eq!(list.items[0].content, "Updated");
    }

    #[test]
    fn test_execute_delete_todo() {
        let mut list = create_test_list();
        let id = list.items[0].id.to_string();
        let mut executor = CommandExecutor::default();

        let commands = vec![FfiCommand::DeleteTodo { id: id.into() }];

        executor.execute_batch(commands, &mut list).unwrap();

        assert!(list.items[0].deleted_at.is_some());
    }

    #[test]
    fn test_temp_id_mapping() {
        let mut list = create_test_list();
        let mut executor = CommandExecutor::default();

        // Create parent with temp_id, then child referencing it
        let commands = vec![
            FfiCommand::CreateTodo {
                content: "Parent".into(),
                parent_id: ROption::RNone,
                temp_id: ROption::RSome("temp-1".into()),
                state: FfiTodoState::Empty,
                priority: ROption::RNone,
                indent_level: 0,
            },
            FfiCommand::CreateTodo {
                content: "Child".into(),
                parent_id: ROption::RSome("temp-1".into()),
                temp_id: ROption::RNone,
                state: FfiTodoState::Empty,
                priority: ROption::RNone,
                indent_level: 1,
            },
        ];

        let created = executor.execute_batch(commands, &mut list).unwrap();

        assert_eq!(created.len(), 2);
        // Child should reference parent
        let child = list.items.iter().find(|i| i.content == "Child").unwrap();
        assert_eq!(child.parent_id, Some(created[0]));
    }

    #[test]
    fn test_update_not_found_returns_error() {
        let mut list = create_test_list();
        let mut executor = CommandExecutor::default();

        let commands = vec![FfiCommand::UpdateTodo {
            id: "00000000-0000-0000-0000-000000000000".into(),
            content: ROption::RSome("Updated".into()),
            state: ROption::RNone,
            priority: ROption::RNone,
            due_date: ROption::RNone,
            description: ROption::RNone,
        }];

        let result = executor.execute_batch(commands, &mut list);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_execute_move_todo_before() {
        let mut list = create_test_list();
        list.items.push(TodoItem::new("Second".to_string(), 0));
        list.items.push(TodoItem::new("Third".to_string(), 0));
        let mut executor = CommandExecutor::default();

        let third_id = list.items[2].id.to_string();
        let first_id = list.items[0].id.to_string();

        // Move third item before first
        let commands = vec![FfiCommand::MoveTodo {
            id: third_id.into(),
            position: FfiMovePosition::Before {
                target_id: first_id.into(),
            },
        }];

        executor.execute_batch(commands, &mut list).unwrap();

        assert_eq!(list.items[0].content, "Third");
        assert_eq!(list.items[1].content, "Existing");
        assert_eq!(list.items[2].content, "Second");
    }

    #[test]
    fn test_execute_move_todo_after() {
        let mut list = create_test_list();
        list.items.push(TodoItem::new("Second".to_string(), 0));
        list.items.push(TodoItem::new("Third".to_string(), 0));
        let mut executor = CommandExecutor::default();

        let first_id = list.items[0].id.to_string();
        let third_id = list.items[2].id.to_string();

        // Move first item after third
        let commands = vec![FfiCommand::MoveTodo {
            id: first_id.into(),
            position: FfiMovePosition::After {
                target_id: third_id.into(),
            },
        }];

        executor.execute_batch(commands, &mut list).unwrap();

        assert_eq!(list.items[0].content, "Second");
        assert_eq!(list.items[1].content, "Third");
        assert_eq!(list.items[2].content, "Existing");
    }

    #[test]
    fn test_execute_move_todo_at_index() {
        let mut list = create_test_list();
        list.items.push(TodoItem::new("Second".to_string(), 0));
        list.items.push(TodoItem::new("Third".to_string(), 0));
        let mut executor = CommandExecutor::default();

        let third_id = list.items[2].id.to_string();

        // Move third item to index 1
        let commands = vec![FfiCommand::MoveTodo {
            id: third_id.into(),
            position: FfiMovePosition::AtIndex { index: 1 },
        }];

        executor.execute_batch(commands, &mut list).unwrap();

        assert_eq!(list.items[0].content, "Existing");
        assert_eq!(list.items[1].content, "Third");
        assert_eq!(list.items[2].content, "Second");
    }

    #[test]
    fn test_delete_not_found_returns_error() {
        let mut list = create_test_list();
        let mut executor = CommandExecutor::default();

        let commands = vec![FfiCommand::DeleteTodo {
            id: "00000000-0000-0000-0000-000000000000".into(),
        }];

        let result = executor.execute_batch(commands, &mut list);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_create_with_state_and_priority() {
        let mut list = create_test_list();
        let mut executor = CommandExecutor::default();

        let commands = vec![FfiCommand::CreateTodo {
            content: "Important task".into(),
            parent_id: ROption::RNone,
            temp_id: ROption::RNone,
            state: FfiTodoState::Exclamation,
            priority: ROption::RSome(FfiPriority::P0),
            indent_level: 0,
        }];

        executor.execute_batch(commands, &mut list).unwrap();

        let item = &list.items[1];
        assert_eq!(item.content, "Important task");
        assert_eq!(item.state, TodoState::Exclamation);
        assert_eq!(item.priority, Some(Priority::P0));
    }

    #[test]
    fn test_update_state_and_due_date() {
        let mut list = create_test_list();
        let id = list.items[0].id.to_string();
        let mut executor = CommandExecutor::default();

        let commands = vec![FfiCommand::UpdateTodo {
            id: id.into(),
            content: ROption::RNone,
            state: ROption::RSome(FfiTodoState::Checked),
            priority: ROption::RNone,
            due_date: ROption::RSome("2025-12-31".into()),
            description: ROption::RSome("A description".into()),
        }];

        executor.execute_batch(commands, &mut list).unwrap();

        let item = &list.items[0];
        assert_eq!(item.state, TodoState::Checked);
        assert_eq!(
            item.due_date,
            Some(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap())
        );
        assert_eq!(item.description, Some("A description".to_string()));
    }

    // ========================================================================
    // Metadata Command Integration Tests
    // ========================================================================

    mod metadata_tests {
        use super::*;
        use crate::storage::database::init_database;
        use std::env;
        use tempfile::TempDir;

        fn setup_test_env() -> TempDir {
            let temp_dir = TempDir::new().unwrap();
            let to_tui_dir = temp_dir.path().join(".to-tui");
            std::fs::create_dir_all(&to_tui_dir).unwrap();
            // SAFETY: Tests run single-threaded
            unsafe {
                env::set_var("HOME", temp_dir.path());
            }
            init_database().unwrap();
            temp_dir
        }

        #[test]
        fn test_set_todo_metadata_command() {
            let _temp = setup_test_env();
            let mut list = create_test_list();
            let todo_id = list.items[0].id.to_string();
            let plugin_name = "test-plugin".to_string();
            let mut executor = CommandExecutor::new(plugin_name.clone());

            let commands = vec![FfiCommand::SetTodoMetadata {
                todo_id: todo_id.clone().into(),
                data: r#"{"key": "value"}"#.into(),
                merge: false,
            }];

            executor.execute_batch(commands, &mut list).unwrap();

            // Verify metadata was stored
            let uuid = Uuid::parse_str(&todo_id).unwrap();
            let stored = metadata::get_todo_metadata(&uuid, &plugin_name).unwrap();
            assert_eq!(stored, r#"{"key": "value"}"#);
        }

        #[test]
        fn test_set_todo_metadata_merge() {
            let _temp = setup_test_env();
            let mut list = create_test_list();
            let todo_id = list.items[0].id.to_string();
            let plugin_name = "test-plugin".to_string();
            let mut executor = CommandExecutor::new(plugin_name.clone());

            // Set initial metadata
            let commands = vec![FfiCommand::SetTodoMetadata {
                todo_id: todo_id.clone().into(),
                data: r#"{"a": 1, "b": 2}"#.into(),
                merge: false,
            }];
            executor.execute_batch(commands, &mut list).unwrap();

            // Merge with new data
            let commands = vec![FfiCommand::SetTodoMetadata {
                todo_id: todo_id.clone().into(),
                data: r#"{"b": 3, "c": 4}"#.into(),
                merge: true,
            }];
            executor.execute_batch(commands, &mut list).unwrap();

            // Verify merge
            let uuid = Uuid::parse_str(&todo_id).unwrap();
            let stored = metadata::get_todo_metadata(&uuid, &plugin_name).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&stored).unwrap();
            assert_eq!(parsed["a"], 1);
            assert_eq!(parsed["b"], 3); // Overwritten
            assert_eq!(parsed["c"], 4); // Added
        }

        #[test]
        fn test_set_project_metadata_command() {
            let _temp = setup_test_env();
            let mut list = create_test_list();
            let plugin_name = "test-plugin".to_string();
            let mut executor = CommandExecutor::new(plugin_name.clone());

            let commands = vec![FfiCommand::SetProjectMetadata {
                project_name: "my-project".into(),
                data: r#"{"project_key": "value"}"#.into(),
                merge: false,
            }];

            executor.execute_batch(commands, &mut list).unwrap();

            // Verify metadata was stored
            let stored = metadata::get_project_metadata("my-project", &plugin_name).unwrap();
            assert_eq!(stored, r#"{"project_key": "value"}"#);
        }

        #[test]
        fn test_delete_todo_metadata_command() {
            let _temp = setup_test_env();
            let mut list = create_test_list();
            let todo_id = list.items[0].id.to_string();
            let plugin_name = "test-plugin".to_string();
            let mut executor = CommandExecutor::new(plugin_name.clone());

            // First set metadata
            let uuid = Uuid::parse_str(&todo_id).unwrap();
            metadata::set_todo_metadata(&uuid, &plugin_name, r#"{"key": "value"}"#, false).unwrap();

            // Then delete via command
            let commands = vec![FfiCommand::DeleteTodoMetadata {
                todo_id: todo_id.into(),
            }];

            executor.execute_batch(commands, &mut list).unwrap();

            // Verify deleted
            let stored = metadata::get_todo_metadata(&uuid, &plugin_name).unwrap();
            assert_eq!(stored, "{}");
        }

        #[test]
        fn test_delete_project_metadata_command() {
            let _temp = setup_test_env();
            let mut list = create_test_list();
            let plugin_name = "test-plugin".to_string();
            let mut executor = CommandExecutor::new(plugin_name.clone());

            // First set metadata
            metadata::set_project_metadata("my-project", &plugin_name, r#"{"key": "value"}"#, false)
                .unwrap();

            // Then delete via command
            let commands = vec![FfiCommand::DeleteProjectMetadata {
                project_name: "my-project".into(),
            }];

            executor.execute_batch(commands, &mut list).unwrap();

            // Verify deleted
            let stored = metadata::get_project_metadata("my-project", &plugin_name).unwrap();
            assert_eq!(stored, "{}");
        }

        #[test]
        fn test_metadata_namespace_isolation() {
            let _temp = setup_test_env();
            let mut list = create_test_list();
            let todo_id = list.items[0].id.to_string();

            // Set metadata from plugin A
            let mut executor_a = CommandExecutor::new("plugin-a".to_string());
            let commands = vec![FfiCommand::SetTodoMetadata {
                todo_id: todo_id.clone().into(),
                data: r#"{"from": "a"}"#.into(),
                merge: false,
            }];
            executor_a.execute_batch(commands, &mut list).unwrap();

            // Set metadata from plugin B
            let mut executor_b = CommandExecutor::new("plugin-b".to_string());
            let commands = vec![FfiCommand::SetTodoMetadata {
                todo_id: todo_id.clone().into(),
                data: r#"{"from": "b"}"#.into(),
                merge: false,
            }];
            executor_b.execute_batch(commands, &mut list).unwrap();

            // Verify isolation
            let uuid = Uuid::parse_str(&todo_id).unwrap();
            let stored_a = metadata::get_todo_metadata(&uuid, "plugin-a").unwrap();
            let stored_b = metadata::get_todo_metadata(&uuid, "plugin-b").unwrap();
            assert_eq!(stored_a, r#"{"from": "a"}"#);
            assert_eq!(stored_b, r#"{"from": "b"}"#);
        }

        #[test]
        fn test_metadata_with_temp_id() {
            let _temp = setup_test_env();
            let mut list = create_test_list();
            let plugin_name = "test-plugin".to_string();
            let mut executor = CommandExecutor::new(plugin_name.clone());

            // Create todo with temp_id, then set metadata using temp_id
            let commands = vec![
                FfiCommand::CreateTodo {
                    content: "New task".into(),
                    parent_id: ROption::RNone,
                    temp_id: ROption::RSome("temp-1".into()),
                    state: FfiTodoState::Empty,
                    priority: ROption::RNone,
                    indent_level: 0,
                },
                FfiCommand::SetTodoMetadata {
                    todo_id: "temp-1".into(),
                    data: r#"{"created_via": "temp_id"}"#.into(),
                    merge: false,
                },
            ];

            let created = executor.execute_batch(commands, &mut list).unwrap();

            // Verify metadata was set on the created todo
            let stored = metadata::get_todo_metadata(&created[0], &plugin_name).unwrap();
            assert_eq!(stored, r#"{"created_via": "temp_id"}"#);
        }

        #[test]
        fn test_invalid_json_rejected() {
            let _temp = setup_test_env();
            let mut list = create_test_list();
            let todo_id = list.items[0].id.to_string();
            let plugin_name = "test-plugin".to_string();
            let mut executor = CommandExecutor::new(plugin_name);

            let commands = vec![FfiCommand::SetTodoMetadata {
                todo_id: todo_id.into(),
                data: "not valid json".into(),
                merge: false,
            }];

            let result = executor.execute_batch(commands, &mut list);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Invalid JSON"));
        }

        #[test]
        fn test_reserved_key_rejected() {
            let _temp = setup_test_env();
            let mut list = create_test_list();
            let todo_id = list.items[0].id.to_string();
            let plugin_name = "test-plugin".to_string();
            let mut executor = CommandExecutor::new(plugin_name);

            let commands = vec![FfiCommand::SetTodoMetadata {
                todo_id: todo_id.into(),
                data: r#"{"_reserved": "value"}"#.into(),
                merge: false,
            }];

            let result = executor.execute_batch(commands, &mut list);
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Keys starting with '_' are reserved"));
        }
    }
}
