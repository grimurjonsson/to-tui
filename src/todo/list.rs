use super::TodoItem;
use anyhow::{Result, anyhow};
use chrono::NaiveDate;
use std::path::PathBuf;

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
        self.items
            .iter()
            .filter(|item| !item.is_complete())
            .cloned()
            .collect()
    }

    #[cfg(test)]
    pub fn toggle_item_state(&mut self, index: usize) -> Result<()> {
        if index >= self.items.len() {
            return Err(anyhow!("Index out of bounds"));
        }
        self.items[index].toggle_state();
        Ok(())
    }

    pub fn delete_item(&mut self, index: usize) -> Result<TodoItem> {
        if index >= self.items.len() {
            return Err(anyhow!("Index out of bounds"));
        }
        Ok(self.items.remove(index))
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
    fn test_toggle_item_state() {
        let mut list = create_test_list();
        list.add_item("Task".to_string());

        assert_eq!(list.items[0].state, TodoState::Empty);
        list.toggle_item_state(0).unwrap();
        assert_eq!(list.items[0].state, TodoState::Checked);
    }

    #[test]
    fn test_delete_item() {
        let mut list = create_test_list();
        list.add_item("Task 1".to_string());
        list.add_item("Task 2".to_string());
        list.add_item("Task 3".to_string());

        let deleted = list.delete_item(1).unwrap();
        assert_eq!(deleted.content, "Task 2");
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
}
