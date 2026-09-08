use super::handlers::operation_error;
use super::models::{DateQuery, ErrorResponse};
use crate::mcp::schemas::TodoItemResponse;
use crate::storage::{database, file};
use crate::todo::ops;
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{
        Html, IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
};
use chrono::Local;
use futures_util::stream;
use serde::Deserialize;
use std::{convert::Infallible, time::Duration};
use tokio::sync::watch;

#[derive(Debug, Clone)]
pub struct LiveState(pub watch::Receiver<u64>);

pub fn observer() -> anyhow::Result<LiveState> {
    std::fs::create_dir_all(crate::utils::paths::get_to_tui_dir()?)?;
    database::init_database()?;
    let conn = database::get_connection()?;
    let mut version: i64 = conn.query_row("PRAGMA data_version", [], |row| row.get(0))?;
    let (sender, receiver) = watch::channel(0u64);
    std::thread::spawn(move || {
        let mut date = Local::now().date_naive();
        let mut sequence = 0u64;
        while !sender.is_closed() {
            std::thread::sleep(Duration::from_millis(100));
            let next = conn.query_row("PRAGMA data_version", [], |row| row.get::<_, i64>(0));
            let today = Local::now().date_naive();
            match next {
                Ok(next) if next != version || today != date => {
                    version = next;
                    date = today;
                    sequence += 1;
                    if sender.send(sequence).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::error!(%error, "Database observer stopped");
                    break;
                }
            }
        }
    });
    Ok(LiveState(receiver))
}

pub async fn events(State(state): State<LiveState>) -> Response {
    let events = stream::unfold((state.0, true), |(mut receiver, first)| async move {
        if !first && receiver.changed().await.is_err() {
            return None;
        }
        let revision = *receiver.borrow_and_update();
        let event = Event::default()
            .event("change")
            .id(revision.to_string())
            .data(Local::now().date_naive().to_string());
        Some((Ok::<_, Infallible>(event), (receiver, false)))
    });
    let mut response = Sse::new(events)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
        .into_response();
    response
        .headers_mut()
        .insert("x-accel-buffering", header::HeaderValue::from_static("no"));
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-cache, no-transform"),
    );
    response
}

pub async fn snapshot(Query(query): Query<DateQuery>) -> Response {
    let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, ops::OpsError> {
        let project = ops::resolve_project(query.project.as_deref())?;
        let today = Local::now().date_naive();
        let date = query.date.unwrap_or(today);
        let list = if date == today { ops::load_list(&project, date)? } else {
            file::load_todos_for_viewing_in_project(&project, date).map_err(|e| ops::OpsError::Storage { message: e.to_string() })?
        };
        Ok(serde_json::json!({"date":list.date,"today":today,"project":project,"revision":list.revision.get(),"read_only":date != today,"items":list.items.iter().map(|item| super::models::todo_response(&TodoItemResponse::from(item))).collect::<Vec<_>>()}))
    }).await;
    match result {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => operation_error(error),
        Err(error) => ErrorResponse::internal(error),
    }
}

#[derive(Debug, Deserialize)]
pub struct MoveRequest {
    pub parent_id: Option<String>,
    pub before_id: Option<String>,
    pub expected_revision: Option<i64>,
}

pub async fn move_todo(
    Path(id): Path<String>,
    Query(query): Query<DateQuery>,
    Json(spec): Json<MoveRequest>,
) -> Response {
    let date = query.date.map(|d| d.to_string());
    tracing::debug!(%id, project = ?query.project, payload = ?spec, "Move task");
    match ops::move_item_at(
        query.project.as_deref(),
        date.as_deref(),
        &id,
        spec.parent_id.as_deref(),
        spec.before_id.as_deref(),
        spec.expected_revision,
    ) {
        Ok(item) => Json(super::models::todo_response(&item)).into_response(),
        Err(error) => operation_error(error),
    }
}

#[derive(Debug, Clone)]
pub struct StartupProject(pub String);

pub async fn index(project: Option<Extension<StartupProject>>) -> Html<String> {
    let project = project.map_or_else(|| "default".to_owned(), |Extension(project)| project.0);
    let escaped = project
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    Html(include_str!("../../web/index.html").replace("__TOTUI_PROJECT__", &escaped))
}
pub async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript")],
        include_str!("../../web/app.js"),
    )
}
pub async fn style() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../../web/style.css"),
    )
}

pub async fn protect_local_writes(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    ) && let Some(origin) = request.headers().get(header::ORIGIN)
    {
        let host = request
            .headers()
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let origin = origin.to_str().unwrap_or("");
        if origin != format!("http://{host}") && origin != format!("https://{host}") {
            return (StatusCode::FORBIDDEN, "Cross-origin writes are disabled").into_response();
        }
    }
    if let Some(expected_today) = request.headers().get("x-totui-today")
        && expected_today.to_str().ok() != Some(Local::now().date_naive().to_string().as_str())
    {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse::new(
                "Conflict: Today changed. Refresh before saving; your draft is preserved.",
            )),
        )
            .into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todo::{TodoItem, TodoList};
    use chrono::{Days, NaiveDate};
    use futures_util::StreamExt;
    use std::process::Command;

    #[test]
    fn test_web_integration_isolated_process() {
        let directory = tempfile::tempdir().unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "api::web::tests::test_web_worker", "--nocapture"])
            .env("TOTUI_DATA_DIR", directory.path())
            .env("TOTUI_WEB_TEST", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn test_external_writer_worker() {
        if std::env::var("TOTUI_EXTERNAL_WRITER").is_err() {
            return;
        }
        ops::create(
            Some("default"),
            None,
            ops::CreateSpec {
                content: "External process".into(),
                ..Default::default()
            },
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_web_worker() {
        if std::env::var("TOTUI_WEB_TEST").is_err() {
            return;
        }
        database::init_database().unwrap();
        database::ensure_default_project_exists().unwrap();
        database::create_project(&crate::project::Project::new("other")).unwrap();
        let router = super::super::create_router().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let client = reqwest::Client::new();
        let mut stream = client
            .get(format!("{address}/api/events"))
            .send()
            .await
            .unwrap()
            .bytes_stream();
        let initial = stream.next().await.unwrap().unwrap();
        assert!(String::from_utf8_lossy(&initial).contains("event: change"));
        let started = std::time::Instant::now();
        let writer = tokio::task::spawn_blocking(|| {
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "api::web::tests::test_external_writer_worker"])
                .env("TOTUI_EXTERNAL_WRITER", "1")
                .output()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(writer.status.success());
        let event = tokio::time::timeout(Duration::from_millis(500), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(String::from_utf8_lossy(&event).contains("event: change"));
        println!("external process to SSE: {:?}", started.elapsed());
        let snapshot: serde_json::Value = client
            .get(format!("{address}/api/snapshot"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(snapshot["items"][0]["content"], "External process");
        let id = snapshot["items"][0]["id"].as_str().unwrap();
        let revision = snapshot["revision"].as_i64().unwrap();
        let stale = serde_json::json!({"content":"First edit", "expected_revision":revision});
        assert_eq!(
            client
                .patch(format!("{address}/api/todos/{id}"))
                .json(&stale)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            client
                .patch(format!("{address}/api/todos/{id}"))
                .json(&stale)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::CONFLICT
        );
        let other: serde_json::Value = client
            .get(format!("{address}/api/snapshot?project=other"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(other["items"].as_array().unwrap().len(), 0);
        drop(stream);
        ops::delete(None, None, id).unwrap();
        let mut reconnect = client
            .get(format!("{address}/api/events"))
            .header("Last-Event-ID", "999999")
            .send()
            .await
            .unwrap()
            .bytes_stream();
        let reset = reconnect.next().await.unwrap().unwrap();
        assert!(String::from_utf8_lossy(&reset).contains("event: change"));
        let snapshot: serde_json::Value = client
            .get(format!("{address}/api/snapshot"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(snapshot["items"].as_array().unwrap().is_empty());

        assert_eq!(
            client
                .post(format!("{address}/api/todos"))
                .header("X-Totui-Today", "2000-01-01")
                .json(&serde_json::json!({"content":"Must not create yesterday"}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::CONFLICT
        );
        let date = Local::now().date_naive();
        let path = crate::utils::paths::get_daily_file_path_for_project("default", date).unwrap();
        let mut first = database::load_list_snapshot(date, "default", path.clone()).unwrap();
        let mut stale_list = first.clone();
        first.add_item("Survives stale save".into());
        database::save_todo_list_for_project(&first, "default").unwrap();
        stale_list.add_item("Stale addition".into());
        assert!(
            database::save_todo_list_for_project(&stale_list, "default")
                .unwrap_err()
                .to_string()
                .starts_with("Conflict:")
        );
        stale_list.items.clear();
        assert!(database::save_todo_list_for_project(&stale_list, "default").is_err());
        let valid_revision = first.revision.get();
        let result = ops::update(
            None,
            None,
            &first.items[0].id.to_string(),
            serde_json::from_value(
                serde_json::json!({"state":"x","priority":"P0","due_date":"2030-01-01"}),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(result.state, "x");
        let list = database::load_list_snapshot(date, "default", path.clone()).unwrap();
        assert!(list.items[0].completed_at.is_some());
        assert!(list.revision.get() > valid_revision);
        ops::update(
            None,
            None,
            &first.items[0].id.to_string(),
            serde_json::from_value(
                serde_json::json!({"state":"-","priority":null,"due_date":null}),
            )
            .unwrap(),
        )
        .unwrap();
        let list = database::load_list_snapshot(date, "default", path.clone()).unwrap();
        assert!(list.items[0].is_complete());
        assert!(list.items[0].priority.is_none());
        assert!(list.items[0].due_date.is_none());
        ops::update(
            None,
            None,
            &first.items[0].id.to_string(),
            serde_json::from_value(serde_json::json!({"state":" "})).unwrap(),
        )
        .unwrap();
        let list = database::load_list_snapshot(date, "default", path).unwrap();
        assert!(list.items[0].completed_at.is_none());

        let old = date.checked_sub_days(Days::new(45)).unwrap();
        let mut source = file::load_todo_list_for_project("other", old).unwrap();
        source.add_item("Carry forward".into());
        file::save_todo_list_for_project(&source, "other").unwrap();
        let rolled = ops::load_list("other", date).unwrap();
        assert_eq!(rolled.items[0].content, "Carry forward");
        assert_ne!(rolled.items[0].id, source.items[0].id);
        assert_eq!(ops::load_list("other", date).unwrap().items.len(), 1);
        let history: serde_json::Value = client
            .get(format!("{address}/api/snapshot?project=other&date={old}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(history["read_only"], true);
        assert_eq!(history["items"][0]["id"], source.items[0].id.to_string());

        let atomic_date = NaiveDate::from_ymd_opt(2031, 1, 1).unwrap();
        let mut batch = TodoList::new(atomic_date, std::path::PathBuf::new());
        batch.items = (0..100)
            .map(|n| TodoItem::new(format!("A{n}"), 0))
            .collect();
        database::save_todo_list_for_project(&batch, "default").unwrap();
        let writer = std::thread::spawn(move || {
            for _ in 0..8 {
                for item in &mut batch.items {
                    item.content = if item.content.starts_with('A') {
                        "B".into()
                    } else {
                        "A".into()
                    };
                }
                database::save_todo_list_for_project(&batch, "default").unwrap();
            }
        });
        while !writer.is_finished() {
            let snapshot =
                database::load_list_snapshot(atomic_date, "default", std::path::PathBuf::new())
                    .unwrap();
            assert_eq!(snapshot.items.len(), 100);
            let prefix = snapshot.items[0].content.chars().next().unwrap();
            assert!(
                snapshot
                    .items
                    .iter()
                    .all(|item| item.content.starts_with(prefix))
            );
        }
        writer.join().unwrap();
        let before =
            database::load_list_snapshot(date, "default", std::path::PathBuf::new()).unwrap();
        let mut changed = before.clone();
        changed.items[0].content = "Must roll back".into();
        let stale_destination = TodoList::new(date, std::path::PathBuf::new());
        assert!(
            database::save_lists_atomically(&[
                (&changed, "default"),
                (&stale_destination, "other")
            ])
            .is_err()
        );
        assert_eq!(
            database::load_list_snapshot(date, "default", std::path::PathBuf::new())
                .unwrap()
                .items,
            before.items
        );
        database::create_project(&crate::project::Project::new("rollover-race")).unwrap();
        let mut source = file::load_todo_list_for_project("rollover-race", old).unwrap();
        source.add_item("Roll once".into());
        file::save_todo_list_for_project(&source, "rollover-race").unwrap();
        let workers: Vec<_> = (0..2)
            .map(|_| std::thread::spawn(move || ops::load_list("rollover-race", date).unwrap()))
            .collect();
        for worker in workers {
            assert_eq!(worker.join().unwrap().items.len(), 1);
        }
        assert_eq!(
            ops::load_list("rollover-race", date).unwrap().items.len(),
            1
        );
        server.abort();
    }
}
