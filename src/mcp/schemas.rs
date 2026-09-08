use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::project::Project;
use crate::todo::{TodoItem, TodoState};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTodosRequest {
    #[schemars(description = "Date in YYYY-MM-DD format. Defaults to today if not provided.")]
    pub date: Option<String>,
    #[schemars(description = "Project name. Defaults to 'default' if not provided.")]
    pub project: Option<String>,
    #[schemars(
        description = "When true, omit completed items: those marked done ('x') or cancelled ('-'). A completed parent is kept if it still has unfinished children. The header counts and hidden_count still reflect the full list. Defaults to false."
    )]
    pub hide_completed: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListProjectsRequest {}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProjectItemResponse {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

impl From<&Project> for ProjectItemResponse {
    fn from(project: &Project) -> Self {
        Self {
            id: project.id.to_string(),
            name: project.name.clone(),
            created_at: project.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProjectListResponse {
    pub count: usize,
    pub projects: Vec<ProjectItemResponse>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateTodoRequest {
    #[schemars(description = "The todo content text. Cannot be empty.")]
    pub content: String,
    #[schemars(description = "Date in YYYY-MM-DD format. Defaults to today if not provided.")]
    pub date: Option<String>,
    #[schemars(
        description = "UUID of parent todo to nest under. Use list_todos to get valid IDs."
    )]
    pub parent_id: Option<String>,
    #[schemars(description = "Due date in YYYY-MM-DD format.")]
    pub due_date: Option<String>,
    #[schemars(description = "Additional notes or description for the todo.")]
    pub description: Option<String>,
    #[schemars(
        description = "Priority: 'P0' (critical), 'P1' (high), or 'P2' (medium). Omit for no priority."
    )]
    pub priority: Option<String>,
    #[schemars(description = "Project name. Defaults to 'default' if not provided.")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateTodoRequest {
    #[serde(default)]
    #[schemars(description = "Clear the due date. Omit or use false to leave it unchanged.")]
    pub clear_due_date: bool,
    #[serde(default)]
    #[schemars(description = "Clear the priority. Omit or use false to leave it unchanged.")]
    pub clear_priority: bool,
    #[schemars(description = "UUID of the todo to update. Use list_todos to get valid IDs.")]
    pub id: String,
    #[schemars(description = "Date in YYYY-MM-DD format. Defaults to today if not provided.")]
    pub date: Option<String>,
    #[schemars(description = "New content text for the todo.")]
    pub content: Option<String>,
    #[schemars(
        description = "New state: ' ' (pending), '*' (in progress), 'x' (done), '?' (question), '!' (important), '-' (cancelled). Set '*' while actively working on an item and 'x' when it is finished."
    )]
    pub state: Option<String>,
    #[schemars(description = "New due date in YYYY-MM-DD format.")]
    pub due_date: Option<String>,
    #[schemars(description = "New description. Empty string clears the description.")]
    pub description: Option<String>,
    #[schemars(description = "New priority: 'P0' (critical), 'P1' (high), or 'P2' (medium).")]
    pub priority: Option<String>,
    #[schemars(description = "Project name. Defaults to 'default' if not provided.")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteTodoRequest {
    #[schemars(description = "UUID of the todo to delete. This also deletes all child todos.")]
    pub id: String,
    #[schemars(description = "Date in YYYY-MM-DD format. Defaults to today if not provided.")]
    pub date: Option<String>,
    #[schemars(description = "Project name. Defaults to 'default' if not provided.")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MarkCompleteRequest {
    #[schemars(
        description = "UUID of the todo to toggle completion. Use list_todos to get valid IDs."
    )]
    pub id: String,
    #[schemars(description = "Date in YYYY-MM-DD format. Defaults to today if not provided.")]
    pub date: Option<String>,
    #[schemars(description = "Project name. Defaults to 'default' if not provided.")]
    pub project: Option<String>,
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
    #[schemars(description = "Priority if set: 'P0' (critical), 'P1' (high), or 'P2' (medium).")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
}

impl From<&TodoItem> for TodoItemResponse {
    fn from(item: &TodoItem) -> Self {
        Self {
            id: item.id.to_string(),
            content: item.content.clone(),
            state: item.state.to_char().to_string(),
            state_description: match item.state {
                TodoState::Empty => "pending",
                TodoState::Checked => "done",
                TodoState::Question => "question",
                TodoState::Exclamation => "important",
                TodoState::InProgress => "in_progress",
                TodoState::Cancelled => "cancelled",
            }
            .to_string(),
            indent_level: item.indent_level,
            parent_id: item.parent_id.map(|id| id.to_string()),
            due_date: item.due_date.map(|d| d.format("%Y-%m-%d").to_string()),
            description: item.description.clone(),
            priority: item.priority.map(|p| p.to_string()),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TodoListResponse {
    pub date: String,
    #[schemars(description = "Number of items returned in 'items'.")]
    pub item_count: usize,
    #[schemars(
        description = "Number of completed items omitted because hide_completed was set. 0 otherwise."
    )]
    pub hidden_count: usize,
    #[schemars(
        description = "Pre-formatted todo list for display. Show this directly to the user."
    )]
    pub formatted: String,
    #[schemars(
        description = "Raw item data for programmatic access. Use 'formatted' for display."
    )]
    pub items: Vec<TodoItemResponse>,
}

impl TodoListResponse {
    pub fn new(date: String, all_items: Vec<TodoItemResponse>, hide_completed: bool) -> Self {
        let done = all_items.iter().filter(|i| i.state == "x").count();
        let total = all_items.len();

        let items: Vec<TodoItemResponse> = if hide_completed {
            let keep: Vec<bool> = (0..all_items.len())
                .map(|i| !Self::is_hidden_when_completed(&all_items, i))
                .collect();
            all_items
                .into_iter()
                .zip(keep)
                .filter_map(|(item, keep)| keep.then_some(item))
                .collect()
        } else {
            all_items
        };

        let hidden_count = total - items.len();
        let formatted = Self::format_list(&date, &items, done, total, hidden_count);
        let item_count = items.len();
        Self {
            date,
            item_count,
            hidden_count,
            formatted,
            items,
        }
    }

    fn is_completed(item: &TodoItemResponse) -> bool {
        matches!(item.state.as_str(), "x" | "-")
    }

    /// A completed item is hidden unless one of its descendants is still unfinished,
    /// in which case it stays as context for that child.
    fn is_hidden_when_completed(items: &[TodoItemResponse], idx: usize) -> bool {
        if !Self::is_completed(&items[idx]) {
            return false;
        }
        let base_indent = items[idx].indent_level;
        for item in items[idx + 1..].iter() {
            if item.indent_level <= base_indent {
                break;
            }
            if !Self::is_completed(item) {
                return false;
            }
        }
        true
    }

    fn format_list(
        date: &str,
        items: &[TodoItemResponse],
        done: usize,
        total: usize,
        hidden_count: usize,
    ) -> String {
        if total == 0 {
            return format!("No todos for {date}");
        }

        let mut lines = Vec::new();
        let hidden = if hidden_count > 0 {
            format!(", {hidden_count} completed hidden")
        } else {
            String::new()
        };
        lines.push(format!("## Todos for {date} ({done}/{total}{hidden})"));
        lines.push(String::new());

        if items.is_empty() {
            lines.push("All items are completed.".to_string());
            return lines.join("\n");
        }

        for (i, item) in items.iter().enumerate() {
            let indent = "  ".repeat(item.indent_level);
            // Use emojis to avoid markdown interpretation
            let checkbox = match item.state.as_str() {
                "x" => "✅",
                "*" => "🔄",
                "?" => "❔",
                "!" => "❗",
                "-" => "🚫",
                _ => {
                    if Self::has_completed_descendants(items, i) {
                        "🔳"
                    } else {
                        "⬜"
                    }
                }
            };
            let priority = item
                .priority
                .as_ref()
                .map(|p| format!("[{p}] "))
                .unwrap_or_default();
            let due = item
                .due_date
                .as_ref()
                .map(|d| format!(" (due: {d})"))
                .unwrap_or_default();
            lines.push(format!(
                "{indent}{checkbox} {priority}{}{due}",
                item.content
            ));
        }

        lines.join("\n")
    }

    fn has_completed_descendants(items: &[TodoItemResponse], idx: usize) -> bool {
        let base_indent = items[idx].indent_level;
        for item in items[idx + 1..].iter() {
            if item.indent_level <= base_indent {
                break;
            }
            if item.state == "x" {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteTodoResponse {
    pub deleted_count: usize,
    pub message: String,
}

// Date, UUID and state parsing live in `todo::ops` (parse_date_arg / parse_id /
// parse_state_arg) so the CLI and this server cannot disagree about what a valid
// input is. Do not reintroduce local copies here.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todo::Priority;

    fn item(content: &str, indent: usize, state: &str) -> TodoItemResponse {
        TodoItemResponse {
            id: format!("id-{content}"),
            content: content.to_string(),
            state: state.to_string(),
            state_description: String::new(),
            indent_level: indent,
            parent_id: None,
            due_date: None,
            description: None,
            priority: None,
        }
    }

    fn with_priority(mut item: TodoItemResponse, priority: &str) -> TodoItemResponse {
        item.priority = Some(priority.to_string());
        item
    }

    #[test]
    fn item_response_carries_priority() {
        let mut todo = TodoItem::new("Fix it".to_string(), 0);
        todo.priority = Some(Priority::P0);

        let response = TodoItemResponse::from(&todo);
        assert_eq!(response.priority.as_deref(), Some("P0"));

        let plain = TodoItem::new("No priority".to_string(), 0);
        assert_eq!(TodoItemResponse::from(&plain).priority, None);
    }

    #[test]
    fn formatted_shows_priority_badge_after_checkbox() {
        let items = vec![with_priority(item("Fix it", 0, " "), "P1")];
        let response = TodoListResponse::new("2026-09-07".to_string(), items, false);
        assert!(
            response.formatted.contains("⬜ [P1] Fix it"),
            "got:\n{}",
            response.formatted
        );
    }

    #[test]
    fn formatted_distinguishes_every_state() {
        let items = vec![
            item("pending", 0, " "),
            item("started", 0, "*"),
            item("finished", 0, "x"),
            item("unsure", 0, "?"),
            item("urgent", 0, "!"),
            item("dropped", 0, "-"),
        ];
        let response = TodoListResponse::new("2026-09-07".to_string(), items, false);
        let f = &response.formatted;
        assert!(f.contains("⬜ pending"), "got:\n{f}");
        assert!(f.contains("🔄 started"), "got:\n{f}");
        assert!(f.contains("✅ finished"), "got:\n{f}");
        assert!(f.contains("❔ unsure"), "got:\n{f}");
        assert!(f.contains("❗ urgent"), "got:\n{f}");
        assert!(f.contains("🚫 dropped"), "got:\n{f}");
    }

    #[test]
    fn hide_completed_omits_done_and_cancelled_items() {
        let items = vec![
            item("todo", 0, " "),
            item("done", 0, "x"),
            item("dropped", 0, "-"),
            item("started", 0, "*"),
        ];
        let response = TodoListResponse::new("2026-09-07".to_string(), items, true);

        let contents: Vec<&str> = response.items.iter().map(|i| i.content.as_str()).collect();
        assert_eq!(contents, vec!["todo", "started"]);
        assert!(
            !response.formatted.contains("done"),
            "got:\n{}",
            response.formatted
        );
        assert!(
            !response.formatted.contains("dropped"),
            "got:\n{}",
            response.formatted
        );
        assert_eq!(response.item_count, 2);
        assert_eq!(response.hidden_count, 2);
    }

    #[test]
    fn hide_completed_keeps_done_parent_that_has_unfinished_children() {
        let items = vec![
            item("parent", 0, "x"),
            item("finished child", 1, "x"),
            item("pending child", 1, " "),
        ];
        let response = TodoListResponse::new("2026-09-07".to_string(), items, true);

        let contents: Vec<&str> = response.items.iter().map(|i| i.content.as_str()).collect();
        assert_eq!(contents, vec!["parent", "pending child"]);
    }

    #[test]
    fn hide_completed_header_still_counts_the_full_list() {
        let items = vec![
            item("a", 0, "x"),
            item("b", 0, "x"),
            item("c", 0, " "),
            item("d", 0, " "),
        ];
        let response = TodoListResponse::new("2026-09-07".to_string(), items, true);
        assert!(
            response
                .formatted
                .starts_with("## Todos for 2026-09-07 (2/4"),
            "got:\n{}",
            response.formatted
        );
        assert!(
            response.formatted.contains("2 completed hidden"),
            "got:\n{}",
            response.formatted
        );
    }

    #[test]
    fn hide_completed_false_returns_everything_with_no_hidden_count() {
        let items = vec![item("a", 0, "x"), item("b", 0, " ")];
        let response = TodoListResponse::new("2026-09-07".to_string(), items, false);
        assert_eq!(response.item_count, 2);
        assert_eq!(response.hidden_count, 0);
        assert!(!response.formatted.contains("hidden"));
    }
}
