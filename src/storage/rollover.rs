use super::database::archive_todos_for_date;
use super::file::{file_exists, load_todo_list, save_todo_list};
use crate::todo::TodoList;
use crate::utils::paths::get_daily_file_path;
use anyhow::Result;
use chrono::{Local, NaiveDate};
use std::collections::HashMap;
use std::io::{self, Write};
use uuid::Uuid;

pub fn check_and_prompt_rollover() -> Result<Option<TodoList>> {
    let today = Local::now().date_naive();

    // Check if today's file already exists
    if file_exists(today)? {
        return Ok(Some(load_todo_list(today)?));
    }

    // Look back up to 30 days for the most recent file with incomplete items
    let mut most_recent_incomplete: Option<(NaiveDate, Vec<crate::todo::TodoItem>)> = None;

    for days_back in 1..=30 {
        if let Some(check_date) = today.checked_sub_days(chrono::Days::new(days_back)) {
            if file_exists(check_date)? {
                let list = load_todo_list(check_date)?;
                let incomplete = list.get_incomplete_items();

                if !incomplete.is_empty() {
                    most_recent_incomplete = Some((check_date, incomplete));
                    break; // Found the most recent, stop searching
                }
            }
        }
    }

    if let Some((source_date, incomplete)) = most_recent_incomplete {
        if prompt_user_for_rollover(&incomplete, source_date)? {
            archive_todos_for_date(source_date)?;
            let list = create_rolled_over_list(today, incomplete)?;
            save_todo_list(&list)?;
            return Ok(Some(list));
        }
    }

    // No rollover needed, create empty list for today
    Ok(Some(TodoList::new(today, get_daily_file_path(today)?)))
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

fn prompt_user_for_rollover(
    incomplete: &[crate::todo::TodoItem],
    source_date: NaiveDate,
) -> Result<bool> {
    let today = Local::now().date_naive();
    let days_ago = (today - source_date).num_days();

    let date_desc = if days_ago == 1 {
        "yesterday".to_string()
    } else {
        format!(
            "{} ({} days ago)",
            source_date.format("%B %d, %Y"),
            days_ago
        )
    };

    println!(
        "\n{} incomplete item(s) found from {}:",
        incomplete.len(),
        date_desc
    );
    for (idx, item) in incomplete.iter().enumerate() {
        let indent = "  ".repeat(item.indent_level);
        println!("  {}{}. {} {}", indent, idx + 1, item.state, item.content);
    }

    print!("\nRoll over incomplete items to today? (Y/n): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    Ok(input.is_empty() || input == "y" || input == "yes")
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
