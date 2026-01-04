use super::TodoList;
use super::state::TodoState;
use anyhow::{Result, anyhow};

impl TodoList {
    pub fn count_children_stats(&self, index: usize) -> (usize, usize) {
        if index >= self.items.len() {
            return (0, 0);
        }

        let (_, end) = self.get_item_range(index).unwrap_or((index, index + 1));
        let children = &self.items[index + 1..end];

        let completed = children
            .iter()
            .filter(|item| item.state == TodoState::Checked)
            .count();
        let total = children.len();

        (completed, total)
    }

    pub fn has_children(&self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        let (start, end) = self.get_item_range(index).unwrap_or((index, index + 1));
        end > start + 1
    }
    pub fn recalculate_parent_ids(&mut self) {
        for i in 0..self.items.len() {
            let indent_level = self.items[i].indent_level;
            if indent_level == 0 {
                self.items[i].parent_id = None;
            } else {
                let parent_id = self.items[..i]
                    .iter()
                    .rev()
                    .find(|item| item.indent_level < indent_level)
                    .map(|item| item.id);
                self.items[i].parent_id = parent_id;
            }
        }
    }

    pub fn get_item_range(&self, index: usize) -> Result<(usize, usize)> {
        if index >= self.items.len() {
            return Err(anyhow!("Index out of bounds"));
        }

        let base_indent = self.items[index].indent_level;
        let mut end = index + 1;

        // Find all children (items with higher indent immediately following)
        while end < self.items.len() && self.items[end].indent_level > base_indent {
            end += 1;
        }

        Ok((index, end))
    }

    /// Move item and all its children up one position. Returns positions moved.
    pub fn move_item_with_children_up(&mut self, index: usize) -> Result<usize> {
        if index == 0 {
            return Err(anyhow!("Cannot move first item up"));
        }

        let (item_start, item_end) = self.get_item_range(index)?;

        if item_start == 0 {
            return Err(anyhow!("Already at top"));
        }

        let current_indent = self.items[item_start].indent_level;

        let mut target_idx = item_start - 1;
        while target_idx > 0 && self.items[target_idx].indent_level > current_indent {
            target_idx -= 1;
        }

        let (target_start, _) = self.get_item_range(target_idx)?;

        if target_start >= item_start {
            return Err(anyhow!("Cannot move up"));
        }

        let displacement = item_start - target_start;
        let mut current_items: Vec<_> = self.items.drain(item_start..item_end).collect();

        let max_indent = if target_start == 0 {
            0
        } else {
            self.items[target_start - 1].indent_level + 1
        };

        let item_indent = current_items[0].indent_level;
        if item_indent > max_indent {
            let diff = item_indent - max_indent;
            for item in &mut current_items {
                item.indent_level = item.indent_level.saturating_sub(diff);
            }
        }

        self.items.splice(target_start..target_start, current_items);
        self.recalculate_parent_ids();
        Ok(displacement)
    }

    /// Move item and all its children down one position. Returns positions moved.
    pub fn move_item_with_children_down(&mut self, index: usize) -> Result<usize> {
        let (item_start, item_end) = self.get_item_range(index)?;

        if item_end >= self.items.len() {
            return Err(anyhow!("Cannot move last item down"));
        }

        let current_indent = self.items[item_start].indent_level;

        let mut target_idx = item_end;
        while target_idx < self.items.len() && self.items[target_idx].indent_level > current_indent
        {
            target_idx += 1;
        }

        if target_idx >= self.items.len() {
            return Err(anyhow!("Cannot move down"));
        }

        let target_indent = self.items[target_idx].indent_level;
        let item_count = item_end - item_start;

        let insert_pos = if current_indent > target_indent {
            target_idx + 1
        } else {
            let (_, target_end) = self.get_item_range(target_idx)?;
            target_end
        };

        let mut current_items: Vec<_> = self.items.drain(item_start..item_end).collect();

        let actual_insert = insert_pos - item_count;
        let max_indent = self.items[actual_insert - 1].indent_level + 1;

        let item_indent = current_items[0].indent_level;
        if item_indent > max_indent {
            let diff = item_indent - max_indent;
            for item in &mut current_items {
                item.indent_level = item.indent_level.saturating_sub(diff);
            }
        }

        self.items
            .splice(actual_insert..actual_insert, current_items);
        self.recalculate_parent_ids();
        Ok(insert_pos - item_end)
    }

    pub fn indent_item(&mut self, index: usize) -> Result<()> {
        if index >= self.items.len() {
            return Err(anyhow!("Index out of bounds"));
        }

        if index == 0 {
            return Err(anyhow!("Cannot indent first item"));
        }

        let prev_indent = self.items[index - 1].indent_level;
        let current_indent = self.items[index].indent_level;

        // Can only indent to at most one level beyond previous item
        if current_indent > prev_indent {
            return Err(anyhow!("Cannot indent beyond parent level"));
        }

        self.items[index].indent_level += 1;
        self.recalculate_parent_ids();
        Ok(())
    }

    pub fn outdent_item(&mut self, index: usize) -> Result<()> {
        if index >= self.items.len() {
            return Err(anyhow!("Index out of bounds"));
        }

        if self.items[index].indent_level == 0 {
            return Err(anyhow!("Cannot outdent top-level item"));
        }

        self.items[index].indent_level -= 1;
        self.recalculate_parent_ids();
        Ok(())
    }

    pub fn indent_item_with_children(&mut self, index: usize) -> Result<()> {
        if index >= self.items.len() {
            return Err(anyhow!("Index out of bounds"));
        }

        if index == 0 {
            return Err(anyhow!("Cannot indent first item"));
        }

        let prev_indent = self.items[index - 1].indent_level;
        let current_indent = self.items[index].indent_level;

        // Can only indent to at most one level beyond previous item
        if current_indent > prev_indent {
            return Err(anyhow!("Cannot indent beyond parent level"));
        }

        // Get the range of this item and its children
        let (start, end) = self.get_item_range(index)?;

        for i in start..end {
            self.items[i].indent_level += 1;
        }

        self.recalculate_parent_ids();
        Ok(())
    }

    pub fn outdent_item_with_children(&mut self, index: usize) -> Result<()> {
        if index >= self.items.len() {
            return Err(anyhow!("Index out of bounds"));
        }

        if self.items[index].indent_level == 0 {
            return Err(anyhow!("Cannot outdent top-level item"));
        }

        // Get the range of this item and its children
        let (start, end) = self.get_item_range(index)?;

        for i in start..end {
            if self.items[i].indent_level > 0 {
                self.items[i].indent_level -= 1;
            }
        }

        self.recalculate_parent_ids();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::path::PathBuf;

    fn create_test_list() -> TodoList {
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        let path = PathBuf::from("/tmp/test.md");
        TodoList::new(date, path)
    }

    #[test]
    fn test_get_item_range() {
        let mut list = create_test_list();
        list.add_item_with_indent("Parent".to_string(), 0);
        list.add_item_with_indent("Child 1".to_string(), 1);
        list.add_item_with_indent("Grandchild".to_string(), 2);
        list.add_item_with_indent("Child 2".to_string(), 1);
        list.add_item_with_indent("Another parent".to_string(), 0);

        let (start, end) = list.get_item_range(0).unwrap();
        assert_eq!(start, 0);
        assert_eq!(end, 4); // Parent + 3 descendants

        let (start, end) = list.get_item_range(4).unwrap();
        assert_eq!(start, 4);
        assert_eq!(end, 5); // No children
    }

    #[test]
    fn test_indent_outdent() {
        let mut list = create_test_list();
        list.add_item("Parent".to_string());
        list.add_item("Child".to_string());

        // Cannot indent first item
        assert!(list.indent_item(0).is_err());

        // Can indent second item
        assert!(list.indent_item(1).is_ok());
        assert_eq!(list.items[1].indent_level, 1);

        // Can outdent
        assert!(list.outdent_item(1).is_ok());
        assert_eq!(list.items[1].indent_level, 0);

        // Cannot outdent top-level
        assert!(list.outdent_item(1).is_err());
    }
}
