use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use std::fs;
use std::path::PathBuf;

pub fn get_to_tui_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    Ok(home.join(".to-tui"))
}

pub fn get_projects_dir() -> Result<PathBuf> {
    let todo_dir = get_to_tui_dir()?;
    Ok(todo_dir.join("projects"))
}

pub fn get_project_dir(project_name: &str) -> Result<PathBuf> {
    let projects_dir = get_projects_dir()?;
    Ok(projects_dir.join(project_name))
}

pub fn get_dailies_dir_for_project(project_name: &str) -> Result<PathBuf> {
    let project_dir = get_project_dir(project_name)?;
    Ok(project_dir.join("dailies"))
}

/// Legacy v1 dailies directory (before projects feature)
pub fn get_legacy_dailies_dir() -> Result<PathBuf> {
    let todo_dir = get_to_tui_dir()?;
    Ok(todo_dir.join("dailies"))
}

pub fn get_config_path() -> Result<PathBuf> {
    let todo_dir = get_to_tui_dir()?;
    Ok(todo_dir.join("config.toml"))
}

pub fn get_database_path() -> Result<PathBuf> {
    let todo_dir = get_to_tui_dir()?;
    Ok(todo_dir.join("todos.db"))
}

pub fn get_pid_file_path() -> Result<PathBuf> {
    let todo_dir = get_to_tui_dir()?;
    Ok(todo_dir.join("server.pid"))
}

pub fn get_ui_cache_path() -> Result<PathBuf> {
    let todo_dir = get_to_tui_dir()?;
    Ok(todo_dir.join("ui_cache.json"))
}

pub fn get_crash_log_path() -> Result<PathBuf> {
    let todo_dir = get_to_tui_dir()?;
    Ok(todo_dir.join("crash.log"))
}

pub fn get_daily_file_path_for_project(project_name: &str, date: NaiveDate) -> Result<PathBuf> {
    let dailies_dir = get_dailies_dir_for_project(project_name)?;
    let filename = format!("{}.md", date.format("%Y-%m-%d"));
    Ok(dailies_dir.join(filename))
}

pub fn ensure_project_directories_exist(project_name: &str) -> Result<()> {
    let dailies_dir = get_dailies_dir_for_project(project_name)?;

    if !dailies_dir.exists() {
        fs::create_dir_all(&dailies_dir)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::DEFAULT_PROJECT_NAME;
    use chrono::NaiveDate;

    #[test]
    fn test_get_to_tui_dir() {
        let dir = get_to_tui_dir().unwrap();
        assert!(dir.to_string_lossy().contains(".to-tui"));
    }

    #[test]
    fn test_get_projects_dir() {
        let dir = get_projects_dir().unwrap();
        assert!(dir.to_string_lossy().contains(".to-tui"));
        assert!(dir.to_string_lossy().ends_with("projects"));
    }

    #[test]
    fn test_get_project_dir() {
        let dir = get_project_dir("Work").unwrap();
        assert!(dir.to_string_lossy().contains(".to-tui"));
        assert!(dir.to_string_lossy().contains("projects"));
        assert!(dir.to_string_lossy().ends_with("Work"));
    }

    #[test]
    fn test_get_dailies_dir_for_project() {
        let dir = get_dailies_dir_for_project("Work").unwrap();
        assert!(dir.to_string_lossy().contains(".to-tui"));
        assert!(dir.to_string_lossy().contains("projects"));
        assert!(dir.to_string_lossy().contains("Work"));
        assert!(dir.to_string_lossy().ends_with("dailies"));
    }

    #[test]
    fn test_get_legacy_dailies_dir() {
        let dir = get_legacy_dailies_dir().unwrap();
        assert!(dir.to_string_lossy().contains(".to-tui"));
        assert!(dir.to_string_lossy().ends_with("dailies"));
        assert!(!dir.to_string_lossy().contains("projects"));
    }

    #[test]
    fn test_get_config_path() {
        let path = get_config_path().unwrap();
        assert!(path.to_string_lossy().contains(".to-tui"));
        assert!(path.to_string_lossy().ends_with("config.toml"));
    }

    #[test]
    fn test_get_daily_file_path_for_project() {
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        let path = get_daily_file_path_for_project("Work", date).unwrap();

        assert!(path.to_string_lossy().contains("projects"));
        assert!(path.to_string_lossy().contains("Work"));
        assert!(path.to_string_lossy().contains("dailies"));
        assert!(path.to_string_lossy().ends_with("2025-12-31.md"));
    }

    #[test]
    fn test_get_daily_file_path_default_project() {
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        let path = get_daily_file_path_for_project(DEFAULT_PROJECT_NAME, date).unwrap();

        assert!(path.to_string_lossy().contains("projects"));
        assert!(path.to_string_lossy().contains("default"));
        assert!(path.to_string_lossy().contains("dailies"));
        assert!(path.to_string_lossy().ends_with("2025-12-31.md"));
    }

    #[test]
    fn test_get_database_path() {
        let path = get_database_path().unwrap();
        assert!(path.to_string_lossy().contains(".to-tui"));
        assert!(path.to_string_lossy().ends_with("todos.db"));
    }

    #[test]
    fn test_get_pid_file_path() {
        let path = get_pid_file_path().unwrap();
        assert!(path.to_string_lossy().contains(".to-tui"));
        assert!(path.to_string_lossy().ends_with("server.pid"));
    }
}
