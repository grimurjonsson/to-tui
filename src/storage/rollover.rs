use super::database::{active_todo_dates_before, archive_todos_for_date_and_project};
use super::file::{
    file_exists_for_project, load_todo_list_for_project, save_todo_list_for_project,
};
use crate::todo::TodoList;
use crate::utils::paths::{get_dailies_dir_for_project, get_daily_file_path_for_project};
use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use uuid::Uuid;

/// Find incomplete items from the newest prior list for a specific project.
/// Returns (source_date, incomplete_items) if found, None otherwise.
pub fn find_rollover_candidates_for_project(
    project_name: &str,
) -> Result<Option<(NaiveDate, Vec<crate::todo::TodoItem>)>> {
    find_rollover_candidates_for_project_at(project_name, Local::now().date_naive())
}

fn find_rollover_candidates_for_project_at(
    project_name: &str,
    today: NaiveDate,
) -> Result<Option<(NaiveDate, Vec<crate::todo::TodoItem>)>> {
    if file_exists_for_project(project_name, today)? {
        return Ok(None);
    }

    let mut dates = daily_dates_for_project(project_name)?;
    dates.extend(active_todo_dates_before(today, project_name)?);

    let Some(source_date) = latest_prior_date(today, dates) else {
        return Ok(None);
    };

    let list = load_todo_list_for_project(project_name, source_date)?;
    let incomplete = list.get_incomplete_items();
    if incomplete.is_empty() {
        return Ok(None);
    }

    Ok(Some((source_date, incomplete)))
}

fn latest_prior_date(
    today: NaiveDate,
    dates: impl IntoIterator<Item = NaiveDate>,
) -> Option<NaiveDate> {
    dates.into_iter().filter(|date| *date < today).max()
}

fn daily_dates_for_project(project_name: &str) -> Result<Vec<NaiveDate>> {
    let dailies_dir = get_dailies_dir_for_project(project_name)?;
    daily_dates_in_dir(&dailies_dir)
}

fn daily_dates_in_dir(dailies_dir: &Path) -> Result<Vec<NaiveDate>> {
    if !dailies_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(dailies_dir).with_context(|| {
        format!(
            "Failed to read dailies directory: {}",
            dailies_dir.display()
        )
    })?;
    let mut dates = Vec::new();

    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
            && let Ok(date) = NaiveDate::parse_from_str(stem, "%Y-%m-%d")
        {
            dates.push(date);
        }
    }

    Ok(dates)
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
    use crate::project::DEFAULT_PROJECT_NAME;
    use crate::todo::{TodoItem, TodoState};
    use tempfile::TempDir;

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

    #[test]
    fn test_daily_dates_in_dir_uses_valid_markdown_date_filenames() {
        let temp_dir = TempDir::new().unwrap();
        for filename in [
            "2026-06-05.md",
            "2026-06-04.md",
            "notes.md",
            "2026-06-03.txt",
        ] {
            fs::write(temp_dir.path().join(filename), "").unwrap();
        }

        let mut dates = daily_dates_in_dir(temp_dir.path()).unwrap();
        dates.sort();

        assert_eq!(
            dates,
            vec![
                NaiveDate::from_ymd_opt(2026, 6, 4).unwrap(),
                NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            ]
        );
    }

    #[test]
    fn test_latest_prior_date_has_no_age_limit() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 13).unwrap();
        let dates = [
            NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
            NaiveDate::from_ymd_opt(2026, 5, 8).unwrap(),
        ];

        assert_eq!(
            latest_prior_date(today, dates),
            Some(NaiveDate::from_ymd_opt(2026, 6, 5).unwrap())
        );
    }
}
