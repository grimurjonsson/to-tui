use crate::storage::database;

use anyhow::{Result, ensure};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Backlog,
    Ready,
    InProgress,
    Review,
    Done,
    Blocked,
}

impl Status {
    fn rank(self) -> u8 {
        match self {
            Self::Backlog => 0,
            Self::Ready => 1,
            Self::InProgress | Self::Blocked => 2,
            Self::Review => 3,
            Self::Done => 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Activity {
    pub actor: String,
    pub at: String,
    pub action: String,
    pub body: String,
    pub from: Option<Status>,
    pub to: Option<Status>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Ticket {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: Status,
    pub assignee: Option<String>,
    pub revision: i64,
    #[serde(default)]
    pub trashed: bool,
    #[serde(default)]
    pub archived: bool,
    pub feedback: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub activity: Vec<Activity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Board {
    pub id: String,
    pub name: String,
    pub tickets: Vec<Ticket>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    View,
    CreateBoard {
        name: String,
    },
    CreateTicket {
        title: String,
        description: String,
        assignee: Option<String>,
    },
    EditTicket {
        id: String,
        expected_revision: i64,
        title: String,
        description: String,
        assignee: Option<String>,
    },
    MoveTicket {
        id: String,
        expected_revision: i64,
        status: Status,
        reason: Option<String>,
    },
    Comment {
        id: String,
        expected_revision: i64,
        body: String,
    },
    ArchiveTicket {
        id: String,
        expected_revision: i64,
    },
    UnarchiveTicket {
        id: String,
        expected_revision: i64,
    },
    TrashTicket {
        id: String,
        expected_revision: i64,
    },
    RestoreTicket {
        id: String,
        expected_revision: i64,
    },
    AddressFeedback {
        id: String,
        expected_revision: i64,
        resolution: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Request {
    pub project: String,
    pub actor: String,
    #[serde(flatten)]
    pub action: Action,
}

pub(crate) fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS kanban_boards (
        id TEXT PRIMARY KEY, project_id TEXT NOT NULL UNIQUE, name TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS kanban_tickets (
        id TEXT PRIMARY KEY, board_id TEXT NOT NULL, revision INTEGER NOT NULL, data TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS kanban_tickets_board ON kanban_tickets(board_id);
        CREATE TRIGGER IF NOT EXISTS kanban_project_delete AFTER DELETE ON projects BEGIN
          DELETE FROM kanban_tickets WHERE board_id IN (SELECT id FROM kanban_boards WHERE project_id=OLD.id);
          DELETE FROM kanban_boards WHERE project_id=OLD.id;
        END;")?;
    Ok(())
}

fn required(value: &str, label: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "{label} must not be empty");
    ensure!(value.len() <= 100_000, "{label} exceeds 100000 bytes");
    Ok(())
}

pub fn execute(request: Request) -> Result<Option<Board>> {
    if let Some(client) = crate::remote::active() {
        return client.call(crate::remote::protocol::Request::Kanban { request });
    }
    std::fs::create_dir_all(crate::utils::paths::get_to_tui_dir()?)?;
    database::init_database()?;
    let mut conn = database::get_connection()?;
    if request.project == crate::project::DEFAULT_PROJECT_NAME {
        conn.execute(
            "INSERT OR IGNORE INTO projects(id,name,created_at) VALUES(?1,?2,?3)",
            params![
                Uuid::new_v4().to_string(),
                request.project,
                Utc::now().to_rfc3339()
            ],
        )?;
    }
    execute_on(&mut conn, request)
}

fn execute_on(conn: &mut Connection, request: Request) -> Result<Option<Board>> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let project_id: String = tx
        .query_row(
            "SELECT id FROM projects WHERE name=?1",
            [&request.project],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", request.project))?;
    let mut board = tx
        .query_row(
            "SELECT id,name FROM kanban_boards WHERE project_id=?1",
            [&project_id],
            |row| {
                Ok(Board {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    tickets: Vec::new(),
                })
            },
        )
        .optional()?;
    if !matches!(request.action, Action::View) {
        required(&request.actor, "Actor")?;
    }
    if let Action::CreateBoard { name } = &request.action {
        required(name, "Board name")?;
        ensure!(board.is_none(), "Project already has a board");
        let created = Board {
            id: Uuid::new_v4().to_string(),
            name: name.trim().into(),
            tickets: Vec::new(),
        };
        tx.execute(
            "INSERT INTO kanban_boards(id,project_id,name) VALUES(?1,?2,?3)",
            params![created.id, project_id, created.name],
        )?;
        board = Some(created);
    }
    let Some(mut board) = board else {
        ensure!(
            matches!(request.action, Action::View),
            "Create a board first"
        );
        tx.commit()?;
        return Ok(None);
    };
    let now = Utc::now().to_rfc3339();
    match request.action {
        Action::View | Action::CreateBoard { .. } => {}
        Action::CreateTicket {
            title,
            description,
            assignee,
        } => {
            required(&title, "Title")?;
            let ticket = Ticket {
                id: Uuid::new_v4().to_string(),
                title,
                description,
                status: Status::Backlog,
                assignee,
                revision: 1,
                trashed: false,
                archived: false,
                feedback: None,
                created_at: now.clone(),
                updated_at: now.clone(),
                activity: vec![Activity {
                    actor: request.actor,
                    at: now,
                    action: "created".into(),
                    body: String::new(),
                    from: None,
                    to: Some(Status::Backlog),
                }],
            };
            tx.execute(
                "INSERT INTO kanban_tickets(id,board_id,revision,data) VALUES(?1,?2,?3,?4)",
                params![
                    ticket.id,
                    board.id,
                    ticket.revision,
                    serde_json::to_string(&ticket)?
                ],
            )?;
        }
        action => {
            let (id, expected_revision) = match &action {
                Action::EditTicket {
                    id,
                    expected_revision,
                    ..
                }
                | Action::MoveTicket {
                    id,
                    expected_revision,
                    ..
                }
                | Action::Comment {
                    id,
                    expected_revision,
                    ..
                }
                | Action::ArchiveTicket {
                    id,
                    expected_revision,
                }
                | Action::UnarchiveTicket {
                    id,
                    expected_revision,
                }
                | Action::TrashTicket {
                    id,
                    expected_revision,
                }
                | Action::RestoreTicket {
                    id,
                    expected_revision,
                }
                | Action::AddressFeedback {
                    id,
                    expected_revision,
                    ..
                } => (id, *expected_revision),
                _ => unreachable!(),
            };
            let data: String = tx
                .query_row(
                    "SELECT data FROM kanban_tickets WHERE id=?1 AND board_id=?2",
                    params![id, board.id],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| anyhow::anyhow!("Ticket not found in this board"))?;
            let mut ticket: Ticket = serde_json::from_str(&data)?;
            ensure!(
                ticket.revision == expected_revision,
                "Ticket changed; reload the board and read new feedback before retrying (current revision {})",
                ticket.revision
            );
            ensure!(
                !ticket.trashed || matches!(action, Action::RestoreTicket { .. }),
                "Restore this ticket from Trash before changing it"
            );
            ensure!(
                !ticket.archived || matches!(action, Action::UnarchiveTicket { .. }),
                "Restore this ticket from Completed before changing it"
            );
            let mut event = Activity {
                actor: request.actor,
                at: now.clone(),
                action: String::new(),
                body: String::new(),
                from: None,
                to: None,
            };
            match action {
                Action::ArchiveTicket { .. } => {
                    ensure!(
                        ticket.status == Status::Done,
                        "Only Done tickets can be archived"
                    );
                    ticket.archived = true;
                    event.action = "archived".into();
                }
                Action::UnarchiveTicket { .. } => {
                    ensure!(ticket.archived, "Ticket is not in Completed");
                    ticket.archived = false;
                    event.action = "unarchived".into();
                }
                Action::TrashTicket { .. } => {
                    ticket.trashed = true;
                    event.action = "trashed".into();
                }
                Action::RestoreTicket { .. } => {
                    ensure!(ticket.trashed, "Ticket is not in Trash");
                    ticket.trashed = false;
                    event.action = "restored".into();
                }
                Action::EditTicket {
                    title,
                    description,
                    assignee,
                    ..
                } => {
                    required(&title, "Title")?;
                    ticket.title = title;
                    ticket.description = description;
                    ticket.assignee = assignee;
                    event.action = "edited".into();
                }
                Action::MoveTicket { status, reason, .. } => {
                    ensure!(status != ticket.status, "Ticket is already in that column");
                    let backward =
                        status.rank() < ticket.status.rank() || ticket.status == Status::Done;
                    if backward {
                        required(
                            reason.as_deref().unwrap_or(""),
                            "Reason for moving a ticket back",
                        )?;
                    }
                    ensure!(
                        status != Status::Done || ticket.feedback.is_none(),
                        "Address outstanding feedback before marking done"
                    );
                    if backward {
                        let reason = reason.clone().unwrap_or_default();
                        ticket.feedback = Some(match ticket.feedback.take() {
                            Some(previous) => format!("{previous}\n\n{reason}"),
                            None => reason,
                        });
                    }
                    event.action = "moved".into();
                    event.body = reason.unwrap_or_default();
                    event.from = Some(ticket.status);
                    event.to = Some(status);
                    ticket.status = status;
                }
                Action::Comment { body, .. } => {
                    required(&body, "Comment")?;
                    event.action = "commented".into();
                    event.body = body;
                }
                Action::AddressFeedback { resolution, .. } => {
                    required(&resolution, "Resolution")?;
                    ensure!(
                        ticket.feedback.is_some(),
                        "Ticket has no outstanding feedback"
                    );
                    ticket.feedback = None;
                    event.action = "feedback_addressed".into();
                    event.body = resolution;
                }
                _ => unreachable!(),
            }
            ticket.activity.push(event);
            ticket.revision += 1;
            ticket.updated_at = now;
            tx.execute(
                "UPDATE kanban_tickets SET revision=?1,data=?2 WHERE id=?3 AND board_id=?4",
                params![
                    ticket.revision,
                    serde_json::to_string(&ticket)?,
                    ticket.id,
                    board.id
                ],
            )?;
        }
    }
    {
        let mut stmt =
            tx.prepare("SELECT data FROM kanban_tickets WHERE board_id=?1 ORDER BY rowid")?;
        for data in stmt.query_map([&board.id], |row| row.get::<_, String>(0))? {
            board.tickets.push(serde_json::from_str(&data?)?);
        }
    }
    tx.commit()?;
    Ok(Some(board))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_can_create_board_in_fresh_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("new-workspace");
        crate::storage::context::with_root(root, || {
            let board = execute(Request {
                project: "default".into(),
                actor: "agent".into(),
                action: Action::CreateBoard {
                    name: "Delivery".into(),
                },
            })
            .unwrap()
            .unwrap();
            assert_eq!(board.name, "Delivery");
        });
    }

    #[test]
    fn test_concurrent_agents_cannot_claim_the_same_revision() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("board.db");
        let mut conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE projects(id TEXT PRIMARY KEY, name TEXT); INSERT INTO projects VALUES('project-id','test');").unwrap();
        init(&conn).unwrap();
        let ticket = ticket(&mut conn);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let handles: Vec<_> = (0..8)
            .map(|index| {
                let path = path.clone();
                let barrier = barrier.clone();
                let id = ticket.id.clone();
                std::thread::spawn(move || {
                    let mut conn = Connection::open(path).unwrap();
                    conn.busy_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    barrier.wait();
                    execute_on(
                        &mut conn,
                        Request {
                            project: "test".into(),
                            actor: format!("agent-{index}"),
                            action: Action::MoveTicket {
                                id,
                                expected_revision: 1,
                                status: Status::InProgress,
                                reason: None,
                            },
                        },
                    )
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(
            results
                .iter()
                .filter_map(|r| r.as_ref().err())
                .all(|e| e.to_string().contains("Ticket changed"))
        );
        let board = run(&mut conn, Action::View).unwrap().unwrap();
        assert_eq!(board.tickets[0].revision, 2);
        assert_eq!(board.tickets[0].activity.len(), 2);
    }

    #[test]
    fn test_startup_migrates_legacy_database_without_changing_todos() {
        let dir = tempfile::TempDir::new().unwrap();
        crate::storage::context::with_root(dir.path().to_path_buf(), || {
            let conn = database::get_connection().unwrap();
            conn.execute_batch("CREATE TABLE todos (
                id TEXT PRIMARY KEY, date TEXT NOT NULL, content TEXT NOT NULL,
                state TEXT NOT NULL, indent_level INTEGER NOT NULL, parent_id TEXT,
                due_date TEXT, description TEXT, position INTEGER NOT NULL,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
                INSERT INTO todos VALUES('legacy','2026-09-09','Keep this task',' ',0,NULL,NULL,'Keep details',0,'2026-09-09','2026-09-09');").unwrap();
            database::init_database().unwrap();
            database::ensure_default_project_exists().unwrap();
            execute(Request {
                project: "default".into(),
                actor: "user".into(),
                action: Action::CreateBoard {
                    name: "Delivery".into(),
                },
            })
            .unwrap();
            database::init_database().unwrap();
            let saved: (String, String, String) = conn
                .query_row(
                    "SELECT content,description,project FROM todos WHERE id='legacy'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(
                saved,
                (
                    "Keep this task".into(),
                    "Keep details".into(),
                    "default".into()
                )
            );
            assert!(
                execute(Request {
                    project: "default".into(),
                    actor: String::new(),
                    action: Action::View
                })
                .unwrap()
                .is_some()
            );
        });
    }

    fn fixture() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE projects(id TEXT PRIMARY KEY, name TEXT); INSERT INTO projects VALUES('project-id','test');").unwrap();
        init(&conn).unwrap();
        init(&conn).unwrap();
        conn
    }

    fn run(conn: &mut Connection, action: Action) -> Result<Option<Board>> {
        execute_on(
            conn,
            Request {
                project: "test".into(),
                actor: "agent".into(),
                action,
            },
        )
    }

    fn ticket(conn: &mut Connection) -> Ticket {
        run(
            conn,
            Action::CreateBoard {
                name: "Delivery".into(),
            },
        )
        .unwrap();
        run(
            conn,
            Action::CreateTicket {
                title: "Implement feature".into(),
                description: "Acceptance criteria".into(),
                assignee: Some("agent".into()),
            },
        )
        .unwrap()
        .unwrap()
        .tickets
        .remove(0)
    }

    #[test]
    fn test_trash_preserves_ticket_and_rejects_stale_restore() {
        let mut conn = fixture();
        let original = ticket(&mut conn);
        let trashed = run(
            &mut conn,
            Action::TrashTicket {
                id: original.id.clone(),
                expected_revision: original.revision,
            },
        )
        .unwrap()
        .unwrap()
        .tickets
        .remove(0);
        assert!(trashed.trashed);
        assert_eq!(trashed.description, original.description);
        assert!(
            run(
                &mut conn,
                Action::RestoreTicket {
                    id: original.id.clone(),
                    expected_revision: original.revision
                }
            )
            .is_err()
        );
        assert!(
            run(
                &mut conn,
                Action::Comment {
                    id: original.id.clone(),
                    expected_revision: trashed.revision,
                    body: "Must restore first".into()
                }
            )
            .is_err()
        );
        let restored = run(
            &mut conn,
            Action::RestoreTicket {
                id: original.id,
                expected_revision: trashed.revision,
            },
        )
        .unwrap()
        .unwrap()
        .tickets
        .remove(0);
        assert!(!restored.trashed);
        assert_eq!(restored.status, original.status);
        assert_eq!(restored.activity.last().unwrap().action, "restored");
    }

    #[test]
    fn test_only_done_can_be_archived_and_stale_restore_is_rejected() {
        let mut conn = fixture();
        let original = ticket(&mut conn);
        let archive = |revision| Action::ArchiveTicket {
            id: original.id.clone(),
            expected_revision: revision,
        };
        assert!(run(&mut conn, archive(1)).is_err());
        run(
            &mut conn,
            Action::MoveTicket {
                id: original.id.clone(),
                expected_revision: 1,
                status: Status::Done,
                reason: None,
            },
        )
        .unwrap();
        let archived = run(&mut conn, archive(2))
            .unwrap()
            .unwrap()
            .tickets
            .remove(0);
        assert!(archived.archived);
        assert_eq!(archived.description, original.description);
        assert!(
            run(
                &mut conn,
                Action::UnarchiveTicket {
                    id: original.id.clone(),
                    expected_revision: 2
                }
            )
            .is_err()
        );
        assert!(
            run(
                &mut conn,
                Action::MoveTicket {
                    id: original.id.clone(),
                    expected_revision: 3,
                    status: Status::Ready,
                    reason: Some("Reopen".into())
                }
            )
            .is_err()
        );
        let restored = run(
            &mut conn,
            Action::UnarchiveTicket {
                id: original.id,
                expected_revision: 3,
            },
        )
        .unwrap()
        .unwrap()
        .tickets
        .remove(0);
        assert!(!restored.archived);
        assert_eq!(restored.status, Status::Done);
        assert_eq!(restored.activity.last().unwrap().action, "unarchived");
    }

    #[test]
    fn test_existing_ticket_without_trash_field_loads() {
        let mut conn = fixture();
        let original = ticket(&mut conn);
        let mut data = serde_json::to_value(&original).unwrap();
        data.as_object_mut().unwrap().remove("trashed");
        data.as_object_mut().unwrap().remove("archived");
        conn.execute(
            "UPDATE kanban_tickets SET data=?1 WHERE id=?2",
            params![data.to_string(), original.id],
        )
        .unwrap();
        let board = run(&mut conn, Action::View).unwrap().unwrap();
        assert!(!board.tickets[0].trashed);
        assert!(!board.tickets[0].archived);
    }

    #[test]
    fn test_reopened_ticket_preserves_reason_until_addressed() {
        let mut conn = fixture();
        let ticket = ticket(&mut conn);
        let move_to = |revision, status, reason| Action::MoveTicket {
            id: ticket.id.clone(),
            expected_revision: revision,
            status,
            reason,
        };
        run(&mut conn, move_to(1, Status::Done, None)).unwrap();
        assert!(run(&mut conn, move_to(2, Status::Ready, None)).is_err());
        let board = run(
            &mut conn,
            move_to(2, Status::Ready, Some("Missing keyboard navigation".into())),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            board.tickets[0].feedback.as_deref(),
            Some("Missing keyboard navigation")
        );
        assert!(run(&mut conn, move_to(3, Status::Done, None)).is_err());
        run(
            &mut conn,
            Action::AddressFeedback {
                id: ticket.id.clone(),
                expected_revision: 3,
                resolution: "Added keyboard navigation and verified it".into(),
            },
        )
        .unwrap();
        let board = run(&mut conn, move_to(4, Status::Done, None))
            .unwrap()
            .unwrap();
        assert!(board.tickets[0].feedback.is_none());
        assert_eq!(
            board.tickets[0].activity[2].body,
            "Missing keyboard navigation"
        );
        assert_eq!(board.tickets[0].activity.len(), 5);
    }

    #[test]
    fn test_comment_invalidates_stale_agent_revision() {
        let mut conn = fixture();
        let ticket = ticket(&mut conn);
        run(
            &mut conn,
            Action::Comment {
                id: ticket.id.clone(),
                expected_revision: 1,
                body: "Please cover empty boards".into(),
            },
        )
        .unwrap();
        assert!(
            run(
                &mut conn,
                Action::MoveTicket {
                    id: ticket.id,
                    expected_revision: 1,
                    status: Status::InProgress,
                    reason: None
                }
            )
            .is_err()
        );
        let board = run(&mut conn, Action::View).unwrap().unwrap();
        assert_eq!(board.tickets[0].revision, 2);
        assert_eq!(board.tickets[0].status, Status::Backlog);
        assert_eq!(
            board.tickets[0].activity[1].body,
            "Please cover empty boards"
        );
    }

    #[test]
    fn test_project_rename_and_delete_preserve_then_remove_board() {
        let mut conn = fixture();
        ticket(&mut conn);
        conn.execute("UPDATE projects SET name='renamed'", [])
            .unwrap();
        let board = execute_on(
            &mut conn,
            Request {
                project: "renamed".into(),
                actor: String::new(),
                action: Action::View,
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(board.tickets.len(), 1);
        conn.execute("DELETE FROM projects", []).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM kanban_tickets", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM kanban_boards", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_ticket_cannot_be_changed_from_another_project() {
        let mut conn = fixture();
        let ticket = ticket(&mut conn);
        conn.execute("INSERT INTO projects VALUES('other-id','other')", [])
            .unwrap();
        execute_on(
            &mut conn,
            Request {
                project: "other".into(),
                actor: "user".into(),
                action: Action::CreateBoard {
                    name: "Other".into(),
                },
            },
        )
        .unwrap();
        assert!(
            execute_on(
                &mut conn,
                Request {
                    project: "other".into(),
                    actor: "user".into(),
                    action: Action::Comment {
                        id: ticket.id,
                        expected_revision: 1,
                        body: "Wrong project".into()
                    }
                }
            )
            .is_err()
        );
    }
}
