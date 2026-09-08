use super::database;
use crate::project::Project;
use crate::remote::protocol::{HistoryList, SyncBatch, SyncSnapshot, TaskResource, TaskVersion};
use crate::todo::TodoList;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

pub(crate) fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS task_versions(id TEXT PRIMARY KEY, revision INTEGER NOT NULL);
        INSERT OR IGNORE INTO task_versions SELECT id,1 FROM todos;
        CREATE TABLE IF NOT EXISTS sync_receipts(id TEXT PRIMARY KEY, body_hash TEXT NOT NULL);
        CREATE TRIGGER IF NOT EXISTS task_version_insert AFTER INSERT ON todos BEGIN
          INSERT INTO task_versions VALUES(NEW.id,1) ON CONFLICT(id) DO UPDATE SET revision=revision+1; END;
        CREATE TRIGGER IF NOT EXISTS task_version_delete AFTER DELETE ON todos BEGIN
          INSERT INTO task_versions VALUES(OLD.id,1) ON CONFLICT(id) DO UPDATE SET revision=revision+1; END;")?;
    let columns = [
        "date",
        "content",
        "state",
        "indent_level",
        "parent_id",
        "due_date",
        "description",
        "priority",
        "collapsed",
        "position",
        "created_at",
        "updated_at",
        "completed_at",
        "deleted_at",
        "project",
    ];
    let changed = columns
        .iter()
        .map(|col| format!("OLD.{col} IS NOT NEW.{col}"))
        .collect::<Vec<_>>()
        .join(" OR ");
    conn.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS task_version_update AFTER UPDATE ON todos WHEN {changed} BEGIN
        INSERT INTO task_versions VALUES(NEW.id,1) ON CONFLICT(id) DO UPDATE SET revision=revision+1; END;"))?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS sync_clock(id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL); INSERT OR IGNORE INTO sync_clock VALUES(1,1);")?;
    for table in ["todos", "projects", "archived_todos"] {
        for operation in ["INSERT", "UPDATE", "DELETE"] {
            conn.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS sync_clock_{table}_{operation} AFTER {operation} ON {table} BEGIN UPDATE sync_clock SET revision=revision+1 WHERE id=1; END;"))?;
        }
    }
    super::change_log::init(conn)?;
    Ok(())
}

fn read_on(conn: &Connection) -> Result<SyncSnapshot> {
    let mut result = SyncSnapshot {
        cursor: Some(super::change_log::cursor(conn)?),
        version: conn.query_row("SELECT revision FROM sync_clock WHERE id=1", [], |r| {
            r.get(0)
        })?,
        ..Default::default()
    };
    let mut statement = conn.prepare("SELECT id,name,created_at FROM projects ORDER BY name")?;
    for row in statement.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })? {
        let (id, name, created) = row?;
        result.projects.push(Project {
            id: Uuid::parse_str(&id)?,
            name,
            created_at: database::parse_timestamp(&created)
                .context("Invalid project creation timestamp")?,
        });
    }
    let mut statement =
        conn.prepare("SELECT project,date FROM list_revisions ORDER BY project,date")?;
    for row in statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (project, date) = row?;
        let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")?;
        result.dates.push((project.clone(), date));
        let archived = database::load_archived_on(conn, date, &project)?;
        if !archived.is_empty() {
            result.history.push(HistoryList {
                project,
                date,
                items: archived,
            });
        }
    }
    let mut resources = BTreeMap::new();
    for (project, date) in &result.dates {
        for item in database::load_items_on(conn, *date, project)? {
            let position = conn.query_row(
                "SELECT position FROM todos WHERE id=?1",
                [item.id.to_string()],
                |r| r.get::<_, i64>(0),
            )?;
            resources.insert(
                item.id,
                TaskResource {
                    project: project.clone(),
                    date: *date,
                    position: usize::try_from(position).map_err(anyhow::Error::from)?,
                    item,
                },
            );
        }
    }
    let mut statement = conn.prepare("SELECT id,revision FROM task_versions ORDER BY id")?;
    for row in statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
        let (id, revision) = row?;
        let id = Uuid::parse_str(&id)?;
        result.tasks.push(TaskVersion {
            id,
            etag: format!("\"{id}-v{revision}\""),
            resource: resources.remove(&id),
        });
    }
    Ok(result)
}

pub(crate) fn snapshot() -> Result<SyncSnapshot> {
    let mut conn = database::get_connection()?;
    let tx = conn.transaction()?;
    let result = read_on(&tx)?;
    tx.commit()?;
    Ok(result)
}

#[derive(Debug)]
pub(crate) enum ApplyError {
    Precondition,
    Invalid(anyhow::Error),
}
impl From<anyhow::Error> for ApplyError {
    fn from(e: anyhow::Error) -> Self {
        Self::Invalid(e)
    }
}
impl From<rusqlite::Error> for ApplyError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Invalid(e.into())
    }
}

pub(crate) fn apply(batch: SyncBatch) -> Result<SyncSnapshot, ApplyError> {
    let hash = crate::api::client_auth::challenge(
        &serde_json::to_string(&batch).map_err(anyhow::Error::from)?,
    );
    let mut conn = database::get_connection()?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let receipt: Option<String> = tx
        .query_row(
            "SELECT body_hash FROM sync_receipts WHERE id=?1",
            [batch.request_id.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(receipt) = receipt {
        if receipt != hash {
            return Err(ApplyError::Invalid(anyhow::anyhow!(
                "Idempotency key was reused with a different request"
            )));
        }
        return Ok(read_on(&tx)?);
    }
    if batch.mutations.is_empty() || batch.mutations.len() > 10000 {
        return Err(ApplyError::Invalid(anyhow::anyhow!(
            "Invalid mutation count"
        )));
    }
    let current = read_on(&tx)?;
    let mut resources = BTreeMap::new();
    let versions: BTreeMap<_, _> = current.tasks.iter().map(|task| (task.id, task)).collect();
    for task in &current.tasks {
        if let Some(resource) = &task.resource {
            resources.insert(task.id, resource.clone());
        }
    }
    let mut ids = HashSet::new();
    let mut affected = HashSet::new();
    for mutation in &batch.mutations {
        if !ids.insert(mutation.id) {
            return Err(ApplyError::Invalid(anyhow::anyhow!(
                "Duplicate task mutation"
            )));
        }
        if versions.get(&mutation.id).map(|task| &task.etag) != mutation.if_match.as_ref() {
            return Err(ApplyError::Precondition);
        }
        if let Some(resource) = resources.get(&mutation.id) {
            affected.insert((resource.project.clone(), resource.date));
        }
        if let Some(resource) = &mutation.resource {
            if resource.item.id != mutation.id
                || resource.item.deleted_at.is_some()
                || !current.projects.iter().any(|p| p.name == resource.project)
            {
                return Err(ApplyError::Invalid(anyhow::anyhow!(
                    "Invalid task or missing destination project"
                )));
            }
            affected.insert((resource.project.clone(), resource.date));
            resources.insert(mutation.id, resource.clone());
        } else {
            resources.remove(&mutation.id);
        }
    }
    for (project, date) in &affected {
        let mut items = resources
            .values()
            .filter(|r| &r.project == project && &r.date == date)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by_key(|r| (r.position, r.item.id));
        let mut parents = HashSet::new();
        for resource in &items {
            if resource
                .item
                .parent_id
                .is_some_and(|id| !parents.contains(&id))
            {
                return Err(ApplyError::Precondition);
            }
            parents.insert(resource.item.id);
        }
        let list = TodoList::with_items(
            *date,
            Default::default(),
            items.into_iter().map(|r| r.item).collect(),
        );
        list.revision
            .set(database::list_revision(&tx, *date, project)?);
        database::save_list_on(&tx, &list, project)?;
    }
    tx.execute(
        "INSERT INTO sync_receipts(id,body_hash) VALUES(?1,?2)",
        params![batch.request_id.to_string(), hash],
    )?;
    let result = read_on(&tx)?;
    tx.commit()?;
    for (project, date) in affected {
        let export = || -> Result<()> {
            crate::utils::paths::ensure_project_directories_exist(&project)?;
            let path = crate::utils::paths::get_daily_file_path_for_project(&project, date)?;
            database::export_current_list(date, &project, &path)
        };
        if let Err(error) = export() {
            tracing::warn!(%error, "Sync committed; export will be retried");
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::context;

    #[test]
    fn test_sync_reads_legacy_sqlite_timestamps_without_changing_versions() {
        let root = tempfile::tempdir().unwrap();
        context::with_root(root.path().to_path_buf(), || {
            crate::storage::ensure_installation_ready().unwrap();
            let date = NaiveDate::from_ymd_opt(2026, 9, 8).unwrap();
            let mut list =
                crate::storage::file::load_todo_list_for_project("default", date).unwrap();
            list.add_item("legacy task".into());
            database::save_todo_list_for_project(&list, "default").unwrap();
            let conn = database::get_connection().unwrap();
            conn.execute("UPDATE projects SET created_at='2026-01-22 22:57:24'", [])
                .unwrap();
            conn.execute("UPDATE todos SET created_at='2026-01-22 22:57:24',updated_at='2026-01-22 22:57:24'", []).unwrap();
            let first = snapshot().unwrap();
            let second = snapshot().unwrap();
            assert_eq!(first, second);
            assert_eq!(
                first.projects[0].created_at.to_rfc3339(),
                "2026-01-22T22:57:24+00:00"
            );
            assert_eq!(
                first.tasks[0].resource.as_ref().unwrap().item.modified_at,
                first.projects[0].created_at
            );
        });
    }
}
