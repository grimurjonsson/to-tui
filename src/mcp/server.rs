use chrono::Local;
use rmcp::{
    Json,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use tracing::{debug, error, info, warn};

use crate::project::{ProjectRegistry, DEFAULT_PROJECT_NAME};
use crate::storage::database::soft_delete_todos;
use crate::storage::file::{file_exists_for_project, load_todo_list_for_project, save_todo_list_for_project};
use crate::storage::rollover::create_rolled_over_list_for_project;
use crate::todo::{TodoItem, TodoList};

use super::errors::{IntoMcpError, McpErrorDetail};
use super::schemas::{
    CreateTodoRequest, DeleteTodoRequest, DeleteTodoResponse, ListProjectsRequest, ListTodosRequest,
    MarkCompleteRequest, ProjectItemResponse, ProjectListResponse, TodoItemResponse, TodoListResponse,
    UpdateTodoRequest, parse_date, parse_state, parse_uuid,
};

#[derive(Clone)]
pub struct TodoMcpServer {
    tool_router: ToolRouter<Self>,
}

impl TodoMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for TodoMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

fn load_list_with_rollover(project: &str, date: chrono::NaiveDate) -> Result<TodoList, McpErrorDetail> {
    let today = Local::now().date_naive();

    if date == today && !file_exists_for_project(project, date).into_mcp_storage_error()? {
        debug!(date = %date, project = %project, "No todos for today, checking for rollover candidates");
        for days_back in 1..=30 {
            if let Some(check_date) = today.checked_sub_days(chrono::Days::new(days_back))
                && file_exists_for_project(project, check_date).into_mcp_storage_error()? {
                    let list = load_todo_list_for_project(project, check_date).into_mcp_storage_error()?;
                    let incomplete = list.get_incomplete_items();

                    if !incomplete.is_empty() {
                        info!(
                            from_date = %check_date,
                            to_date = %today,
                            project = %project,
                            count = incomplete.len(),
                            "Rolling over incomplete todos"
                        );
                        let rolled_list =
                            create_rolled_over_list_for_project(project, today, incomplete).into_mcp_storage_error()?;
                        save_todo_list_for_project(&rolled_list, project).into_mcp_storage_error()?;
                        return Ok(rolled_list);
                    }
                    break;
                }
        }
    }

    load_todo_list_for_project(project, date).into_mcp_storage_error()
}

fn get_validated_project(project: Option<&str>) -> Result<String, McpErrorDetail> {
    let project_name = project.unwrap_or(DEFAULT_PROJECT_NAME).to_string();

    let registry = ProjectRegistry::load().into_mcp_storage_error()?;

    if registry.get_by_name(&project_name).is_none() {
        return Err(McpErrorDetail::not_found(
            format!("Project '{}' not found", project_name),
            "Use list_projects to see available projects",
        ));
    }

    Ok(project_name)
}

fn format_error(detail: McpErrorDetail) -> String {
    if detail.retryable {
        warn!(code = %detail.code, message = %detail.message, "Retryable error occurred");
    } else {
        error!(code = %detail.code, message = %detail.message, "Non-retryable error occurred");
    }
    serde_json::to_string(&detail).unwrap_or(detail.message)
}

fn parse_date_or_err(date_str: Option<&str>) -> Result<chrono::NaiveDate, String> {
    parse_date(date_str).map_err(|msg| {
        format_error(McpErrorDetail::invalid_input(&msg, "Use YYYY-MM-DD format"))
    })
}

fn parse_uuid_or_err(id_str: &str) -> Result<uuid::Uuid, String> {
    parse_uuid(id_str).map_err(|msg| {
        format_error(McpErrorDetail::invalid_input(&msg, "Use list_todos to get valid IDs"))
    })
}

#[tool_router]
impl TodoMcpServer {
    #[tool(
        name = "list_todos",
        description = "List all todos for a specific date and project. Defaults to today and 'default' project. Automatically rolls over incomplete todos from previous days if today's list is empty. Response includes a 'formatted' field - display it directly as markdown to the user."
    )]
    async fn list_todos(
        &self,
        params: Parameters<ListTodosRequest>,
    ) -> Result<Json<TodoListResponse>, String> {
        info!(date = ?params.0.date, project = ?params.0.project, "list_todos called");

        let project = get_validated_project(params.0.project.as_deref()).map_err(format_error)?;
        let date = parse_date_or_err(params.0.date.as_deref())?;

        let list = load_list_with_rollover(&project, date).map_err(format_error)?;

        let items: Vec<TodoItemResponse> = list.items.iter().map(TodoItemResponse::from).collect();
        let response = TodoListResponse::new(list.date.format("%Y-%m-%d").to_string(), items);

        info!(date = %date, project = %project, count = response.item_count, "list_todos returning items");
        Ok(Json(response))
    }

    #[tool(
        name = "list_projects",
        description = "List all available projects. Returns project names, IDs, and creation dates."
    )]
    async fn list_projects(
        &self,
        _params: Parameters<ListProjectsRequest>,
    ) -> Result<Json<ProjectListResponse>, String> {
        info!("list_projects called");

        let registry = ProjectRegistry::load()
            .into_mcp_storage_error()
            .map_err(format_error)?;

        let projects: Vec<ProjectItemResponse> = registry
            .list_sorted()
            .iter()
            .map(|p| ProjectItemResponse::from(*p))
            .collect();

        let count = projects.len();
        let response = ProjectListResponse { count, projects };

        info!(count = count, "list_projects returning projects");
        Ok(Json(response))
    }

    #[tool(
        name = "create_todo",
        description = "Create a new todo item in a project. Optionally nest under a parent todo by providing parent_id."
    )]
    async fn create_todo(
        &self,
        params: Parameters<CreateTodoRequest>,
    ) -> Result<Json<TodoItemResponse>, String> {
        let req = params.0;
        info!(
            content = %req.content,
            date = ?req.date,
            project = ?req.project,
            parent_id = ?req.parent_id,
            "create_todo called"
        );

        if req.content.trim().is_empty() {
            return Err(format_error(McpErrorDetail::validation_error(
                "Content cannot be empty",
                "Provide a non-empty string for the todo content",
            )));
        }

        let project = get_validated_project(req.project.as_deref()).map_err(format_error)?;
        let date = parse_date_or_err(req.date.as_deref())?;

        let mut list = load_list_with_rollover(&project, date).map_err(format_error)?;

        let due_date = req
            .due_date
            .as_deref()
            .map(|s| parse_date_or_err(Some(s)))
            .transpose()?;

        let (indent_level, insert_index) = if let Some(ref parent_id_str) = req.parent_id {
            let parent_id = parse_uuid_or_err(parent_id_str)?;

            match list.find_insert_position_for_child(parent_id) {
                Some((indent, idx)) => (indent, idx),
                None => {
                    return Err(format_error(McpErrorDetail::not_found(
                        format!("Parent todo with id '{parent_id_str}' not found"),
                        "Use list_todos to get valid parent IDs",
                    )));
                }
            }
        } else {
            (0, list.items.len())
        };

        let mut item = TodoItem::new(req.content, indent_level);
        item.parent_id = req.parent_id.as_deref().and_then(|s| parse_uuid(s).ok());
        item.due_date = due_date;
        item.description = req.description;

        let response = TodoItemResponse::from(&item);
        list.items.insert(insert_index, item);

        save_todo_list_for_project(&list, &project)
            .into_mcp_storage_error().map_err(format_error)?;

        info!(id = %response.id, content = %response.content, project = %project, "create_todo completed");
        Ok(Json(response))
    }

    #[tool(
        name = "update_todo",
        description = "Update an existing todo's content, state, due date, or description. State values: ' ' (empty/pending), '*' (in progress), 'x' (done), '?' (question), '!' (important)"
    )]
    async fn update_todo(
        &self,
        params: Parameters<UpdateTodoRequest>,
    ) -> Result<Json<TodoItemResponse>, String> {
        let req = params.0;
        info!(
            id = %req.id,
            date = ?req.date,
            project = ?req.project,
            content = ?req.content,
            state = ?req.state,
            "update_todo called"
        );

        let project = get_validated_project(req.project.as_deref()).map_err(format_error)?;
        let id = parse_uuid_or_err(&req.id)?;
        let date = parse_date_or_err(req.date.as_deref())?;

        let mut list = load_list_with_rollover(&project, date).map_err(format_error)?;

        let item = list
            .items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| {
                format_error(McpErrorDetail::not_found(
                    format!("Todo with id '{}' not found on {}", req.id, date),
                    "Use list_todos to verify the todo exists on this date",
                ))
            })?;

        if let Some(ref content) = req.content {
            if content.trim().is_empty() {
                return Err(format_error(McpErrorDetail::validation_error(
                    "Content cannot be empty",
                    "Provide a non-empty string or omit the content field",
                )));
            }
            item.content = content.clone();
        }

        if let Some(ref state_str) = req.state {
            let state = parse_state(state_str).ok_or_else(|| {
                format_error(McpErrorDetail::invalid_state(format!(
                    "Invalid state '{state_str}'. "
                )))
            })?;
            item.state = state;
        }

        if let Some(ref due_date_str) = req.due_date {
            let due_date = parse_date_or_err(Some(due_date_str))?;
            item.due_date = Some(due_date);
        }

        if let Some(ref description) = req.description {
            item.description = if description.is_empty() {
                None
            } else {
                Some(description.clone())
            };
        }

        let response = TodoItemResponse::from(&*item);

        save_todo_list_for_project(&list, &project)
            .into_mcp_storage_error().map_err(format_error)?;

        info!(id = %response.id, state = %response.state, project = %project, "update_todo completed");
        Ok(Json(response))
    }

    #[tool(
        name = "delete_todo",
        description = "Delete a todo and all its children from a project. This action is irreversible."
    )]
    async fn delete_todo(
        &self,
        params: Parameters<DeleteTodoRequest>,
    ) -> Result<Json<DeleteTodoResponse>, String> {
        let req = params.0;
        info!(id = %req.id, date = ?req.date, project = ?req.project, "delete_todo called");

        let project = get_validated_project(req.project.as_deref()).map_err(format_error)?;
        let id = parse_uuid_or_err(&req.id)?;
        let date = parse_date_or_err(req.date.as_deref())?;

        let mut list = load_list_with_rollover(&project, date).map_err(format_error)?;

        let idx = list
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| {
                format_error(McpErrorDetail::not_found(
                    format!("Todo with id '{}' not found on {}", req.id, date),
                    "Use list_todos to verify the todo exists on this date",
                ))
            })?;

        let (start, end) = list
            .get_item_range(idx)
            .into_mcp_storage_error().map_err(format_error)?;

        let deleted_count = end - start;

        let ids: Vec<_> = list.items[start..end].iter().map(|item| item.id).collect();
        soft_delete_todos(&ids, date)
            .into_mcp_storage_error().map_err(format_error)?;

        list.items.drain(start..end);
        list.recalculate_parent_ids();

        save_todo_list_for_project(&list, &project)
            .into_mcp_storage_error().map_err(format_error)?;

        info!(deleted_count = deleted_count, project = %project, "delete_todo completed");
        Ok(Json(DeleteTodoResponse {
            deleted_count,
            message: format!("Deleted {deleted_count} item(s)"),
        }))
    }

    #[tool(
        name = "mark_complete",
        description = "Toggle completion status: marks a todo as done [x] if pending, or pending [ ] if already done."
    )]
    async fn mark_complete(
        &self,
        params: Parameters<MarkCompleteRequest>,
    ) -> Result<Json<TodoItemResponse>, String> {
        let req = params.0;
        info!(id = %req.id, date = ?req.date, project = ?req.project, "mark_complete called");

        let project = get_validated_project(req.project.as_deref()).map_err(format_error)?;
        let id = parse_uuid_or_err(&req.id)?;
        let date = parse_date_or_err(req.date.as_deref())?;

        let mut list = load_list_with_rollover(&project, date).map_err(format_error)?;

        let item = list
            .items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| {
                format_error(McpErrorDetail::not_found(
                    format!("Todo with id '{}' not found on {}", req.id, date),
                    "Use list_todos to verify the todo exists on this date",
                ))
            })?;

        item.toggle_state();
        let response = TodoItemResponse::from(&*item);

        save_todo_list_for_project(&list, &project)
            .into_mcp_storage_error().map_err(format_error)?;

        info!(id = %response.id, new_state = %response.state, project = %project, "mark_complete completed");
        Ok(Json(response))
    }
}

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for TodoMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Todo list management server.\n\n\
                TOOLS:\n\
                - list_todos: List todos. Response has 'formatted' field - display it directly as markdown.\n\
                - create_todo: Create new todo. Can nest under parent via parent_id.\n\
                - update_todo: Update content/state/due_date. States: ' '=pending, 'x'=done, '?'=question, '!'=important\n\
                - delete_todo: Delete todo and children.\n\
                - mark_complete: Toggle done/pending.\n\
                - list_projects: List all available projects.\n\n\
                DISPLAY GUIDELINES:\n\
                - For list_todos: Display the 'formatted' field directly as markdown. Do NOT create tables.\n\
                - For single items: Show as '[ ] content' or '[x] content' format.\n\
                - Dates use YYYY-MM-DD format.\n\
                - IDs are UUIDs - use list_todos to get valid IDs.\n\
                - All tools accept optional 'project' parameter. Defaults to 'default' if not provided."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
