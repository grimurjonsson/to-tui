use super::state::TodoState;
use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TodoItem {
    pub id: Uuid,
    pub content: String,
    pub state: TodoState,
    pub indent_level: usize,
    pub parent_id: Option<Uuid>,
    pub due_date: Option<NaiveDate>,
    pub description: Option<String>,
    pub collapsed: bool,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl TodoItem {
    pub fn new(content: String, indent_level: usize) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            content,
            state: TodoState::Empty,
            indent_level,
            parent_id: None,
            due_date: None,
            description: None,
            collapsed: false,
            created_at: now,
            modified_at: now,
            completed_at: None,
            deleted_at: None,
        }
    }

    #[cfg(test)]
    pub fn with_state(content: String, state: TodoState, indent_level: usize) -> Self {
        let now = Utc::now();
        let completed_at = if state == TodoState::Checked {
            Some(now)
        } else {
            None
        };
        Self {
            id: Uuid::new_v4(),
            content,
            state,
            indent_level,
            parent_id: None,
            due_date: None,
            description: None,
            collapsed: false,
            created_at: now,
            modified_at: now,
            completed_at,
            deleted_at: None,
        }
    }

    pub fn full(
        content: String,
        state: TodoState,
        indent_level: usize,
        parent_id: Option<Uuid>,
        due_date: Option<NaiveDate>,
        description: Option<String>,
        collapsed: bool,
    ) -> Self {
        let now = Utc::now();
        let completed_at = if state == TodoState::Checked {
            Some(now)
        } else {
            None
        };
        Self {
            id: Uuid::new_v4(),
            content,
            state,
            indent_level,
            parent_id,
            due_date,
            description,
            collapsed,
            created_at: now,
            modified_at: now,
            completed_at,
            deleted_at: None,
        }
    }

    pub fn toggle_state(&mut self) {
        let was_complete = self.state.is_complete();
        self.state = self.state.toggle();
        self.update_completed_at(was_complete);
    }

    pub fn cycle_state(&mut self) {
        let was_complete = self.state.is_complete();
        self.state = self.state.cycle();
        self.update_completed_at(was_complete);
    }

    fn update_completed_at(&mut self, was_complete: bool) {
        let is_complete = self.state.is_complete();
        self.modified_at = Utc::now();
        if is_complete && !was_complete {
            self.completed_at = Some(Utc::now());
        } else if !is_complete && was_complete {
            self.completed_at = None;
        }
    }

    pub fn is_complete(&self) -> bool {
        self.state.is_complete()
    }

    #[cfg(test)]
    pub fn can_indent(&self, prev_indent: Option<usize>) -> bool {
        match prev_indent {
            None => false,
            Some(prev) => self.indent_level <= prev,
        }
    }

    #[cfg(test)]
    pub fn can_outdent(&self) -> bool {
        self.indent_level > 0
    }

    #[cfg(test)]
    pub fn indent(&mut self) {
        self.indent_level += 1;
    }

    #[cfg(test)]
    pub fn outdent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let item = TodoItem::new("Test task".to_string(), 0);
        assert_eq!(item.content, "Test task");
        assert_eq!(item.state, TodoState::Empty);
        assert_eq!(item.indent_level, 0);
    }

    #[test]
    fn test_with_state() {
        let item = TodoItem::with_state("Done task".to_string(), TodoState::Checked, 1);
        assert_eq!(item.content, "Done task");
        assert_eq!(item.state, TodoState::Checked);
        assert_eq!(item.indent_level, 1);
    }

    #[test]
    fn test_toggle_state() {
        let mut item = TodoItem::new("Task".to_string(), 0);
        assert_eq!(item.state, TodoState::Empty);

        item.toggle_state();
        assert_eq!(item.state, TodoState::Checked);

        item.toggle_state();
        assert_eq!(item.state, TodoState::Empty);
    }

    #[test]
    fn test_cycle_state() {
        let mut item = TodoItem::new("Task".to_string(), 0);
        assert_eq!(item.state, TodoState::Empty);

        item.cycle_state();
        assert_eq!(item.state, TodoState::InProgress);

        item.cycle_state();
        assert_eq!(item.state, TodoState::Checked);

        item.cycle_state();
        assert_eq!(item.state, TodoState::Question);

        item.cycle_state();
        assert_eq!(item.state, TodoState::Exclamation);

        item.cycle_state();
        assert_eq!(item.state, TodoState::Empty);
    }

    #[test]
    fn test_is_complete() {
        let mut item = TodoItem::new("Task".to_string(), 0);
        assert!(!item.is_complete());

        item.state = TodoState::Checked;
        assert!(item.is_complete());
    }

    #[test]
    fn test_can_indent() {
        let item = TodoItem::new("Task".to_string(), 0);
        assert!(!item.can_indent(None)); // First item can't indent
        assert!(item.can_indent(Some(0)));
        assert!(item.can_indent(Some(1)));
        assert!(item.can_indent(Some(2)));

        let item2 = TodoItem::new("Task".to_string(), 1);
        assert!(!item2.can_indent(Some(0))); // Can't indent beyond prev + 1
        assert!(item2.can_indent(Some(1)));
    }

    #[test]
    fn test_can_outdent() {
        let item = TodoItem::new("Task".to_string(), 0);
        assert!(!item.can_outdent());

        let item2 = TodoItem::new("Task".to_string(), 1);
        assert!(item2.can_outdent());
    }

    #[test]
    fn test_indent_outdent() {
        let mut item = TodoItem::new("Task".to_string(), 0);

        item.indent();
        assert_eq!(item.indent_level, 1);

        item.indent();
        assert_eq!(item.indent_level, 2);

        item.outdent();
        assert_eq!(item.indent_level, 1);

        item.outdent();
        assert_eq!(item.indent_level, 0);

        item.outdent(); // Should not go negative
        assert_eq!(item.indent_level, 0);
    }
}
