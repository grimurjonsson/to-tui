//! FFI-safe type definitions for todo items.
//!
//! These types use abi_stable to ensure stable memory layout across
//! different compiler versions and dynamic library boundaries.

use abi_stable::std_types::{ROption, RString};
use abi_stable::StableAbi;

/// FFI-safe representation of a todo item state.
#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiTodoState {
    Empty = 0,
    Checked = 1,
    Question = 2,
    Exclamation = 3,
    InProgress = 4,
    Cancelled = 5,
}

/// FFI-safe representation of a priority level.
#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiPriority {
    P0 = 0,
    P1 = 1,
    P2 = 2,
}

/// FFI-safe representation of a todo item.
///
/// This struct mirrors the native `TodoItem` but uses FFI-safe types:
/// - `RString` instead of `String`
/// - `ROption` instead of `Option`
/// - `u32` instead of `usize` (platform-independent)
/// - `i64` timestamps instead of `DateTime<Utc>`
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiTodoItem {
    /// UUID as string
    pub id: RString,
    /// Todo item content
    pub content: RString,
    /// Current state
    pub state: FfiTodoState,
    /// Optional priority level
    pub priority: ROption<FfiPriority>,
    /// Due date in YYYY-MM-DD format
    pub due_date: ROption<RString>,
    /// Optional description
    pub description: ROption<RString>,
    /// Parent item UUID as string
    pub parent_id: ROption<RString>,
    /// Indentation level (u32 for FFI safety)
    pub indent_level: u32,
    /// Creation timestamp (Unix millis)
    pub created_at: i64,
    /// Last modification timestamp (Unix millis)
    pub modified_at: i64,
    /// Completion timestamp (Unix millis), if completed
    pub completed_at: ROption<i64>,
    /// Position in the list (0-indexed, set by host during query)
    pub position: u32,
}
