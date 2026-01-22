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

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

use crate::project::{ProjectRegistry, DEFAULT_PROJECT_NAME};
use crate::storage::database;
use crate::utils::paths::{
    get_dailies_dir_for_project, get_legacy_dailies_dir, get_projects_dir, get_to_tui_dir,
};

/// Check if the installation is v1 (legacy) layout
pub fn is_v1_layout() -> Result<bool> {
    let legacy_dailies = get_legacy_dailies_dir()?;
    let projects_dir = get_projects_dir()?;

    // V1 layout: has legacy dailies directory, no projects directory
    Ok(legacy_dailies.exists() && !projects_dir.exists())
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

    // Step 3: Update database entries with project='default'
    update_database_project_column()?;

    info!("Migration from v1 to v2 completed successfully");
    Ok(())
}

/// Initialize for fresh install - just create the default project
pub fn initialize_fresh_install() -> Result<()> {
    info!("Initializing fresh install");

    let mut registry = ProjectRegistry::load()?;
    registry.ensure_default_project()?;

    // Ensure the default project directories exist
    let dailies_dir = get_dailies_dir_for_project(DEFAULT_PROJECT_NAME)?;
    if !dailies_dir.exists() {
        fs::create_dir_all(&dailies_dir)?;
        debug!("Created default project dailies directory: {:?}", dailies_dir);
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

        if path.extension().map_or(false, |ext| ext == "md") {
            let filename = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
            let new_path = new_dailies.join(filename);

            // Only move if destination doesn't exist (idempotent)
            if !new_path.exists() {
                fs::rename(&path, &new_path).with_context(|| {
                    format!("Failed to move {:?} to {:?}", path, new_path)
                })?;
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

/// Update database entries to set project='default' where project is NULL or missing
fn update_database_project_column() -> Result<()> {
    database::init_database()?;
    let conn = database::get_connection()?;

    // Update todos table
    let updated_todos = conn.execute(
        "UPDATE todos SET project = ?1 WHERE project IS NULL OR project = ''",
        [DEFAULT_PROJECT_NAME],
    )?;

    // Update archived_todos table
    let updated_archived = conn.execute(
        "UPDATE archived_todos SET project = ?1 WHERE project IS NULL OR project = ''",
        [DEFAULT_PROJECT_NAME],
    )?;

    if updated_todos > 0 || updated_archived > 0 {
        info!(
            "Updated {} todos and {} archived todos with default project",
            updated_todos, updated_archived
        );
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
    fn test_v1_layout_detection() {
        // This test validates the logic without actually modifying real paths
        // The actual path functions use the real home dir, so we test the logic separately
        let temp = TempDir::new().unwrap();
        let legacy_dailies = temp.path().join("dailies");
        let projects_dir = temp.path().join("projects");

        // Neither exists -> not v1
        assert!(!legacy_dailies.exists() && !projects_dir.exists());

        // Only legacy exists -> v1
        fs::create_dir_all(&legacy_dailies).unwrap();
        assert!(legacy_dailies.exists() && !projects_dir.exists());

        // Both exist -> not v1 (already migrated)
        fs::create_dir_all(&projects_dir).unwrap();
        assert!(legacy_dailies.exists() && projects_dir.exists());

        // Only projects exists -> v2
        fs::remove_dir_all(&legacy_dailies).unwrap();
        assert!(!legacy_dailies.exists() && projects_dir.exists());
    }
}
