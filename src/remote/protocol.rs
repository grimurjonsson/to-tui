use crate::project::Project;
use crate::todo::{TodoItem, TodoList};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub date: NaiveDate,
    pub items: Vec<TodoItem>,
    pub revision: i64,
}

impl From<&TodoList> for Snapshot {
    fn from(list: &TodoList) -> Self {
        Self {
            date: list.date,
            items: list.items.clone(),
            revision: list.revision.get(),
        }
    }
}

impl Snapshot {
    pub fn into_list(self, path: PathBuf) -> TodoList {
        let list = TodoList::with_items(self.date, path, self.items);
        list.revision.set(self.revision);
        list
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum Request {
    Info,
    Load {
        project: String,
        date: NaiveDate,
        history: bool,
    },
    Exists {
        project: String,
        date: NaiveDate,
    },
    Save {
        lists: Vec<(String, Snapshot)>,
    },
    Candidates {
        project: String,
    },
    Rollover {
        project: String,
        source_date: NaiveDate,
        items: Vec<TodoItem>,
    },
    Projects,
    CreateProject {
        project: Project,
    },
    RenameProject {
        old: String,
        new: String,
    },
    DeleteProject {
        name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskResource {
    pub project: String,
    pub date: NaiveDate,
    pub position: usize,
    pub item: TodoItem,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskVersion {
    pub id: uuid::Uuid,
    pub etag: String,
    pub resource: Option<TaskResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryList {
    pub project: String,
    pub date: NaiveDate,
    pub items: Vec<TodoItem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncSnapshot {
    #[serde(default)]
    pub cursor: Option<SyncCursor>,
    pub version: i64,
    pub projects: Vec<Project>,
    pub tasks: Vec<TaskVersion>,
    pub history: Vec<HistoryList>,
    pub dates: Vec<(String, NaiveDate)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mutation {
    pub id: uuid::Uuid,
    pub if_match: Option<String>,
    pub resource: Option<TaskResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncBatch {
    pub request_id: uuid::Uuid,
    pub mutations: Vec<Mutation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncCursor {
    pub epoch: uuid::Uuid,
    pub sequence: i64,
}

impl std::fmt::Display for SyncCursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.epoch, self.sequence)
    }
}

impl std::str::FromStr for SyncCursor {
    type Err = anyhow::Error;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        use anyhow::{Context, ensure};
        let (epoch, sequence) = value.split_once(':').context("Invalid sync cursor")?;
        let sequence = sequence.parse()?;
        ensure!(sequence >= 0, "Invalid sync cursor");
        Ok(Self {
            epoch: epoch.parse()?,
            sequence,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDelta {
    pub from: SyncCursor,
    pub cursor: SyncCursor,
    pub version: i64,
    pub tasks: Vec<TaskVersion>,
    pub projects: Option<Vec<Project>>,
    pub history: Vec<HistoryList>,
    pub dates: Option<Vec<(String, NaiveDate)>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum SyncChanges {
    Delta(SyncDelta),
    Reset,
}

impl SyncSnapshot {
    pub fn apply_delta(&mut self, delta: SyncDelta) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.cursor.as_ref() == Some(&delta.from),
            "Sync cursor gap; reconnect to catch up"
        );
        let mut tasks: std::collections::BTreeMap<_, _> =
            self.tasks.drain(..).map(|t| (t.id, t)).collect();
        for task in delta.tasks {
            tasks.insert(task.id, task);
        }
        self.tasks = tasks.into_values().collect();
        if let Some(projects) = delta.projects {
            self.projects = projects;
        }
        if let Some(dates) = delta.dates {
            self.dates = dates;
        }
        for history in delta.history {
            self.history
                .retain(|h| h.project != history.project || h.date != history.date);
            if !history.items.is_empty() {
                self.history.push(history);
            }
        }
        self.history
            .sort_by(|a, b| (&a.project, a.date).cmp(&(&b.project, b.date)));
        self.version = delta.version;
        self.cursor = Some(delta.cursor);
        Ok(())
    }
}
