use axum::{
    Json,
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Local;
use uuid::Uuid;

use crate::storage::file::{load_todo_list, save_todo_list};
use crate::todo::TodoItem;

use super::models::{
    CreateTodoRequest, DateQuery, ErrorResponse, TodoListResponse, TodoResponse, UpdateTodoRequest,
    parse_state,
};

pub async fn list_todos(Query(query): Query<DateQuery>) -> impl IntoResponse {
    let date = query.date.unwrap_or_else(|| Local::now().date_naive());

    match load_todo_list(date) {
        Ok(list) => {
            let response = TodoListResponse {
                date: list.date,
                items: list.items.iter().map(TodoResponse::from).collect(),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response(),
    }
}

pub async fn create_todo(
    Query(query): Query<DateQuery>,
    Json(req): Json<CreateTodoRequest>,
) -> impl IntoResponse {
    let date = query.date.unwrap_or_else(|| Local::now().date_naive());

    let mut list = match load_todo_list(date) {
        Ok(l) => l,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            )
                .into_response();
        }
    };

    let (indent_level, insert_index) = if let Some(parent_id) = req.parent_id {
        match list.items.iter().position(|item| item.id == parent_id) {
            Some(parent_idx) => {
                let parent_indent = list.items[parent_idx].indent_level;
                let mut insert_at = parent_idx + 1;
                while insert_at < list.items.len()
                    && list.items[insert_at].indent_level > parent_indent
                {
                    insert_at += 1;
                }
                (parent_indent + 1, insert_at)
            }
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse::new("Parent not found")),
                )
                    .into_response();
            }
        }
    } else {
        (0, list.items.len())
    };

    let mut item = TodoItem::new(req.content, indent_level);
    item.parent_id = req.parent_id;
    item.due_date = req.due_date;
    item.description = req.description;

    let response = TodoResponse::from(&item);
    list.items.insert(insert_index, item);

    if let Err(e) = save_todo_list(&list) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response();
    }

    (StatusCode::CREATED, Json(response)).into_response()
}

pub async fn delete_todo(
    Path(id): Path<Uuid>,
    Query(query): Query<DateQuery>,
) -> impl IntoResponse {
    let date = query.date.unwrap_or_else(|| Local::now().date_naive());

    let mut list = match load_todo_list(date) {
        Ok(l) => l,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            )
                .into_response();
        }
    };

    let Some(idx) = list.items.iter().position(|item| item.id == id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new("Todo not found")),
        )
            .into_response();
    };

    let (start, end) = match list.get_item_range(idx) {
        Ok(range) => range,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            )
                .into_response();
        }
    };

    list.items.drain(start..end);
    list.recalculate_parent_ids();

    if let Err(e) = save_todo_list(&list) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response();
    }

    StatusCode::NO_CONTENT.into_response()
}

pub async fn update_todo(
    Path(id): Path<Uuid>,
    Query(query): Query<DateQuery>,
    Json(req): Json<UpdateTodoRequest>,
) -> impl IntoResponse {
    let date = query.date.unwrap_or_else(|| Local::now().date_naive());

    let mut list = match load_todo_list(date) {
        Ok(l) => l,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            )
                .into_response();
        }
    };

    let Some(item) = list.items.iter_mut().find(|item| item.id == id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new("Todo not found")),
        )
            .into_response();
    };

    if let Some(content) = req.content {
        item.content = content;
    }

    if let Some(state_str) = req.state {
        match parse_state(&state_str) {
            Some(state) => item.state = state,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse::new(format!(
                        "Invalid state: {state_str}. Use ' ', 'x', '?', or '!'"
                    ))),
                )
                    .into_response();
            }
        }
    }

    if let Some(due_date) = req.due_date {
        item.due_date = Some(due_date);
    }

    if let Some(description) = req.description {
        item.description = if description.is_empty() {
            None
        } else {
            Some(description)
        };
    }

    let response = TodoResponse::from(&*item);

    if let Err(e) = save_todo_list(&list) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response();
    }

    (StatusCode::OK, Json(response)).into_response()
}
