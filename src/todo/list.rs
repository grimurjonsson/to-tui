use super::priority::Priority;
use super::TodoItem;
use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TodoList {
    pub date: NaiveDate,
    pub items: Vec<TodoItem>,
    pub file_path: PathBuf,
}

impl TodoList {
    pub fn new(date: NaiveDate, file_path: PathBuf) -> Self {
        Self {
            date,
            items: Vec::new(),
            file_path,
        }
    }

    pub fn with_items(date: NaiveDate, file_path: PathBuf, items: Vec<TodoItem>) -> Self {
        Self {
            date,
            items,
            file_path,
        }
    }

    pub fn add_item(&mut self, content: String) {
        self.items.push(TodoItem::new(content, 0));
    }

    pub fn add_item_with_indent(&mut self, content: String, indent_level: usize) {
        self.items.push(TodoItem::new(content, indent_level));
    }

    pub fn get_incomplete_items(&self) -> Vec<TodoItem> {
        if self.items.is_empty() {
            return Vec::new();
        }

        let id_to_item: std::collections::HashMap<Uuid, &TodoItem> =
            self.items.iter().map(|item| (item.id, item)).collect();

        let mut include_ids: HashSet<Uuid> = HashSet::new();

        for item in &self.items {
            if !item.is_complete() {
                include_ids.insert(item.id);
                self.collect_ancestor_ids(item, &id_to_item, &mut include_ids);
            }
        }

        self.items
            .iter()
            .filter(|item| include_ids.contains(&item.id))
            .cloned()
            .collect()
    }

    fn collect_ancestor_ids(
        &self,
        item: &TodoItem,
        id_to_item: &std::collections::HashMap<Uuid, &TodoItem>,
        include_ids: &mut HashSet<Uuid>,
    ) {
        let mut current_parent_id = item.parent_id;
        while let Some(parent_id) = current_parent_id {
            if let Some(parent) = id_to_item.get(&parent_id) {
                include_ids.insert(parent.id);
                current_parent_id = parent.parent_id;
            } else {
                break;
            }
        }
    }

    #[cfg(test)]
    pub fn toggle_item_state(&mut self, index: usize) -> Result<()> {
        if index >= self.items.len() {
            return Err(anyhow!("Index out of bounds"));
        }
        self.items[index].toggle_state();
        Ok(())
    }

    #[cfg(test)]
    pub fn remove_item(&mut self, index: usize) -> Result<TodoItem> {
        if index >= self.items.len() {
            return Err(anyhow!("Index out of bounds"));
        }
        Ok(self.items.remove(index))
    }

    /// Returns the set of indices that should be hidden due to collapsed parents
    pub fn build_hidden_indices(&self) -> HashSet<usize> {
        let mut hidden = HashSet::new();
        let mut i = 0;
        while i < self.items.len() {
            if self.items[i].collapsed {
                let base_indent = self.items[i].indent_level;
                let mut j = i + 1;
                while j < self.items.len() && self.items[j].indent_level > base_indent {
                    hidden.insert(j);
                    j += 1;
                }
                i = j;
            } else {
                i += 1;
            }
        }
        hidden
    }

    pub fn remove_item_range(&mut self, start: usize, end: usize) -> Result<Vec<TodoItem>> {
        if start >= self.items.len() || end > self.items.len() || start >= end {
            return Err(anyhow!("Invalid range"));
        }
        Ok(self.items.drain(start..end).collect())
    }

    pub fn insert_item(
        &mut self,
        index: usize,
        content: String,
        indent_level: usize,
    ) -> Result<()> {
        if index > self.items.len() {
            return Err(anyhow!("Index out of bounds"));
        }
        self.items
            .insert(index, TodoItem::new(content, indent_level));
        Ok(())
    }

    /// Sort todos by priority at every level, keeping children grouped with their parents.
    /// Sort order: P0 (highest) -> P1 -> P2 -> None (lowest)
    /// Within same priority, maintains original relative order (stable sort).
    pub fn sort_by_priority(&mut self) {
        if self.items.is_empty() {
            return;
        }

        // Helper function to get sort key for priority
        fn priority_sort_key(priority: Option<Priority>) -> u8 {
            match priority {
                Some(Priority::P0) => 0,
                Some(Priority::P1) => 1,
                Some(Priority::P2) => 2,
                None => 3,
            }
        }

        // Recursively sort items at a given indent level
        // Returns sorted items with their subtrees
        fn sort_at_level(items: &[TodoItem], target_level: usize) -> Vec<TodoItem> {
            if items.is_empty() {
                return Vec::new();
            }

            // Group items at target_level with their children
            let mut groups: Vec<(u8, Vec<TodoItem>)> = Vec::new();
            let mut i = 0;

            while i < items.len() {
                let item = &items[i];
                if item.indent_level == target_level {
                    // Find end of this item's subtree (all items with indent > target_level)
                    let mut end = i + 1;
                    while end < items.len() && items[end].indent_level > target_level {
                        end += 1;
                    }

                    // Get subtree and recursively sort children
                    let mut subtree = vec![item.clone()];
                    if end > i + 1 {
                        // Has children - recursively sort them
                        let children = &items[i + 1..end];
                        subtree.extend(sort_at_level(children, target_level + 1));
                    }

                    let sort_key = priority_sort_key(item.priority);
                    groups.push((sort_key, subtree));
                    i = end;
                } else {
                    // Item at different level - shouldn't happen at top call, handle gracefully
                    let sort_key = priority_sort_key(item.priority);
                    groups.push((sort_key, vec![item.clone()]));
                    i += 1;
                }
            }

            // Stable sort groups by priority key
            groups.sort_by_key(|(key, _)| *key);

            // Flatten back to vec
            groups.into_iter().flat_map(|(_, items)| items).collect()
        }

        // Sort starting from root level (0)
        self.items = sort_at_level(&self.items, 0);

        // Recalculate parent IDs after reordering
        self.recalculate_parent_ids();
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todo::TodoState;
    use chrono::{Datelike, NaiveDate};

    fn create_test_list() -> TodoList {
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        let path = PathBuf::from("/tmp/test.md");
        TodoList::new(date, path)
    }

    #[test]
    fn test_new() {
        let list = create_test_list();
        assert!(list.items.is_empty());
        assert_eq!(list.date.year(), 2025);
    }

    #[test]
    fn test_add_item() {
        let mut list = create_test_list();
        list.add_item("Task 1".to_string());
        list.add_item("Task 2".to_string());

        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].content, "Task 1");
        assert_eq!(list.items[1].content, "Task 2");
    }

    #[test]
    fn test_add_item_with_indent() {
        let mut list = create_test_list();
        list.add_item_with_indent("Parent".to_string(), 0);
        list.add_item_with_indent("Child".to_string(), 1);

        assert_eq!(list.items[0].indent_level, 0);
        assert_eq!(list.items[1].indent_level, 1);
    }

    #[test]
    fn test_get_incomplete_items() {
        let mut list = create_test_list();
        list.add_item("Task 1".to_string());
        list.add_item("Task 2".to_string());
        list.add_item("Task 3".to_string());

        list.items[1].state = TodoState::Checked;

        let incomplete = list.get_incomplete_items();
        assert_eq!(incomplete.len(), 2);
        assert_eq!(incomplete[0].content, "Task 1");
        assert_eq!(incomplete[1].content, "Task 3");
    }

    #[test]
    fn test_get_incomplete_items_includes_complete_parent_with_incomplete_child() {
        let mut list = create_test_list();
        list.add_item("Parent".to_string());
        list.add_item("Child".to_string());

        let parent_id = list.items[0].id;
        list.items[0].state = TodoState::Checked;
        list.items[1].parent_id = Some(parent_id);
        list.items[1].indent_level = 1;

        let incomplete = list.get_incomplete_items();
        assert_eq!(incomplete.len(), 2);
        assert_eq!(incomplete[0].content, "Parent");
        assert_eq!(incomplete[1].content, "Child");
    }

    #[test]
    fn test_get_incomplete_items_includes_complete_ancestors() {
        let mut list = create_test_list();
        list.add_item("Grandparent".to_string());
        list.add_item("Parent".to_string());
        list.add_item("Child".to_string());

        let grandparent_id = list.items[0].id;
        let parent_id = list.items[1].id;

        list.items[0].state = TodoState::Checked;
        list.items[1].state = TodoState::Checked;
        list.items[1].parent_id = Some(grandparent_id);
        list.items[1].indent_level = 1;
        list.items[2].parent_id = Some(parent_id);
        list.items[2].indent_level = 2;

        let incomplete = list.get_incomplete_items();
        assert_eq!(incomplete.len(), 3);
        assert_eq!(incomplete[0].content, "Grandparent");
        assert_eq!(incomplete[1].content, "Parent");
        assert_eq!(incomplete[2].content, "Child");
    }

    #[test]
    fn test_get_incomplete_items_excludes_complete_parent_without_incomplete_children() {
        let mut list = create_test_list();
        list.add_item("Parent".to_string());
        list.add_item("Child".to_string());

        let parent_id = list.items[0].id;
        list.items[0].state = TodoState::Checked;
        list.items[1].state = TodoState::Checked;
        list.items[1].parent_id = Some(parent_id);
        list.items[1].indent_level = 1;

        let incomplete = list.get_incomplete_items();
        assert!(incomplete.is_empty());
    }

    #[test]
    fn test_toggle_item_state() {
        let mut list = create_test_list();
        list.add_item("Task".to_string());

        assert_eq!(list.items[0].state, TodoState::Empty);
        list.toggle_item_state(0).unwrap();
        assert_eq!(list.items[0].state, TodoState::Checked);
    }

    #[test]
    fn test_remove_item() {
        let mut list = create_test_list();
        list.add_item("Task 1".to_string());
        list.add_item("Task 2".to_string());
        list.add_item("Task 3".to_string());

        let removed = list.remove_item(1).unwrap();
        assert_eq!(removed.content, "Task 2");
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[1].content, "Task 3");
    }

    #[test]
    fn test_insert_item() {
        let mut list = create_test_list();
        list.add_item("Task 1".to_string());
        list.add_item("Task 3".to_string());

        list.insert_item(1, "Task 2".to_string(), 0).unwrap();

        assert_eq!(list.items.len(), 3);
        assert_eq!(list.items[1].content, "Task 2");
    }

    #[test]
    fn test_is_empty() {
        let mut list = create_test_list();
        assert!(list.is_empty());

        list.add_item("Task".to_string());
        assert!(!list.is_empty());
    }

    #[test]
    fn test_len() {
        let mut list = create_test_list();
        assert_eq!(list.len(), 0);

        list.add_item("Task 1".to_string());
        list.add_item("Task 2".to_string());
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_sort_by_priority_basic() {
        let mut list = create_test_list();
        list.add_item("No priority".to_string());
        list.add_item("P2 task".to_string());
        list.add_item("P0 task".to_string());
        list.add_item("P1 task".to_string());

        // Set priorities
        list.items[1].priority = Some(Priority::P2);
        list.items[2].priority = Some(Priority::P0);
        list.items[3].priority = Some(Priority::P1);

        list.sort_by_priority();

        // Should be: P0, P1, P2, None
        assert_eq!(list.items[0].content, "P0 task");
        assert_eq!(list.items[1].content, "P1 task");
        assert_eq!(list.items[2].content, "P2 task");
        assert_eq!(list.items[3].content, "No priority");
    }

    #[test]
    fn test_sort_by_priority_preserves_hierarchy() {
        let mut list = create_test_list();

        // Create: Low priority parent with children, then high priority parent with children
        list.add_item_with_indent("Low priority parent".to_string(), 0);
        list.add_item_with_indent("Low child 1".to_string(), 1);
        list.add_item_with_indent("Low child 2".to_string(), 1);
        list.add_item_with_indent("High priority parent".to_string(), 0);
        list.add_item_with_indent("High child 1".to_string(), 1);

        list.items[0].priority = Some(Priority::P2);
        list.items[3].priority = Some(Priority::P0);

        // Recalculate parent IDs to set up hierarchy
        list.recalculate_parent_ids();

        list.sort_by_priority();

        // High priority should come first with its children
        assert_eq!(list.items[0].content, "High priority parent");
        assert_eq!(list.items[1].content, "High child 1");
        assert_eq!(list.items[1].indent_level, 1);

        // Low priority should come after with its children
        assert_eq!(list.items[2].content, "Low priority parent");
        assert_eq!(list.items[3].content, "Low child 1");
        assert_eq!(list.items[4].content, "Low child 2");
        assert_eq!(list.items[3].indent_level, 1);
        assert_eq!(list.items[4].indent_level, 1);
    }

    #[test]
    fn test_sort_by_priority_stable() {
        let mut list = create_test_list();

        // Multiple items with same priority - order should be preserved
        list.add_item("First P1".to_string());
        list.add_item("Second P1".to_string());
        list.add_item("Third P1".to_string());

        list.items[0].priority = Some(Priority::P1);
        list.items[1].priority = Some(Priority::P1);
        list.items[2].priority = Some(Priority::P1);

        list.sort_by_priority();

        // Order should be preserved (stable sort)
        assert_eq!(list.items[0].content, "First P1");
        assert_eq!(list.items[1].content, "Second P1");
        assert_eq!(list.items[2].content, "Third P1");
    }

    #[test]
    fn test_sort_by_priority_empty_list() {
        let mut list = create_test_list();
        list.sort_by_priority(); // Should not panic
        assert!(list.items.is_empty());
    }

    #[test]
    fn test_sort_by_priority_recalculates_parent_ids() {
        let mut list = create_test_list();

        list.add_item_with_indent("P1 parent".to_string(), 0);
        list.add_item_with_indent("P1 child".to_string(), 1);
        list.add_item_with_indent("P0 parent".to_string(), 0);
        list.add_item_with_indent("P0 child".to_string(), 1);

        list.items[0].priority = Some(Priority::P1);
        list.items[2].priority = Some(Priority::P0);
        list.recalculate_parent_ids();

        list.sort_by_priority();

        // After sort, P0 parent should be first
        assert_eq!(list.items[0].content, "P0 parent");
        assert_eq!(list.items[1].content, "P0 child");

        // P0 child should have P0 parent as its parent
        assert_eq!(list.items[1].parent_id, Some(list.items[0].id));

        // P1 parent should be second
        assert_eq!(list.items[2].content, "P1 parent");
        assert_eq!(list.items[3].content, "P1 child");

        // P1 child should have P1 parent as its parent
        assert_eq!(list.items[3].parent_id, Some(list.items[2].id));
    }
}
