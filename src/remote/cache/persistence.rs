use super::State;
use crate::remote::protocol::{HistoryList, SyncSnapshot};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

#[derive(Debug)]
pub(super) struct Store {
    connection: Connection,
}

impl Store {
    pub(super) fn open(root: &Path) -> Result<(Self, Option<State>)> {
        let connection = Connection::open(root.join("sync-cache.db"))?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             CREATE TABLE IF NOT EXISTS cache_state(id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS cache_meta(id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS cache_records(
                 kind TEXT NOT NULL, key TEXT NOT NULL, position INTEGER NOT NULL, value TEXT NOT NULL,
                 PRIMARY KEY(kind,key));",
        )?;
        let mut store = Self { connection };
        let metadata: Option<String> = store
            .connection
            .query_row("SELECT value FROM cache_meta WHERE id=1", [], |r| r.get(0))
            .optional()?;
        if let Some(metadata) = metadata {
            let mut state: State =
                serde_json::from_str(&metadata).context("Could not read remote cache metadata")?;
            let mut statement = store
                .connection
                .prepare("SELECT kind,key,value FROM cache_records ORDER BY position,key")?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                let kind: String = row.get(0)?;
                let key: String = row.get(1)?;
                let value: String = row.get(2)?;
                match kind.as_str() {
                    "entry" => {
                        state
                            .tasks
                            .insert(key.parse()?, serde_json::from_str(&value)?);
                    }
                    "task" => state.snapshot.tasks.push(serde_json::from_str(&value)?),
                    "history" => state.snapshot.history.push(serde_json::from_str(&value)?),
                    _ => anyhow::bail!("Unknown remote cache record kind: {kind}"),
                }
            }
            drop(rows);
            drop(statement);
            return Ok((store, Some(state)));
        }
        let legacy: Option<String> = store
            .connection
            .query_row("SELECT value FROM cache_state WHERE id=1", [], |r| r.get(0))
            .optional()?;
        let state = legacy
            .map(|value| serde_json::from_str::<State>(&value))
            .transpose()
            .context("Could not read remote cache")?;
        if let Some(state) = &state {
            store.save(None, state)?;
        }
        Ok((store, state))
    }

    pub(super) fn save(&mut self, previous: Option<&State>, state: &State) -> Result<()> {
        let metadata = serde_json::json!({
            "snapshot": {
                "cursor": state.snapshot.cursor,
                "version": state.snapshot.version,
                "projects": state.snapshot.projects,
                "dates": state.snapshot.dates,
                "tasks": [],
                "history": [],
            },
            "tasks": {},
            "pending": state.pending,
            "generation": state.generation,
        });
        let transaction = self.connection.transaction()?;
        if previous.is_none() {
            transaction.execute("DELETE FROM cache_records", [])?;
        }
        write_records(
            &transaction,
            "entry",
            previous
                .into_iter()
                .flat_map(|s| &s.tasks)
                .map(|(id, entry)| (id.to_string(), (0, entry)))
                .collect(),
            state
                .tasks
                .iter()
                .map(|(id, entry)| (id.to_string(), (0, entry)))
                .collect(),
        )?;
        write_records(
            &transaction,
            "task",
            previous
                .into_iter()
                .flat_map(|s| s.snapshot.tasks.iter().enumerate())
                .map(|(i, task)| (task.id.to_string(), (i, task)))
                .collect(),
            state
                .snapshot
                .tasks
                .iter()
                .enumerate()
                .map(|(i, task)| (task.id.to_string(), (i, task)))
                .collect(),
        )?;
        write_records(
            &transaction,
            "history",
            previous
                .map(|s| history_records(&s.snapshot))
                .transpose()?
                .unwrap_or_default(),
            history_records(&state.snapshot)?,
        )?;
        transaction.execute("INSERT INTO cache_meta VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET value=excluded.value", [metadata.to_string()])?;
        if previous.is_none() {
            transaction.execute("INSERT INTO cache_state VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET value=excluded.value", [r#"{"format":2}"#])?;
        }
        transaction.commit()?;
        Ok(())
    }
}

fn write_records<T: Serialize + PartialEq>(
    transaction: &Transaction<'_>,
    kind: &str,
    previous: BTreeMap<String, (usize, &T)>,
    next: BTreeMap<String, (usize, &T)>,
) -> Result<()> {
    let mut delete =
        transaction.prepare_cached("DELETE FROM cache_records WHERE kind=?1 AND key=?2")?;
    for key in previous.keys().filter(|key| !next.contains_key(*key)) {
        delete.execute(params![kind, key])?;
    }
    let mut update = transaction.prepare_cached("INSERT INTO cache_records(kind,key,position,value) VALUES(?1,?2,?3,?4) ON CONFLICT(kind,key) DO UPDATE SET position=excluded.position,value=excluded.value")?;
    for (key, (position, value)) in &next {
        if previous.get(key) != Some(&(*position, *value)) {
            update.execute(params![
                kind,
                key,
                i64::try_from(*position)?,
                serde_json::to_string(value)?
            ])?;
        }
    }
    Ok(())
}

fn history_records(snapshot: &SyncSnapshot) -> Result<BTreeMap<String, (usize, &HistoryList)>> {
    snapshot
        .history
        .iter()
        .enumerate()
        .map(|(i, history)| {
            Ok((
                serde_json::to_string(&(&history.project, history.date))?,
                (i, history),
            ))
        })
        .collect()
}
