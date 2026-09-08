use super::{
    Client,
    protocol::{Mutation, Request, SyncBatch, SyncSnapshot, TaskResource, TaskVersion},
};
use crate::todo::{TodoItem, TodoList};
use anyhow::{Context, Result, ensure};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Conflict {
    pub id: Uuid,
    pub base: Option<TaskResource>,
    pub local: Option<TaskResource>,
    pub server: Option<TaskVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    server: Option<TaskVersion>,
    local: Option<TaskResource>,
    conflict: Option<Conflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct State {
    snapshot: SyncSnapshot,
    tasks: BTreeMap<Uuid, Entry>,
    pending: Option<SyncBatch>,
    generation: i64,
}

#[derive(Debug, Clone, Default)]
pub struct SyncStatus {
    pub pending: usize,
    pub conflicts: usize,
    pub error: Option<String>,
}

struct Data {
    state: State,
    views: BTreeMap<(String, NaiveDate, i64), Vec<TodoItem>>,
    error: Option<String>,
    stream_error: Option<String>,
}

pub struct Cache {
    _lease: Mutex<rusqlite::Connection>,
    path: PathBuf,
    data: Mutex<Data>,
    wake: Condvar,
}

fn equivalent(a: &Option<TaskResource>, b: &Option<TaskResource>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            let mut a = a.clone();
            a.item.modified_at = b.item.modified_at;
            a.item.completed_at = b.item.completed_at;
            a == *b
        }
        _ => a == b,
    }
}

fn merge(
    base: &Option<TaskResource>,
    local: &Option<TaskResource>,
    server: &Option<TaskResource>,
) -> Option<Option<TaskResource>> {
    if equivalent(local, base) {
        return Some(server.clone());
    }
    if equivalent(server, base) || equivalent(local, server) {
        return Some(local.clone());
    }
    None
}

impl Cache {
    pub fn exists(root: &Path) -> bool {
        root.join("sync-cache.db").exists()
    }

    pub fn open(root: &Path, initial: impl FnOnce() -> Result<SyncSnapshot>) -> Result<Arc<Self>> {
        std::fs::create_dir_all(root)?;
        let lease = rusqlite::Connection::open(root.join("sync-cache-lock.db"))?;
        lease.busy_timeout(Duration::ZERO)?;
        lease
            .execute_batch("BEGIN EXCLUSIVE")
            .context("This remote workspace is already open in another TUI")?;
        let path = root.join("sync-cache.db");
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch("CREATE TABLE IF NOT EXISTS cache_state(id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL)")?;
        use rusqlite::OptionalExtension;
        let saved: Option<String> = conn
            .query_row("SELECT value FROM cache_state WHERE id=1", [], |r| r.get(0))
            .optional()?;
        let state = if let Some(saved) = saved {
            serde_json::from_str(&saved).context("Could not read remote cache")?
        } else {
            let snapshot = initial()?;
            State {
                tasks: snapshot
                    .tasks
                    .iter()
                    .map(|task| {
                        (
                            task.id,
                            Entry {
                                server: Some(task.clone()),
                                local: task.resource.clone(),
                                conflict: None,
                            },
                        )
                    })
                    .collect(),
                snapshot,
                pending: None,
                generation: 1,
            }
        };
        let cache = Arc::new(Self {
            _lease: Mutex::new(lease),
            path,
            data: Mutex::new(Data {
                state,
                views: BTreeMap::new(),
                error: None,
                stream_error: None,
            }),
            wake: Condvar::new(),
        });
        cache.persist(&cache.lock()?.state)?;
        Ok(cache)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Data>> {
        self.data
            .lock()
            .map_err(|_| anyhow::anyhow!("Remote cache lock failed"))
    }

    fn persist(&self, state: &State) -> Result<()> {
        let conn = rusqlite::Connection::open(&self.path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute("INSERT INTO cache_state VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET value=excluded.value", [serde_json::to_string(state)?])?;
        Ok(())
    }

    pub fn start(self: &Arc<Self>, client: Client) {
        self.start_events(client.clone());
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || {
            while let Some(cache) = weak.upgrade() {
                if Arc::strong_count(&cache) == 1 {
                    break;
                }
                let result = cache.cycle(&client);
                if let Ok(mut data) = cache.lock() {
                    data.error = result.err().map(|e| format!("{e:#}"));
                }
                if let Ok(data) = cache.lock() {
                    let _wait = cache.wake.wait_timeout(data, Duration::from_secs(3));
                }
                drop(cache);
            }
        });
    }

    fn start_events(self: &Arc<Self>, client: Client) {
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || {
            let mut failures: u32 = 0;
            loop {
                let Some(cache) = weak.upgrade() else { break };
                if Arc::strong_count(&cache) == 1 {
                    break;
                }
                let cursor = cache
                    .lock()
                    .ok()
                    .and_then(|d| d.state.snapshot.cursor.clone());
                drop(cache);
                let result = if let Some(cursor) = cursor {
                    client.event_stream(&cursor, |changes| {
                        let cache = weak.upgrade().context("Cache closed")?;
                        cache.receive_changes(changes, &client)
                    })
                } else {
                    weak.upgrade().context("Cache closed").and_then(|cache| {
                        cache.receive_changes(super::protocol::SyncChanges::Reset, &client)
                    })
                };
                let waiting = result
                    .as_ref()
                    .err()
                    .is_some_and(|e| e.downcast_ref::<UploadPending>().is_some());
                failures = if result.is_ok() || waiting {
                    0
                } else {
                    (failures + 1).min(5)
                };
                if let Some(cache) = weak.upgrade() {
                    if let Ok(mut data) = cache.lock()
                        && !waiting
                    {
                        data.stream_error = result.err().map(|e| format!("{e:#}"));
                    }
                } else {
                    break;
                }
                std::thread::sleep(if waiting {
                    Duration::from_millis(100)
                } else {
                    Duration::from_millis(
                        (1u64 << failures) * 500 + (Uuid::new_v4().as_u128() % 250) as u64,
                    )
                });
            }
        });
    }

    fn receive_changes(
        &self,
        changes: super::protocol::SyncChanges,
        client: &Client,
    ) -> Result<()> {
        use super::protocol::SyncChanges;
        let reset = if matches!(changes, SyncChanges::Reset) {
            Some(client.sync_snapshot()?)
        } else {
            None
        };
        let mut data = self.lock()?;
        if data.state.pending.is_some() {
            return Err(UploadPending.into());
        }
        let mut snapshot = data.state.snapshot.clone();
        match changes {
            SyncChanges::Delta(mut delta) => {
                let current = snapshot.cursor.as_ref().context("Missing sync cursor")?;
                ensure!(current.epoch == delta.cursor.epoch, "Sync epoch changed");
                if delta.cursor.sequence <= current.sequence {
                    data.stream_error = None;
                    return Ok(());
                }
                ensure!(
                    delta.from.sequence <= current.sequence,
                    "Sync gap; reconnecting"
                );
                delta.from = current.clone();
                snapshot.apply_delta(delta)?;
            }
            SyncChanges::Reset => {
                snapshot = reset.context("Missing reset snapshot")?;
                ensure!(
                    snapshot.cursor.is_some(),
                    "Upgrade the remote server to support SSE synchronization"
                );
            }
        }
        let mut next = data.state.clone();
        if next.snapshot.cursor.as_ref().map(|c| c.epoch)
            != snapshot.cursor.as_ref().map(|c| c.epoch)
            || snapshot.version < next.snapshot.version
        {
            next.snapshot.version = 0;
        }
        reconcile(&mut next, snapshot, None);
        self.persist(&next)?;
        data.state = next;
        data.stream_error = None;
        Ok(())
    }

    fn update_snapshot(&self, client: &Client, batch: Option<&SyncBatch>) -> Result<SyncSnapshot> {
        use super::protocol::SyncChanges;
        let mut snapshot = self.lock()?.state.snapshot.clone();
        let Some(cursor) = &snapshot.cursor else {
            return if let Some(batch) = batch {
                client.sync_apply(batch)
            } else {
                client.sync_snapshot()
            };
        };
        let changes = if let Some(batch) = batch {
            client.sync_apply_changes(batch, cursor)?
        } else {
            client.sync_changes(cursor)?
        };
        match changes {
            SyncChanges::Delta(delta) => {
                snapshot.apply_delta(delta)?;
                Ok(snapshot)
            }
            SyncChanges::Reset => client.sync_snapshot(),
        }
    }

    pub fn generation(&self) -> Result<i64> {
        Ok(self.lock()?.state.generation)
    }

    pub fn status(&self) -> SyncStatus {
        match self.lock() {
            Ok(data) => SyncStatus {
                pending: data
                    .state
                    .tasks
                    .values()
                    .filter(|entry| {
                        !equivalent(
                            &entry.local,
                            &entry.server.as_ref().and_then(|s| s.resource.clone()),
                        )
                    })
                    .count()
                    .max(
                        data.state
                            .pending
                            .as_ref()
                            .map_or(0, |batch| batch.mutations.len()),
                    ),
                conflicts: data
                    .state
                    .tasks
                    .values()
                    .filter(|entry| entry.conflict.is_some())
                    .count(),
                error: data.error.clone().or_else(|| data.stream_error.clone()),
            },
            Err(error) => SyncStatus {
                error: Some(error.to_string()),
                ..Default::default()
            },
        }
    }

    pub fn conflict(&self) -> Option<Conflict> {
        self.lock()
            .ok()?
            .state
            .tasks
            .values()
            .find_map(|entry| entry.conflict.clone())
    }

    pub fn resolve(&self, conflict: &Conflict, resource: Option<TaskResource>) -> Result<()> {
        let mut data = self.lock()?;
        let mut next = data.state.clone();
        let entry = next
            .tasks
            .get_mut(&conflict.id)
            .context("Conflict no longer exists")?;
        ensure!(
            entry.conflict.as_ref() == Some(conflict),
            "The task changed again; review the updated conflict"
        );
        ensure!(
            resource.as_ref().is_none_or(|r| r.item.id == conflict.id),
            "Task ID cannot change"
        );
        entry.local = resource;
        entry.conflict = None;
        next.generation += 1;
        self.persist(&next)?;
        data.state = next;
        self.wake.notify_one();
        Ok(())
    }

    pub fn load(
        &self,
        project: &str,
        date: NaiveDate,
        history: bool,
        path: PathBuf,
    ) -> Result<TodoList> {
        let mut data = self.lock()?;
        ensure!(
            data.state
                .snapshot
                .projects
                .iter()
                .any(|p| p.name == project),
            "Project does not exist in the cache"
        );
        let mut items = data
            .state
            .tasks
            .values()
            .filter_map(|e| e.local.as_ref())
            .filter(|r| r.project == project && r.date == date)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by_key(|r| (r.position, r.item.id));
        let mut items = items.into_iter().map(|r| r.item).collect::<Vec<_>>();
        if history
            && let Some(archived) = data
                .state
                .snapshot
                .history
                .iter()
                .find(|h| h.project == project && h.date == date)
        {
            items = archived.items.clone();
        }
        let revision = data.state.generation;
        data.views
            .insert((project.into(), date, revision), items.clone());
        let list = TodoList::with_items(date, path, items);
        list.revision.set(revision);
        Ok(list)
    }

    pub fn save(&self, lists: &[(&TodoList, &str)]) -> Result<()> {
        let mut data = self.lock()?;
        let mut next = data.state.clone();
        let mut changes = BTreeMap::<Uuid, (Option<TaskResource>, Option<TaskResource>)>::new();
        for (list, project) in lists {
            let view = data
                .views
                .get(&(project.to_string(), list.date, list.revision.get()))
                .context("Cached view expired; reload before saving")?;
            for (position, item) in view.iter().enumerate() {
                changes.entry(item.id).or_default().0 = Some(TaskResource {
                    project: project.to_string(),
                    date: list.date,
                    position,
                    item: item.clone(),
                });
            }
            for (position, item) in list.items.iter().enumerate() {
                changes.entry(item.id).or_default().1 = Some(TaskResource {
                    project: project.to_string(),
                    date: list.date,
                    position,
                    item: item.clone(),
                });
            }
        }
        for (id, (base, local)) in changes {
            if equivalent(&base, &local) {
                continue;
            }
            let entry = next.tasks.entry(id).or_insert(Entry {
                server: None,
                local: None,
                conflict: None,
            });
            match merge(&base, &local, &entry.local) {
                Some(merged) => entry.local = merged,
                None => {
                    entry.conflict = Some(Conflict {
                        id,
                        base,
                        local: local.clone(),
                        server: entry.server.clone(),
                    });
                    entry.local = local;
                }
            }
        }
        for entry in next.tasks.values_mut() {
            if let Some(conflict) = &mut entry.conflict {
                conflict.local = entry.local.clone();
            }
        }
        next.generation += 1;
        let revision = next.generation;
        for (list, project) in lists {
            let mut visible: Vec<_> = next
                .tasks
                .values()
                .filter_map(|e| e.local.as_ref())
                .filter(|r| r.project == *project && r.date == list.date)
                .collect();
            visible.sort_by_key(|r| (r.position, r.item.id));
            if visible.iter().map(|r| &r.item).ne(list.items.iter()) {
                next.generation = revision + 1;
            }
        }
        for (list, project) in lists {
            if !next
                .snapshot
                .dates
                .contains(&(project.to_string(), list.date))
            {
                next.snapshot.dates.push((project.to_string(), list.date));
            }
        }
        self.persist(&next)?;
        data.state = next;
        for (list, project) in lists {
            data.views.insert(
                (project.to_string(), list.date, revision),
                list.items.clone(),
            );
            list.revision.set(revision);
        }
        self.wake.notify_one();
        Ok(())
    }

    pub fn call(&self, request: &Request) -> Result<Option<serde_json::Value>> {
        let data = self.lock()?;
        match request {
            Request::Projects => Ok(Some(serde_json::to_value(&data.state.snapshot.projects)?)),
            Request::Exists { project, date } => Ok(Some(serde_json::json!(
                data.state
                    .snapshot
                    .dates
                    .contains(&(project.clone(), *date))
            ))),
            Request::Candidates { project } => {
                let today = chrono::Local::now().date_naive();
                let candidate = if data
                    .state
                    .snapshot
                    .dates
                    .contains(&(project.clone(), today))
                {
                    None
                } else {
                    data.state
                        .snapshot
                        .dates
                        .iter()
                        .filter(|(p, date)| p == project && *date < today)
                        .map(|(_, date)| *date)
                        .max()
                };
                let result = candidate.and_then(|date| {
                    let mut resources = data
                        .state
                        .tasks
                        .values()
                        .filter_map(|e| e.local.as_ref())
                        .filter(|r| r.project == *project && r.date == date)
                        .cloned()
                        .collect::<Vec<_>>();
                    resources.sort_by_key(|r| (r.position, r.item.id));
                    let list = TodoList::with_items(
                        date,
                        PathBuf::new(),
                        resources.into_iter().map(|r| r.item).collect(),
                    );
                    let items = list.get_incomplete_items();
                    (!items.is_empty()).then_some((date, items))
                });
                Ok(Some(serde_json::to_value(result)?))
            }
            _ => Ok(None),
        }
    }

    pub fn refresh_changes(&self, client: &Client) -> Result<()> {
        self.refresh(self.update_snapshot(client, None)?)
    }

    pub fn refresh(&self, snapshot: SyncSnapshot) -> Result<()> {
        let mut data = self.lock()?;
        let mut next = data.state.clone();
        reconcile(&mut next, snapshot, None);
        self.persist(&next)?;
        data.state = next;
        Ok(())
    }

    fn cycle(&self, client: &Client) -> Result<()> {
        let batch = {
            let mut data = self.lock()?;
            if data.state.pending.is_none() {
                let mutations = data
                    .state
                    .tasks
                    .iter()
                    .filter(|(_, entry)| {
                        entry.conflict.is_none()
                            && !equivalent(
                                &entry.local,
                                &entry.server.as_ref().and_then(|s| s.resource.clone()),
                            )
                    })
                    .map(|(id, entry)| Mutation {
                        id: *id,
                        if_match: entry.server.as_ref().map(|s| s.etag.clone()),
                        resource: entry.local.clone(),
                    })
                    .collect::<Vec<_>>();
                if !mutations.is_empty() {
                    let mut next = data.state.clone();
                    next.pending = Some(SyncBatch {
                        request_id: Uuid::new_v4(),
                        mutations,
                    });
                    self.persist(&next)?;
                    data.state = next;
                }
            }
            data.state.pending.clone()
        };
        if let Some(batch) = batch {
            match self.update_snapshot(client, Some(&batch)) {
                Ok(snapshot) => {
                    let mut data = self.lock()?;
                    let mut next = data.state.clone();
                    reconcile(&mut next, snapshot, Some(&batch));
                    next.pending = None;
                    self.persist(&next)?;
                    data.state = next;
                }
                Err(error) if error.downcast_ref::<super::PreconditionFailed>().is_some() => {
                    let snapshot = self.update_snapshot(client, None)?;
                    let mut data = self.lock()?;
                    let mut next = data.state.clone();
                    reconcile(&mut next, snapshot, None);
                    if batch.mutations.iter().all(|m| {
                        next.tasks
                            .get(&m.id)
                            .and_then(|e| e.server.as_ref())
                            .map(|s| &s.etag)
                            == m.if_match.as_ref()
                    }) {
                        for m in &batch.mutations {
                            if let Some(entry) = next.tasks.get_mut(&m.id) {
                                entry.conflict = Some(Conflict {
                                    id: m.id,
                                    base: entry.server.as_ref().and_then(|s| s.resource.clone()),
                                    local: entry.local.clone(),
                                    server: entry.server.clone(),
                                });
                            }
                        }
                    }
                    next.pending = None;
                    self.persist(&next)?;
                    data.state = next;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct UploadPending;
impl std::fmt::Display for UploadPending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Waiting for upload acknowledgement")
    }
}
impl std::error::Error for UploadPending {}

fn reconcile(state: &mut State, snapshot: SyncSnapshot, submitted: Option<&SyncBatch>) {
    if snapshot.version < state.snapshot.version {
        return;
    }
    let mut changed = state.snapshot.projects != snapshot.projects
        || state.snapshot.history != snapshot.history
        || state.snapshot.dates != snapshot.dates;
    let remote: BTreeMap<_, _> = snapshot
        .tasks
        .iter()
        .map(|task| (task.id, task.clone()))
        .collect();
    let ids: BTreeSet<_> = state.tasks.keys().chain(remote.keys()).copied().collect();
    for id in ids {
        let server = remote.get(&id).cloned();
        let entry = state.tasks.entry(id).or_insert(Entry {
            server: None,
            local: None,
            conflict: None,
        });
        let base = submitted
            .and_then(|batch| batch.mutations.iter().find(|m| m.id == id))
            .map(|m| m.resource.clone())
            .unwrap_or_else(|| entry.server.as_ref().and_then(|s| s.resource.clone()));
        let incoming = server.as_ref().and_then(|s| s.resource.clone());
        if let Some(conflict) = &entry.conflict {
            entry.conflict = Some(Conflict {
                id,
                base: conflict.base.clone(),
                local: entry.local.clone(),
                server: server.clone(),
            });
        } else if !equivalent(&base, &entry.local)
            && submitted.is_none()
            && entry.server.as_ref().map(|s| &s.etag) != server.as_ref().map(|s| &s.etag)
        {
            entry.conflict = Some(Conflict {
                id,
                base,
                local: entry.local.clone(),
                server: server.clone(),
            });
            changed = true;
        } else {
            match merge(&base, &entry.local, &incoming) {
                Some(merged) => {
                    changed |= entry.local != merged;
                    entry.local = merged;
                }
                None => {
                    entry.conflict = Some(Conflict {
                        id,
                        base,
                        local: entry.local.clone(),
                        server: server.clone(),
                    });
                    changed = true;
                }
            }
        }
        entry.server = server;
    }
    state.snapshot = snapshot;
    if changed {
        state.generation += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;

    fn initial() -> SyncSnapshot {
        let date = chrono::Local::now().date_naive();
        SyncSnapshot {
            cursor: None,
            version: 1,
            projects: vec![Project::default_project()],
            dates: vec![("default".into(), date)],
            history: vec![],
            tasks: (0..2)
                .map(|position| {
                    let item = TodoItem::new(format!("task {position}"), 0);
                    TaskVersion {
                        id: item.id,
                        etag: "v1".into(),
                        resource: Some(TaskResource {
                            item,
                            position,
                            project: "default".into(),
                            date,
                        }),
                    }
                })
                .collect(),
        }
    }

    fn list(cache: &Cache) -> TodoList {
        cache
            .load(
                "default",
                chrono::Local::now().date_naive(),
                false,
                PathBuf::new(),
            )
            .unwrap()
    }

    #[test]
    fn test_offline_edits_and_queue_survive_restart() {
        let root = tempfile::tempdir().unwrap();
        let cache = Cache::open(root.path(), || Ok(initial())).unwrap();
        let mut edited = list(&cache);
        edited.items[0].content = "offline edit".into();
        cache.save(&[(&edited, "default")]).unwrap();
        let unreachable = Client::new(
            super::super::RemoteConfig {
                url: "http://127.0.0.1:1".into(),
                user_id: Some("alice".into()),
            },
            None,
        )
        .unwrap();
        assert!(cache.cycle(&unreachable).is_err());
        let request_id = cache
            .lock()
            .unwrap()
            .state
            .pending
            .as_ref()
            .unwrap()
            .request_id;
        drop(cache);
        let cache = Cache::open(root.path(), || {
            panic!("Warm cache must not require network")
        })
        .unwrap();
        assert_eq!(list(&cache).items[0].content, "offline edit");
        assert_eq!(
            cache
                .lock()
                .unwrap()
                .state
                .pending
                .as_ref()
                .unwrap()
                .request_id,
            request_id
        );
        assert_eq!(cache.status().pending, 1);
    }

    #[test]
    fn test_other_tasks_merge_but_different_fields_on_same_task_conflict() {
        let root = tempfile::tempdir().unwrap();
        let mut server = initial();
        let cache = Cache::open(root.path(), || Ok(server.clone())).unwrap();
        let mut edited = list(&cache);
        edited.items[0].content = "mine".into();
        cache.save(&[(&edited, "default")]).unwrap();
        server.version += 1;
        server.tasks[1].etag = "v2".into();
        server.tasks[1].resource.as_mut().unwrap().item.content = "other task edit".into();
        cache.refresh(server.clone()).unwrap();
        assert!(cache.conflict().is_none());
        assert_eq!(list(&cache).items[1].content, "other task edit");
        server.version += 1;
        server.tasks[0].etag = "v2".into();
        server.tasks[0].resource.as_mut().unwrap().item.description = Some("server notes".into());
        cache.refresh(server.clone()).unwrap();
        let conflict = cache.conflict().unwrap();
        assert_eq!(conflict.local.as_ref().unwrap().item.content, "mine");
        cache.refresh(server.clone()).unwrap();
        assert_eq!(cache.conflict(), Some(conflict.clone()));
        server.version += 1;
        server.tasks[0].etag = "v3".into();
        server.tasks[0].resource = None;
        cache.refresh(server).unwrap();
        assert!(cache.resolve(&conflict, conflict.local.clone()).is_err());
        let updated = cache.conflict().unwrap();
        cache.resolve(&updated, updated.local.clone()).unwrap();
        assert!(cache.conflict().is_none());
        assert_eq!(
            cache.lock().unwrap().state.tasks[&updated.id]
                .server
                .as_ref()
                .unwrap()
                .etag,
            "v3"
        );
    }

    #[test]
    fn test_edits_during_upload_remain_queued_without_false_conflicts() {
        let root = tempfile::tempdir().unwrap();
        let mut server = initial();
        let cache = Cache::open(root.path(), || Ok(server.clone())).unwrap();
        let mut edited = list(&cache);
        edited.items[0].content = "first edit".into();
        cache.save(&[(&edited, "default")]).unwrap();
        let id = edited.items[0].id;
        let first = cache.lock().unwrap().state.tasks[&id].local.clone();
        let batch = SyncBatch {
            request_id: Uuid::new_v4(),
            mutations: vec![Mutation {
                id,
                if_match: Some("v1".into()),
                resource: first.clone(),
            }],
        };
        edited.items[0].content = "second edit".into();
        cache.save(&[(&edited, "default")]).unwrap();
        let generation = cache.generation().unwrap();
        server.version += 1;
        server.tasks[0].etag = "v2".into();
        server.tasks[0].resource = first;
        reconcile(&mut cache.lock().unwrap().state, server, Some(&batch));
        assert!(cache.conflict().is_none());
        assert_eq!(list(&cache).items[0].content, "second edit");
        assert_eq!(cache.generation().unwrap(), generation);
        assert_eq!(cache.status().pending, 1);
    }

    #[test]
    fn test_stale_visible_list_does_not_erase_new_remote_task() {
        let root = tempfile::tempdir().unwrap();
        let mut server = initial();
        let cache = Cache::open(root.path(), || Ok(server.clone())).unwrap();
        let mut edited = list(&cache);
        let item = TodoItem::new("new on server".into(), 0);
        server.tasks.push(TaskVersion {
            id: item.id,
            etag: "v1".into(),
            resource: Some(TaskResource {
                item,
                project: "default".into(),
                date: edited.date,
                position: 2,
            }),
        });
        server.version += 1;
        cache.refresh(server).unwrap();
        edited.items[0].content = "mine".into();
        cache.save(&[(&edited, "default")]).unwrap();
        assert_eq!(list(&cache).items.len(), 3);
        assert!(cache.generation().unwrap() > edited.revision.get());
    }

    #[test]
    fn test_second_process_cannot_overwrite_open_cache() {
        let root = tempfile::tempdir().unwrap();
        let cache = Cache::open(root.path(), || Ok(initial())).unwrap();
        assert!(Cache::open(root.path(), || Ok(initial())).is_err());
        drop(cache);
        assert!(Cache::open(root.path(), || panic!()).is_ok());
    }
    #[test]
    fn test_sync_cycle_conflict_resolution_and_lost_response_retry() {
        let server = super::super::tests::Server::start();
        let client = server.client("session=alice");
        let root = tempfile::tempdir().unwrap();
        let cache = Cache::open(root.path(), || client.sync_snapshot()).unwrap();
        let mut edited = list(&cache);
        edited.add_item("new offline".into());
        cache.save(&[(&edited, "default")]).unwrap();
        cache.cycle(&client).unwrap();
        assert_eq!(cache.status().pending, 0);
        let mut edited = list(&cache);
        let mut other = client
            .load("default", edited.date, false, PathBuf::new())
            .unwrap();
        edited.items[0].content = "mine".into();
        cache.save(&[(&edited, "default")]).unwrap();
        other.items[0].description = Some("remote change".into());
        client.save(&[(&other, "default")]).unwrap();
        cache.cycle(&client).unwrap();
        let conflict = cache.conflict().unwrap();
        cache.resolve(&conflict, conflict.local.clone()).unwrap();
        cache.cycle(&client).unwrap();
        assert_eq!(cache.status().pending, 0);
        assert_eq!(
            client
                .load("default", edited.date, false, PathBuf::new())
                .unwrap()
                .items[0]
                .content,
            "mine"
        );
        let mut edited = list(&cache);
        edited.add_item("create with lost response".into());
        cache.save(&[(&edited, "default")]).unwrap();
        let pending = {
            let mut data = cache.lock().unwrap();
            let id = edited.items[1].id;
            let batch = SyncBatch {
                request_id: Uuid::new_v4(),
                mutations: vec![Mutation {
                    id,
                    if_match: None,
                    resource: data.state.tasks[&id].local.clone(),
                }],
            };
            data.state.pending = Some(batch.clone());
            cache.persist(&data.state).unwrap();
            batch
        };
        client.sync_apply(&pending).unwrap();
        drop(cache);
        let cache = Cache::open(root.path(), || panic!()).unwrap();
        cache.cycle(&client).unwrap();
        assert_eq!(cache.status().pending, 0);
        assert!(cache.conflict().is_none());
        assert_eq!(
            client
                .load("default", edited.date, false, PathBuf::new())
                .unwrap()
                .items
                .len(),
            2
        );
    }
    #[test]
    fn test_stream_before_upload_ack_does_not_create_false_conflict() {
        let server = super::super::tests::Server::start();
        let client = server.client("session=alice");
        let root = tempfile::tempdir().unwrap();
        let cache = Cache::open(root.path(), || client.sync_snapshot()).unwrap();
        let mut edited = list(&cache);
        edited.add_item("mine".into());
        cache.save(&[(&edited, "default")]).unwrap();
        let (batch, cursor) = {
            let mut data = cache.lock().unwrap();
            let id = edited.items[0].id;
            let batch = SyncBatch {
                request_id: Uuid::new_v4(),
                mutations: vec![Mutation {
                    id,
                    if_match: None,
                    resource: data.state.tasks[&id].local.clone(),
                }],
            };
            data.state.pending = Some(batch.clone());
            (batch, data.state.snapshot.cursor.clone().unwrap())
        };
        let pushed = client.sync_apply_changes(&batch, &cursor).unwrap();
        assert!(
            cache
                .receive_changes(pushed.clone(), &client)
                .unwrap_err()
                .downcast_ref::<UploadPending>()
                .is_some()
        );
        cache.cycle(&client).unwrap();
        cache.receive_changes(pushed, &client).unwrap();
        assert_eq!(cache.status().pending, 0);
        assert!(cache.conflict().is_none());
        let persisted = cache.lock().unwrap().state.snapshot.cursor.clone();
        drop(cache);
        let cache = Cache::open(root.path(), || panic!()).unwrap();
        assert_eq!(cache.lock().unwrap().state.snapshot.cursor, persisted);
        let mut edited = list(&cache);
        edited.items[0].content = "local title".into();
        cache.save(&[(&edited, "default")]).unwrap();
        let mut remote = client
            .load("default", edited.date, false, PathBuf::new())
            .unwrap();
        remote.items[0].description = Some("server notes".into());
        client.save(&[(&remote, "default")]).unwrap();
        cache
            .receive_changes(client.sync_changes(&persisted.unwrap()).unwrap(), &client)
            .unwrap();
        assert!(cache.conflict().is_some());
    }
}
