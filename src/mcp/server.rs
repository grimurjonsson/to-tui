use rmcp::{
    Json,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use tracing::{error, info, warn};

use crate::project::ProjectRegistry;
use crate::todo::ops;

use super::errors::{IntoMcpError, McpErrorDetail};
use super::schemas::{
    CreateTodoRequest, DeleteTodoRequest, DeleteTodoResponse, ListProjectsRequest,
    ListTodosRequest, MarkCompleteRequest, ProjectItemResponse, ProjectListResponse,
    TodoItemResponse, TodoListResponse, UpdateTodoRequest,
};

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
struct KanbanResponse {
    board: Option<crate::kanban::Board>,
}

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

/// Render any ops-layer failure onto the MCP wire format.
fn ops_err(err: crate::todo::ops::OpsError) -> String {
    format_error(McpErrorDetail::from(err))
}

fn format_error(detail: McpErrorDetail) -> String {
    if detail.retryable {
        warn!(code = %detail.code, message = %detail.message, "Retryable error occurred");
    } else {
        error!(code = %detail.code, message = %detail.message, "Non-retryable error occurred");
    }
    serde_json::to_string(&detail).unwrap_or(detail.message)
}

#[tool_router]
impl TodoMcpServer {
    #[tool(
        name = "kanban",
        description = "Manage a persistent project kanban board. Use action=view to discover tickets, comments, outstanding feedback and revisions. Create a board with create_board, then create_ticket. Move tickets through backlog, ready, in_progress, review, done, blocked using move_ticket. Every ticket mutation requires expected_revision from the latest board. Always read activity and feedback before working; backward moves require a reason. Address feedback with address_feedback and a resolution before marking done. Use trash_ticket to remove a ticket from active work and restore_ticket to recover it with its history intact. Use archive_ticket for Done tickets to move them into Completed, and unarchive_ticket to restore them to Done. Tickets with trashed=true or archived=true are not active work. Set actor to your agent name."
    )]
    async fn kanban(
        &self,
        params: Parameters<crate::kanban::Request>,
    ) -> Result<Json<KanbanResponse>, String> {
        crate::kanban::execute(params.0)
            .map(|board| Json(KanbanResponse { board }))
            .map_err(|error| error.to_string())
    }

    #[tool(
        name = "list_todos",
        description = "List todos for a specific date and project. Defaults to today and 'default' project. Automatically rolls over incomplete todos from previous days if today's list is empty. Set hide_completed=true to omit done and cancelled items. Each item carries its state, priority (P0/P1/P2, if set), due date and description. Response includes a 'formatted' field - display it directly as markdown to the user."
    )]
    async fn list_todos(
        &self,
        params: Parameters<ListTodosRequest>,
    ) -> Result<Json<TodoListResponse>, String> {
        info!(date = ?params.0.date, project = ?params.0.project, "list_todos called");

        let result =
            ops::list(params.0.project.as_deref(), params.0.date.as_deref()).map_err(ops_err)?;

        let response = TodoListResponse::new(
            result.date.format("%Y-%m-%d").to_string(),
            result.items,
            params.0.hide_completed.unwrap_or(false),
        );

        info!(count = response.item_count, "list_todos returning items");
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
        description = "Create a new todo item in a project. Optionally nest under a parent todo by providing parent_id, and set a priority (P0/P1/P2). New items start pending; use update_todo to set '*' (in progress) when you begin working on one."
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

        let spec = ops::CreateSpec {
            content: req.content,
            description: req.description,
            state: None,
            due_date: req.due_date,
            parent_id: req.parent_id,
            priority: req.priority,
        };

        let response =
            ops::create(req.project.as_deref(), req.date.as_deref(), spec).map_err(ops_err)?;

        info!(id = %response.id, content = %response.content, "create_todo completed");
        Ok(Json(response))
    }

    #[tool(
        name = "update_todo",
        description = "Update an existing todo's content, state, priority, due date, or description. State values: ' ' (pending), '*' (in progress), 'x' (done), '?' (question), '!' (important), '-' (cancelled). While working on an item set it to '*'; when finished set it to 'x'. Priority values: 'P0' (critical), 'P1' (high), 'P2' (medium)."
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

        let spec = ops::UpdateSpec {
            placement: None,
            expected_revision: None,
            clear_due_date: req.clear_due_date,
            clear_priority: req.clear_priority,
            content: req.content,
            description: req.description,
            state: req.state,
            due_date: req.due_date,
            priority: req.priority,
        };

        let response = ops::update(req.project.as_deref(), req.date.as_deref(), &req.id, spec)
            .map_err(ops_err)?;

        info!(id = %response.id, state = %response.state, "update_todo completed");
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

        let removed =
            ops::delete(req.project.as_deref(), req.date.as_deref(), &req.id).map_err(ops_err)?;
        let deleted_count = removed.len();

        info!(deleted_count = deleted_count, "delete_todo completed");
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

        let response = ops::toggle_complete(req.project.as_deref(), req.date.as_deref(), &req.id)
            .map_err(ops_err)?;

        info!(id = %response.id, new_state = %response.state, "mark_complete completed");
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
                - list_todos: List todos. Response has 'formatted' field - display it directly as markdown. Pass hide_completed=true to omit done/cancelled items.\n\
                - create_todo: Create new todo. Can nest under parent via parent_id; optional priority P0/P1/P2.\n\
                - update_todo: Update content/state/priority/due_date/description.\n\
                - delete_todo: Delete todo and children.\n\
                - mark_complete: Toggle done/pending.\n\
                - list_projects: List all available projects.\n\n\
                STATES: ' '=pending, '*'=in progress, 'x'=done, '?'=question, '!'=important, '-'=cancelled.\n\
                PRIORITIES: P0=critical, P1=high, P2=medium (optional; shown as [P0] etc. in 'formatted').\n\n\
                WORKING ON ITEMS:\n\
                - When you start working on an item, set its state to '*' (in progress) so the user sees it live in the TUI.\n\
                - When you finish it, set it to 'x' (done). Use '?' when blocked on a user decision, '!' to flag attention, '-' to cancel.\n\
                - Keep exactly one item in progress at a time.\n\n\
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_constructs_with_valid_tool_schemas() {
        let _server = TodoMcpServer::new();
    }
}
