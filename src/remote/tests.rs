use super::*;
use crate::project::Project;
use crate::storage::{context, database, file};
use axum::{
    Router,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use chrono::{Days, Local};
use std::process::Command;

pub(super) struct Server {
    url: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    _root: tempfile::TempDir,
}

impl Server {
    pub(super) fn start() -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().to_path_buf();
        let (sender, receiver) = std::sync::mpsc::channel();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let gateway = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    let gateway_url = format!("http://{}/auth", gateway.local_addr().unwrap());
                    let auth = Router::new().route(
                        "/auth",
                        get(|headers: HeaderMap| async move {
                            match headers.get("cookie").and_then(|v| v.to_str().ok()) {
                                Some("session=alice") => {
                                    (StatusCode::ACCEPTED, [("x-auth-request-user", "alice")])
                                        .into_response()
                                }
                                Some("session=bob") => {
                                    (StatusCode::ACCEPTED, [("x-auth-request-user", "bob")])
                                        .into_response()
                                }
                                _ => StatusCode::UNAUTHORIZED.into_response(),
                            }
                        }),
                    );
                    let gateway_task = tokio::spawn(async move {
                        axum::serve(gateway, auth).await.unwrap();
                    });
                    let router = context::with_root(path, || {
                        crate::api::create_authenticated_router(&gateway_url)
                    })
                    .unwrap();
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    sender
                        .send(format!("http://{}", listener.local_addr().unwrap()))
                        .unwrap();
                    axum::serve(listener, router)
                        .with_graceful_shutdown(async {
                            let _ = stopped.await;
                        })
                        .await
                        .unwrap();
                    gateway_task.abort();
                });
        });
        Self {
            url: receiver.recv().unwrap(),
            stop: Some(stop),
            thread: Some(thread),
            _root: root,
        }
    }

    pub(super) fn client(&self, cookie: &str) -> Client {
        let mut config = RemoteConfig {
            url: self.url.clone(),
            user_id: None,
        };
        let user = Client::new(config.clone(), Some(cookie))
            .unwrap()
            .user()
            .unwrap();
        config.user_id = Some(user.id);
        Client::new(config, Some(cookie)).unwrap()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

#[test]
fn test_remote_auth_isolation_conflicts_and_atomic_move() {
    let server = Server::start();
    let alice = server.client("session=alice");
    let bob = server.client("session=bob");
    alice.check().unwrap();
    let today = Local::now().date_naive();
    let mut first = alice.load("default", today, false, PathBuf::new()).unwrap();
    first.add_item("Private parent".into());
    first.add_item_with_indent("Private child".into(), 1);
    first.items[1].parent_id = Some(first.items[0].id);
    first.items[0].description = Some("Details".into());
    first.items[0].priority = Some(crate::todo::Priority::P1);
    alice.save(&[(&first, "default")]).unwrap();
    assert!(
        bob.load("default", today, false, PathBuf::new())
            .unwrap()
            .items
            .is_empty()
    );
    let loaded = alice.load("default", today, false, PathBuf::new()).unwrap();
    assert_eq!(loaded.items, first.items);
    let stale = loaded.clone();
    first.items[0].content = "Updated parent".into();
    alice.save(&[(&first, "default")]).unwrap();
    assert!(
        alice
            .save(&[(&stale, "default")])
            .unwrap_err()
            .to_string()
            .starts_with("Conflict:")
    );
    let project = Project::new("work");
    alice
        .call::<()>(Request::CreateProject { project })
        .unwrap();
    let mut dest = alice.load("work", today, false, PathBuf::new()).unwrap();
    dest.items = first.items.clone();
    first.items.clear();
    alice.save(&[(&first, "default"), (&dest, "work")]).unwrap();
    assert!(
        alice
            .load("default", today, false, PathBuf::new())
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        alice
            .load("work", today, false, PathBuf::new())
            .unwrap()
            .items
            .len(),
        2
    );
    let expired = Client::new(alice.config.clone(), Some("session=expired")).unwrap();
    assert!(
        expired
            .check()
            .unwrap_err()
            .to_string()
            .contains("login required")
    );
    let switched = Client::new(alice.config.clone(), Some("session=bob")).unwrap();
    assert!(
        switched
            .check()
            .unwrap_err()
            .to_string()
            .starts_with("Conflict:")
    );
    assert!(
        alice
            .load("../escape", today, false, PathBuf::new())
            .is_err()
    );
    assert!(
        alice
            .call::<()>(Request::DeleteProject {
                name: "default".into()
            })
            .is_err()
    );
    alice
        .call::<()>(Request::RenameProject {
            old: "work".into(),
            new: "renamed".into(),
        })
        .unwrap();
    assert_eq!(
        alice
            .load("renamed", today, false, PathBuf::new())
            .unwrap()
            .items
            .len(),
        2
    );
    alice
        .call::<()>(Request::DeleteProject {
            name: "renamed".into(),
        })
        .unwrap();
    alice
        .call::<()>(Request::CreateProject {
            project: Project::new("renamed"),
        })
        .unwrap();
    assert!(
        alice
            .load("renamed", today, false, PathBuf::new())
            .unwrap()
            .items
            .is_empty()
    );
    dest.date = today.checked_sub_days(Days::new(1)).unwrap();
    assert!(alice.save(&[(&dest, "work")]).is_err());
}

#[test]
fn test_remote_adapter_isolated_process() {
    let root = tempfile::tempdir().unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "remote::tests::test_remote_adapter_worker",
            "--nocapture",
        ])
        .env("TOTUI_REMOTE_TEST", "1")
        .env("TOTUI_DATA_DIR", root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.path().join("todos.db").exists());
}

#[test]
fn test_remote_adapter_worker() {
    if std::env::var("TOTUI_REMOTE_TEST").as_deref() != Ok("1") {
        return;
    }
    let server = Server::start();
    let client = server.client("session=alice");
    activate(client).unwrap();
    let today = Local::now().date_naive();
    let mut list = file::load_todo_list_for_project("default", today).unwrap();
    list.add_item("Through the native TUI storage adapter".into());
    file::save_todo_list_for_project(&list, "default").unwrap();
    assert_eq!(
        file::load_todo_list_for_project("default", today)
            .unwrap()
            .items,
        list.items
    );
    assert_eq!(database::load_projects().unwrap().len(), 1);
    assert!(database::get_connection().is_err());
    let background =
        std::thread::spawn(move || file::load_todo_list_for_project("default", today).unwrap());
    assert_eq!(background.join().unwrap().items, list.items);
    assert!(!crate::utils::paths::get_database_path().unwrap().exists());
}

#[test]
fn test_remote_url_and_credential_validation() {
    for url in [
        "http://example.com",
        "https://user:secret@example.com",
        "https://example.com/path",
        "https://example.com/?token=x",
        "file:///tmp/server",
        "https://example.com/#x",
    ] {
        assert!(validate_url(url).is_err(), "{url}");
    }
    assert_eq!(
        validate_url("https://totui.gimmi.is/").unwrap(),
        "https://totui.gimmi.is"
    );
    assert!(validate_name("../escape").is_err());
    assert!(Client::cookie("session=x\r\nHost: bad").is_err());
    let config = RemoteConfig {
        url: "https://totui.gimmi.is".into(),
        user_id: Some("alice".into()),
    };
    let a = Client::new(config.clone(), None).unwrap();
    let b = Client::new(
        RemoteConfig {
            user_id: Some("bob".into()),
            ..config
        },
        None,
    )
    .unwrap();
    assert_ne!(
        a.workspace_path("/tmp/root".into()),
        b.workspace_path("/tmp/root".into())
    );
}

#[test]
fn test_remote_rollover_and_history() {
    let server = Server::start();
    let client = server.client("session=alice");
    let yesterday = Local::now()
        .date_naive()
        .checked_sub_days(Days::new(1))
        .unwrap();
    let root = server
        ._root
        .path()
        .join("users")
        .join(client.config.user_id.as_ref().unwrap());
    context::with_root(root, || {
        let mut list = file::load_todo_list_for_project("default", yesterday).unwrap();
        list.add_item("Carry forward".into());
        file::save_todo_list_for_project(&list, "default").unwrap();
    });
    let (date, items): (chrono::NaiveDate, Vec<crate::todo::TodoItem>) = client
        .call::<Option<_>>(Request::Candidates {
            project: "default".into(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(date, yesterday);
    let snapshot: Snapshot = client
        .call(Request::Rollover {
            project: "default".into(),
            source_date: date,
            items,
        })
        .unwrap();
    assert_eq!(snapshot.date, Local::now().date_naive());
    assert_eq!(snapshot.items[0].content, "Carry forward");
    let history = client
        .load("default", yesterday, true, PathBuf::new())
        .unwrap();
    assert_eq!(history.items[0].content, "Carry forward");
}

#[test]
fn test_remote_credentials_are_private_and_separate_from_config() {
    let root = tempfile::tempdir().unwrap();
    context::with_root(root.path().to_path_buf(), || {
        save_cookie("home", "session=secret").unwrap();
        let path = credential_path("home").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let mut config = crate::config::Config::default();
        config.remotes.insert(
            "home".into(),
            RemoteConfig {
                url: "https://totui.gimmi.is".into(),
                user_id: None,
            },
        );
        config.save().unwrap();
        assert!(
            !std::fs::read_to_string(crate::utils::paths::get_config_path().unwrap())
                .unwrap()
                .contains("secret")
        );
        save_cookie("home", "session=replaced").unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "session=replaced");
    });
}

fn browser_consent(http: &reqwest::blocking::Client, url: &str) -> String {
    let response = http
        .get(url)
        .header("Cookie", "session=alice")
        .send()
        .unwrap();
    assert_eq!(response.status(), 200);
    let body = response.text().unwrap();
    body.split("name=\"consent\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned()
}

#[test]
fn test_browser_login_pkce_callback_token_and_revocation() {
    use crate::api::client_auth::Exchange;
    let server = Server::start();
    let alice = server.client("session=alice");
    let bob = server.client("session=bob");
    let flow = login::BrowserLogin::new(&alice.config).unwrap();
    let http = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    assert_eq!(http.get(flow.url()).send().unwrap().status(), 401);
    let consent = browser_consent(&http, flow.url());
    let form = format!("consent={consent}&action=approve");
    let endpoint = format!("{}/remote/login", server.url);
    assert_eq!(
        http.post(&endpoint)
            .header("Cookie", "session=bob")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form.clone())
            .send()
            .unwrap()
            .status(),
        400
    );
    let approval = http
        .post(&endpoint)
        .header("Cookie", "session=alice")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form.clone())
        .send()
        .unwrap();
    assert_eq!(approval.status(), 303);
    let callback = approval.headers()["location"].to_str().unwrap().to_owned();
    assert!(callback.starts_with("http://127.0.0.1:"));
    let callback_url = reqwest::Url::parse(&callback).unwrap();
    let code = callback_url
        .query_pairs()
        .find(|(key, _)| key == "code")
        .unwrap()
        .1
        .into_owned();
    let exchange_url = format!("{}/api/remote/login/exchange", server.url);
    assert_eq!(
        http.post(&exchange_url)
            .json(&Exchange {
                code: code.clone(),
                verifier: "wrong".repeat(20)
            })
            .send()
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        http.post(endpoint)
            .header("Cookie", "session=alice")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form)
            .send()
            .unwrap()
            .status(),
        400
    );
    let finish = std::thread::spawn(move || flow.finish());
    let mut bad_callback = callback_url.clone();
    bad_callback.set_query(Some(&format!("state=wrong&code={code}")));
    assert_eq!(http.get(bad_callback).send().unwrap().status(), 400);
    assert_eq!(http.get(callback).send().unwrap().status(), 200);
    let token = finish.join().unwrap().unwrap();
    let native = Client::with_token(alice.config.clone(), &token.access_token).unwrap();
    assert_eq!(native.user().unwrap().id, alice.user().unwrap().id);
    native.check().unwrap();
    assert!(
        Client::with_token(bob.config.clone(), &token.access_token)
            .unwrap()
            .user()
            .is_err()
    );
    assert_eq!(
        http.get(format!("{}/api/todos", server.url))
            .bearer_auth(&token.access_token)
            .send()
            .unwrap()
            .status(),
        401
    );
    let mut list = native
        .load("default", Local::now().date_naive(), false, PathBuf::new())
        .unwrap();
    list.add_item("Browser-authenticated task".into());
    native.save(&[(&list, "default")]).unwrap();
    assert_eq!(
        alice
            .load("default", list.date, false, PathBuf::new())
            .unwrap()
            .items,
        list.items
    );
    let root = server._root.path();
    let conn = rusqlite::Connection::open(root.join("users.db")).unwrap();
    let stored: String = conn
        .query_row("SELECT token_hash FROM client_tokens", [], |r| r.get(0))
        .unwrap();
    assert_ne!(stored, token.access_token);
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM client_grants WHERE code_hash=?1",
            [crate::api::client_auth::challenge(&code)],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    native.revoke().unwrap();
    native.revoke().unwrap();
    assert!(native.user().is_err());
}

#[test]
fn test_browser_login_cancel_and_expired_grant() {
    let server = Server::start();
    let alice = server.client("session=alice");
    let flow = login::BrowserLogin::new(&alice.config).unwrap();
    let http = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let consent = browser_consent(&http, flow.url());
    let endpoint = format!("{}/remote/login", server.url);
    let approval = http
        .post(&endpoint)
        .header("Cookie", "session=alice")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("consent={consent}&action=cancel"))
        .send()
        .unwrap();
    let callback = approval.headers()["location"].to_str().unwrap().to_owned();
    let finish = std::thread::spawn(move || flow.finish());
    assert_eq!(http.get(callback).send().unwrap().status(), 200);
    assert!(
        finish
            .join()
            .unwrap()
            .err()
            .unwrap()
            .to_string()
            .contains("cancelled")
    );
    let flow = login::BrowserLogin::new(&alice.config).unwrap();
    let consent = browser_consent(&http, flow.url());
    let conn = rusqlite::Connection::open(server._root.path().join("users.db")).unwrap();
    conn.execute("UPDATE client_grants SET expires=0", [])
        .unwrap();
    assert_eq!(
        http.post(endpoint)
            .header("Cookie", "session=alice")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("consent={consent}&action=approve"))
            .send()
            .unwrap()
            .status(),
        400
    );
    let invalid = flow.url().replace("port=", "port=0&unused=");
    assert_eq!(
        http.get(invalid)
            .header("Cookie", "session=alice")
            .send()
            .unwrap()
            .status(),
        400
    );
}

#[test]
fn test_task_etags_stale_writes_idempotency_and_unrelated_edits() {
    use super::protocol::{Mutation, SyncBatch, TaskVersion};
    let server = Server::start();
    let client = server.client("session=alice");
    let date = Local::now().date_naive();
    let mut list = client.load("default", date, false, PathBuf::new()).unwrap();
    list.add_item("first".into());
    list.add_item("second".into());
    client.save(&[(&list, "default")]).unwrap();
    let original = client.sync_snapshot().unwrap();
    let first = original
        .tasks
        .iter()
        .find(|t| t.id == list.items[0].id)
        .unwrap();
    let second = original
        .tasks
        .iter()
        .find(|t| t.id == list.items[1].id)
        .unwrap();
    list.items[0].description = Some("web/MCP notes".into());
    client.save(&[(&list, "default")]).unwrap();
    let changed = client.sync_snapshot().unwrap();
    assert_ne!(
        changed
            .tasks
            .iter()
            .find(|t| t.id == first.id)
            .unwrap()
            .etag,
        first.etag
    );
    assert_eq!(
        changed
            .tasks
            .iter()
            .find(|t| t.id == second.id)
            .unwrap()
            .etag,
        second.etag
    );
    let mut resource = first.resource.clone().unwrap();
    resource.item.content = "stale title".into();
    let stale = SyncBatch {
        request_id: uuid::Uuid::new_v4(),
        mutations: vec![Mutation {
            id: first.id,
            if_match: Some(first.etag.clone()),
            resource: Some(resource.clone()),
        }],
    };
    assert!(
        client
            .sync_apply(&stale)
            .unwrap_err()
            .downcast_ref::<PreconditionFailed>()
            .is_some()
    );
    let http = reqwest::blocking::Client::new();
    let endpoint = format!("{}/api/remote/items/{}", server.url, first.id);
    let get = http
        .get(&endpoint)
        .header("Cookie", "session=alice")
        .send()
        .unwrap();
    let tag = get.headers()["etag"].clone();
    let task: TaskVersion = get.json().unwrap();
    assert_eq!(tag.to_str().unwrap(), task.etag);
    assert_eq!(
        http.put(&endpoint)
            .header("Cookie", "session=alice")
            .header("If-Match", &first.etag)
            .json(&resource)
            .send()
            .unwrap()
            .status()
            .as_u16(),
        412
    );
    assert_eq!(
        http.put(&endpoint)
            .header("Cookie", "session=alice")
            .json(&resource)
            .send()
            .unwrap()
            .status()
            .as_u16(),
        428
    );
    let mut resource = second.resource.clone().unwrap();
    resource.item.content = "independent edit".into();
    let mut batch = SyncBatch {
        request_id: uuid::Uuid::new_v4(),
        mutations: vec![Mutation {
            id: second.id,
            if_match: Some(second.etag.clone()),
            resource: Some(resource),
        }],
    };
    let result = client.sync_apply(&batch).unwrap();
    let replay = client.sync_apply(&batch).unwrap();
    assert_eq!(result, replay);
    batch.mutations[0].resource.as_mut().unwrap().item.content = "different body".into();
    assert!(client.sync_apply(&batch).is_err());
    let final_list = client.load("default", date, false, PathBuf::new()).unwrap();
    assert_eq!(
        final_list.items[0].description.as_deref(),
        Some("web/MCP notes")
    );
    assert_eq!(final_list.items[1].content, "independent edit");
}

#[test]
fn test_sync_batch_moves_tree_atomically_and_rejects_stale_member() {
    use super::protocol::{Mutation, SyncBatch};
    let server = Server::start();
    let client = server.client("session=alice");
    client
        .call::<()>(Request::CreateProject {
            project: Project::new("destination"),
        })
        .unwrap();
    let date = Local::now().date_naive();
    let mut list = client.load("default", date, false, PathBuf::new()).unwrap();
    list.add_item("parent".into());
    list.add_item_with_indent("child".into(), 1);
    list.items[1].parent_id = Some(list.items[0].id);
    client.save(&[(&list, "default")]).unwrap();
    let snapshot = client.sync_snapshot().unwrap();
    let mut batch = SyncBatch {
        request_id: uuid::Uuid::new_v4(),
        mutations: snapshot
            .tasks
            .iter()
            .map(|task| {
                let mut resource = task.resource.clone().unwrap();
                resource.project = "destination".into();
                Mutation {
                    id: task.id,
                    if_match: Some(task.etag.clone()),
                    resource: Some(resource),
                }
            })
            .collect(),
    };
    list.items[1].content = "changed child".into();
    client.save(&[(&list, "default")]).unwrap();
    assert!(
        client
            .sync_apply(&batch)
            .unwrap_err()
            .downcast_ref::<PreconditionFailed>()
            .is_some()
    );
    assert!(
        client
            .load("destination", date, false, PathBuf::new())
            .unwrap()
            .items
            .is_empty()
    );
    let latest = client.sync_snapshot().unwrap();
    for mutation in &mut batch.mutations {
        let task = latest.tasks.iter().find(|t| t.id == mutation.id).unwrap();
        mutation.if_match = Some(task.etag.clone());
        mutation.resource.as_mut().unwrap().item = task.resource.as_ref().unwrap().item.clone();
    }
    client.sync_apply(&batch).unwrap();
    assert!(
        client
            .load("default", date, false, PathBuf::new())
            .unwrap()
            .items
            .is_empty()
    );
    let moved = client
        .load("destination", date, false, PathBuf::new())
        .unwrap();
    assert_eq!(moved.items[1].parent_id, Some(moved.items[0].id));
    assert_eq!(moved.items[1].content, "changed child");
}

#[test]
fn test_sse_pushes_only_changed_task_and_resumes_after_disconnect() {
    use super::protocol::{SyncChanges, SyncCursor};
    let server = Server::start();
    let client = server.client("session=alice");
    let date = Local::now().date_naive();
    let mut list = client.load("default", date, false, PathBuf::new()).unwrap();
    list.add_item("first".into());
    list.add_item("second".into());
    client.save(&[(&list, "default")]).unwrap();
    let snapshot = client.sync_snapshot().unwrap();
    let cursor = snapshot.cursor.unwrap();
    let streamed = client.clone();
    let (send, recv) = std::sync::mpsc::channel();
    let task = std::thread::spawn(move || {
        streamed
            .event_stream(&cursor, |change| {
                let SyncChanges::Delta(delta) = change else {
                    panic!()
                };
                let done = !delta.tasks.is_empty();
                send.send(delta).unwrap();
                if done {
                    anyhow::bail!("test complete");
                }
                Ok(())
            })
            .unwrap_err();
    });
    let first = recv.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(first.tasks.is_empty());
    let started = std::time::Instant::now();
    list.items[0].content = "web edit".into();
    client.save(&[(&list, "default")]).unwrap();
    let pushed = recv.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!(pushed.tasks.len(), 1);
    assert_eq!(
        pushed.tasks[0].resource.as_ref().unwrap().item.content,
        "web edit"
    );
    task.join().unwrap();
    list.items.remove(1);
    client.save(&[(&list, "default")]).unwrap();
    let mut resumed = None;
    client
        .event_stream(&pushed.cursor, |change| {
            resumed = Some(change);
            anyhow::bail!("test complete")
        })
        .unwrap_err();
    let SyncChanges::Delta(delta) = resumed.unwrap() else {
        panic!()
    };
    assert_eq!(delta.tasks.len(), 1);
    assert!(delta.tasks[0].resource.is_none());
    let bob = server.client("session=bob");
    assert!(matches!(
        bob.sync_changes(&delta.cursor).unwrap(),
        SyncChanges::Reset
    ));
    let _: SyncCursor = delta.cursor.to_string().parse().unwrap();
}
