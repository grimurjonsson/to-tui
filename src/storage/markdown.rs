use crate::todo::{TodoItem, TodoList, TodoState};
use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use std::path::PathBuf;

pub fn serialize_todo_list_clean(list: &TodoList) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "# Todo List - {}\n\n",
        list.date.format("%B %d, %Y")
    ));

    for item in &list.items {
        let indent = "  ".repeat(item.indent_level);
        
        let due_suffix = item.due_date
            .map(|d| format!(" @due({})", d.format("%Y-%m-%d")))
            .unwrap_or_default();
        
        output.push_str(&format!(
            "{}- [{}] {}{}\n",
            indent,
            item.state.to_char(),
            item.content,
            due_suffix
        ));
        
        if let Some(ref desc) = item.description {
            for line in desc.lines() {
                output.push_str(&format!("{indent}  > {line}\n"));
            }
        }
    }

    output
}

pub fn parse_todo_list(content: &str, date: NaiveDate, file_path: PathBuf) -> Result<TodoList> {
    let mut items: Vec<TodoItem> = Vec::new();
    let mut pending_description: Option<String> = None;

    for line in content.lines() {
        if line.trim().is_empty() || line.trim().starts_with('#') {
            continue;
        }

        let trimmed = line.trim_start();
        
        if let Some(stripped) = trimmed.strip_prefix('>') {
            if let Some(ref mut desc) = pending_description {
                desc.push('\n');
                desc.push_str(stripped.trim());
            } else {
                pending_description = Some(stripped.trim().to_string());
            }
            continue;
        }

        if let Some(desc) = pending_description.take() {
            if let Some(last_item) = items.last_mut() {
                last_item.description = Some(desc);
            }
        }

        if let Some(mut item) = parse_todo_line(line)? {
            let parent_id = find_parent_id(&items, item.indent_level);
            item.parent_id = parent_id;
            items.push(item);
        }
    }

    if let Some(desc) = pending_description.take() {
        if let Some(last_item) = items.last_mut() {
            last_item.description = Some(desc);
        }
    }

    Ok(TodoList::with_items(date, file_path, items))
}

fn find_parent_id(items: &[TodoItem], indent_level: usize) -> Option<uuid::Uuid> {
    if indent_level == 0 {
        return None;
    }
    
    for item in items.iter().rev() {
        if item.indent_level < indent_level {
            return Some(item.id);
        }
    }
    None
}

fn parse_todo_line(line: &str) -> Result<Option<TodoItem>> {
    let indent_level = line.len() - line.trim_start().len();
    let indent_level = indent_level / 2;

    let trimmed = line.trim_start();

    if !trimmed.starts_with("- [") {
        return Ok(None);
    }

    if trimmed.len() < 5 {
        return Err(anyhow!("Invalid checkbox format"));
    }

    let state_char = trimmed.chars().nth(3).ok_or_else(|| anyhow!("Missing state character"))?;
    let state = TodoState::from_char(state_char)
        .ok_or_else(|| anyhow!("Invalid state character: {state_char}"))?;

    let raw_content = if trimmed.len() > 5 {
        trimmed[5..].trim()
    } else {
        ""
    };

    let (content, id) = parse_id(raw_content);
    let (content, due_date) = parse_due_date(&content);

    let mut item = TodoItem::full(
        content,
        state,
        indent_level,
        None,
        due_date,
        None,
        false,
    );
    
    if let Some(parsed_id) = id {
        item.id = parsed_id;
    }

    Ok(Some(item))
}

fn parse_id(content: &str) -> (String, Option<uuid::Uuid>) {
    if let Some(start) = content.find("@id(") {
        if let Some(end) = content[start..].find(')') {
            let id_str = &content[start + 4..start + end];
            let id = uuid::Uuid::parse_str(id_str).ok();
            
            let mut cleaned = String::new();
            cleaned.push_str(content[..start].trim());
            if start + end + 1 < content.len() {
                let suffix = content[start + end + 1..].trim();
                if !suffix.is_empty() {
                    if !cleaned.is_empty() {
                        cleaned.push(' ');
                    }
                    cleaned.push_str(suffix);
                }
            }
            return (cleaned, id);
        }
    }
    (content.to_string(), None)
}

fn parse_due_date(content: &str) -> (String, Option<NaiveDate>) {
    if let Some(start) = content.find("@due(") {
        if let Some(end) = content[start..].find(')') {
            let date_str = &content[start + 5..start + end];
            let due_date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok();
            
            let mut cleaned = String::new();
            cleaned.push_str(content[..start].trim());
            if start + end + 1 < content.len() {
                let suffix = content[start + end + 1..].trim();
                if !suffix.is_empty() {
                    if !cleaned.is_empty() {
                        cleaned.push(' ');
                    }
                    cleaned.push_str(suffix);
                }
            }
            return (cleaned, due_date);
        }
    }
    (content.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn create_test_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()
    }

    fn create_test_path() -> PathBuf {
        PathBuf::from("/tmp/2025-12-31.md")
    }

    #[test]
    fn test_serialize_empty_list() {
        let date = create_test_date();
        let path = create_test_path();
        let list = TodoList::new(date, path);

        let markdown = serialize_todo_list_clean(&list);
        assert!(markdown.contains("# Todo List - December 31, 2025"));
    }

    #[test]
    fn test_serialize_simple_list() {
        let date = create_test_date();
        let path = create_test_path();
        let mut list = TodoList::new(date, path);

        list.add_item("Task 1".to_string());
        list.add_item("Task 2".to_string());

        let markdown = serialize_todo_list_clean(&list);
        assert!(markdown.contains("- [ ] Task 1"));
        assert!(markdown.contains("- [ ] Task 2"));
    }

    #[test]
    fn test_serialize_with_states() {
        let date = create_test_date();
        let path = create_test_path();
        let mut list = TodoList::new(date, path);

        list.add_item("Empty task".to_string());
        list.add_item("Checked task".to_string());
        list.add_item("Question task".to_string());
        list.add_item("Important task".to_string());

        list.items[1].state = TodoState::Checked;
        list.items[2].state = TodoState::Question;
        list.items[3].state = TodoState::Exclamation;

        let markdown = serialize_todo_list_clean(&list);
        assert!(markdown.contains("- [ ] Empty task"));
        assert!(markdown.contains("- [x] Checked task"));
        assert!(markdown.contains("- [?] Question task"));
        assert!(markdown.contains("- [!] Important task"));
    }

    #[test]
    fn test_serialize_with_indentation() {
        let date = create_test_date();
        let path = create_test_path();
        let mut list = TodoList::new(date, path);

        list.add_item_with_indent("Parent".to_string(), 0);
        list.add_item_with_indent("Child 1".to_string(), 1);
        list.add_item_with_indent("Grandchild".to_string(), 2);
        list.add_item_with_indent("Child 2".to_string(), 1);

        let markdown = serialize_todo_list_clean(&list);
        assert!(markdown.contains("- [ ] Parent\n"));
        assert!(markdown.contains("\n  - [ ] Child 1\n"));
        assert!(markdown.contains("\n    - [ ] Grandchild\n"));
        assert!(markdown.contains("\n  - [ ] Child 2\n"));
    }

    #[test]
    fn test_parse_simple_list() {
        let content = r#"# Todo List - December 31, 2025

- [ ] Task 1
- [x] Task 2
- [?] Task 3
- [!] Task 4
"#;

        let date = create_test_date();
        let path = create_test_path();
        let list = parse_todo_list(content, date, path).unwrap();

        assert_eq!(list.items.len(), 4);
        assert_eq!(list.items[0].content, "Task 1");
        assert_eq!(list.items[0].state, TodoState::Empty);
        assert_eq!(list.items[1].content, "Task 2");
        assert_eq!(list.items[1].state, TodoState::Checked);
        assert_eq!(list.items[2].content, "Task 3");
        assert_eq!(list.items[2].state, TodoState::Question);
        assert_eq!(list.items[3].content, "Task 4");
        assert_eq!(list.items[3].state, TodoState::Exclamation);
    }

    #[test]
    fn test_parse_with_indentation() {
        let content = r#"# Todo List - December 31, 2025

- [ ] Parent
  - [ ] Child 1
    - [ ] Grandchild
  - [ ] Child 2
"#;

        let date = create_test_date();
        let path = create_test_path();
        let list = parse_todo_list(content, date, path).unwrap();

        assert_eq!(list.items.len(), 4);
        assert_eq!(list.items[0].indent_level, 0);
        assert_eq!(list.items[1].indent_level, 1);
        assert_eq!(list.items[2].indent_level, 2);
        assert_eq!(list.items[3].indent_level, 1);
    }

    #[test]
    fn test_round_trip() {
        let date = create_test_date();
        let path = create_test_path();
        let mut list = TodoList::new(date, path.clone());

        list.add_item_with_indent("Parent".to_string(), 0);
        list.add_item_with_indent("Child".to_string(), 1);
        list.items[1].state = TodoState::Checked;

        let markdown = serialize_todo_list_clean(&list);
        let parsed = parse_todo_list(&markdown, date, path).unwrap();

        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].content, "Parent");
        assert_eq!(parsed.items[0].indent_level, 0);
        assert_eq!(parsed.items[0].state, TodoState::Empty);
        assert_eq!(parsed.items[1].content, "Child");
        assert_eq!(parsed.items[1].indent_level, 1);
        assert_eq!(parsed.items[1].state, TodoState::Checked);
    }

    #[test]
    fn test_parse_skips_non_checkbox_lines() {
        let content = r#"# Todo List - December 31, 2025

Some random text
- [ ] Task 1
Another line
- [x] Task 2

Empty line above
"#;

        let date = create_test_date();
        let path = create_test_path();
        let list = parse_todo_list(content, date, path).unwrap();

        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].content, "Task 1");
        assert_eq!(list.items[1].content, "Task 2");
    }
}
