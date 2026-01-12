use axum::{Json, body::Body, http::StatusCode, response::{IntoResponse, Response}};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::todo::{TodoItem, TodoState};

#[derive(Debug, Serialize)]
pub struct TodoResponse {
    pub id: Uuid,
    pub content: String,
    pub state: String,
    pub indent_level: usize,
    pub parent_id: Option<Uuid>,
    pub due_date: Option<NaiveDate>,
    pub description: Option<String>,
}

impl From<&TodoItem> for TodoResponse {
    fn from(item: &TodoItem) -> Self {
        Self {
            id: item.id,
            content: item.content.clone(),
            state: item.state.to_char().to_string(),
            indent_level: item.indent_level,
            parent_id: item.parent_id,
            due_date: item.due_date,
            description: item.description.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TodoListResponse {
    pub date: NaiveDate,
    pub items: Vec<TodoResponse>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTodoRequest {
    pub content: String,
    pub parent_id: Option<Uuid>,
    pub due_date: Option<NaiveDate>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTodoRequest {
    pub content: Option<String>,
    pub state: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DateQuery {
    pub date: Option<NaiveDate>,
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
        (StatusCode::INTERNAL_SERVER_ERROR, Json(Self::new(e.to_string()))).into_response()
    }

    pub fn not_found(message: impl Into<String>) -> Response<Body> {
        (StatusCode::NOT_FOUND, Json(Self::new(message))).into_response()
    }

    pub fn bad_request(message: impl Into<String>) -> Response<Body> {
        (StatusCode::BAD_REQUEST, Json(Self::new(message))).into_response()
    }
}

pub fn parse_state(s: &str) -> Option<TodoState> {
    match s.trim() {
        " " | "" => Some(TodoState::Empty),
        "x" | "X" => Some(TodoState::Checked),
        "?" => Some(TodoState::Question),
        "!" => Some(TodoState::Exclamation),
        "*" => Some(TodoState::InProgress),
        _ => None,
    }
}
