//! FFI-safe event types for plugin hooks.
//!
//! This module defines the event types that plugins can subscribe to and handle.
//! Events are emitted by the host when todo items are created, modified, completed,
//! deleted, or when a project is loaded.

use abi_stable::std_types::{RString, RVec};
use abi_stable::StableAbi;

use crate::host_api::FfiCommand;
use crate::types::FfiTodoItem;

// ============================================================================
// FfiEventType - Event type enum for subscription
// ============================================================================

/// FFI-safe event type enum for subscription filtering.
///
/// Plugins use this to declare which events they want to receive.
#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiEventType {
    /// Emitted when a new todo is added.
    OnAdd = 0,
    /// Emitted when a todo is modified.
    OnModify = 1,
    /// Emitted when a todo is marked complete.
    OnComplete = 2,
    /// Emitted when a todo is deleted.
    OnDelete = 3,
    /// Emitted when a project is loaded.
    OnLoad = 4,
}

// ============================================================================
// FfiEventSource - Source of the event
// ============================================================================

/// FFI-safe enum indicating the source of an event.
///
/// Allows plugins to filter or handle events differently based on origin.
#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiEventSource {
    /// User action via TUI.
    Manual = 0,
    /// Daily rollover process.
    Rollover = 1,
    /// Another plugin created/modified the item.
    Plugin = 2,
    /// REST API action.
    Api = 3,
}

// ============================================================================
// FfiFieldChange - Which field was modified
// ============================================================================

/// FFI-safe enum indicating which field was changed in a modify event.
#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiFieldChange {
    /// Content text was changed.
    Content = 0,
    /// State was changed.
    State = 1,
    /// Due date was changed.
    DueDate = 2,
    /// Priority was changed.
    Priority = 3,
    /// Description was changed.
    Description = 4,
    /// Indent level was changed.
    Indent = 5,
    /// Parent ID was changed.
    Parent = 6,
    /// Multiple fields were changed at once.
    Multiple = 7,
}

// ============================================================================
// FfiEvent - The main event enum
// ============================================================================

/// FFI-safe event enum for plugin hooks.
///
/// Each variant represents a different lifecycle event that plugins can respond to.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum FfiEvent {
    /// A new todo item was added.
    OnAdd {
        /// The newly added todo item.
        todo: FfiTodoItem,
        /// Source of the add operation.
        source: FfiEventSource,
    },
    /// A todo item was modified.
    OnModify {
        /// The modified todo item (with new values).
        todo: FfiTodoItem,
        /// Which field was changed.
        field_changed: FfiFieldChange,
    },
    /// A todo item was marked complete.
    OnComplete {
        /// The completed todo item.
        todo: FfiTodoItem,
    },
    /// A todo item was deleted.
    OnDelete {
        /// The deleted todo item (before deletion).
        todo: FfiTodoItem,
    },
    /// A project was loaded (at startup or project switch).
    OnLoad {
        /// Name of the loaded project.
        project_name: RString,
        /// Current date in YYYY-MM-DD format.
        date: RString,
    },
}

// ============================================================================
// FfiHookResponse - Response from event handler
// ============================================================================

/// FFI-safe response from a plugin's event handler.
///
/// Contains commands to be executed in response to the event.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiHookResponse {
    /// Commands to execute in response to the event.
    pub commands: RVec<FfiCommand>,
}

impl Default for FfiHookResponse {
    fn default() -> Self {
        Self {
            commands: RVec::new(),
        }
    }
}

// ============================================================================
// FfiEvent helpers
// ============================================================================

impl FfiEvent {
    /// Get the event type for this event.
    pub fn event_type(&self) -> FfiEventType {
        match self {
            FfiEvent::OnAdd { .. } => FfiEventType::OnAdd,
            FfiEvent::OnModify { .. } => FfiEventType::OnModify,
            FfiEvent::OnComplete { .. } => FfiEventType::OnComplete,
            FfiEvent::OnDelete { .. } => FfiEventType::OnDelete,
            FfiEvent::OnLoad { .. } => FfiEventType::OnLoad,
        }
    }

    /// Get the todo item if this event contains one.
    ///
    /// Returns `Some` for OnAdd, OnModify, OnComplete, OnDelete events.
    /// Returns `None` for OnLoad events (which don't carry a todo).
    pub fn todo(&self) -> Option<&FfiTodoItem> {
        match self {
            FfiEvent::OnAdd { todo, .. } => Some(todo),
            FfiEvent::OnModify { todo, .. } => Some(todo),
            FfiEvent::OnComplete { todo } => Some(todo),
            FfiEvent::OnDelete { todo } => Some(todo),
            FfiEvent::OnLoad { .. } => None,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FfiTodoState;
    use abi_stable::std_types::ROption;

    fn make_test_todo() -> FfiTodoItem {
        FfiTodoItem {
            id: "test-uuid-123".into(),
            content: "Test todo content".into(),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            due_date: ROption::RNone,
            description: ROption::RNone,
            parent_id: ROption::RNone,
            indent_level: 0,
            created_at: 1706000000000,
            modified_at: 1706000000000,
            completed_at: ROption::RNone,
            position: 0,
        }
    }

    #[test]
    fn test_event_type_on_add() {
        let todo = make_test_todo();
        let event = FfiEvent::OnAdd {
            todo,
            source: FfiEventSource::Manual,
        };
        assert!(matches!(event.event_type(), FfiEventType::OnAdd));
    }

    #[test]
    fn test_event_type_on_modify() {
        let todo = make_test_todo();
        let event = FfiEvent::OnModify {
            todo,
            field_changed: FfiFieldChange::Content,
        };
        assert!(matches!(event.event_type(), FfiEventType::OnModify));
    }

    #[test]
    fn test_event_type_on_complete() {
        let todo = make_test_todo();
        let event = FfiEvent::OnComplete { todo };
        assert!(matches!(event.event_type(), FfiEventType::OnComplete));
    }

    #[test]
    fn test_event_type_on_delete() {
        let todo = make_test_todo();
        let event = FfiEvent::OnDelete { todo };
        assert!(matches!(event.event_type(), FfiEventType::OnDelete));
    }

    #[test]
    fn test_event_type_on_load() {
        let event = FfiEvent::OnLoad {
            project_name: "test-project".into(),
            date: "2026-01-26".into(),
        };
        assert!(matches!(event.event_type(), FfiEventType::OnLoad));
    }

    #[test]
    fn test_todo_returns_some_for_add() {
        let todo = make_test_todo();
        let expected_id = todo.id.clone();
        let event = FfiEvent::OnAdd {
            todo,
            source: FfiEventSource::Manual,
        };
        let extracted = event.todo();
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap().id, expected_id);
    }

    #[test]
    fn test_todo_returns_some_for_modify() {
        let todo = make_test_todo();
        let event = FfiEvent::OnModify {
            todo,
            field_changed: FfiFieldChange::State,
        };
        assert!(event.todo().is_some());
    }

    #[test]
    fn test_todo_returns_some_for_complete() {
        let todo = make_test_todo();
        let event = FfiEvent::OnComplete { todo };
        assert!(event.todo().is_some());
    }

    #[test]
    fn test_todo_returns_some_for_delete() {
        let todo = make_test_todo();
        let event = FfiEvent::OnDelete { todo };
        assert!(event.todo().is_some());
    }

    #[test]
    fn test_todo_returns_none_for_load() {
        let event = FfiEvent::OnLoad {
            project_name: "test".into(),
            date: "2026-01-26".into(),
        };
        assert!(event.todo().is_none());
    }

    #[test]
    fn test_hook_response_default() {
        let response = FfiHookResponse::default();
        assert!(response.commands.is_empty());
    }

    #[test]
    fn test_event_source_variants() {
        assert_eq!(FfiEventSource::Manual as u8, 0);
        assert_eq!(FfiEventSource::Rollover as u8, 1);
        assert_eq!(FfiEventSource::Plugin as u8, 2);
        assert_eq!(FfiEventSource::Api as u8, 3);
    }

    #[test]
    fn test_field_change_variants() {
        assert_eq!(FfiFieldChange::Content as u8, 0);
        assert_eq!(FfiFieldChange::State as u8, 1);
        assert_eq!(FfiFieldChange::DueDate as u8, 2);
        assert_eq!(FfiFieldChange::Priority as u8, 3);
        assert_eq!(FfiFieldChange::Description as u8, 4);
        assert_eq!(FfiFieldChange::Indent as u8, 5);
        assert_eq!(FfiFieldChange::Parent as u8, 6);
        assert_eq!(FfiFieldChange::Multiple as u8, 7);
    }
}
