# REST API

**Directory**: `src/api/`

## Purpose

HTTP REST API (port 48372) providing programmatic access to todos. Auto-starts with TUI.

## Components

### routes.rs
```rust
pub fn create_router() -> Router {
    Router::new()
        .route("/todos", get(list_todos).post(create_todo))
        .route("/todos/:id", get(get_todo).put(update_todo).delete(delete_todo))
        .layer(CorsLayer::permissive())
}
```

### Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /todos | list_todos | Get all todos for date |
| POST | /todos | create_todo | Create new todo |
| GET | /todos/:id | get_todo | Get single todo |
| PUT | /todos/:id | update_todo | Update todo |
| DELETE | /todos/:id | delete_todo | Delete todo |

### handlers.rs
Request handlers that interact with storage layer:
- Query params: `date` (YYYY-MM-DD)
- JSON request/response bodies
- Error responses with appropriate status codes

### models.rs
```rust
pub struct CreateTodoRequest {
    pub content: String,
    pub parent_id: Option<Uuid>,
    pub due_date: Option<String>,
    pub description: Option<String>,
}

pub struct UpdateTodoRequest {
    pub content: Option<String>,
    pub state: Option<String>,
    pub due_date: Option<String>,
    pub description: Option<String>,
}

pub struct TodoResponse {
    pub id: Uuid,
    pub content: String,
    pub state: String,
    // ... all TodoItem fields
}
```

## CORS

Permissive CORS enabled for local development/integrations.
