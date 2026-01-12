use super::database::archive_todos_for_date;
use super::file::{file_exists, load_todo_list, save_todo_list};
use crate::todo::TodoList;
use crate::utils::paths::get_daily_file_path;
use anyhow::Result;
use chrono::{Local, NaiveDate};
use std::collections::HashMap;
use uuid::Uuid;

/// Find incomplete items from the most recent previous day (up to 30 days back).
/// Returns (source_date, incomplete_items) if found, None otherwise.
pub fn find_rollover_candidates() -> Result<Option<(NaiveDate, Vec<crate::todo::TodoItem>)>> {
    let today = Local::now().date_naive();

    // Check if today's file already exists - no rollover needed
    if file_exists(today)? {
        return Ok(None);
    }

    // Look back up to 30 days for the most recent file with incomplete items
    for days_back in 1..=30 {
        if let Some(check_date) = today.checked_sub_days(chrono::Days::new(days_back))
            && file_exists(check_date)? {
                let list = load_todo_list(check_date)?;
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

/// Execute the rollover: archive old todos and create new list with rolled-over items.
pub fn execute_rollover(
    source_date: NaiveDate,
    items: Vec<crate::todo::TodoItem>,
) -> Result<TodoList> {
    let today = Local::now().date_naive();
    archive_todos_for_date(source_date)?;
    let list = create_rolled_over_list(today, items)?;
    save_todo_list(&list)?;
    Ok(list)
}

pub fn create_rolled_over_list(
    date: NaiveDate,
    mut items: Vec<crate::todo::TodoItem>,
) -> Result<TodoList> {
    let file_path = get_daily_file_path(date)?;

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

        let list = create_rolled_over_list(today, items).unwrap();

        assert_eq!(list.items.len(), 2);
        assert_eq!(list.date, today);
        assert_eq!(list.items[0].content, "Task 1");
        assert_eq!(list.items[1].content, "Task 2");
    }
}
