use super::models::{DateQuery, ErrorResponse, ProjectListResponse, ProjectResponse};
use crate::project::ProjectRegistry;
use crate::todo::ops;
use axum::{
    Json,
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

pub fn operation_error(error: ops::OpsError) -> Response {
    tracing::debug!(error = %error, "Mutation rejected");
    let status = match &error {
        ops::OpsError::NotFound { .. } => StatusCode::NOT_FOUND,
        ops::OpsError::Storage { message } if message.starts_with("Conflict:") => {
            StatusCode::CONFLICT
        }
        ops::OpsError::Storage { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    };
    (status, Json(ErrorResponse::new(error.to_string()))).into_response()
}

pub async fn list_todos(Query(query): Query<DateQuery>) -> Response {
    super::web::snapshot(Query(query)).await
}

pub async fn create_todo(
    Query(query): Query<DateQuery>,
    Json(spec): Json<ops::CreateSpec>,
) -> Response {
    tracing::debug!(project = ?query.project, date = ?query.date, payload = ?spec, "Create task");
    let date = query.date.map(|d| d.to_string());
    match ops::create(query.project.as_deref(), date.as_deref(), spec) {
        Ok(item) => (
            StatusCode::CREATED,
            Json(super::models::todo_response(&item)),
        )
            .into_response(),
        Err(error) => operation_error(error),
    }
}

pub async fn update_todo(
    Path(id): Path<Uuid>,
    Query(query): Query<DateQuery>,
    Json(spec): Json<ops::UpdateSpec>,
) -> Response {
    tracing::debug!(%id, project = ?query.project, date = ?query.date, payload = ?spec, "Update task");
    let date = query.date.map(|d| d.to_string());
    match ops::update(
        query.project.as_deref(),
        date.as_deref(),
        &id.to_string(),
        spec,
    ) {
        Ok(item) => Json(super::models::todo_response(&item)).into_response(),
        Err(error) => operation_error(error),
    }
}

pub async fn delete_todo(Path(id): Path<Uuid>, Query(query): Query<DateQuery>) -> Response {
    let date = query.date.map(|d| d.to_string());
    match ops::delete_at(
        query.project.as_deref(),
        date.as_deref(),
        &id.to_string(),
        query.revision,
    ) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => operation_error(error),
    }
}

pub async fn list_projects() -> Response {
    match ProjectRegistry::load() {
        Ok(registry) => Json(ProjectListResponse {
            projects: registry
                .list_sorted()
                .iter()
                .map(|p| ProjectResponse::from(*p))
                .collect(),
        })
        .into_response(),
        Err(error) => ErrorResponse::internal(error),
    }
}
