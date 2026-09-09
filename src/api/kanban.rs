use crate::kanban::{Action, Request};
use crate::storage::context;

use axum::{
    Json,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct BoardQuery {
    pub project: String,
}

pub async fn page() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../../web/kanban.html"))
}

pub async fn script() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/javascript")],
        include_str!("../../web/kanban.js"),
    )
}

pub async fn style() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        include_str!("../../web/kanban.css"),
    )
}

pub async fn view(Query(query): Query<BoardQuery>) -> Response {
    respond(Request {
        project: query.project,
        actor: String::new(),
        action: Action::View,
    })
    .await
}

pub async fn mutate(Json(request): Json<Request>) -> Response {
    respond(request).await
}

async fn respond(request: Request) -> Response {
    match context::blocking(move || crate::kanban::execute(request)).await {
        Ok(Ok(board)) => Json(board).into_response(),
        Ok(Err(error)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error.to_string()})),
        )
            .into_response(),
        Err(error) => super::models::ErrorResponse::internal(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database;

    use futures_util::StreamExt;
    use serde_json::{Value, json};
    use std::{process::Command, time::Duration};

    #[test]
    fn test_kanban_http_isolated_process() {
        let directory = tempfile::tempdir().unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "api::kanban::tests::test_kanban_http_worker",
                "--nocapture",
            ])
            .env("TOTUI_DATA_DIR", directory.path())
            .env("TOTUI_KANBAN_HTTP_TEST", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    async fn test_kanban_http_worker() {
        if std::env::var("TOTUI_KANBAN_HTTP_TEST").is_err() {
            return;
        }
        database::init_database().unwrap();
        database::ensure_default_project_exists().unwrap();
        let router = crate::api::create_router().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let client = reqwest::Client::new();
        for asset in ["kanban", "kanban.js", "kanban.css"] {
            assert_eq!(
                client
                    .get(format!("{address}/{asset}"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                StatusCode::OK
            );
        }
        let endpoint = format!("{address}/api/kanban");
        let post = |action: Value| {
            let mut request = json!({"project":"default", "actor":"web-user"});
            request
                .as_object_mut()
                .unwrap()
                .extend(action.as_object().unwrap().clone());
            client.post(&endpoint).json(&request).send()
        };
        assert!(
            client
                .get(format!("{endpoint}?project=default"))
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
                .is_null()
        );
        let created = post(json!({"action":"create_board", "name":"Delivery"}))
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::OK);
        let mut stream = client
            .get(format!("{address}/api/events"))
            .send()
            .await
            .unwrap()
            .bytes_stream();
        stream.next().await.unwrap().unwrap();
        let board: Value = post(json!({"action":"create_ticket", "title":"Implement search", "description":"Cover empty results", "assignee":"agent"})).await.unwrap().json().await.unwrap();
        let event = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(String::from_utf8_lossy(&event).contains("event: change"));
        let id = board["tickets"][0]["id"].as_str().unwrap();
        assert_eq!(
            post(json!({"action":"move_ticket", "id":id, "expected_revision":1, "status":"done"}))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            post(json!({"action":"move_ticket", "id":id, "expected_revision":2, "status":"ready"}))
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        let reopened: Value = post(json!({"action":"move_ticket", "id":id, "expected_revision":2, "status":"ready", "reason":"Empty search fails"})).await.unwrap().json().await.unwrap();
        assert_eq!(reopened["tickets"][0]["feedback"], "Empty search fails");
        assert_eq!(
            post(
                json!({"action":"comment", "id":id, "expected_revision":2, "body":"Stale comment"})
            )
            .await
            .unwrap()
            .status(),
            StatusCode::BAD_REQUEST
        );
        let commented: Value = post(json!({"action":"comment", "id":id, "expected_revision":3, "body":"Please add a regression test"})).await.unwrap().json().await.unwrap();
        assert_eq!(
            commented["tickets"][0]["activity"][3]["body"],
            "Please add a regression test"
        );
        assert_eq!(
            client
                .post(&endpoint)
                .header("Origin", "https://untrusted.example")
                .json(&json!({"project":"default", "actor":"user", "action":"view"}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        let discovered: Value = client.post(format!("{address}/api/remote/v1")).header("x-totui-expected-user", "local").json(&json!({"operation":"kanban", "request":{"project":"default", "actor":"agent", "action":"view"}})).send().await.unwrap().json().await.unwrap();
        assert_eq!(discovered["tickets"][0]["feedback"], "Empty search fails");
        server.abort();
    }
}
