//! Host API implementation for plugins.
//!
//! This module provides the `PluginHostApiImpl` struct that implements the
//! `HostApi` trait, giving plugins query access to the todo list and projects.

use abi_stable::std_types::{ROption, RString, RVec};
use std::collections::HashSet;
use totui_plugin_interface::{
    FfiProjectContext, FfiStateFilter, FfiTodoItem, FfiTodoMetadata, FfiTodoNode, FfiTodoQuery,
    HostApi,
};
use uuid::Uuid;

use crate::storage::metadata;

use crate::project::Project;
use crate::todo::{TodoList, TodoState};

/// Host API implementation that provides query access to plugins.
///
/// This struct holds immutable references to the current state,
/// allowing plugins to query todos and projects safely.
pub struct PluginHostApiImpl<'a> {
    /// The current todo list
    pub todo_list: &'a TodoList,
    /// The current project
    pub current_project: &'a Project,
    /// All projects where this plugin is enabled
    pub enabled_projects: HashSet<String>,
    /// Name of the plugin using this API (for access checks)
    pub plugin_name: String,
}

impl<'a> PluginHostApiImpl<'a> {
    /// Create a new PluginHostApiImpl.
    ///
    /// # Arguments
    /// * `todo_list` - Reference to the current todo list
    /// * `current_project` - Reference to the current project
    /// * `enabled_projects` - Set of project names where this plugin is enabled
    /// * `plugin_name` - Name of the plugin using this API
    pub fn new(
        todo_list: &'a TodoList,
        current_project: &'a Project,
        enabled_projects: HashSet<String>,
        plugin_name: String,
    ) -> Self {
        Self {
            todo_list,
            current_project,
            enabled_projects,
            plugin_name,
        }
    }

    /// Check if a project name is accessible by this plugin.
    fn can_access_project(&self, project_name: &str) -> bool {
        self.enabled_projects.contains(project_name)
    }

    /// Build a tree of FfiTodoNode from the flat todo list.
    fn build_tree(&self) -> RVec<FfiTodoNode> {
        let items = &self.todo_list.items;
        if items.is_empty() {
            return RVec::new();
        }

        // Build tree by iterating and collecting children under parents
        let mut result = RVec::new();
        let mut i = 0;

        while i < items.len() {
            let item = &items[i];
            // Only process root items (indent_level == 0) at top level
            if item.indent_level == 0 && item.deleted_at.is_none() {
                let (node, next_idx) = self.build_node(i);
                result.push(node);
                i = next_idx;
            } else {
                i += 1;
            }
        }

        result
    }

    /// Build a single FfiTodoNode and its children recursively.
    /// Returns the node and the index of the next item to process.
    fn build_node(&self, start_idx: usize) -> (FfiTodoNode, usize) {
        let items = &self.todo_list.items;
        let item = &items[start_idx];
        let base_indent = item.indent_level;

        let mut ffi_item: FfiTodoItem = item.into();
        ffi_item.position = start_idx as u32;

        let mut children = RVec::new();
        let mut i = start_idx + 1;

        // Collect all children (items with indent > base_indent)
        while i < items.len() {
            let child = &items[i];
            // Stop when we hit an item at same or lower indent level
            if child.indent_level <= base_indent {
                break;
            }
            // Only process direct children (indent_level == base_indent + 1)
            if child.indent_level == base_indent + 1 && child.deleted_at.is_none() {
                let (child_node, next_idx) = self.build_node(i);
                children.push(child_node);
                i = next_idx;
            } else {
                i += 1;
            }
        }

        (
            FfiTodoNode {
                item: ffi_item,
                children,
                position: start_idx as u32,
            },
            i,
        )
    }
}

impl HostApi for PluginHostApiImpl<'_> {
    fn current_project(&self) -> FfiProjectContext {
        self.current_project.into()
    }

    fn list_projects(&self) -> RVec<FfiProjectContext> {
        // For now, return only current project since full project list
        // requires loading from DB. Future: Pass project registry reference.
        let mut projects = RVec::new();
        if self.can_access_project(&self.current_project.name) {
            projects.push(self.current_project.into());
        }
        projects
    }

    fn query_todos(&self, query: FfiTodoQuery) -> RVec<FfiTodoItem> {
        // Check project access
        if let ROption::RSome(ref project_name) = query.project {
            let name = project_name.to_string();
            if !self.can_access_project(&name) {
                return RVec::new();
            }
            // If querying a different project, we can't access it from current list
            if name != self.current_project.name {
                return RVec::new();
            }
        }

        let items = &self.todo_list.items;
        let mut result = RVec::new();

        for (idx, item) in items.iter().enumerate() {
            // Filter deleted items unless include_deleted is true
            if !query.include_deleted && item.deleted_at.is_some() {
                continue;
            }

            // Apply state filter
            if let ROption::RSome(ref state_filter) = query.state_filter {
                match state_filter {
                    FfiStateFilter::Done => {
                        if item.state != TodoState::Checked {
                            continue;
                        }
                    }
                    FfiStateFilter::Pending => {
                        if item.state == TodoState::Checked {
                            continue;
                        }
                    }
                    FfiStateFilter::All => {
                        // No filtering
                    }
                }
            }

            // Filter by parent_id
            if let ROption::RSome(ref parent_id_str) = query.parent_id {
                if let Ok(parent_uuid) = Uuid::parse_str(parent_id_str) {
                    match item.parent_id {
                        Some(pid) if pid == parent_uuid => {}
                        _ => continue,
                    }
                } else {
                    continue;
                }
            }

            // Filter by date range (using created_at)
            if let ROption::RSome(ref date_from_str) = query.date_from
                && let Ok(date_from) =
                    chrono::NaiveDate::parse_from_str(date_from_str, "%Y-%m-%d")
                && item.created_at.date_naive() < date_from
            {
                continue;
            }

            if let ROption::RSome(ref date_to_str) = query.date_to
                && let Ok(date_to) = chrono::NaiveDate::parse_from_str(date_to_str, "%Y-%m-%d")
                && item.created_at.date_naive() > date_to
            {
                continue;
            }

            // Item passed all filters, add to result with position
            let mut ffi_item: FfiTodoItem = item.into();
            ffi_item.position = idx as u32;
            result.push(ffi_item);
        }

        result
    }

    fn get_todo(&self, id: RString) -> ROption<FfiTodoItem> {
        let Ok(uuid) = Uuid::parse_str(&id) else {
            return ROption::RNone;
        };

        for (idx, item) in self.todo_list.items.iter().enumerate() {
            if item.id == uuid && item.deleted_at.is_none() {
                let mut ffi_item: FfiTodoItem = item.into();
                ffi_item.position = idx as u32;
                return ROption::RSome(ffi_item);
            }
        }

        ROption::RNone
    }

    fn query_todos_tree(&self) -> RVec<FfiTodoNode> {
        self.build_tree()
    }

    fn get_todo_metadata(&self, todo_id: RString) -> RString {
        let Ok(uuid) = Uuid::parse_str(&todo_id) else {
            return "{}".into();
        };

        match metadata::get_todo_metadata(&uuid, &self.plugin_name) {
            Ok(data) => data.into(),
            Err(_) => "{}".into(),
        }
    }

    fn get_todo_metadata_batch(&self, todo_ids: RVec<RString>) -> RVec<FfiTodoMetadata> {
        let mut results = RVec::new();
        for todo_id in todo_ids.iter() {
            let data = self.get_todo_metadata(todo_id.clone());
            results.push(FfiTodoMetadata {
                todo_id: todo_id.clone(),
                data,
            });
        }
        results
    }

    fn get_project_metadata(&self, project_name: RString) -> RString {
        // Check project access
        if !self.can_access_project(&project_name) {
            return "{}".into();
        }

        match metadata::get_project_metadata(&project_name, &self.plugin_name) {
            Ok(data) => data.into(),
            Err(_) => "{}".into(),
        }
    }

    fn query_todos_by_metadata(&self, key: RString, value: RString) -> RVec<FfiTodoItem> {
        // Search through all todos and filter by metadata key/value match
        // This is a simple implementation - could be optimized with DB query later
        let mut results = RVec::new();

        for (idx, item) in self.todo_list.items.iter().enumerate() {
            if item.deleted_at.is_some() {
                continue;
            }

            // Get metadata for this todo
            let metadata_str = match metadata::get_todo_metadata(&item.id, &self.plugin_name) {
                Ok(data) => data,
                Err(_) => continue,
            };

            // Parse and check for key/value match
            if let Ok(metadata_json) = serde_json::from_str::<serde_json::Value>(&metadata_str)
                && let Some(found_value) = metadata_json.get(key.as_str())
                && let Ok(search_value) = serde_json::from_str::<serde_json::Value>(&value)
                && found_value == &search_value
            {
                let mut ffi_item: FfiTodoItem = item.into();
                ffi_item.position = idx as u32;
                results.push(ffi_item);
            }
        }

        results
    }

    fn list_projects_with_metadata(&self) -> RVec<RString> {
        // Query database for projects with metadata for this plugin
        // For now, return projects from enabled_projects that have metadata
        let mut results = RVec::new();

        for project_name in &self.enabled_projects {
            let metadata_str =
                match metadata::get_project_metadata(project_name, &self.plugin_name) {
                    Ok(data) => data,
                    Err(_) => continue,
                };

            // Only include if metadata is not empty
            if metadata_str != "{}" {
                results.push(project_name.clone().into());
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todo::{TodoItem, TodoList};
    use chrono::Local;
    use std::path::PathBuf;

    fn create_test_list() -> TodoList {
        let date = Local::now().date_naive();
        let mut list = TodoList {
            date,
            items: vec![],
            file_path: PathBuf::from("/tmp/test.md"),
        };

        // Add root item
        let root = TodoItem::new("Root".to_string(), 0);
        let root_id = root.id;
        list.items.push(root);

        // Add child
        let mut child = TodoItem::new("Child".to_string(), 1);
        child.parent_id = Some(root_id);
        list.items.push(child);

        list
    }

    #[test]
    fn test_current_project() {
        let list = create_test_list();
        let project = Project::default_project();
        let api = PluginHostApiImpl::new(
            &list,
            &project,
            HashSet::from(["default".to_string()]),
            "test-plugin".to_string(),
        );

        let ctx = api.current_project();
        assert_eq!(ctx.name.to_string(), "default");
    }

    #[test]
    fn test_query_todos_returns_items_with_position() {
        let list = create_test_list();
        let project = Project::default_project();
        let api = PluginHostApiImpl::new(
            &list,
            &project,
            HashSet::from(["default".to_string()]),
            "test-plugin".to_string(),
        );

        let query = FfiTodoQuery::default();
        let results = api.query_todos(query);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].position, 0);
        assert_eq!(results[1].position, 1);
    }

    #[test]
    fn test_query_todos_tree_nests_children() {
        let list = create_test_list();
        let project = Project::default_project();
        let api = PluginHostApiImpl::new(
            &list,
            &project,
            HashSet::from(["default".to_string()]),
            "test-plugin".to_string(),
        );

        let tree = api.query_todos_tree();

        // Should have 1 root node
        assert_eq!(tree.len(), 1);
        // Root should have 1 child
        assert_eq!(tree[0].children.len(), 1);
    }

    #[test]
    fn test_get_todo_found() {
        let list = create_test_list();
        let project = Project::default_project();
        let first_id = list.items[0].id.to_string();
        let api = PluginHostApiImpl::new(
            &list,
            &project,
            HashSet::from(["default".to_string()]),
            "test-plugin".to_string(),
        );

        let result = api.get_todo(first_id.into());
        assert!(result.is_some());
    }

    #[test]
    fn test_get_todo_not_found() {
        let list = create_test_list();
        let project = Project::default_project();
        let api = PluginHostApiImpl::new(
            &list,
            &project,
            HashSet::from(["default".to_string()]),
            "test-plugin".to_string(),
        );

        let result = api.get_todo("00000000-0000-0000-0000-000000000000".into());
        assert!(result.is_none());
    }

    #[test]
    fn test_query_todos_filters_by_state_done() {
        let mut list = create_test_list();
        list.items[0].state = TodoState::Checked;
        let project = Project::default_project();
        let api = PluginHostApiImpl::new(
            &list,
            &project,
            HashSet::from(["default".to_string()]),
            "test-plugin".to_string(),
        );

        let query = FfiTodoQuery {
            state_filter: ROption::RSome(FfiStateFilter::Done),
            ..FfiTodoQuery::default()
        };
        let results = api.query_todos(query);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content.to_string(), "Root");
    }

    #[test]
    fn test_query_todos_filters_by_state_pending() {
        let mut list = create_test_list();
        list.items[0].state = TodoState::Checked;
        let project = Project::default_project();
        let api = PluginHostApiImpl::new(
            &list,
            &project,
            HashSet::from(["default".to_string()]),
            "test-plugin".to_string(),
        );

        let query = FfiTodoQuery {
            state_filter: ROption::RSome(FfiStateFilter::Pending),
            ..FfiTodoQuery::default()
        };
        let results = api.query_todos(query);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content.to_string(), "Child");
    }

    #[test]
    fn test_query_todos_filters_deleted() {
        let mut list = create_test_list();
        list.items[0].deleted_at = Some(chrono::Utc::now());
        let project = Project::default_project();
        let api = PluginHostApiImpl::new(
            &list,
            &project,
            HashSet::from(["default".to_string()]),
            "test-plugin".to_string(),
        );

        // Default excludes deleted
        let query = FfiTodoQuery::default();
        let results = api.query_todos(query);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content.to_string(), "Child");

        // With include_deleted
        let query = FfiTodoQuery {
            include_deleted: true,
            ..FfiTodoQuery::default()
        };
        let results = api.query_todos(query);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_list_projects_returns_current_if_enabled() {
        let list = create_test_list();
        let project = Project::default_project();
        let api = PluginHostApiImpl::new(
            &list,
            &project,
            HashSet::from(["default".to_string()]),
            "test-plugin".to_string(),
        );

        let projects = api.list_projects();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name.to_string(), "default");
    }

    #[test]
    fn test_list_projects_empty_if_not_enabled() {
        let list = create_test_list();
        let project = Project::default_project();
        let api = PluginHostApiImpl::new(
            &list,
            &project,
            HashSet::from(["other-project".to_string()]),
            "test-plugin".to_string(),
        );

        let projects = api.list_projects();
        assert_eq!(projects.len(), 0);
    }

    #[test]
    fn test_query_todos_returns_empty_for_inaccessible_project() {
        let list = create_test_list();
        let project = Project::default_project();
        let api = PluginHostApiImpl::new(
            &list,
            &project,
            HashSet::from(["default".to_string()]),
            "test-plugin".to_string(),
        );

        let query = FfiTodoQuery {
            project: ROption::RSome("other-project".into()),
            ..FfiTodoQuery::default()
        };
        let results = api.query_todos(query);
        assert!(results.is_empty());
    }
}
