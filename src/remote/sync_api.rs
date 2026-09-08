use super::protocol::{Mutation, SyncBatch, TaskResource};
use crate::storage::{context, sync};
use axum::{
    Json,
    extract::{Path, Query},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

pub async fn snapshot() -> Response {
    match context::blocking(sync::snapshot).await {
        Ok(Ok(snapshot)) => Json(snapshot).into_response(),
        Ok(Err(error)) | Err(error) => crate::api::models::ErrorResponse::internal(error),
    }
}

async fn commit(batch: SyncBatch, since: Option<super::protocol::SyncCursor>) -> Response {
    match context::blocking(move || sync::apply(batch)).await {
        Ok(Ok(snapshot)) => {
            if let Some(cursor) = since {
                match context::blocking(move || crate::storage::change_log::changes(cursor)).await {
                    Ok(Ok(changes)) => Json(changes).into_response(),
                    Ok(Err(error)) | Err(error) => {
                        crate::api::models::ErrorResponse::internal(error)
                    }
                }
            } else {
                Json(snapshot).into_response()
            }
        }
        Ok(Err(sync::ApplyError::Precondition)) => (
            StatusCode::PRECONDITION_FAILED,
            "Task changed; fetch its latest ETag before retrying",
        )
            .into_response(),
        Ok(Err(sync::ApplyError::Invalid(error))) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":error.to_string()})),
        )
            .into_response(),
        Err(error) => crate::api::models::ErrorResponse::internal(error),
    }
}

pub async fn apply(
    headers: HeaderMap,
    Query(query): Query<super::events::Since>,
    Json(batch): Json<SyncBatch>,
) -> Response {
    if !headers.contains_key("x-totui-expected-user") {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let since = match query.since.map(|s| s.parse()).transpose() {
        Ok(since) => since,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    commit(batch, since).await
}

pub async fn get_item(Path(id): Path<Uuid>) -> Response {
    match context::blocking(sync::snapshot).await {
        Ok(Ok(snapshot)) => match snapshot.tasks.into_iter().find(|task| task.id == id) {
            Some(task) => {
                let etag = match HeaderValue::from_str(&task.etag) {
                    Ok(tag) => tag,
                    Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                };
                let mut response = (
                    if task.resource.is_some() {
                        StatusCode::OK
                    } else {
                        StatusCode::GONE
                    },
                    Json(task),
                )
                    .into_response();
                response.headers_mut().insert("etag", etag);
                response
            }
            None => StatusCode::NOT_FOUND.into_response(),
        },
        Ok(Err(error)) | Err(error) => crate::api::models::ErrorResponse::internal(error),
    }
}

async fn mutate_item(id: Uuid, headers: HeaderMap, resource: Option<TaskResource>) -> Response {
    let if_match = headers
        .get("if-match")
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned);
    if if_match.is_none()
        && !(resource.is_some() && headers.get("if-none-match").is_some_and(|h| h == "*"))
    {
        return (
            StatusCode::PRECONDITION_REQUIRED,
            "Supply If-Match, or If-None-Match: * for a new task",
        )
            .into_response();
    }
    let request_id = match headers.get("idempotency-key") {
        Some(value) => match value.to_str().ok().and_then(|s| Uuid::parse_str(s).ok()) {
            Some(id) => id,
            None => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => Uuid::new_v4(),
    };
    let result = context::blocking(move || {
        sync::apply(SyncBatch {
            request_id,
            mutations: vec![Mutation {
                id,
                if_match,
                resource,
            }],
        })
    })
    .await;
    match result {
        Ok(Ok(snapshot)) => {
            let Some(task) = snapshot.tasks.into_iter().find(|task| task.id == id) else {
                return StatusCode::NO_CONTENT.into_response();
            };
            let etag = match HeaderValue::from_str(&task.etag) {
                Ok(tag) => tag,
                Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            };
            let mut response = Json(task).into_response();
            response.headers_mut().insert("etag", etag);
            response
        }
        Ok(Err(sync::ApplyError::Precondition)) => StatusCode::PRECONDITION_FAILED.into_response(),
        Ok(Err(sync::ApplyError::Invalid(error))) => {
            (StatusCode::BAD_REQUEST, error.to_string()).into_response()
        }
        Err(error) => crate::api::models::ErrorResponse::internal(error),
    }
}

pub async fn put_item(
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(resource): Json<TaskResource>,
) -> Response {
    mutate_item(id, headers, Some(resource)).await
}
pub async fn delete_item(Path(id): Path<Uuid>, headers: HeaderMap) -> Response {
    mutate_item(id, headers, None).await
}
