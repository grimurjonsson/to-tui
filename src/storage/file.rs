use super::database;
use super::markdown::{parse_todo_list, serialize_todo_list_clean};
use crate::todo::TodoList;
use crate::utils::paths::{ensure_directories_exist, get_daily_file_path};
use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::fs;

pub fn load_todo_list(date: NaiveDate) -> Result<TodoList> {
    ensure_directories_exist()?;
    database::init_database()?;

    let file_path = get_daily_file_path(date)?;

    if database::has_todos_for_date(date)? {
        let items = database::load_todos_for_date(date)?;
        return Ok(TodoList::with_items(date, file_path, items));
    }

    if file_path.exists() {
        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let list = parse_todo_list(&content, date, file_path.clone())
            .with_context(|| "Failed to parse todo list")?;

        if !list.items.is_empty() {
            database::save_todo_list(&list)?;
        }

        return Ok(list);
    }

    Ok(TodoList::new(date, file_path))
}

pub fn save_todo_list(list: &TodoList) -> Result<()> {
    ensure_directories_exist()?;
    database::init_database()?;

    database::save_todo_list(list)?;

    let content = serialize_todo_list_clean(list);

    let temp_path = list.file_path.with_extension("tmp");

    fs::write(&temp_path, content)
        .with_context(|| format!("Failed to write to temp file: {}", temp_path.display()))?;

    fs::rename(&temp_path, &list.file_path).with_context(|| {
        format!(
            "Failed to rename temp file to: {}",
            list.file_path.display()
        )
    })?;

    Ok(())
}

pub fn file_exists(date: NaiveDate) -> Result<bool> {
    database::init_database()?;

    if database::has_todos_for_date(date)? {
        return Ok(true);
    }

    let file_path = get_daily_file_path(date)?;
    Ok(file_path.exists())
}

#[cfg(test)]
mod tests {
    use super::super::markdown::serialize_todo_list_clean;
    use super::*;
    use chrono::NaiveDate;
    use tempfile::TempDir;

    fn setup_test_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_save_and_load_todo_list() {
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        let temp_dir = setup_test_dir();
        let file_path = temp_dir.path().join("2025-12-31.md");

        let mut list = TodoList::new(date, file_path.clone());
        list.add_item("Test task 1".to_string());
        list.add_item("Test task 2".to_string());

        let content = serialize_todo_list_clean(&list);
        fs::write(&file_path, content).unwrap();

        let loaded_content = fs::read_to_string(&file_path).unwrap();
        let loaded_list = parse_todo_list(&loaded_content, date, file_path).unwrap();

        assert_eq!(loaded_list.items.len(), 2);
        assert_eq!(loaded_list.items[0].content, "Test task 1");
        assert_eq!(loaded_list.items[1].content, "Test task 2");
    }

    #[test]
    fn test_serialize_and_deserialize_preserves_data() {
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        let temp_dir = setup_test_dir();
        let file_path = temp_dir.path().join("test.md");

        let mut list = TodoList::new(date, file_path.clone());
        list.add_item_with_indent("Parent".to_string(), 0);
        list.add_item_with_indent("Child".to_string(), 1);
        list.items[1].state = crate::todo::TodoState::Checked;

        let markdown = serialize_todo_list_clean(&list);
        let parsed = parse_todo_list(&markdown, date, file_path).unwrap();

        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].content, "Parent");
        assert_eq!(parsed.items[1].content, "Child");
        assert_eq!(parsed.items[1].state, crate::todo::TodoState::Checked);
    }
}
