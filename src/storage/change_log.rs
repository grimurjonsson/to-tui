use super::database;
use crate::remote::protocol::{
    HistoryList, SyncChanges, SyncCursor, SyncDelta, TaskResource, TaskVersion,
};
use anyhow::{Context, Result};
use chrono::NaiveDate;
use rusqlite::Connection;
use std::collections::BTreeSet;
use uuid::Uuid;

pub(crate) fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS sync_journal(seq INTEGER PRIMARY KEY AUTOINCREMENT,kind TEXT NOT NULL,id TEXT,project TEXT,date TEXT);
        CREATE TABLE IF NOT EXISTS sync_epoch(id INTEGER PRIMARY KEY CHECK(id=1),value TEXT NOT NULL);
        CREATE TRIGGER IF NOT EXISTS sync_journal_prune AFTER INSERT ON sync_journal BEGIN DELETE FROM sync_journal WHERE seq<=NEW.seq-100000; END;")?;
    conn.execute(
        "INSERT OR IGNORE INTO sync_epoch VALUES(1,?1)",
        [Uuid::new_v4().to_string()],
    )?;
    for (table, kind, date) in [
        ("todos", "task", "date"),
        ("projects", "project", ""),
        ("archived_todos", "history", "original_date"),
        ("list_revisions", "dates", "date"),
    ] {
        for op in ["INSERT", "UPDATE", "DELETE"] {
            let mut statements = String::new();
            for row in if op == "UPDATE" {
                vec!["OLD", "NEW"]
            } else if op == "DELETE" {
                vec!["OLD"]
            } else {
                vec!["NEW"]
            } {
                let id = if kind == "task" {
                    format!("{row}.id")
                } else {
                    "NULL".into()
                };
                let (project, date) = if kind == "history" {
                    (format!("{row}.project"), format!("{row}.{date}"))
                } else {
                    ("NULL".into(), "NULL".into())
                };
                statements.push_str(&format!("INSERT INTO sync_journal(kind,id,project,date) VALUES('{kind}',{id},{project},{date});"));
            }
            let condition = if table == "todos" && op == "UPDATE" {
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
                format!(
                    "WHEN {}",
                    columns
                        .iter()
                        .map(|c| format!("OLD.{c} IS NOT NEW.{c}"))
                        .collect::<Vec<_>>()
                        .join(" OR ")
                )
            } else if table == "list_revisions" && op == "UPDATE" {
                "WHEN OLD.project IS NOT NEW.project OR OLD.date IS NOT NEW.date".into()
            } else {
                String::new()
            };
            conn.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS journal_{table}_{op} AFTER {op} ON {table} {condition} BEGIN {statements} END;"))?;
        }
    }
    Ok(())
}

pub(crate) fn cursor(conn: &Connection) -> Result<SyncCursor> {
    let epoch: String =
        conn.query_row("SELECT value FROM sync_epoch WHERE id=1", [], |r| r.get(0))?;
    let sequence = conn.query_row(
        "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='sync_journal'),0)",
        [],
        |r| r.get(0),
    )?;
    Ok(SyncCursor {
        epoch: epoch.parse()?,
        sequence,
    })
}

pub(crate) fn changes(from: SyncCursor) -> Result<SyncChanges> {
    let mut conn = database::get_connection()?;
    let tx = conn.transaction()?;
    let cursor = cursor(&tx)?;
    if from.epoch != cursor.epoch
        || from.sequence > cursor.sequence
        || from.sequence < (cursor.sequence - 100000).max(0)
    {
        return Ok(SyncChanges::Reset);
    }
    let mut tasks = BTreeSet::new();
    let mut history = BTreeSet::new();
    let mut projects = false;
    let mut dates = false;
    {
        let mut stmt =
            tx.prepare("SELECT kind,id,project,date FROM sync_journal WHERE seq>?1 ORDER BY seq")?;
        for row in stmt.query_map([from.sequence], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })? {
            let (kind, id, project, date) = row?;
            match kind.as_str() {
                "task" => {
                    tasks.insert(id.context("Missing task ID")?.parse::<Uuid>()?);
                }
                "project" => projects = true,
                "dates" => dates = true,
                "history" => {
                    history.insert((
                        project.context("Missing project")?,
                        date.context("Missing date")?,
                    ));
                }
                _ => anyhow::bail!("Unknown change kind"),
            }
        }
    }
    let mut delta = SyncDelta {
        from,
        cursor,
        version: tx.query_row("SELECT revision FROM sync_clock WHERE id=1", [], |r| {
            r.get(0)
        })?,
        tasks: Vec::new(),
        projects: None,
        dates: None,
        history: Vec::new(),
    };
    for id in tasks {
        let revision: i64 = tx.query_row(
            "SELECT revision FROM task_versions WHERE id=?1",
            [id.to_string()],
            |r| r.get(0),
        )?;
        let resource = if let Some(item) = database::load_item_on(&tx, id)? {
            let (project, date, position): (String, String, i64) = tx.query_row(
                "SELECT project,date,position FROM todos WHERE id=?1",
                [id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            Some(TaskResource {
                item,
                project,
                date: NaiveDate::parse_from_str(&date, "%Y-%m-%d")?,
                position: position.try_into()?,
            })
        } else {
            None
        };
        delta.tasks.push(TaskVersion {
            id,
            etag: format!("\"{id}-v{revision}\""),
            resource,
        });
    }
    if projects {
        let mut stmt = tx.prepare("SELECT id,name,created_at FROM projects ORDER BY name")?;
        let mut result = Vec::new();
        for row in stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })? {
            let (id, name, created) = row?;
            result.push(crate::project::Project {
                id: id.parse()?,
                name,
                created_at: database::parse_timestamp(&created)
                    .context("Invalid project timestamp")?,
            });
        }
        delta.projects = Some(result);
    }
    if dates {
        let mut stmt =
            tx.prepare("SELECT project,date FROM list_revisions ORDER BY project,date")?;
        let mut result = Vec::new();
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (project, date) = row?;
            result.push((project, NaiveDate::parse_from_str(&date, "%Y-%m-%d")?));
        }
        delta.dates = Some(result);
    }
    for (project, date) in history {
        let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")?;
        delta.history.push(HistoryList {
            items: database::load_archived_on(&tx, date, &project)?,
            project,
            date,
        });
    }
    tx.commit()?;
    Ok(SyncChanges::Delta(delta))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{context, file, sync};

    #[test]
    fn test_deltas_cover_edits_deletions_projects_and_history() {
        let root = tempfile::tempdir().unwrap();
        context::with_root(root.path().to_path_buf(), || {
            crate::storage::ensure_installation_ready().unwrap();
            let date = chrono::Local::now().date_naive();
            let mut list = file::load_todo_list_for_project("default", date).unwrap();
            list.add_item("first".into());
            list.add_item("second".into());
            database::save_todo_list_for_project(&list, "default").unwrap();
            let mut baseline = sync::snapshot().unwrap();
            list.items[0].content = "edited".into();
            database::save_todo_list_for_project(&list, "default").unwrap();
            let SyncChanges::Delta(delta) = changes(baseline.cursor.clone().unwrap()).unwrap()
            else {
                panic!()
            };
            assert_eq!(delta.tasks.len(), 1);
            assert!(delta.projects.is_none() && delta.history.is_empty() && delta.dates.is_none());
            baseline.apply_delta(delta).unwrap();
            assert_eq!(baseline, sync::snapshot().unwrap());
            let deleted = list.items.remove(1).id;
            database::save_todo_list_for_project(&list, "default").unwrap();
            let SyncChanges::Delta(delta) = changes(baseline.cursor.clone().unwrap()).unwrap()
            else {
                panic!()
            };
            assert_eq!(delta.tasks.len(), 1);
            assert_eq!(delta.tasks[0].id, deleted);
            assert!(delta.tasks[0].resource.is_none());
            baseline.apply_delta(delta).unwrap();
            assert_eq!(baseline, sync::snapshot().unwrap());
            database::create_project(&crate::project::Project::new("work")).unwrap();
            database::rename_project("work", "renamed").unwrap();
            let conn = database::get_connection().unwrap();
            conn.execute("INSERT INTO archived_todos SELECT id,date,?1,content,state,indent_level,parent_id,due_date,description,priority,collapsed,position,created_at,updated_at,completed_at,deleted_at,project FROM todos WHERE id=?2", rusqlite::params![chrono::Utc::now().to_rfc3339(),list.items[0].id.to_string()]).unwrap();
            let SyncChanges::Delta(delta) = changes(baseline.cursor.clone().unwrap()).unwrap()
            else {
                panic!()
            };
            assert!(
                delta
                    .projects
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(|p| p.name == "renamed")
            );
            assert_eq!(delta.history.len(), 1);
            baseline.apply_delta(delta).unwrap();
            assert_eq!(baseline, sync::snapshot().unwrap());
            conn.execute("DELETE FROM archived_todos", []).unwrap();
            database::delete_project("renamed").unwrap();
            let SyncChanges::Delta(delta) = changes(baseline.cursor.clone().unwrap()).unwrap()
            else {
                panic!()
            };
            assert!(delta.history[0].items.is_empty());
            baseline.apply_delta(delta).unwrap();
            assert_eq!(baseline, sync::snapshot().unwrap());
        });
    }

    #[test]
    fn test_expired_foreign_and_future_cursors_require_snapshot() {
        let root = tempfile::tempdir().unwrap();
        context::with_root(root.path().to_path_buf(), || {
            crate::storage::ensure_installation_ready().unwrap();
            let conn = database::get_connection().unwrap();
            let current = cursor(&conn).unwrap();
            let mut foreign = current.clone();
            foreign.epoch = Uuid::new_v4();
            assert!(matches!(changes(foreign).unwrap(), SyncChanges::Reset));
            let mut future = current.clone();
            future.sequence += 1;
            assert!(matches!(changes(future).unwrap(), SyncChanges::Reset));
            conn.execute(
                "UPDATE sqlite_sequence SET seq=200000 WHERE name='sync_journal'",
                [],
            )
            .unwrap();
            assert!(matches!(changes(current).unwrap(), SyncChanges::Reset));
        });
    }
}
