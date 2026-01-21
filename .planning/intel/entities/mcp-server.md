# MCP Server

**Directory**: `src/mcp/`

## Purpose

Model Context Protocol server enabling AI/LLM integration with the todo system. Provides structured tools for reading and manipulating todos.

## Components

### server.rs
```rust
pub struct TodoMcpServer;

impl ServerHandler for TodoMcpServer {
    // Implements rmcp::ServerHandler trait
}
```

### Tools Provided

1. **list_todos**
   - Input: `{ date?: string }` (YYYY-MM-DD, defaults to today)
   - Output: Formatted markdown list of todos
   - Shows hierarchy, states, due dates

2. **create_todo**
   - Input: `{ content: string, parent_id?: uuid, due_date?: string, description?: string }`
   - Output: Created todo with UUID
   - Supports nesting under parent

3. **update_todo**
   - Input: `{ id: uuid, content?: string, state?: string, due_date?: string, description?: string }`
   - Output: Updated todo
   - Partial updates supported

4. **delete_todo**
   - Input: `{ id: uuid }`
   - Output: Confirmation
   - Deletes item and all children

5. **mark_complete**
   - Input: `{ id: uuid }`
   - Output: Updated todo
   - Toggles between done/pending

### schemas.rs
JSON Schema definitions for each tool's input/output using `schemars`.

### errors.rs
- `McpErrorDetail` - Structured error info
- `IntoMcpError` trait - Convert errors to MCP format

## Binary

`src/bin/totui-mcp.rs` - Standalone MCP server binary for use with Claude Desktop, OpenCode, etc.
