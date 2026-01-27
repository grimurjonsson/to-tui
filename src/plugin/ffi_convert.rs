//! Bidirectional conversion between native types and FFI-safe types.
//!
//! This module provides `From` and `TryFrom` implementations for converting
//! between the application's native types (`TodoItem`, `TodoState`, `Priority`)
//! and their FFI-safe counterparts (`FfiTodoItem`, `FfiTodoState`, `FfiPriority`).

use abi_stable::std_types::{ROption, RString};
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use totui_plugin_interface::{FfiPriority, FfiProjectContext, FfiTodoItem, FfiTodoState};
use uuid::Uuid;

use crate::project::Project;
use crate::todo::{Priority, TodoItem, TodoState};

// ============================================================================
// Project -> FfiProjectContext
// ============================================================================

impl From<&Project> for FfiProjectContext {
    fn from(project: &Project) -> Self {
        FfiProjectContext {
            id: project.id.to_string().into(),
            name: project.name.clone().into(),
            created_at: project.created_at.timestamp_millis(),
        }
    }
}

// ============================================================================
// TodoState <-> FfiTodoState
// ============================================================================

impl From<TodoState> for FfiTodoState {
    fn from(state: TodoState) -> Self {
        match state {
            TodoState::Empty => FfiTodoState::Empty,
            TodoState::Checked => FfiTodoState::Checked,
            TodoState::Question => FfiTodoState::Question,
            TodoState::Exclamation => FfiTodoState::Exclamation,
            TodoState::InProgress => FfiTodoState::InProgress,
            TodoState::Cancelled => FfiTodoState::Cancelled,
        }
    }
}

impl From<FfiTodoState> for TodoState {
    fn from(state: FfiTodoState) -> Self {
        match state {
            FfiTodoState::Empty => TodoState::Empty,
            FfiTodoState::Checked => TodoState::Checked,
            FfiTodoState::Question => TodoState::Question,
            FfiTodoState::Exclamation => TodoState::Exclamation,
            FfiTodoState::InProgress => TodoState::InProgress,
            FfiTodoState::Cancelled => TodoState::Cancelled,
        }
    }
}

// ============================================================================
// Priority <-> FfiPriority
// ============================================================================

impl From<Priority> for FfiPriority {
    fn from(priority: Priority) -> Self {
        match priority {
            Priority::P0 => FfiPriority::P0,
            Priority::P1 => FfiPriority::P1,
            Priority::P2 => FfiPriority::P2,
        }
    }
}

impl From<FfiPriority> for Priority {
    fn from(priority: FfiPriority) -> Self {
        match priority {
            FfiPriority::P0 => Priority::P0,
            FfiPriority::P1 => Priority::P1,
            FfiPriority::P2 => Priority::P2,
        }
    }
}

// ============================================================================
// TodoItem -> FfiTodoItem (infallible)
// ============================================================================

impl From<&TodoItem> for FfiTodoItem {
    fn from(item: &TodoItem) -> Self {
        FfiTodoItem {
            id: item.id.to_string().into(),
            content: item.content.clone().into(),
            state: item.state.into(),
            priority: item.priority.map(Into::into).into(),
            due_date: item
                .due_date
                .map(|d| d.format("%Y-%m-%d").to_string().into())
                .into(),
            description: item.description.clone().map(Into::into).into(),
            parent_id: item.parent_id.map(|u| u.to_string().into()).into(),
            indent_level: item.indent_level as u32,
            created_at: item.created_at.timestamp_millis(),
            modified_at: item.modified_at.timestamp_millis(),
            completed_at: item.completed_at.map(|dt| dt.timestamp_millis()).into(),
            // Position is set by host during query, default to 0
            position: 0,
        }
    }
}

// ============================================================================
// FfiTodoItem -> TodoItem (fallible)
// ============================================================================

impl TryFrom<FfiTodoItem> for TodoItem {
    type Error = anyhow::Error;

    fn try_from(ffi: FfiTodoItem) -> Result<Self> {
        let id = Uuid::parse_str(&ffi.id).with_context(|| format!("Invalid UUID: {}", ffi.id))?;

        let due_date: Option<NaiveDate> = if let ROption::RSome(ref date_str) = ffi.due_date {
            Some(
                NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                    .with_context(|| format!("Invalid due_date format: {}", date_str))?,
            )
        } else {
            None
        };

        let parent_id: Option<Uuid> = if let ROption::RSome(ref parent_str) = ffi.parent_id {
            Some(
                Uuid::parse_str(parent_str)
                    .with_context(|| format!("Invalid parent_id UUID: {}", parent_str))?,
            )
        } else {
            None
        };

        let created_at = timestamp_millis_to_datetime(ffi.created_at)
            .with_context(|| format!("Invalid created_at timestamp: {}", ffi.created_at))?;

        let modified_at = timestamp_millis_to_datetime(ffi.modified_at)
            .with_context(|| format!("Invalid modified_at timestamp: {}", ffi.modified_at))?;

        let completed_at: Option<DateTime<Utc>> =
            if let ROption::RSome(ts) = ffi.completed_at {
                Some(
                    timestamp_millis_to_datetime(ts)
                        .with_context(|| format!("Invalid completed_at timestamp: {}", ts))?,
                )
            } else {
                None
            };

        Ok(TodoItem {
            id,
            content: ffi.content.into(),
            state: ffi.state.into(),
            priority: Option::<FfiPriority>::from(ffi.priority).map(Into::into),
            due_date,
            description: Option::<RString>::from(ffi.description).map(Into::into),
            parent_id,
            indent_level: ffi.indent_level as usize,
            created_at,
            modified_at,
            completed_at,
            // UI-only field, default to false
            collapsed: false,
            // Host never passes deleted items to plugins
            deleted_at: None,
        })
    }
}

/// Convert Unix timestamp in milliseconds to DateTime<Utc>.
fn timestamp_millis_to_datetime(millis: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_millis_opt(millis).single()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_state_roundtrip() {
        let states = [
            TodoState::Empty,
            TodoState::Checked,
            TodoState::Question,
            TodoState::Exclamation,
            TodoState::InProgress,
            TodoState::Cancelled,
        ];

        for state in states {
            let ffi: FfiTodoState = state.into();
            let back: TodoState = ffi.into();
            assert_eq!(state, back);
        }
    }

    #[test]
    fn test_priority_roundtrip() {
        let priorities = [Priority::P0, Priority::P1, Priority::P2];

        for priority in priorities {
            let ffi: FfiPriority = priority.into();
            let back: Priority = ffi.into();
            assert_eq!(priority, back);
        }
    }

    #[test]
    fn test_todo_item_roundtrip() {
        let item = TodoItem::new("Test task".to_string(), 2);
        let ffi: FfiTodoItem = (&item).into();
        let back: TodoItem = ffi.try_into().unwrap();

        assert_eq!(item.id, back.id);
        assert_eq!(item.content, back.content);
        assert_eq!(item.state, back.state);
        assert_eq!(item.indent_level, back.indent_level);
        assert_eq!(item.priority, back.priority);
        assert_eq!(item.due_date, back.due_date);
        assert_eq!(item.description, back.description);
        assert_eq!(item.parent_id, back.parent_id);
        // Timestamps may lose sub-millisecond precision but should be within 1ms
        assert!(
            (item.created_at.timestamp_millis() - back.created_at.timestamp_millis()).abs() <= 1
        );
        assert!(
            (item.modified_at.timestamp_millis() - back.modified_at.timestamp_millis()).abs() <= 1
        );
        // UI-only fields are reset
        assert!(!back.collapsed);
        assert!(back.deleted_at.is_none());
    }

    #[test]
    fn test_todo_item_with_optional_fields_roundtrip() {
        let mut item = TodoItem::new("Task with details".to_string(), 1);
        item.priority = Some(Priority::P0);
        item.due_date = Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
        item.description = Some("A longer description".to_string());
        item.parent_id = Some(Uuid::new_v4());
        item.state = TodoState::Checked;
        item.completed_at = Some(Utc::now());

        let ffi: FfiTodoItem = (&item).into();
        let back: TodoItem = ffi.try_into().unwrap();

        assert_eq!(item.id, back.id);
        assert_eq!(item.content, back.content);
        assert_eq!(item.state, back.state);
        assert_eq!(item.priority, back.priority);
        assert_eq!(item.due_date, back.due_date);
        assert_eq!(item.description, back.description);
        assert_eq!(item.parent_id, back.parent_id);
        assert!(back.completed_at.is_some());
    }

    #[test]
    fn test_invalid_uuid_returns_error() {
        let ffi = FfiTodoItem {
            id: "not-a-uuid".into(),
            content: "Test".into(),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            due_date: ROption::RNone,
            description: ROption::RNone,
            parent_id: ROption::RNone,
            indent_level: 0,
            created_at: Utc::now().timestamp_millis(),
            modified_at: Utc::now().timestamp_millis(),
            completed_at: ROption::RNone,
            position: 0,
        };

        let result: Result<TodoItem> = ffi.try_into();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid UUID"));
    }

    #[test]
    fn test_invalid_date_returns_error() {
        let ffi = FfiTodoItem {
            id: Uuid::new_v4().to_string().into(),
            content: "Test".into(),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            due_date: ROption::RSome("not-a-date".into()),
            description: ROption::RNone,
            parent_id: ROption::RNone,
            indent_level: 0,
            created_at: Utc::now().timestamp_millis(),
            modified_at: Utc::now().timestamp_millis(),
            completed_at: ROption::RNone,
            position: 0,
        };

        let result: Result<TodoItem> = ffi.try_into();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid due_date format"));
    }
}
