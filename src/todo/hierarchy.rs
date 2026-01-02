use super::TodoList;
use anyhow::{anyhow, Result};

impl TodoList {
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

    /// Move item and all its children up by one position
    pub fn move_item_with_children_up(&mut self, index: usize) -> Result<()> {
        if index == 0 {
            return Err(anyhow!("Cannot move first item up"));
        }

        let (item_start, item_end) = self.get_item_range(index)?;

        // Find the previous item's range
        let prev_idx = item_start - 1;
        let prev_base_indent = self.items[prev_idx].indent_level;

        // Find the start of the previous item group
        let mut prev_start = prev_idx;
        while prev_start > 0 && self.items[prev_start - 1].indent_level > prev_base_indent {
            prev_start -= 1;
        }

        // Extract both item groups
        let item_count = item_end - item_start;
        let prev_count = item_start - prev_start;

        // Remove current item group first (higher index)
        let current_items: Vec<_> = self.items.drain(item_start..item_end).collect();

        // Now remove previous item group (indices have shifted)
        let prev_items: Vec<_> = self.items.drain(prev_start..prev_start + prev_count).collect();

        self.items.splice(prev_start..prev_start, current_items);
        self.items.splice(prev_start + item_count..prev_start + item_count, prev_items);

        self.recalculate_parent_ids();
        Ok(())
    }

    pub fn move_item_with_children_down(&mut self, index: usize) -> Result<()> {
        let (item_start, item_end) = self.get_item_range(index)?;

        if item_end >= self.items.len() {
            return Err(anyhow!("Cannot move last item down"));
        }

        // Find the next item's range
        let (next_start, next_end) = self.get_item_range(item_end)?;

        let item_count = item_end - item_start;
        let next_count = next_end - next_start;

        // Remove next item group first (higher index)
        let next_items: Vec<_> = self.items.drain(next_start..next_end).collect();

        // Now remove current item group (indices have shifted)
        let current_items: Vec<_> = self.items.drain(item_start..item_start + item_count).collect();

        self.items.splice(item_start..item_start, next_items);
        self.items.splice(item_start + next_count..item_start + next_count, current_items);

        self.recalculate_parent_ids();
        Ok(())
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
