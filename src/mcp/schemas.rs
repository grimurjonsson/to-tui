use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::todo::{TodoItem, TodoState};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTodosRequest {
    #[schemars(description = "Date in YYYY-MM-DD format. Defaults to today if not provided.")]
    pub date: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateTodoRequest {
    #[schemars(description = "The todo content text. Cannot be empty.")]
    pub content: String,
    #[schemars(description = "Date in YYYY-MM-DD format. Defaults to today if not provided.")]
    pub date: Option<String>,
    #[schemars(description = "UUID of parent todo to nest under. Use list_todos to get valid IDs.")]
    pub parent_id: Option<String>,
    #[schemars(description = "Due date in YYYY-MM-DD format.")]
    pub due_date: Option<String>,
    #[schemars(description = "Additional notes or description for the todo.")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateTodoRequest {
    #[schemars(description = "UUID of the todo to update. Use list_todos to get valid IDs.")]
    pub id: String,
    #[schemars(description = "Date in YYYY-MM-DD format. Defaults to today if not provided.")]
    pub date: Option<String>,
    #[schemars(description = "New content text for the todo.")]
    pub content: Option<String>,
    #[schemars(description = "New state: ' ' (empty/pending), 'x' (done), '?' (question), '!' (important)")]
    pub state: Option<String>,
    #[schemars(description = "New due date in YYYY-MM-DD format.")]
    pub due_date: Option<String>,
    #[schemars(description = "New description. Empty string clears the description.")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteTodoRequest {
    #[schemars(description = "UUID of the todo to delete. This also deletes all child todos.")]
    pub id: String,
    #[schemars(description = "Date in YYYY-MM-DD format. Defaults to today if not provided.")]
    pub date: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MarkCompleteRequest {
    #[schemars(description = "UUID of the todo to toggle completion. Use list_todos to get valid IDs.")]
    pub id: String,
    #[schemars(description = "Date in YYYY-MM-DD format. Defaults to today if not provided.")]
    pub date: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TodoItemResponse {
    pub id: String,
    pub content: String,
    pub state: String,
    pub state_description: String,
    pub indent_level: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl From<&TodoItem> for TodoItemResponse {
    fn from(item: &TodoItem) -> Self {
        Self {
            id: item.id.to_string(),
            content: item.content.clone(),
            state: item.state.to_char().to_string(),
            state_description: match item.state {
                TodoState::Empty => "pending".to_string(),
                TodoState::Checked => "done".to_string(),
                TodoState::Question => "question".to_string(),
                TodoState::Exclamation => "important".to_string(),
            },
            indent_level: item.indent_level,
            parent_id: item.parent_id.map(|id| id.to_string()),
            due_date: item.due_date.map(|d| d.format("%Y-%m-%d").to_string()),
            description: item.description.clone(),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TodoListResponse {
    pub date: String,
    pub item_count: usize,
    #[schemars(description = "Pre-formatted todo list for display. Show this directly to the user.")]
    pub formatted: String,
    #[schemars(description = "Raw item data for programmatic access. Use 'formatted' for display.")]
    pub items: Vec<TodoItemResponse>,
}

impl TodoListResponse {
    pub fn new(date: String, items: Vec<TodoItemResponse>) -> Self {
        let formatted = Self::format_list(&date, &items);
        let item_count = items.len();
        Self {
            date,
            item_count,
            formatted,
            items,
        }
    }

    fn format_list(date: &str, items: &[TodoItemResponse]) -> String {
        if items.is_empty() {
            return format!("No todos for {date}");
        }

        let mut lines = Vec::new();
        let (done, total) = items.iter().fold((0, 0), |(done, total), item| {
            (done + if item.state == "x" { 1 } else { 0 }, total + 1)
        });

        lines.push(format!("## Todos for {date} ({done}/{total})"));
        lines.push(String::new());

        for item in items {
            let indent = "  ".repeat(item.indent_level);
            // Use emojis to avoid markdown interpretation
            let checkbox = match item.state.as_str() {
                "x" => "✅",
                "?" => "❔",
                "!" => "❗",
                _ => "⬜",
            };
            let due = item.due_date.as_ref().map(|d| format!(" (due: {d})")).unwrap_or_default();
            lines.push(format!("{}{} {}{}", indent, checkbox, item.content, due));
        }

        lines.join("\n")
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteTodoResponse {
    pub deleted_count: usize,
    pub message: String,
}

pub fn parse_date(date_str: Option<&str>) -> Result<NaiveDate, String> {
    match date_str {
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| format!("Invalid date format '{s}'. Use YYYY-MM-DD format.")),
        None => Ok(chrono::Local::now().date_naive()),
    }
}

pub fn parse_uuid(id_str: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id_str)
        .map_err(|_| format!("Invalid UUID format '{id_str}'. Use list_todos to get valid IDs."))
}

pub fn parse_state(state_str: &str) -> Option<TodoState> {
    match state_str.trim() {
        " " | "" => Some(TodoState::Empty),
        "x" | "X" => Some(TodoState::Checked),
        "?" => Some(TodoState::Question),
        "!" => Some(TodoState::Exclamation),
        _ => None,
    }
}
