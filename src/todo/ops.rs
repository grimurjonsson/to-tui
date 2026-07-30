//! The single implementation of todo operations.
//!
//! Both entry points delegate here: the `totui todo *` CLI subcommands and the
//! MCP server's tool handlers. Nothing in this module knows about either — it
//! returns [`OpsError`], which the CLI renders as a plain message and the MCP
//! layer converts into its wire-format `McpErrorDetail`.
//!
//! Items are serialized through `mcp::schemas::TodoItemResponse`, so the CLI and
//! the MCP server emit byte-identical JSON for the same item.

use chrono::{Local, NaiveDate};
use serde::Deserialize;
use uuid::Uuid;

use crate::mcp::schemas::TodoItemResponse;
use crate::project::{DEFAULT_PROJECT_NAME, ProjectRegistry};
use crate::storage::database::soft_delete_todos_for_project;
use crate::storage::file::{
    file_exists_for_project, load_todo_list_for_project, save_todo_list_for_project,
};
use crate::storage::rollover::create_rolled_over_list_for_project;
use crate::todo::{Priority, TodoItem, TodoList, TodoState};

/// Every state a todo can hold, in the order they are documented to callers.
pub const VALID_STATES: &str =
    "' ' (pending), '*' (in progress), 'x' (done), '?' (question), '!' (important), '-' (cancelled)";

/// Failure kinds, carrying enough structure for the MCP layer to rebuild its
/// error codes and suggestions without this module depending on them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpsError {
    NotFound {
        message: String,
        suggestion: String,
    },
    InvalidInput {
        message: String,
        suggestion: String,
    },
    InvalidState {
        message: String,
    },
    Validation {
        message: String,
        suggestion: String,
    },
    Storage {
        message: String,
    },
}

impl OpsError {
    pub fn message(&self) -> &str {
        match self {
            Self::NotFound { message, .. }
            | Self::InvalidInput { message, .. }
            | Self::InvalidState { message }
            | Self::Validation { message, .. }
            | Self::Storage { message } => message,
        }
    }

    pub fn suggestion(&self) -> Option<&str> {
        match self {
            Self::NotFound { suggestion, .. }
            | Self::InvalidInput { suggestion, .. }
            | Self::Validation { suggestion, .. } => Some(suggestion),
            Self::InvalidState { .. } | Self::Storage { .. } => None,
        }
    }

    fn storage(e: impl std::fmt::Display) -> Self {
        Self::Storage {
            message: e.to_string(),
        }
    }
}

impl std::fmt::Display for OpsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for OpsError {}

/// Fields settable when creating a todo. Deserialized directly from `--json`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSpec {
    pub content: String,
    #[serde(default)]
    pub description: Option<String>,
    /// One of `" "`, `"*"`, `"x"`, `"?"`, `"!"`, `"-"`. Defaults to pending.
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    /// `"p0"`, `"p1"` or `"p2"`.
    #[serde(default)]
    pub priority: Option<String>,
}

/// Fields settable when updating a todo. Omitted fields are left unchanged.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateSpec {
    #[serde(default)]
    pub content: Option<String>,
    /// An empty string clears the description.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
}

/// A loaded list. `date` is the list's own date, which after a rollover is today
/// rather than the date originally asked for.
pub struct ListResult {
    pub date: NaiveDate,
    pub items: Vec<TodoItemResponse>,
}

/// Resolve a project name, defaulting to the configured default project.
///
/// Errors when the project is not registered, so a typo does not silently
/// create todos somewhere nobody is looking.
pub fn resolve_project(project: Option<&str>) -> Result<String, OpsError> {
    let name = project.unwrap_or(DEFAULT_PROJECT_NAME).to_string();
    let registry = ProjectRegistry::load().map_err(OpsError::storage)?;
    if registry.get_by_name(&name).is_none() {
        return Err(OpsError::NotFound {
            message: format!("Project '{name}' not found"),
            suggestion: "Use list_projects to see available projects".to_string(),
        });
    }
    Ok(name)
}

pub fn parse_date_arg(date: Option<&str>) -> Result<NaiveDate, OpsError> {
    match date {
        Some(s) => {
            NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| OpsError::InvalidInput {
                message: format!("Invalid date format '{s}'"),
                suggestion: "Use YYYY-MM-DD format".to_string(),
            })
        }
        None => Ok(Local::now().date_naive()),
    }
}

pub fn parse_id(id: &str) -> Result<Uuid, OpsError> {
    Uuid::parse_str(id).map_err(|_| OpsError::InvalidInput {
        message: format!("Invalid UUID format '{id}'"),
        suggestion: "Use list_todos to get valid IDs".to_string(),
    })
}

fn parse_state_arg(s: &str) -> Result<TodoState, OpsError> {
    TodoState::parse(s).ok_or_else(|| OpsError::InvalidState {
        message: format!("Invalid state '{s}'. Valid: {VALID_STATES}."),
    })
}

fn parse_priority_arg(s: &str) -> Result<Priority, OpsError> {
    match s.trim().to_ascii_lowercase().as_str() {
        "p0" | "0" => Ok(Priority::P0),
        "p1" | "1" => Ok(Priority::P1),
        "p2" | "2" => Ok(Priority::P2),
        _ => Err(OpsError::InvalidInput {
            message: format!("Invalid priority '{s}'"),
            suggestion: "Valid: p0, p1, p2".to_string(),
        }),
    }
}

fn not_found_todo(id: &str, date: NaiveDate) -> OpsError {
    OpsError::NotFound {
        message: format!("Todo with id '{id}' not found on {date}"),
        suggestion: "Use list_todos to verify the todo exists on this date".to_string(),
    }
}

/// Load a project's list for `date`, rolling incomplete items forward when the
/// date is today and today has no file yet.
///
/// This is the ONLY implementation of the rollover-on-load behavior. Both the
/// CLI and the MCP server reach it, so they cannot disagree about what "today's
/// list" means.
pub fn load_list(project: &str, date: NaiveDate) -> Result<TodoList, OpsError> {
    let today = Local::now().date_naive();

    if date == today && !file_exists_for_project(project, date).map_err(OpsError::storage)? {
        for days_back in 1..=30 {
            let Some(check) = today.checked_sub_days(chrono::Days::new(days_back)) else {
                break;
            };
            if file_exists_for_project(project, check).map_err(OpsError::storage)? {
                let list = load_todo_list_for_project(project, check).map_err(OpsError::storage)?;
                let incomplete = list.get_incomplete_items();
                if !incomplete.is_empty() {
                    let rolled = create_rolled_over_list_for_project(project, today, incomplete)
                        .map_err(OpsError::storage)?;
                    save_todo_list_for_project(&rolled, project).map_err(OpsError::storage)?;
                    return Ok(rolled);
                }
                break;
            }
        }
    }

    load_todo_list_for_project(project, date).map_err(OpsError::storage)
}

fn save(list: &TodoList, project: &str) -> Result<(), OpsError> {
    save_todo_list_for_project(list, project).map_err(OpsError::storage)
}

pub fn create(
    project: Option<&str>,
    date: Option<&str>,
    spec: CreateSpec,
) -> Result<TodoItemResponse, OpsError> {
    if spec.content.trim().is_empty() {
        return Err(OpsError::Validation {
            message: "Content cannot be empty".to_string(),
            suggestion: "Provide a non-empty string for the todo content".to_string(),
        });
    }

    let project = resolve_project(project)?;
    let date = parse_date_arg(date)?;
    let mut list = load_list(&project, date)?;

    let (indent_level, insert_index) = match spec.parent_id.as_deref() {
        Some(pid) => {
            let parent = parse_id(pid)?;
            list.find_insert_position_for_child(parent)
                .ok_or_else(|| OpsError::NotFound {
                    message: format!("Parent todo with id '{pid}' not found"),
                    suggestion: "Use list_todos to get valid parent IDs".to_string(),
                })?
        }
        None => (0, list.items.len()),
    };

    let mut item = TodoItem::new(spec.content, indent_level);
    item.parent_id = spec.parent_id.as_deref().map(parse_id).transpose()?;
    item.description = spec.description;
    item.due_date = spec
        .due_date
        .as_deref()
        .map(|d| parse_date_arg(Some(d)))
        .transpose()?;
    if let Some(ref s) = spec.state {
        item.state = parse_state_arg(s)?;
    }
    if let Some(ref p) = spec.priority {
        item.priority = Some(parse_priority_arg(p)?);
    }

    let response = TodoItemResponse::from(&item);
    list.items.insert(insert_index, item);
    save(&list, &project)?;
    Ok(response)
}

pub fn update(
    project: Option<&str>,
    date: Option<&str>,
    id: &str,
    spec: UpdateSpec,
) -> Result<TodoItemResponse, OpsError> {
    let project = resolve_project(project)?;
    let date = parse_date_arg(date)?;
    let uuid = parse_id(id)?;

    // Validate before mutating so a bad value cannot leave a half-applied patch.
    let state = spec.state.as_deref().map(parse_state_arg).transpose()?;
    let due_date = spec
        .due_date
        .as_deref()
        .map(|d| parse_date_arg(Some(d)))
        .transpose()?;
    let priority = spec.priority.as_deref().map(parse_priority_arg).transpose()?;
    if let Some(ref c) = spec.content
        && c.trim().is_empty()
    {
        return Err(OpsError::Validation {
            message: "Content cannot be empty".to_string(),
            suggestion: "Provide a non-empty string or omit the content field".to_string(),
        });
    }

    let mut list = load_list(&project, date)?;
    let item = list
        .items
        .iter_mut()
        .find(|i| i.id == uuid)
        .ok_or_else(|| not_found_todo(id, date))?;

    if let Some(c) = spec.content {
        item.content = c;
    }
    if let Some(s) = state {
        item.state = s;
    }
    if let Some(d) = due_date {
        item.due_date = Some(d);
    }
    if let Some(p) = priority {
        item.priority = Some(p);
    }
    if let Some(d) = spec.description {
        item.description = if d.is_empty() { None } else { Some(d) };
    }

    let response = TodoItemResponse::from(&*item);
    save(&list, &project)?;
    Ok(response)
}

pub fn get(
    project: Option<&str>,
    date: Option<&str>,
    id: &str,
) -> Result<TodoItemResponse, OpsError> {
    let project = resolve_project(project)?;
    let date = parse_date_arg(date)?;
    let uuid = parse_id(id)?;
    let list = load_list(&project, date)?;
    list.items
        .iter()
        .find(|i| i.id == uuid)
        .map(TodoItemResponse::from)
        .ok_or_else(|| not_found_todo(id, date))
}

pub fn list(project: Option<&str>, date: Option<&str>) -> Result<ListResult, OpsError> {
    let project = resolve_project(project)?;
    let date = parse_date_arg(date)?;
    let list = load_list(&project, date)?;
    Ok(ListResult {
        date: list.date,
        items: list.items.iter().map(TodoItemResponse::from).collect(),
    })
}

/// Toggle a todo between done and pending.
pub fn toggle_complete(
    project: Option<&str>,
    date: Option<&str>,
    id: &str,
) -> Result<TodoItemResponse, OpsError> {
    let project = resolve_project(project)?;
    let date = parse_date_arg(date)?;
    let uuid = parse_id(id)?;

    let mut list = load_list(&project, date)?;
    let item = list
        .items
        .iter_mut()
        .find(|i| i.id == uuid)
        .ok_or_else(|| not_found_todo(id, date))?;

    item.toggle_state();
    let response = TodoItemResponse::from(&*item);
    save(&list, &project)?;
    Ok(response)
}

/// Re-parent a todo, carrying its whole subtree with it.
///
/// `new_parent` of `None` moves the item to the top level. Indent levels of the
/// moved subtree are shifted by the same delta so its internal shape survives.
pub fn move_item(
    project: Option<&str>,
    date: Option<&str>,
    id: &str,
    new_parent: Option<&str>,
) -> Result<TodoItemResponse, OpsError> {
    let project = resolve_project(project)?;
    let date = parse_date_arg(date)?;
    let uuid = parse_id(id)?;
    let mut list = load_list(&project, date)?;

    let idx = list
        .items
        .iter()
        .position(|i| i.id == uuid)
        .ok_or_else(|| not_found_todo(id, date))?;
    let (start, end) = list.get_item_range(idx).map_err(OpsError::storage)?;

    // Resolve the destination before detaching anything, so a bad target leaves
    // the list untouched.
    let (target_indent, mut insert_at) = match new_parent {
        Some(pid) => {
            let parent_uuid = parse_id(pid)?;
            if parent_uuid == uuid {
                return Err(OpsError::Validation {
                    message: "A todo cannot be its own parent".to_string(),
                    suggestion: "Pick a different parent, or omit it to move to the top level"
                        .to_string(),
                });
            }
            let parent_idx = list
                .items
                .iter()
                .position(|i| i.id == parent_uuid)
                .ok_or_else(|| OpsError::NotFound {
                    message: format!("Parent todo with id '{pid}' not found"),
                    suggestion: "Use list_todos to get valid parent IDs".to_string(),
                })?;
            if parent_idx >= start && parent_idx < end {
                return Err(OpsError::Validation {
                    message: "Cannot move a todo inside its own subtree".to_string(),
                    suggestion: "Pick a parent outside the item being moved".to_string(),
                });
            }
            list.find_insert_position_for_child(parent_uuid)
                .ok_or_else(|| not_found_todo(pid, date))?
        }
        None => (0, list.items.len()),
    };

    let moved: Vec<TodoItem> = list.items.drain(start..end).collect();

    // Draining shifts everything after the removed slice left.
    if insert_at > start {
        insert_at -= end - start;
    }
    insert_at = insert_at.min(list.items.len());

    let old_indent = moved[0].indent_level;
    for (offset, mut item) in moved.into_iter().enumerate() {
        item.indent_level = target_indent + (item.indent_level - old_indent);
        list.items.insert(insert_at + offset, item);
    }

    list.recalculate_parent_ids();
    let response = TodoItemResponse::from(&list.items[insert_at]);
    save(&list, &project)?;
    Ok(response)
}

/// Delete a todo and every descendant. Returns the ids removed.
///
/// Dropping items from the list and saving is NOT enough: the save path upserts,
/// so removed rows survive. The database soft-delete is what actually retires them.
pub fn delete(
    project: Option<&str>,
    date: Option<&str>,
    id: &str,
) -> Result<Vec<String>, OpsError> {
    let project = resolve_project(project)?;
    let date = parse_date_arg(date)?;
    let uuid = parse_id(id)?;
    let mut list = load_list(&project, date)?;

    let idx = list
        .items
        .iter()
        .position(|i| i.id == uuid)
        .ok_or_else(|| not_found_todo(id, date))?;

    let (start, end) = list.get_item_range(idx).map_err(OpsError::storage)?;

    let ids: Vec<Uuid> = list.items[start..end].iter().map(|i| i.id).collect();
    let removed: Vec<String> = ids.iter().map(|i| i.to_string()).collect();

    soft_delete_todos_for_project(&ids, date, &project).map_err(OpsError::storage)?;

    list.items.drain(start..end);
    list.recalculate_parent_ids();
    save(&list, &project)?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::errors::McpErrorDetail;

    // --- parsing -----------------------------------------------------------

    #[test]
    fn parses_every_documented_state() {
        for (input, expected) in [
            (" ", TodoState::Empty),
            ("*", TodoState::InProgress),
            ("x", TodoState::Checked),
            ("?", TodoState::Question),
            ("!", TodoState::Exclamation),
            ("-", TodoState::Cancelled),
        ] {
            assert_eq!(parse_state_arg(input).unwrap(), expected, "input {input:?}");
        }
    }

    #[test]
    fn every_state_in_valid_states_doc_actually_parses() {
        // Guards against the docs listing a state the parser rejects, which is
        // exactly the drift that left '*' undocumented for so long.
        for token in ["' '", "'*'", "'x'", "'?'", "'!'", "'-'"] {
            let ch = token.trim_matches('\'');
            assert!(
                VALID_STATES.contains(token),
                "{token} missing from VALID_STATES"
            );
            assert!(parse_state_arg(ch).is_ok(), "{ch:?} does not parse");
        }
    }

    #[test]
    fn rejects_unknown_state_and_names_the_valid_ones() {
        let err = parse_state_arg("z").unwrap_err();
        assert!(matches!(err, OpsError::InvalidState { .. }));
        assert!(err.message().contains("Invalid state 'z'"));
        assert!(err.message().contains("in progress"));
        assert!(err.message().contains("cancelled"));
    }

    #[test]
    fn parses_priority_case_and_bare_digit() {
        assert_eq!(parse_priority_arg("p0").unwrap(), Priority::P0);
        assert_eq!(parse_priority_arg("P1").unwrap(), Priority::P1);
        assert_eq!(parse_priority_arg("2").unwrap(), Priority::P2);
        assert!(parse_priority_arg("p9").is_err());
    }

    #[test]
    fn parse_date_arg_defaults_to_today() {
        assert_eq!(parse_date_arg(None).unwrap(), Local::now().date_naive());
        assert_eq!(
            parse_date_arg(Some("2026-07-30")).unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 30).unwrap()
        );
        assert!(parse_date_arg(Some("30-07-2026")).is_err());
    }

    #[test]
    fn parse_id_rejects_non_uuid() {
        assert!(parse_id("not-a-uuid").is_err());
        assert!(parse_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
    }

    // --- specs -------------------------------------------------------------

    #[test]
    fn create_spec_accepts_every_settable_field() {
        let spec: CreateSpec = serde_json::from_str(
            r#"{"content":"c","description":"d","state":"*","due_date":"2026-08-01",
                 "parent_id":"550e8400-e29b-41d4-a716-446655440000","priority":"p1"}"#,
        )
        .expect("full spec should deserialize");
        assert_eq!(spec.content, "c");
        assert_eq!(spec.state.as_deref(), Some("*"));
        assert_eq!(spec.priority.as_deref(), Some("p1"));
    }

    #[test]
    fn create_spec_rejects_unknown_fields() {
        // deny_unknown_fields: a typo must fail loudly rather than be dropped.
        assert!(serde_json::from_str::<CreateSpec>(r#"{"content":"c","contnet":"typo"}"#).is_err());
    }

    #[test]
    fn update_spec_is_all_optional() {
        let spec: UpdateSpec = serde_json::from_str("{}").expect("empty patch is valid");
        assert!(spec.content.is_none());
        assert!(spec.state.is_none());
    }

    // --- move --------------------------------------------------------------
    //
    // move_item needs a live store, so these cover the index arithmetic that
    // decides where a drained subtree lands — the part most likely to be wrong.

    /// Mirrors the reinsertion maths in `move_item`.
    fn reinsert_index(start: usize, end: usize, insert_at: usize, len_after: usize) -> usize {
        let adjusted = if insert_at > start {
            insert_at - (end - start)
        } else {
            insert_at
        };
        adjusted.min(len_after)
    }

    #[test]
    fn moving_forward_accounts_for_the_drained_slice() {
        // items 1..3 removed, target was index 5 -> shifts left by 2
        assert_eq!(reinsert_index(1, 3, 5, 4), 3);
    }

    #[test]
    fn moving_backward_needs_no_adjustment() {
        // target index 0 is before the removed slice, so it is unaffected
        assert_eq!(reinsert_index(3, 5, 0, 4), 0);
    }

    #[test]
    fn reinsert_never_runs_past_the_end() {
        assert_eq!(reinsert_index(0, 2, 9, 3), 3);
    }

    #[test]
    fn indent_delta_preserves_subtree_shape() {
        // A subtree rooted at indent 0 with a child at 1 and grandchild at 2,
        // moved under a parent at indent 1, must become 1 / 2 / 3.
        let old_root_indent = 0usize;
        let target_indent = 1usize;
        let shape = [0usize, 1, 2];
        let moved: Vec<usize> = shape
            .iter()
            .map(|lvl| target_indent + (lvl - old_root_indent))
            .collect();
        assert_eq!(moved, vec![1, 2, 3]);
    }

    // --- error mapping -----------------------------------------------------
    //
    // The MCP wire contract is these five codes. If a mapping changes, an MCP
    // client's error handling breaks, so each variant is pinned.

    #[test]
    fn maps_every_variant_to_its_mcp_code() {
        let cases = [
            (
                OpsError::NotFound {
                    message: "m".into(),
                    suggestion: "s".into(),
                },
                "NOT_FOUND",
                true,
            ),
            (
                OpsError::InvalidInput {
                    message: "m".into(),
                    suggestion: "s".into(),
                },
                "INVALID_INPUT",
                true,
            ),
            (
                OpsError::InvalidState {
                    message: "m".into(),
                },
                "INVALID_STATE",
                true,
            ),
            (
                OpsError::Validation {
                    message: "m".into(),
                    suggestion: "s".into(),
                },
                "VALIDATION_ERROR",
                true,
            ),
            (
                OpsError::Storage {
                    message: "m".into(),
                },
                "STORAGE_ERROR",
                false,
            ),
        ];

        for (err, expected_code, expected_retryable) in cases {
            let detail: McpErrorDetail = err.clone().into();
            assert_eq!(detail.code, expected_code, "for {err:?}");
            assert_eq!(detail.retryable, expected_retryable, "for {err:?}");
            assert_eq!(detail.message, "m");
        }
    }

    #[test]
    fn carries_the_suggestion_through_to_mcp() {
        let detail: McpErrorDetail = OpsError::NotFound {
            message: "gone".into(),
            suggestion: "look elsewhere".into(),
        }
        .into();
        assert_eq!(detail.suggestion.as_deref(), Some("look elsewhere"));
    }

    #[test]
    fn invalid_state_suggestion_lists_all_six_states() {
        let detail: McpErrorDetail = OpsError::InvalidState {
            message: "bad".into(),
        }
        .into();
        let suggestion = detail.suggestion.expect("invalid state carries a suggestion");
        for token in ["' '", "'*'", "'x'", "'?'", "'!'", "'-'"] {
            assert!(suggestion.contains(token), "{token} missing from {suggestion}");
        }
    }

    #[test]
    fn display_is_the_bare_message_for_cli_output() {
        let err = OpsError::Validation {
            message: "Content cannot be empty".into(),
            suggestion: "ignored on the CLI".into(),
        };
        assert_eq!(err.to_string(), "Content cannot be empty");
    }
}
