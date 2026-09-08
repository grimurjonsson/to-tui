//! Migration system for upgrading v1 installations to v2 project structure
//!
//! V1 layout:
//! ```text
//! ~/.to-tui/
//! ├── dailies/
//! │   └── YYYY-MM-DD.md
//! ├── config.toml
//! └── todos.db
//! ```
//!
//! V2 layout:
//! ```text
//! ~/.to-tui/
//! ├── projects/
//! │   └── default/
//! │       └── dailies/
//! │           └── YYYY-MM-DD.md
//! ├── projects.toml
//! ├── config.toml
//! └── todos.db
//! ```

use crate::project::{DEFAULT_PROJECT_NAME, ProjectRegistry};
use crate::storage::database;
use crate::utils::paths::{get_dailies_dir_for_project, get_legacy_dailies_dir, get_to_tui_dir};
use anyhow::{Context, Result};
use tracing::{debug, info};

use std::fs;
use std::path::PathBuf;

/// Check if the installation is v1 (legacy) layout
pub fn is_v1_layout() -> Result<bool> {
    let legacy_dailies = get_legacy_dailies_dir()?;
    Ok(legacy_dailies.exists())
}

/// Check if this is a fresh install (no data directory at all)
pub fn is_fresh_install() -> Result<bool> {
    let to_tui_dir = get_to_tui_dir()?;
    Ok(!to_tui_dir.exists())
}

/// Run the migration from v1 to v2 layout
/// This is idempotent - safe to run multiple times
pub fn migrate_v1_to_v2() -> Result<()> {
    info!("Starting v1 to v2 migration");

    // Step 1: Ensure project registry exists with default project
    let mut registry = ProjectRegistry::load()?;
    registry.ensure_default_project()?;
    info!("Ensured default project exists in registry");

    // Step 2: Move dailies from ~/.to-tui/dailies/ to ~/.to-tui/projects/default/dailies/
    migrate_dailies_directory()?;

    info!("Migration from v1 to v2 completed successfully");
    Ok(())
}

/// Initialize for fresh install - just create the default project
pub fn initialize_fresh_install() -> Result<()> {
    info!("Initializing fresh install");
    fs::create_dir_all(get_to_tui_dir()?)?;

    let mut registry = ProjectRegistry::load()?;
    registry.ensure_default_project()?;

    // Ensure the default project directories exist
    let dailies_dir = get_dailies_dir_for_project(DEFAULT_PROJECT_NAME)?;
    if !dailies_dir.exists() {
        fs::create_dir_all(&dailies_dir)?;
        debug!(
            "Created default project dailies directory: {:?}",
            dailies_dir
        );
    }

    info!("Fresh install initialized with default project");
    Ok(())
}

/// Move files from legacy dailies to default project dailies
fn migrate_dailies_directory() -> Result<()> {
    let legacy_dailies = get_legacy_dailies_dir()?;
    let new_dailies = get_dailies_dir_for_project(DEFAULT_PROJECT_NAME)?;

    if !legacy_dailies.exists() {
        debug!("No legacy dailies directory to migrate");
        return Ok(());
    }

    // Create the new directory structure
    if !new_dailies.exists() {
        fs::create_dir_all(&new_dailies)
            .with_context(|| format!("Failed to create directory: {:?}", new_dailies))?;
    }

    // Move all .md files from legacy to new location
    let entries = fs::read_dir(&legacy_dailies)
        .with_context(|| format!("Failed to read directory: {:?}", legacy_dailies))?;

    let mut moved_count = 0;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "md") {
            let filename = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
            let new_path = new_dailies.join(filename);

            // Only move if destination doesn't exist (idempotent)
            if !new_path.exists() {
                fs::rename(&path, &new_path)
                    .with_context(|| format!("Failed to move {:?} to {:?}", path, new_path))?;
                debug!("Moved {:?} to {:?}", path, new_path);
                moved_count += 1;
            } else {
                debug!("Skipping {:?} - already exists at destination", path);
            }
        }
    }

    info!("Moved {} dailies files to default project", moved_count);

    // Try to remove the legacy dailies directory if empty
    if is_dir_empty(&legacy_dailies)? {
        fs::remove_dir(&legacy_dailies).ok();
        debug!("Removed empty legacy dailies directory");
    }

    Ok(())
}

/// Check if a directory is empty
fn is_dir_empty(path: &PathBuf) -> Result<bool> {
    let mut entries = fs::read_dir(path)?;
    Ok(entries.next().is_none())
}

/// Run the appropriate migration/initialization based on current state
/// Call this on startup to ensure the installation is properly set up
pub fn ensure_installation_ready() -> Result<()> {
    if is_fresh_install()? {
        initialize_fresh_install()?;
    } else if is_v1_layout()? {
        migrate_v1_to_v2()?;
    } else {
        // V2 layout already - just ensure default project exists
        let mut registry = ProjectRegistry::load()?;
        registry.ensure_default_project()?;
    }

    // Always sync projects from todos table to catch any orphaned projects
    // This handles edge cases where todos exist with a project name but the
    // project wasn't properly registered
    let synced = database::sync_projects_from_todos()?;
    if synced > 0 {
        info!("Auto-registered {} orphaned projects from todos", synced);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_dir_empty() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().to_path_buf();

        // Empty dir
        assert!(is_dir_empty(&path).unwrap());

        // Non-empty dir
        fs::write(temp.path().join("test.txt"), "content").unwrap();
        assert!(!is_dir_empty(&path).unwrap());
    }

    #[test]
    fn test_v1_layout_detection_resumes_partial_migration() {
        let temp = TempDir::new().unwrap();
        crate::storage::context::with_root(temp.path().to_path_buf(), || {
            assert!(!is_v1_layout().unwrap());
            fs::create_dir_all(temp.path().join("dailies")).unwrap();
            assert!(is_v1_layout().unwrap());
            fs::create_dir_all(temp.path().join("projects")).unwrap();
            assert!(is_v1_layout().unwrap());
            fs::remove_dir(temp.path().join("dailies")).unwrap();
            assert!(!is_v1_layout().unwrap());
        });
    }
    fn create_legacy_database(root: &std::path::Path, project_column: bool) {
        let conn = rusqlite::Connection::open(root.join("todos.db")).unwrap();
        for (table, date_column) in [("todos", "date"), ("archived_todos", "original_date")] {
            conn.execute_batch(&format!(
                "CREATE TABLE {table} (
                    id TEXT PRIMARY KEY, {date_column} TEXT NOT NULL,
                    content TEXT NOT NULL, state TEXT NOT NULL,
                    indent_level INTEGER NOT NULL, parent_id TEXT,
                    due_date TEXT, description TEXT, position INTEGER NOT NULL,
                    created_at TEXT NOT NULL, updated_at TEXT NOT NULL
                );
                INSERT INTO {table} VALUES (
                    '11111111-1111-4111-8111-111111111111', '2026-09-08',
                    'Keep my task', '!', 0, NULL, '2026-09-09', 'Keep my notes',
                    0, '2026-09-08T10:00:00Z', '2026-09-08T10:00:00Z'
                );"
            ))
            .unwrap();
            if project_column {
                conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN project TEXT;"))
                    .unwrap();
            }
        }
        conn.execute_batch(
            "ALTER TABLE archived_todos ADD COLUMN archived_at TEXT;
             UPDATE archived_todos SET archived_at = '2026-09-08T11:00:00Z';",
        )
        .unwrap();
    }

    #[test]
    fn test_startup_migrates_legacy_database_in_place_idempotently() {
        for project_column in [false, true] {
            let temp = TempDir::new().unwrap();
            create_legacy_database(temp.path(), project_column);
            fs::write(temp.path().join("config.toml"), "existing configuration").unwrap();
            let legacy = temp.path().join("dailies");
            let destination = temp.path().join("projects/default/dailies");
            fs::create_dir_all(&legacy).unwrap();
            fs::create_dir_all(&destination).unwrap();
            fs::write(legacy.join("2026-09-08.md"), "legacy daily").unwrap();
            fs::write(legacy.join("2026-09-07.md"), "legacy conflict").unwrap();
            fs::write(destination.join("2026-09-07.md"), "existing daily").unwrap();
            crate::storage::context::with_root(temp.path().to_path_buf(), || {
                for _ in 0..2 {
                    ensure_installation_ready().unwrap();
                    let list = database::load_list_snapshot(
                        chrono::NaiveDate::from_ymd_opt(2026, 9, 8).unwrap(),
                        DEFAULT_PROJECT_NAME,
                        destination.join("2026-09-08.md"),
                    )
                    .unwrap();
                    assert_eq!(list.items.len(), 1);
                    let item = &list.items[0];
                    assert_eq!(item.id.to_string(), "11111111-1111-4111-8111-111111111111");
                    assert_eq!(item.content, "Keep my task");
                    assert_eq!(item.description.as_deref(), Some("Keep my notes"));
                    assert_eq!(item.state, crate::todo::TodoState::Exclamation);
                    let conn = database::get_connection().unwrap();
                    let archived: (String, String, i64, String) = conn
                        .query_row(
                            "SELECT content, description, collapsed, project FROM archived_todos",
                            [],
                            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                        )
                        .unwrap();
                    assert_eq!(
                        archived,
                        (
                            "Keep my task".into(),
                            "Keep my notes".into(),
                            0,
                            "default".into()
                        )
                    );
                    assert_eq!(
                        conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                            .unwrap(),
                        "ok"
                    );
                    assert!(!temp.path().join("users.db").exists());
                    assert!(!temp.path().join("users").exists());
                }
            });
            assert_eq!(
                fs::read_to_string(destination.join("2026-09-08.md")).unwrap(),
                "legacy daily"
            );
            assert_eq!(
                fs::read_to_string(destination.join("2026-09-07.md")).unwrap(),
                "existing daily"
            );
            assert_eq!(
                fs::read_to_string(legacy.join("2026-09-07.md")).unwrap(),
                "legacy conflict"
            );
            assert_eq!(
                fs::read_to_string(temp.path().join("config.toml")).unwrap(),
                "existing configuration"
            );
        }
    }

    #[test]
    fn test_startup_initializes_missing_data_directory() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("new");
        crate::storage::context::with_root(root.clone(), || {
            ensure_installation_ready().unwrap();
            ensure_installation_ready().unwrap();
        });
        assert!(root.join("todos.db").exists());
        assert!(root.join("projects/default/dailies").exists());
    }

    #[test]
    fn test_failed_database_migration_rolls_back_and_can_retry() {
        let temp = TempDir::new().unwrap();
        create_legacy_database(temp.path(), false);
        let conn = rusqlite::Connection::open(temp.path().join("todos.db")).unwrap();
        conn.execute_batch("CREATE TABLE todo_metadata (id TEXT PRIMARY KEY, todo_id TEXT,
                plugin_name TEXT, data TEXT, external_id TEXT, created_at TEXT, updated_at TEXT);
            INSERT INTO todo_metadata (id, plugin_name, external_id) VALUES ('a', 'plugin', 'duplicate'), ('b', 'plugin', 'duplicate');").unwrap();
        crate::storage::context::with_root(temp.path().to_path_buf(), || {
            assert!(database::init_database().is_err());
        });
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM pragma_table_info('todos') WHERE name = 'project'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        conn.execute("DELETE FROM todo_metadata WHERE id = 'b'", [])
            .unwrap();
        crate::storage::context::with_root(temp.path().to_path_buf(), || {
            ensure_installation_ready().unwrap();
        });
        assert_eq!(
            conn.query_row("SELECT content FROM todos", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "Keep my task"
        );
    }
}
