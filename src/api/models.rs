use axum::{
    Json,
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::project::Project;

#[derive(Debug, Deserialize)]
pub struct DateQuery {
    pub date: Option<NaiveDate>,
    pub project: Option<String>,
    pub revision: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl From<&Project> for ProjectResponse {
    fn from(project: &Project) -> Self {
        Self {
            id: project.id,
            name: project.name.clone(),
            created_at: project.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectResponse>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl ErrorResponse {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            error: message.into(),
        }
    }

    pub fn internal(e: impl std::fmt::Display) -> Response<Body> {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Self::new(e.to_string())),
        )
            .into_response()
    }
}

pub fn todo_response(item: &crate::mcp::schemas::TodoItemResponse) -> serde_json::Value {
    serde_json::json!({"id":item.id,"content":item.content,"state":item.state,"state_description":item.state_description,
        "indent_level":item.indent_level,"parent_id":item.parent_id,"due_date":item.due_date,"description":item.description,"priority":item.priority})
}
