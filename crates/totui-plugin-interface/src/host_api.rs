//! FFI-safe types for the Host API.
//!
//! This module defines the types and trait used by plugins to query and mutate
//! todos through the host application. All types are FFI-safe via abi_stable.

use abi_stable::sabi_trait;
use abi_stable::std_types::{ROption, RString, RVec};
use abi_stable::StableAbi;

use crate::types::{FfiPriority, FfiTodoItem, FfiTodoState};

// ============================================================================
// FfiCommand - Commands plugins emit to mutate todos
// ============================================================================

/// FFI-safe command enum for plugin mutations.
///
/// Plugins return these commands to request changes to the todo list.
/// The host interprets and executes them, preserving undo/redo history.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum FfiCommand {
    /// Create a new todo item.
    CreateTodo {
        /// Todo content text
        content: RString,
        /// Optional parent item UUID (for nested todos)
        parent_id: ROption<RString>,
        /// Optional temporary ID for correlation (plugin-assigned)
        temp_id: ROption<RString>,
        /// Initial state
        state: FfiTodoState,
        /// Optional priority
        priority: ROption<FfiPriority>,
        /// Indentation level
        indent_level: u32,
    },
    /// Update an existing todo item.
    UpdateTodo {
        /// UUID of item to update
        id: RString,
        /// New content (if changing)
        content: ROption<RString>,
        /// New state (if changing)
        state: ROption<FfiTodoState>,
        /// New priority (if changing)
        priority: ROption<FfiPriority>,
        /// New due date in YYYY-MM-DD format (if changing)
        due_date: ROption<RString>,
        /// New description (if changing)
        description: ROption<RString>,
    },
    /// Delete a todo item.
    DeleteTodo {
        /// UUID of item to delete
        id: RString,
    },
    /// Move a todo item to a new position.
    MoveTodo {
        /// UUID of item to move
        id: RString,
        /// Target position
        position: FfiMovePosition,
    },

    /// Set metadata for a todo item.
    SetTodoMetadata {
        /// UUID of the todo
        todo_id: RString,
        /// JSON data as string (validated by host)
        data: RString,
        /// If true, merge with existing data; if false, replace entirely
        merge: bool,
    },

    /// Set metadata for a project.
    SetProjectMetadata {
        /// Project name
        project_name: RString,
        /// JSON data as string (validated by host)
        data: RString,
        /// If true, merge with existing data; if false, replace entirely
        merge: bool,
    },

    /// Delete metadata for a todo item.
    DeleteTodoMetadata {
        /// UUID of the todo
        todo_id: RString,
    },

    /// Delete metadata for a project.
    DeleteProjectMetadata {
        /// Project name
        project_name: RString,
    },
}

// ============================================================================
// FfiMovePosition - Position specifier for move operations
// ============================================================================

/// FFI-safe position specifier for move operations.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum FfiMovePosition {
    /// Move before another item.
    Before {
        /// UUID of target item
        target_id: RString,
    },
    /// Move after another item.
    After {
        /// UUID of target item
        target_id: RString,
    },
    /// Move to a specific index.
    AtIndex {
        /// Target index in the list
        index: u32,
    },
}

// ============================================================================
// FfiProjectContext - Project information for plugins
// ============================================================================

/// FFI-safe project context.
///
/// Provides plugins with information about the current project.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiProjectContext {
    /// Project UUID as string
    pub id: RString,
    /// Project display name
    pub name: RString,
    /// Creation timestamp (Unix millis)
    pub created_at: i64,
}

// ============================================================================
// FfiTodoQuery - Query parameters for filtering todos
// ============================================================================

/// FFI-safe query parameters for filtering todos.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiTodoQuery {
    /// Project to query (None = current project)
    pub project: ROption<RString>,
    /// Filter by state
    pub state_filter: ROption<FfiStateFilter>,
    /// Filter by parent UUID
    pub parent_id: ROption<RString>,
    /// Include soft-deleted items
    pub include_deleted: bool,
    /// Filter by date range start (YYYY-MM-DD)
    pub date_from: ROption<RString>,
    /// Filter by date range end (YYYY-MM-DD)
    pub date_to: ROption<RString>,
}

impl Default for FfiTodoQuery {
    fn default() -> Self {
        Self {
            project: ROption::RNone,
            state_filter: ROption::RNone,
            parent_id: ROption::RNone,
            include_deleted: false,
            date_from: ROption::RNone,
            date_to: ROption::RNone,
        }
    }
}

// ============================================================================
// FfiStateFilter - State filter enum
// ============================================================================

/// FFI-safe state filter for queries.
#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiStateFilter {
    /// Only completed (Checked) items
    Done = 0,
    /// Only pending (non-Checked) items
    Pending = 1,
    /// All items regardless of state
    All = 2,
}

// ============================================================================
// FfiTodoNode - Tree node for hierarchical queries
// ============================================================================

/// FFI-safe tree node for hierarchical todo queries.
///
/// Contains a todo item and its children, enabling tree-structured results.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiTodoNode {
    /// The todo item at this node
    pub item: FfiTodoItem,
    /// Child nodes
    pub children: RVec<FfiTodoNode>,
    /// Position in the list (0-indexed)
    pub position: u32,
}

// ============================================================================
// FfiTodoMetadata - Result type for batch metadata queries
// ============================================================================

/// FFI-safe metadata result for batch queries.
///
/// Contains a todo ID and its associated metadata JSON string.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiTodoMetadata {
    /// UUID of the todo as string
    pub todo_id: RString,
    /// JSON metadata string (empty {} if no metadata)
    pub data: RString,
}

// ============================================================================
// HostApi - The trait plugins use to interact with the host
// ============================================================================

/// The Host API trait that the host implements for plugins.
///
/// Plugins receive a reference to this trait to query the todo list.
/// The `#[sabi_trait]` attribute generates `HostApi_TO`, a type-erased
/// FFI-safe trait object.
#[sabi_trait]
pub trait HostApi: Send + Sync {
    /// Get current project context.
    fn current_project(&self) -> FfiProjectContext;

    /// List all available projects (where this plugin is enabled).
    fn list_projects(&self) -> RVec<FfiProjectContext>;

    /// Query todos with filters.
    fn query_todos(&self, query: FfiTodoQuery) -> RVec<FfiTodoItem>;

    /// Get a single todo by UUID.
    fn get_todo(&self, id: RString) -> ROption<FfiTodoItem>;

    /// Get todos as tree structure (children nested under parents).
    fn query_todos_tree(&self) -> RVec<FfiTodoNode>;

    /// Get metadata for a single todo (returns empty {} if none).
    /// Plugin only sees its own metadata (auto-namespaced by plugin name).
    fn get_todo_metadata(&self, todo_id: RString) -> RString;

    /// Get metadata for multiple todos (batch operation).
    /// Returns vec of (todo_id, data) pairs. Missing metadata returns {} data.
    fn get_todo_metadata_batch(&self, todo_ids: RVec<RString>) -> RVec<FfiTodoMetadata>;

    /// Get metadata for a project (returns empty {} if none).
    fn get_project_metadata(&self, project_name: RString) -> RString;

    /// Query todos that have metadata matching a key/value.
    /// The key is a JSON path (e.g., "ticket_id"), value is a JSON value string.
    fn query_todos_by_metadata(&self, key: RString, value: RString) -> RVec<FfiTodoItem>;

    /// List projects that have metadata for this plugin.
    #[sabi(last_prefix_field)]
    fn list_projects_with_metadata(&self) -> RVec<RString>;
}
