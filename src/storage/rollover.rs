use super::database::archive_todos_for_date_and_project;
use super::file::{
    file_exists_for_project, load_todo_list_for_project, save_todo_list_for_project,
};
use crate::project::DEFAULT_PROJECT_NAME;
use crate::todo::TodoList;
use crate::utils::paths::get_daily_file_path_for_project;
use anyhow::Result;
use chrono::{Local, NaiveDate};
use std::collections::HashMap;
use uuid::Uuid;

/// Find incomplete items from the most recent previous day for a specific project.
/// Returns (source_date, incomplete_items) if found, None otherwise.
pub fn find_rollover_candidates_for_project(
    project_name: &str,
) -> Result<Option<(NaiveDate, Vec<crate::todo::TodoItem>)>> {
    let today = Local::now().date_naive();

    // Check if today's file already exists - no rollover needed
    if file_exists_for_project(project_name, today)? {
        return Ok(None);
    }

    // Look back up to 30 days for the most recent file with incomplete items
    for days_back in 1..=30 {
        if let Some(check_date) = today.checked_sub_days(chrono::Days::new(days_back))
            && file_exists_for_project(project_name, check_date)?
        {
            let list = load_todo_list_for_project(project_name, check_date)?;
            let incomplete = list.get_incomplete_items();

            if !incomplete.is_empty() {
                return Ok(Some((check_date, incomplete)));
            }
            // Found a file but no incomplete items, stop searching
            break;
        }
    }

    Ok(None)
}

/// Legacy: Find rollover candidates for the default project
/// Use find_rollover_candidates_for_project() for project-aware code
pub fn find_rollover_candidates() -> Result<Option<(NaiveDate, Vec<crate::todo::TodoItem>)>> {
    find_rollover_candidates_for_project(DEFAULT_PROJECT_NAME)
}

/// Execute the rollover for a specific project: archive old todos and create new list.
pub fn execute_rollover_for_project(
    project_name: &str,
    source_date: NaiveDate,
    items: Vec<crate::todo::TodoItem>,
) -> Result<TodoList> {
    let today = Local::now().date_naive();
    archive_todos_for_date_and_project(source_date, project_name)?;
    let list = create_rolled_over_list_for_project(project_name, today, items)?;
    save_todo_list_for_project(&list, project_name)?;
    Ok(list)
}

/// Legacy: Execute rollover for the default project
/// Use execute_rollover_for_project() for project-aware code
pub fn execute_rollover(
    source_date: NaiveDate,
    items: Vec<crate::todo::TodoItem>,
) -> Result<TodoList> {
    execute_rollover_for_project(DEFAULT_PROJECT_NAME, source_date, items)
}

pub fn create_rolled_over_list_for_project(
    project_name: &str,
    date: NaiveDate,
    mut items: Vec<crate::todo::TodoItem>,
) -> Result<TodoList> {
    let file_path = get_daily_file_path_for_project(project_name, date)?;

    let mut old_to_new_id: HashMap<Uuid, Uuid> = HashMap::new();

    for item in &mut items {
        let new_id = Uuid::new_v4();
        old_to_new_id.insert(item.id, new_id);
        item.id = new_id;
    }

    for item in &mut items {
        if let Some(old_parent_id) = item.parent_id {
            item.parent_id = old_to_new_id.get(&old_parent_id).copied();
        }
    }

    Ok(TodoList::with_items(date, file_path, items))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todo::{TodoItem, TodoState};

    #[test]
    fn test_create_rolled_over_list() {
        let today = Local::now().date_naive();
        let items = vec![
            TodoItem::with_state("Task 1".to_string(), TodoState::Empty, 0),
            TodoItem::with_state("Task 2".to_string(), TodoState::Question, 0),
        ];

        let list = create_rolled_over_list_for_project(DEFAULT_PROJECT_NAME, today, items).unwrap();

        assert_eq!(list.items.len(), 2);
        assert_eq!(list.date, today);
        assert_eq!(list.items[0].content, "Task 1");
        assert_eq!(list.items[1].content, "Task 2");
    }
}
