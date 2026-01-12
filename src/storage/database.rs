use crate::todo::{TodoItem, TodoList, TodoState};
use crate::utils::paths::get_to_tui_dir;
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use uuid::Uuid;

/// Parse an RFC3339 timestamp string into a DateTime<Utc>
fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn get_db_path() -> Result<PathBuf> {
    let dir = get_to_tui_dir()?;
    Ok(dir.join("todos.db"))
}

/// Raw data extracted from a database row before conversion to TodoItem
struct TodoRowData {
    id_str: String,
    content: String,
    state_str: String,
    indent_level: usize,
    parent_id_str: Option<String>,
    due_date_str: Option<String>,
    description: Option<String>,
    collapsed: i32,
    created_at_str: Option<String>,
    updated_at_str: Option<String>,
    completed_at_str: Option<String>,
    deleted_at_str: Option<String>,
}

impl TodoRowData {
    fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        let indent_level: i64 = row.get(3)?;
        Ok(Self {
            id_str: row.get(0)?,
            content: row.get(1)?,
            state_str: row.get(2)?,
            indent_level: indent_level as usize,
            parent_id_str: row.get(4)?,
            due_date_str: row.get(5)?,
            description: row.get(6)?,
            collapsed: row.get(7).unwrap_or(0),
            created_at_str: row.get(8).ok(),
            updated_at_str: row.get(9).ok(),
            completed_at_str: row.get(10).ok().flatten(),
            deleted_at_str: row.get(11).ok().flatten(),
        })
    }

    fn into_todo_item(self) -> TodoItem {
        let id = Uuid::parse_str(&self.id_str).unwrap_or_else(|_| Uuid::new_v4());
        let state = TodoState::from_char(self.state_str.chars().next().unwrap_or(' '))
            .unwrap_or(TodoState::Empty);
        let parent_id = self.parent_id_str.and_then(|s| Uuid::parse_str(&s).ok());
        let due_date = self
            .due_date_str
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

        let mut todo = TodoItem::new(self.content, self.indent_level);
        todo.id = id;
        todo.state = state;
        todo.parent_id = parent_id;
        todo.due_date = due_date;
        todo.description = self.description;
        todo.collapsed = self.collapsed != 0;

        if let Some(s) = self.created_at_str
            && let Some(dt) = parse_rfc3339(&s) {
                todo.created_at = dt;
            }
        if let Some(s) = self.updated_at_str
            && let Some(dt) = parse_rfc3339(&s) {
                todo.modified_at = dt;
            }
        if let Some(s) = self.completed_at_str {
            todo.completed_at = parse_rfc3339(&s);
        }
        if let Some(s) = self.deleted_at_str {
            todo.deleted_at = parse_rfc3339(&s);
        }

        todo
    }
}

pub fn get_connection() -> Result<Connection> {
    let db_path = get_db_path()?;
    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database at {db_path:?}"))?;
    Ok(conn)
}

pub fn init_database() -> Result<()> {
    let conn = get_connection()?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS todos (
            id TEXT PRIMARY KEY,
            date TEXT NOT NULL,
            content TEXT NOT NULL,
            state TEXT NOT NULL,
            indent_level INTEGER NOT NULL,
            parent_id TEXT,
            due_date TEXT,
            description TEXT,
            collapsed INTEGER NOT NULL DEFAULT 0,
            position INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            completed_at TEXT,
            deleted_at TEXT
        )",
        [],
    )?;

    conn.execute(
        "ALTER TABLE todos ADD COLUMN collapsed INTEGER NOT NULL DEFAULT 0",
        [],
    )
    .ok();

    conn.execute("ALTER TABLE todos ADD COLUMN completed_at TEXT", [])
        .ok();

    conn.execute("ALTER TABLE todos ADD COLUMN deleted_at TEXT", [])
        .ok();

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_todos_date ON todos(date)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_todos_parent_id ON todos(parent_id)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS archived_todos (
            id TEXT PRIMARY KEY,
            original_date TEXT NOT NULL,
            archived_at TEXT NOT NULL,
            content TEXT NOT NULL,
            state TEXT NOT NULL,
            indent_level INTEGER NOT NULL,
            parent_id TEXT,
            due_date TEXT,
            description TEXT,
            collapsed INTEGER NOT NULL DEFAULT 0,
            position INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            completed_at TEXT,
            deleted_at TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_archived_todos_original_date ON archived_todos(original_date)",
        [],
    )?;

    conn.execute(
        "ALTER TABLE archived_todos ADD COLUMN completed_at TEXT",
        [],
    )
    .ok();

    conn.execute("ALTER TABLE archived_todos ADD COLUMN deleted_at TEXT", [])
        .ok();

    Ok(())
}

pub fn load_todos_for_date(date: NaiveDate) -> Result<Vec<TodoItem>> {
    let conn = get_connection()?;
    let date_str = date.format("%Y-%m-%d").to_string();

    let mut stmt = conn.prepare(
        "SELECT id, content, state, indent_level, parent_id, due_date, description, collapsed, created_at, updated_at, completed_at, deleted_at
         FROM todos
         WHERE date = ?1 AND deleted_at IS NULL
         ORDER BY position ASC",
    )?;

    let items = stmt.query_map([&date_str], TodoRowData::from_row)?;

    let mut result = Vec::new();
    for item in items {
        result.push(item?.into_todo_item());
    }

    Ok(result)
}

pub fn soft_delete_todos(ids: &[Uuid], date: NaiveDate) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    let conn = get_connection()?;
    let date_str = date.format("%Y-%m-%d").to_string();
    let now = chrono::Utc::now().to_rfc3339();

    for id in ids {
        let id_str = id.to_string();
        conn.execute(
            "UPDATE todos SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND date = ?3",
            params![now, id_str, date_str],
        )?;
    }

    Ok(())
}

pub fn save_todo_list(list: &TodoList) -> Result<()> {
    let conn = get_connection()?;
    let date_str = list.date.format("%Y-%m-%d").to_string();

    conn.execute(
        "DELETE FROM todos WHERE date = ?1 AND deleted_at IS NULL",
        [&date_str],
    )?;

    let mut stmt = conn.prepare(
        "INSERT INTO todos (id, date, content, state, indent_level, parent_id, due_date, description, collapsed, position, created_at, updated_at, completed_at, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
    )?;

    for (position, item) in list.items.iter().enumerate() {
        let id_str = item.id.to_string();
        let state_str = item.state.to_char().to_string();
        let parent_id_str = item.parent_id.map(|id| id.to_string());
        let due_date_str = item.due_date.map(|d| d.format("%Y-%m-%d").to_string());
        let collapsed_int: i32 = if item.collapsed { 1 } else { 0 };
        let created_at_str = item.created_at.to_rfc3339();
        let modified_at_str = item.modified_at.to_rfc3339();
        let completed_at_str = item.completed_at.map(|dt| dt.to_rfc3339());
        let deleted_at_str = item.deleted_at.map(|dt| dt.to_rfc3339());

        stmt.execute(params![
            id_str,
            date_str,
            item.content,
            state_str,
            item.indent_level as i64,
            parent_id_str,
            due_date_str,
            item.description,
            collapsed_int,
            position as i64,
            created_at_str,
            modified_at_str,
            completed_at_str,
            deleted_at_str,
        ])?;
    }

    Ok(())
}

pub fn has_todos_for_date(date: NaiveDate) -> Result<bool> {
    let conn = get_connection()?;
    let date_str = date.format("%Y-%m-%d").to_string();

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM todos WHERE date = ?1 AND deleted_at IS NULL",
        [&date_str],
        |row| row.get(0),
    )?;

    Ok(count > 0)
}

pub fn archive_todos_for_date(date: NaiveDate) -> Result<usize> {
    let conn = get_connection()?;
    let date_str = date.format("%Y-%m-%d").to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let count = conn.execute(
        "INSERT INTO archived_todos (id, original_date, archived_at, content, state, indent_level, parent_id, due_date, description, collapsed, position, created_at, updated_at, completed_at, deleted_at)
         SELECT id, date, ?1, content, state, indent_level, parent_id, due_date, description, collapsed, position, created_at, updated_at, completed_at, deleted_at
         FROM todos WHERE date = ?2",
        params![now, date_str],
    )?;

    conn.execute("DELETE FROM todos WHERE date = ?1", [&date_str])?;

    Ok(count)
}

pub fn load_archived_todos_for_date(date: NaiveDate) -> Result<Vec<TodoItem>> {
    let conn = get_connection()?;
    let date_str = date.format("%Y-%m-%d").to_string();

    let mut stmt = conn.prepare(
        "SELECT id, content, state, indent_level, parent_id, due_date, description, collapsed, created_at, updated_at, completed_at, deleted_at
         FROM archived_todos
         WHERE original_date = ?1 AND deleted_at IS NULL
         ORDER BY position ASC",
    )?;

    let items = stmt.query_map([&date_str], TodoRowData::from_row)?;

    let mut result = Vec::new();
    for item in items {
        result.push(item?.into_todo_item());
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todo::{TodoItem, TodoList, TodoState};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Connection) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let conn = Connection::open(&db_path).unwrap();

        conn.execute(
            "CREATE TABLE todos (
                id TEXT PRIMARY KEY,
                date TEXT NOT NULL,
                content TEXT NOT NULL,
                state TEXT NOT NULL,
                indent_level INTEGER NOT NULL,
                parent_id TEXT,
                due_date TEXT,
                description TEXT,
                position INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                completed_at TEXT,
                deleted_at TEXT
            )",
            [],
        )
        .unwrap();

        conn.execute("CREATE INDEX idx_todos_date ON todos(date)", [])
            .unwrap();

        conn.execute("CREATE INDEX idx_todos_parent_id ON todos(parent_id)", [])
            .unwrap();

        (temp_dir, conn)
    }

    fn create_test_list(date: NaiveDate) -> TodoList {
        TodoList::new(date, PathBuf::from("/tmp/test.md"))
    }

    fn save_to_test_db(conn: &Connection, list: &TodoList) {
        let date_str = list.date.format("%Y-%m-%d").to_string();

        conn.execute("DELETE FROM todos WHERE date = ?1", [&date_str])
            .unwrap();

        let mut stmt = conn.prepare(
            "INSERT INTO todos (id, date, content, state, indent_level, parent_id, due_date, description, position, created_at, updated_at, completed_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
        ).unwrap();

        for (position, item) in list.items.iter().enumerate() {
            let id_str = item.id.to_string();
            let state_str = item.state.to_char().to_string();
            let parent_id_str = item.parent_id.map(|id| id.to_string());
            let due_date_str = item.due_date.map(|d| d.format("%Y-%m-%d").to_string());
            let created_at_str = item.created_at.to_rfc3339();
            let modified_at_str = item.modified_at.to_rfc3339();
            let completed_at_str = item.completed_at.map(|dt| dt.to_rfc3339());
            let deleted_at_str = item.deleted_at.map(|dt| dt.to_rfc3339());

            stmt.execute(params![
                id_str,
                date_str,
                item.content,
                state_str,
                item.indent_level as i64,
                parent_id_str,
                due_date_str,
                item.description,
                position as i64,
                created_at_str,
                modified_at_str,
                completed_at_str,
                deleted_at_str,
            ])
            .unwrap();
        }
    }

    fn load_from_test_db(conn: &Connection, date: NaiveDate) -> Vec<TodoItem> {
        let date_str = date.format("%Y-%m-%d").to_string();

        let mut stmt = conn
            .prepare(
                "SELECT id, content, state, indent_level, parent_id, due_date, description 
             FROM todos 
             WHERE date = ?1 
             ORDER BY position ASC",
            )
            .unwrap();

        let items = stmt
            .query_map([&date_str], |row| {
                let id_str: String = row.get(0)?;
                let content: String = row.get(1)?;
                let state_str: String = row.get(2)?;
                let indent_level: i64 = row.get(3)?;
                let indent_level = indent_level as usize;
                let parent_id_str: Option<String> = row.get(4)?;
                let due_date_str: Option<String> = row.get(5)?;
                let description: Option<String> = row.get(6)?;

                Ok((
                    id_str,
                    content,
                    state_str,
                    indent_level,
                    parent_id_str,
                    due_date_str,
                    description,
                ))
            })
            .unwrap();

        let mut result = Vec::new();
        for item in items {
            let (
                id_str,
                content,
                state_str,
                indent_level,
                parent_id_str,
                due_date_str,
                description,
            ) = item.unwrap();

            let id = Uuid::parse_str(&id_str).unwrap();
            let state = TodoState::from_char(state_str.chars().next().unwrap()).unwrap();
            let parent_id = parent_id_str.and_then(|s| Uuid::parse_str(&s).ok());
            let due_date =
                due_date_str.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

            let mut todo = TodoItem::new(content, indent_level);
            todo.id = id;
            todo.state = state;
            todo.parent_id = parent_id;
            todo.due_date = due_date;
            todo.description = description;

            result.push(todo);
        }

        result
    }

    #[test]
    fn test_database_schema() {
        let (_temp_dir, conn) = setup_test_db();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"todos".to_string()));
    }

    #[test]
    fn test_save_and_load_preserves_order() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut list = create_test_list(date);
        list.add_item("First".to_string());
        list.add_item("Second".to_string());
        list.add_item("Third".to_string());

        // Store original IDs
        let original_ids: Vec<Uuid> = list.items.iter().map(|i| i.id).collect();

        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].content, "First");
        assert_eq!(loaded[1].content, "Second");
        assert_eq!(loaded[2].content, "Third");

        // Verify IDs are preserved
        assert_eq!(loaded[0].id, original_ids[0]);
        assert_eq!(loaded[1].id, original_ids[1]);
        assert_eq!(loaded[2].id, original_ids[2]);
    }

    #[test]
    fn test_move_item_up_updates_positions() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut list = create_test_list(date);
        list.add_item("First".to_string());
        list.add_item("Second".to_string());
        list.add_item("Third".to_string());

        let second_id = list.items[1].id;
        let first_id = list.items[0].id;

        // Move "Second" up (swap with "First")
        list.move_item_with_children_up(1).unwrap();

        // Verify in-memory order changed
        assert_eq!(list.items[0].content, "Second");
        assert_eq!(list.items[1].content, "First");
        assert_eq!(list.items[0].id, second_id);
        assert_eq!(list.items[1].id, first_id);

        // Save and reload
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        // Verify database order matches
        assert_eq!(loaded[0].content, "Second");
        assert_eq!(loaded[1].content, "First");
        assert_eq!(loaded[2].content, "Third");
        assert_eq!(loaded[0].id, second_id);
        assert_eq!(loaded[1].id, first_id);
    }

    #[test]
    fn test_move_item_down_updates_positions() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut list = create_test_list(date);
        list.add_item("First".to_string());
        list.add_item("Second".to_string());
        list.add_item("Third".to_string());

        let first_id = list.items[0].id;
        let second_id = list.items[1].id;

        // Move "First" down (swap with "Second")
        list.move_item_with_children_down(0).unwrap();

        // Verify in-memory order changed
        assert_eq!(list.items[0].content, "Second");
        assert_eq!(list.items[1].content, "First");

        // Save and reload
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        // Verify database order matches
        assert_eq!(loaded[0].content, "Second");
        assert_eq!(loaded[1].content, "First");
        assert_eq!(loaded[2].content, "Third");
        assert_eq!(loaded[0].id, second_id);
        assert_eq!(loaded[1].id, first_id);
    }

    #[test]
    fn test_indent_updates_parent_id_in_database() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut list = create_test_list(date);
        list.add_item("Parent".to_string());
        list.add_item("Child".to_string());

        let parent_id = list.items[0].id;
        let child_id = list.items[1].id;

        // Initially no parent relationship
        assert!(list.items[1].parent_id.is_none());

        // Indent second item to make it a child of first
        list.indent_item(1).unwrap();

        // Verify parent_id was set
        assert_eq!(list.items[1].indent_level, 1);
        assert_eq!(list.items[1].parent_id, Some(parent_id));

        // Save and reload
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        // Verify database has correct parent_id
        assert_eq!(loaded[1].id, child_id);
        assert_eq!(loaded[1].indent_level, 1);
        assert_eq!(loaded[1].parent_id, Some(parent_id));
    }

    #[test]
    fn test_outdent_updates_parent_id_in_database() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut list = create_test_list(date);
        list.add_item("Parent".to_string());
        list.add_item_with_indent("Child".to_string(), 1);
        list.recalculate_parent_ids();

        let parent_id = list.items[0].id;

        // Initially has parent relationship
        assert_eq!(list.items[1].parent_id, Some(parent_id));

        // Outdent second item to remove parent relationship
        list.outdent_item(1).unwrap();

        // Verify parent_id was cleared
        assert_eq!(list.items[1].indent_level, 0);
        assert!(list.items[1].parent_id.is_none());

        // Save and reload
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        // Verify database has no parent_id
        assert_eq!(loaded[1].indent_level, 0);
        assert!(loaded[1].parent_id.is_none());
    }

    #[test]
    fn test_move_child_between_parents() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut list = create_test_list(date);
        // Create: Parent1 > Child, Parent2
        list.add_item("Parent1".to_string());
        list.add_item_with_indent("Child".to_string(), 1);
        list.add_item("Parent2".to_string());
        list.recalculate_parent_ids();

        let parent1_id = list.items[0].id;
        let child_id = list.items[1].id;
        let parent2_id = list.items[2].id;

        // Child starts under Parent1
        assert_eq!(list.items[1].parent_id, Some(parent1_id));

        // Move Parent2 up (will be between Parent1 and Child)
        // This doesn't change child's parent since indent levels determine parentage

        // Instead, let's move Child down past Parent2, then it should become child of Parent2
        // First outdent child to make it sibling
        list.outdent_item(1).unwrap();
        assert!(list.items[1].parent_id.is_none());

        // Now move child down past Parent2
        list.move_item_with_children_down(1).unwrap();

        // Order is now: Parent1, Parent2, Child (all at level 0)
        assert_eq!(list.items[0].content, "Parent1");
        assert_eq!(list.items[1].content, "Parent2");
        assert_eq!(list.items[2].content, "Child");

        // Indent child under Parent2
        list.indent_item(2).unwrap();

        // Verify parent_id changed to Parent2
        assert_eq!(list.items[2].parent_id, Some(parent2_id));
        assert_eq!(list.items[2].id, child_id);

        // Save and reload
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        // Verify database reflects new parent relationship
        assert_eq!(loaded[2].content, "Child");
        assert_eq!(loaded[2].parent_id, Some(parent2_id));
    }

    #[test]
    fn test_move_item_with_children_preserves_hierarchy() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut list = create_test_list(date);
        // Create: Item1, Parent > Child > Grandchild
        list.add_item("Item1".to_string());
        list.add_item("Parent".to_string());
        list.add_item_with_indent("Child".to_string(), 1);
        list.add_item_with_indent("Grandchild".to_string(), 2);
        list.recalculate_parent_ids();

        let item1_id = list.items[0].id;
        let parent_id = list.items[1].id;
        let child_id = list.items[2].id;
        let grandchild_id = list.items[3].id;

        // Verify initial hierarchy
        assert!(list.items[1].parent_id.is_none()); // Parent has no parent
        assert_eq!(list.items[2].parent_id, Some(parent_id)); // Child -> Parent
        assert_eq!(list.items[3].parent_id, Some(child_id)); // Grandchild -> Child

        // Move Parent (with children) up past Item1
        list.move_item_with_children_up(1).unwrap();

        // Order is now: Parent, Child, Grandchild, Item1
        assert_eq!(list.items[0].content, "Parent");
        assert_eq!(list.items[1].content, "Child");
        assert_eq!(list.items[2].content, "Grandchild");
        assert_eq!(list.items[3].content, "Item1");

        // IDs should be preserved
        assert_eq!(list.items[0].id, parent_id);
        assert_eq!(list.items[1].id, child_id);
        assert_eq!(list.items[2].id, grandchild_id);
        assert_eq!(list.items[3].id, item1_id);

        // Parent relationships should be recalculated and preserved
        assert!(list.items[0].parent_id.is_none()); // Parent still has no parent
        assert_eq!(list.items[1].parent_id, Some(parent_id)); // Child -> Parent
        assert_eq!(list.items[2].parent_id, Some(child_id)); // Grandchild -> Child

        // Save and reload
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        // Verify database preserves order and hierarchy
        assert_eq!(loaded[0].content, "Parent");
        assert_eq!(loaded[1].content, "Child");
        assert_eq!(loaded[2].content, "Grandchild");
        assert_eq!(loaded[3].content, "Item1");

        assert!(loaded[0].parent_id.is_none());
        assert_eq!(loaded[1].parent_id, Some(parent_id));
        assert_eq!(loaded[2].parent_id, Some(child_id));
    }

    #[test]
    fn test_indent_item_with_children_updates_all_parent_ids() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut list = create_test_list(date);
        // Create: Grandparent, Parent > Child
        list.add_item("Grandparent".to_string());
        list.add_item("Parent".to_string());
        list.add_item_with_indent("Child".to_string(), 1);
        list.recalculate_parent_ids();

        let grandparent_id = list.items[0].id;
        let parent_id = list.items[1].id;
        let child_id = list.items[2].id;

        // Parent is at level 0, Child is at level 1 under Parent
        assert!(list.items[1].parent_id.is_none());
        assert_eq!(list.items[2].parent_id, Some(parent_id));

        // Indent Parent (and its children) under Grandparent
        list.indent_item_with_children(1).unwrap();

        // Now Parent is at level 1 under Grandparent, Child is at level 2 under Parent
        assert_eq!(list.items[1].indent_level, 1);
        assert_eq!(list.items[2].indent_level, 2);
        assert_eq!(list.items[1].parent_id, Some(grandparent_id));
        assert_eq!(list.items[2].parent_id, Some(parent_id));

        // Save and reload
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        // Verify database reflects new hierarchy
        assert_eq!(loaded[0].id, grandparent_id);
        assert_eq!(loaded[1].id, parent_id);
        assert_eq!(loaded[2].id, child_id);

        assert!(loaded[0].parent_id.is_none());
        assert_eq!(loaded[1].parent_id, Some(grandparent_id));
        assert_eq!(loaded[2].parent_id, Some(parent_id));
    }

    #[test]
    fn test_remove_item_updates_positions() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut list = create_test_list(date);
        list.add_item("First".to_string());
        list.add_item("Second".to_string());
        list.add_item("Third".to_string());

        let third_id = list.items[2].id;

        list.remove_item(1).unwrap();

        // Order is now: First, Third
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].content, "First");
        assert_eq!(list.items[1].content, "Third");

        // Save and reload
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        // Verify database has correct items and positions
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].content, "First");
        assert_eq!(loaded[1].content, "Third");
        assert_eq!(loaded[1].id, third_id);
    }

    #[test]
    fn test_insert_item_updates_positions() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut list = create_test_list(date);
        list.add_item("First".to_string());
        list.add_item("Third".to_string());

        let first_id = list.items[0].id;
        let third_id = list.items[1].id;

        // Insert "Second" at position 1
        list.insert_item(1, "Second".to_string(), 0).unwrap();

        // Order is now: First, Second, Third
        assert_eq!(list.items.len(), 3);
        assert_eq!(list.items[0].content, "First");
        assert_eq!(list.items[1].content, "Second");
        assert_eq!(list.items[2].content, "Third");

        let second_id = list.items[1].id;

        // Save and reload
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        // Verify database has correct items and positions
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].content, "First");
        assert_eq!(loaded[1].content, "Second");
        assert_eq!(loaded[2].content, "Third");
        assert_eq!(loaded[0].id, first_id);
        assert_eq!(loaded[1].id, second_id);
        assert_eq!(loaded[2].id, third_id);
    }

    #[test]
    fn test_multiple_operations_preserve_ids() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut list = create_test_list(date);
        list.add_item("A".to_string());
        list.add_item("B".to_string());
        list.add_item("C".to_string());

        let a_id = list.items[0].id;
        let b_id = list.items[1].id;
        let c_id = list.items[2].id;

        // Perform multiple operations
        list.move_item_with_children_down(0).unwrap(); // B, A, C
        list.indent_item(1).unwrap(); // B > A, C
        list.move_item_with_children_down(0).unwrap(); // C, B > A

        // Save and reload
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        // Verify all IDs are preserved
        let loaded_ids: Vec<Uuid> = loaded.iter().map(|i| i.id).collect();
        assert!(loaded_ids.contains(&a_id));
        assert!(loaded_ids.contains(&b_id));
        assert!(loaded_ids.contains(&c_id));

        // Verify order: C, B, A
        assert_eq!(loaded[0].content, "C");
        assert_eq!(loaded[1].content, "B");
        assert_eq!(loaded[2].content, "A");

        // Verify A is child of B
        assert_eq!(loaded[2].parent_id, Some(b_id));
    }

    #[test]
    fn test_state_changes_persisted() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut list = create_test_list(date);
        list.add_item("Task".to_string());

        let task_id = list.items[0].id;

        // Toggle state
        list.items[0].toggle_state();
        assert_eq!(list.items[0].state, TodoState::Checked);

        // Save and reload
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        // Verify state is persisted
        assert_eq!(loaded[0].id, task_id);
        assert_eq!(loaded[0].state, TodoState::Checked);

        // Change to different state
        list.items[0].cycle_state(); // Goes to Question
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        assert_eq!(loaded[0].state, TodoState::Question);
    }

    #[test]
    fn test_due_date_and_description_persisted() {
        let (_temp_dir, conn) = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        let due = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();

        let mut list = create_test_list(date);
        let mut item = TodoItem::new("Task with details".to_string(), 0);
        item.due_date = Some(due);
        item.description = Some("This is a description".to_string());
        let item_id = item.id;
        list.items.push(item);

        // Save and reload
        save_to_test_db(&conn, &list);
        let loaded = load_from_test_db(&conn, date);

        // Verify due_date and description are persisted
        assert_eq!(loaded[0].id, item_id);
        assert_eq!(loaded[0].due_date, Some(due));
        assert_eq!(
            loaded[0].description,
            Some("This is a description".to_string())
        );
    }
}
