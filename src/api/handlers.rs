use axum::{
    Json,
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Local;
use uuid::Uuid;

use crate::project::{ProjectRegistry, DEFAULT_PROJECT_NAME};
use crate::storage::file::{load_todo_list_for_project, save_todo_list_for_project};
use crate::todo::TodoItem;

use super::models::{
    CreateTodoRequest, DateQuery, ErrorResponse, ProjectListResponse, ProjectResponse,
    TodoListResponse, TodoResponse, UpdateTodoRequest, parse_state,
};

/// Helper to get project name with validation
fn get_validated_project(project: Option<String>) -> Result<String, axum::response::Response<axum::body::Body>> {
    let project_name = project.unwrap_or_else(|| DEFAULT_PROJECT_NAME.to_string());

    // Validate project exists
    let registry = match ProjectRegistry::load() {
        Ok(r) => r,
        Err(e) => return Err(ErrorResponse::internal(e)),
    };

    if registry.get_by_name(&project_name).is_none() {
        return Err(ErrorResponse::not_found(format!("Project not found: {project_name}")));
    }

    Ok(project_name)
}

pub async fn list_todos(Query(query): Query<DateQuery>) -> impl IntoResponse {
    let date = query.date.unwrap_or_else(|| Local::now().date_naive());
    let project_name = match get_validated_project(query.project) {
        Ok(p) => p,
        Err(e) => return e,
    };

    match load_todo_list_for_project(&project_name, date) {
        Ok(list) => {
            let response = TodoListResponse {
                date: list.date,
                items: list.items.iter().map(TodoResponse::from).collect(),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => ErrorResponse::internal(e),
    }
}

pub async fn create_todo(
    Query(query): Query<DateQuery>,
    Json(req): Json<CreateTodoRequest>,
) -> impl IntoResponse {
    let date = query.date.unwrap_or_else(|| Local::now().date_naive());
    let project_name = match get_validated_project(query.project) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let mut list = match load_todo_list_for_project(&project_name, date) {
        Ok(l) => l,
        Err(e) => return ErrorResponse::internal(e),
    };

    let (indent_level, insert_index) = if let Some(parent_id) = req.parent_id {
        match list.find_insert_position_for_child(parent_id) {
            Some((indent, idx)) => (indent, idx),
            None => return ErrorResponse::bad_request("Parent not found"),
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

    if let Err(e) = save_todo_list_for_project(&list, &project_name) {
        return ErrorResponse::internal(e);
    }

    (StatusCode::CREATED, Json(response)).into_response()
}

pub async fn delete_todo(
    Path(id): Path<Uuid>,
    Query(query): Query<DateQuery>,
) -> impl IntoResponse {
    let date = query.date.unwrap_or_else(|| Local::now().date_naive());
    let project_name = match get_validated_project(query.project) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let mut list = match load_todo_list_for_project(&project_name, date) {
        Ok(l) => l,
        Err(e) => return ErrorResponse::internal(e),
    };

    let Some(idx) = list.items.iter().position(|item| item.id == id) else {
        return ErrorResponse::not_found("Todo not found");
    };

    let (start, end) = match list.get_item_range(idx) {
        Ok(range) => range,
        Err(e) => return ErrorResponse::internal(e),
    };

    list.items.drain(start..end);
    list.recalculate_parent_ids();

    if let Err(e) = save_todo_list_for_project(&list, &project_name) {
        return ErrorResponse::internal(e);
    }

    StatusCode::NO_CONTENT.into_response()
}

pub async fn update_todo(
    Path(id): Path<Uuid>,
    Query(query): Query<DateQuery>,
    Json(req): Json<UpdateTodoRequest>,
) -> impl IntoResponse {
    let date = query.date.unwrap_or_else(|| Local::now().date_naive());
    let project_name = match get_validated_project(query.project) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let mut list = match load_todo_list_for_project(&project_name, date) {
        Ok(l) => l,
        Err(e) => return ErrorResponse::internal(e),
    };

    let Some(item) = list.items.iter_mut().find(|item| item.id == id) else {
        return ErrorResponse::not_found("Todo not found");
    };

    if let Some(content) = req.content {
        item.content = content;
    }

    if let Some(state_str) = req.state {
        match parse_state(&state_str) {
            Some(state) => item.state = state,
            None => {
                return ErrorResponse::bad_request(format!(
                    "Invalid state: {state_str}. Use ' ', 'x', '?', or '!'"
                ));
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

    if let Err(e) = save_todo_list_for_project(&list, &project_name) {
        return ErrorResponse::internal(e);
    }

    (StatusCode::OK, Json(response)).into_response()
}

pub async fn list_projects() -> impl IntoResponse {
    match ProjectRegistry::load() {
        Ok(registry) => {
            let projects: Vec<ProjectResponse> = registry
                .list_sorted()
                .iter()
                .map(|p| ProjectResponse::from(*p))
                .collect();
            let response = ProjectListResponse { projects };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => ErrorResponse::internal(e),
    }
}
