# Phase 10: Metadata & Database - Research

**Researched:** 2026-01-26
**Domain:** Plugin metadata storage with SQLite JSON functions
**Confidence:** HIGH

## Summary

This phase enables plugins to persist custom JSON metadata attached to todos and projects. The research confirms that SQLite's JSON functions are built into the bundled version used by rusqlite 0.38, eliminating the need for additional dependencies. The established patterns in the codebase (FfiCommand for mutations, undo/redo snapshots, soft deletes) naturally extend to metadata operations.

The key architectural insight is that metadata should be stored in separate tables (not embedded in todo/project columns) to maintain clean separation between core data and plugin data. This allows namespace-isolated queries using SQLite's `json_extract()` and `json_patch()` functions while keeping the existing database schema stable.

**Primary recommendation:** Create `todo_metadata` and `project_metadata` tables with JSON columns keyed by (item_id, plugin_name), use `json_patch()` for merge operations and `json_set()` for replace operations, add new `FfiMetadataCommand` variants for metadata CRUD, and integrate with the existing undo/redo system by including metadata operations in the command batch.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rusqlite | 0.38 | SQLite database access | Already in use, JSON1 built into bundled SQLite (since 3.38.0) |
| serde_json | 1.0 | JSON serialization | Already in project, used for parsing/validating metadata |
| abi_stable | 0.11 | FFI-safe types | Already in use, can pass JSON as RString |
| uuid | 1.11 | UUID handling | Already in use for item/project IDs |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| RString | abi_stable | FFI-safe String | Pass JSON strings across FFI boundary |
| RResult | abi_stable | FFI-safe Result | Return errors from metadata operations |
| RVec | abi_stable | FFI-safe Vec | Batch metadata query results |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| JSON column | Separate columns per field | JSON is flexible for plugin-specific data; columns would require schema migration |
| Separate metadata tables | Embedded JSON in todo/project tables | Separate tables keep core schema clean, easier to query by plugin |
| json_patch() for merge | Manual key-by-key update | json_patch() is atomic, handles nested objects correctly |
| RString for JSON | RawValueBox (abi_stable serde_json feature) | RString is simpler, validation happens host-side anyway |

**Installation:** No new dependencies needed. SQLite JSON functions are enabled by default in rusqlite's bundled SQLite (version 3.38.0+).

## Architecture Patterns

### Recommended Module Structure

```
src/storage/
    database.rs         # Existing - add metadata table init
    metadata.rs         # NEW - metadata CRUD operations
crates/totui-plugin-interface/src/
    host_api.rs         # Extend HostApi trait with metadata methods
    metadata.rs         # NEW - FfiMetadataCommand, FfiMetadataQuery types
src/plugin/
    command_executor.rs # Extend to handle FfiMetadataCommand
    host_impl.rs        # Implement metadata query methods
```

### Pattern 1: Namespaced Metadata Tables

**What:** Separate tables for todo and project metadata, keyed by (item_id, plugin_name)
**When to use:** All metadata storage - maintains clean separation, enables efficient per-plugin queries

**Schema:**
```sql
CREATE TABLE IF NOT EXISTS todo_metadata (
    id TEXT PRIMARY KEY,                    -- UUID
    todo_id TEXT NOT NULL,                  -- FK to todos.id
    plugin_name TEXT NOT NULL,              -- Namespace key from plugin.toml
    data TEXT NOT NULL DEFAULT '{}',        -- JSON blob
    created_at TEXT NOT NULL,               -- RFC3339
    updated_at TEXT NOT NULL,               -- RFC3339
    UNIQUE(todo_id, plugin_name)
);
CREATE INDEX idx_todo_metadata_todo ON todo_metadata(todo_id);
CREATE INDEX idx_todo_metadata_plugin ON todo_metadata(plugin_name);

CREATE TABLE IF NOT EXISTS project_metadata (
    id TEXT PRIMARY KEY,                    -- UUID
    project_name TEXT NOT NULL,             -- FK to projects.name
    plugin_name TEXT NOT NULL,              -- Namespace key
    data TEXT NOT NULL DEFAULT '{}',        -- JSON blob
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(project_name, plugin_name)
);
CREATE INDEX idx_project_metadata_project ON project_metadata(project_name);
CREATE INDEX idx_project_metadata_plugin ON project_metadata(plugin_name);
```

### Pattern 2: FfiMetadataCommand for Undo Integration

**What:** Extend FfiCommand enum with metadata operations
**When to use:** All metadata mutations - preserves single undo point for batch operations

**Design:**
```rust
// In totui-plugin-interface crate, extend FfiCommand

#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum FfiCommand {
    // ... existing variants ...

    /// Set metadata for a todo item
    SetTodoMetadata {
        /// UUID of the todo
        todo_id: RString,
        /// JSON data as string (validated by host)
        data: RString,
        /// If true, merge with existing data; if false, replace entirely
        merge: bool,
    },

    /// Set metadata for a project
    SetProjectMetadata {
        /// Project name
        project_name: RString,
        /// JSON data as string
        data: RString,
        /// If true, merge with existing data; if false, replace entirely
        merge: bool,
    },

    /// Delete metadata for a todo item
    DeleteTodoMetadata {
        todo_id: RString,
    },

    /// Delete metadata for a project
    DeleteProjectMetadata {
        project_name: RString,
    },
}
```

### Pattern 3: HostApi Metadata Query Methods

**What:** Extend HostApi trait with metadata query methods
**When to use:** Plugin needs to read its own metadata

**Design:**
```rust
// Extend the HostApi trait in host_api.rs

#[sabi_trait]
pub trait HostApi: Send + Sync {
    // ... existing methods ...

    /// Get metadata for a single todo (returns empty {} if none)
    fn get_todo_metadata(&self, todo_id: RString) -> RString;

    /// Get metadata for multiple todos (batch operation)
    fn get_todo_metadata_batch(&self, todo_ids: RVec<RString>) -> RVec<FfiTodoMetadata>;

    /// Get metadata for a project (returns empty {} if none)
    fn get_project_metadata(&self, project_name: RString) -> RString;

    /// Query todos that have metadata matching a key/value
    fn query_todos_by_metadata(&self, key: RString, value: RString) -> RVec<FfiTodoItem>;

    /// List projects that have metadata for this plugin
    #[sabi(last_prefix_field)]
    fn list_projects_with_metadata(&self) -> RVec<RString>;
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiTodoMetadata {
    pub todo_id: RString,
    pub data: RString,  // JSON string, empty {} if no metadata
}
```

### Pattern 4: JSON Merge vs Replace

**What:** Use `json_patch()` for merge, direct assignment for replace
**When to use:** `set_metadata(todo_id, data, merge: bool)` per CONTEXT.md decision

**SQL Examples:**
```sql
-- Replace: Simply overwrite
UPDATE todo_metadata SET data = ?1, updated_at = ?2 WHERE todo_id = ?3 AND plugin_name = ?4

-- Merge: Use json_patch (RFC 7396 MergePatch)
UPDATE todo_metadata
SET data = json_patch(data, ?1), updated_at = ?2
WHERE todo_id = ?3 AND plugin_name = ?4

-- Notes on json_patch behavior:
-- json_patch('{"a":1,"b":2}', '{"c":3}')       -> '{"a":1,"b":2,"c":3}'  (add key)
-- json_patch('{"a":1,"b":2}', '{"a":9}')       -> '{"a":9,"b":2}'        (update key)
-- json_patch('{"a":1,"b":2}', '{"b":null}')    -> '{"a":1}'              (delete key)
```

### Pattern 5: Metadata Query with json_extract

**What:** Use `json_extract()` for querying by metadata values
**When to use:** `query_todos_by_metadata(key, value)` searches

**SQL Example:**
```sql
-- Find todos where metadata.ticket_id = 'JIRA-123'
SELECT t.id, t.content, t.state, ...
FROM todos t
JOIN todo_metadata m ON t.id = m.todo_id
WHERE m.plugin_name = ?1  -- Always filter by current plugin
  AND json_extract(m.data, ?2) = ?3  -- ?2 = '$.ticket_id', ?3 = 'JIRA-123'
  AND t.deleted_at IS NULL;

-- Prefix query: Find all keys starting with 'jira.'
SELECT t.id, t.content, ...
FROM todos t
JOIN todo_metadata m ON t.id = m.todo_id
CROSS JOIN json_each(m.data)
WHERE m.plugin_name = ?1
  AND json_each.key LIKE ?2  -- ?2 = 'jira.%'
  AND t.deleted_at IS NULL;
```

### Anti-Patterns to Avoid

- **Storing metadata in todo row itself:** Violates separation of concerns, complicates schema, metadata doesn't survive schema changes
- **Plugin reading other plugins' metadata:** Always filter by current plugin name, never expose cross-namespace access
- **Keys starting with `_`:** Reserved for future host use per CONTEXT.md; reject at validation
- **Returning null for missing metadata:** Per CONTEXT.md, always return `{}` (empty JSON object)
- **Skipping JSON validation:** Always validate JSON before storage to catch malformed data early

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON merge/patch | Manual key iteration | `json_patch()` SQL function | Handles nested objects, null deletion, edge cases |
| JSON extraction | String parsing | `json_extract()` SQL function | Handles escaping, types, paths correctly |
| JSON validation | Regex or custom parser | `serde_json::from_str()` | Comprehensive, handles all edge cases |
| Namespace isolation | Manual filtering | SQL WHERE clause on plugin_name | Always enforced at query level |
| Undo for metadata | Separate undo stack | Include in FfiCommand batch | Atomic with todo changes |

**Key insight:** SQLite's JSON functions (built-in since 3.38.0) handle all the complex JSON manipulation. The host only needs to validate JSON before storage and construct the right SQL queries.

## Common Pitfalls

### Pitfall 1: Undo Granularity for Metadata

**What goes wrong:** Metadata changes create separate undo entries from todo changes
**Why it happens:** Treating metadata as separate from todo operations
**How to avoid:** Include `SetTodoMetadata`/`SetProjectMetadata` in same FfiCommand batch as todo operations; single `save_undo()` covers all
**Warning signs:** User needs multiple undos to reverse one plugin action

### Pitfall 2: Missing Metadata on Todo Duplication

**What goes wrong:** Duplicated todo doesn't have its metadata copied
**Why it happens:** Forgetting to copy metadata when handling rollover or manual duplication
**How to avoid:** When creating new todo from existing (rollover, duplicate), copy corresponding metadata row with new todo_id
**Warning signs:** Plugin loses context after rollover

### Pitfall 3: Orphaned Metadata After Hard Delete

**What goes wrong:** Metadata rows remain after todo is permanently deleted
**Why it happens:** Not cascading deletes or not cleaning up during archival
**How to avoid:**
- Soft delete: Keep metadata (recoverable)
- Archive: Move metadata to archived table with todo
- Hard delete (rare): CASCADE or explicit cleanup
**Warning signs:** Growing metadata table, queries returning stale data

### Pitfall 4: Reserved Key Prefix Validation

**What goes wrong:** Plugin stores keys starting with `_`, future host feature conflicts
**Why it happens:** Not validating JSON keys before storage
**How to avoid:** In `set_metadata` host implementation, parse JSON and reject if any top-level key starts with `_`
**Warning signs:** Storage succeeds but data semantics break later

### Pitfall 5: Empty Object vs Null Confusion

**What goes wrong:** Plugin code doesn't handle missing metadata correctly
**Why it happens:** Returning `null`, `None`, or empty string instead of `{}`
**How to avoid:** Per CONTEXT.md, `get_metadata` always returns `{}` for missing entries, never null
**Warning signs:** Plugin code has special-case null handling scattered throughout

### Pitfall 6: JSON Type Mismatch in Queries

**What goes wrong:** `query_todos_by_metadata("count", "5")` doesn't match `{"count": 5}`
**Why it happens:** JSON string "5" vs JSON number 5 are different types
**How to avoid:** Document that value parameter in `query_todos_by_metadata` is a JSON value (parse with `json()`), not raw text
**Warning signs:** Queries return no results for numeric/boolean metadata values

## Code Examples

### Database Table Initialization

```rust
// src/storage/database.rs - add to init_database()

conn.execute(
    "CREATE TABLE IF NOT EXISTS todo_metadata (
        id TEXT PRIMARY KEY,
        todo_id TEXT NOT NULL,
        plugin_name TEXT NOT NULL,
        data TEXT NOT NULL DEFAULT '{}',
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        UNIQUE(todo_id, plugin_name)
    )",
    [],
)?;

conn.execute(
    "CREATE INDEX IF NOT EXISTS idx_todo_metadata_todo ON todo_metadata(todo_id)",
    [],
)?;

conn.execute(
    "CREATE INDEX IF NOT EXISTS idx_todo_metadata_plugin ON todo_metadata(plugin_name)",
    [],
)?;

// Similar for project_metadata table...
```

### Set Metadata with Merge Option

```rust
// src/storage/metadata.rs

pub fn set_todo_metadata(
    todo_id: &Uuid,
    plugin_name: &str,
    data: &str,
    merge: bool,
) -> Result<()> {
    // Validate JSON and check for reserved keys
    let value: serde_json::Value = serde_json::from_str(data)
        .with_context(|| format!("Invalid JSON: {}", data))?;

    if let serde_json::Value::Object(map) = &value {
        for key in map.keys() {
            if key.starts_with('_') {
                anyhow::bail!("Keys starting with '_' are reserved: {}", key);
            }
        }
    }

    let conn = get_connection()?;
    let now = Utc::now().to_rfc3339();
    let todo_id_str = todo_id.to_string();

    if merge {
        // Try update with merge first
        let updated = conn.execute(
            "UPDATE todo_metadata
             SET data = json_patch(data, ?1), updated_at = ?2
             WHERE todo_id = ?3 AND plugin_name = ?4",
            params![data, now, todo_id_str, plugin_name],
        )?;

        // If no row existed, insert
        if updated == 0 {
            conn.execute(
                "INSERT INTO todo_metadata (id, todo_id, plugin_name, data, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![Uuid::new_v4().to_string(), todo_id_str, plugin_name, data, now],
            )?;
        }
    } else {
        // Upsert with replace
        conn.execute(
            "INSERT INTO todo_metadata (id, todo_id, plugin_name, data, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(todo_id, plugin_name) DO UPDATE SET data = ?4, updated_at = ?5",
            params![Uuid::new_v4().to_string(), todo_id_str, plugin_name, data, now],
        )?;
    }

    Ok(())
}
```

### Get Metadata (Always Returns {} for Missing)

```rust
// src/storage/metadata.rs

pub fn get_todo_metadata(todo_id: &Uuid, plugin_name: &str) -> Result<String> {
    let conn = get_connection()?;
    let todo_id_str = todo_id.to_string();

    let result: Result<String, _> = conn.query_row(
        "SELECT data FROM todo_metadata WHERE todo_id = ?1 AND plugin_name = ?2",
        params![todo_id_str, plugin_name],
        |row| row.get(0),
    );

    match result {
        Ok(data) => Ok(data),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok("{}".to_string()),
        Err(e) => Err(e.into()),
    }
}
```

### Query Todos by Metadata Value

```rust
// src/storage/metadata.rs

pub fn query_todos_by_metadata(
    plugin_name: &str,
    key: &str,
    value: &str,
    date: NaiveDate,
    project_name: &str,
) -> Result<Vec<TodoItem>> {
    let conn = get_connection()?;
    let date_str = date.format("%Y-%m-%d").to_string();
    let json_path = format!("$.{}", key);

    // Parse value as JSON to handle type matching
    let json_value = format!("json('{}')", value.replace('\'', "''"));

    let mut stmt = conn.prepare(&format!(
        "SELECT t.id, t.content, t.state, t.indent_level, t.parent_id,
                t.due_date, t.description, t.priority, t.collapsed,
                t.created_at, t.updated_at, t.completed_at, t.deleted_at
         FROM todos t
         JOIN todo_metadata m ON t.id = m.todo_id
         WHERE m.plugin_name = ?1
           AND json_extract(m.data, ?2) = {}
           AND t.date = ?3
           AND t.project = ?4
           AND t.deleted_at IS NULL
         ORDER BY t.position ASC",
        json_value
    ))?;

    // ... query execution and mapping to TodoItem ...
}
```

### Copy Metadata on Todo Rollover

```rust
// src/storage/rollover.rs - extend rollover logic

// When copying incomplete todo to new day:
fn copy_todo_with_metadata(
    old_item: &TodoItem,
    new_item: &TodoItem,
    conn: &Connection,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    // Copy all metadata from old todo to new todo
    conn.execute(
        "INSERT INTO todo_metadata (id, todo_id, plugin_name, data, created_at, updated_at)
         SELECT ?1, ?2, plugin_name, data, ?3, ?3
         FROM todo_metadata WHERE todo_id = ?4",
        params![
            Uuid::new_v4().to_string(),
            new_item.id.to_string(),
            now,
            old_item.id.to_string()
        ],
    )?;

    Ok(())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| JSON1 extension opt-in | JSON built into SQLite core | SQLite 3.38.0 (2022-02) | No compile flags needed |
| Text JSON storage | JSONB binary format available | SQLite 3.45.0 (2024-01) | Performance option (text is fine for our use) |
| Manual JSON parsing | SQLite JSON functions | SQLite 3.9.0 (2015-10) | Rich query capabilities |

**Deprecated/outdated:**
- `-DSQLITE_ENABLE_JSON1` flag: No longer needed since SQLite 3.38.0
- WASM plugins for Rust-to-Rust: Per project decision, native FFI via abi_stable preferred

## Open Questions

### 1. Archived Todo Metadata

**What we know:** Todos move to `archived_todos` during rollover
**What's unclear:** Should metadata move to a separate `archived_todo_metadata` table, or should the FK just reference `archived_todos`?
**Recommendation:** Create `archived_todo_metadata` table, copy metadata during archive operation. This keeps archive queries clean and matches the existing pattern.

### 2. Metadata Size Limits

**What we know:** CONTEXT.md says "No size limit - trust plugins, SQLite handles it"
**What's unclear:** Should there be an advisory warning for very large metadata blobs?
**Recommendation:** Log a warning if metadata exceeds 64KB but don't reject. This provides visibility without breaking functionality.

### 3. Batch Operation Return Types

**What we know:** CONTEXT.md marks this as "Claude's discretion"
**What's unclear:** What should `set_metadata_batch` return?
**Recommendation:** Return `RVec<RResult<RString, RString>>` - one result per input item, success returns the todo_id, error returns the error message. This allows partial success handling.

## Sources

### Primary (HIGH confidence)

- [SQLite JSON Functions Documentation](https://www.sqlite.org/json1.html) - json_extract, json_patch, json_set behavior
- Codebase analysis: `src/storage/database.rs` - Existing SQLite patterns, table creation, soft deletes
- Codebase analysis: `crates/totui-plugin-interface/src/host_api.rs` - HostApi trait extension pattern
- Codebase analysis: `src/plugin/command_executor.rs` - FfiCommand handling, undo integration
- [rusqlite bundled feature](https://github.com/rusqlite/rusqlite) - SQLite 3.38.0+ has JSON built-in

### Secondary (MEDIUM confidence)

- [Beekeeper Studio SQLite JSON Guide](https://www.beekeeperstudio.io/blog/sqlite-json) - Best practices for JSON storage patterns
- [abi_stable documentation](https://docs.rs/abi_stable/latest/abi_stable/) - RString, serde_json feature for FFI

### Tertiary (LOW confidence)

- Phase 9 patterns extrapolated to metadata - Assumes same approach works for different data type

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Uses only existing dependencies, SQLite JSON is well-documented
- Architecture: HIGH - Follows established codebase patterns for tables, commands, undo
- Pitfalls: HIGH - Based on SQLite JSON documentation and codebase conventions

**Research date:** 2026-01-26
**Valid until:** 2026-02-26 (stable domain, no fast-moving external deps)
