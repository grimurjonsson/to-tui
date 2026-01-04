use super::mode::Mode;
use crate::keybindings::{KeyBinding, KeybindingCache};
use crate::storage::file::load_todo_list;
use crate::todo::{TodoItem, TodoList};
use crate::ui::theme::Theme;
use anyhow::Result;
use std::time::Instant;

const MAX_UNDO_HISTORY: usize = 50;

pub struct AppState {
    pub todo_list: TodoList,
    pub cursor_position: usize,
    pub mode: Mode,
    pub edit_buffer: String,
    pub edit_cursor_pos: usize,
    pub should_quit: bool,
    pub show_help: bool,
    pub theme: Theme,
    pub keybindings: KeybindingCache,
    pub pending_key: Option<KeyBinding>,
    pub pending_key_time: Option<Instant>,
    pub timeoutlen: u64,
    pub unsaved_changes: bool,
    pub last_save_time: Option<Instant>,
    pub is_creating_new_item: bool,
    pub pending_indent_level: usize,
    pub undo_stack: Vec<(TodoList, usize)>,
    pub selection_anchor: Option<usize>,
}

impl AppState {
    pub fn new(
        todo_list: TodoList,
        theme: Theme,
        keybindings: KeybindingCache,
        timeoutlen: u64,
    ) -> Self {
        Self {
            todo_list,
            cursor_position: 0,
            mode: Mode::Navigate,
            edit_buffer: String::new(),
            edit_cursor_pos: 0,
            should_quit: false,
            show_help: false,
            theme,
            keybindings,
            pending_key: None,
            pending_key_time: None,
            timeoutlen,
            unsaved_changes: false,
            last_save_time: None,
            is_creating_new_item: false,
            pending_indent_level: 0,
            undo_stack: Vec::new(),
            selection_anchor: None,
        }
    }

    pub fn save_undo(&mut self) {
        if self.undo_stack.len() >= MAX_UNDO_HISTORY {
            self.undo_stack.remove(0);
        }
        self.undo_stack
            .push((self.todo_list.clone(), self.cursor_position));
    }

    pub fn undo(&mut self) -> bool {
        if let Some((list, cursor)) = self.undo_stack.pop() {
            self.todo_list = list;
            self.cursor_position = cursor;
            self.unsaved_changes = true;
            true
        } else {
            false
        }
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            while self.cursor_position > 0 && self.is_item_hidden(self.cursor_position) {
                self.cursor_position -= 1;
            }
        }
    }

    pub fn move_cursor_down(&mut self) {
        if !self.todo_list.items.is_empty() && self.cursor_position < self.todo_list.items.len() - 1
        {
            self.cursor_position += 1;
            while self.cursor_position < self.todo_list.items.len() - 1
                && self.is_item_hidden(self.cursor_position)
            {
                self.cursor_position += 1;
            }
            if self.is_item_hidden(self.cursor_position) && self.cursor_position > 0 {
                self.cursor_position -= 1;
                while self.cursor_position > 0 && self.is_item_hidden(self.cursor_position) {
                    self.cursor_position -= 1;
                }
            }
        }
    }

    fn is_item_hidden(&self, index: usize) -> bool {
        if index >= self.todo_list.items.len() {
            return false;
        }
        let target_indent = self.todo_list.items[index].indent_level;
        for i in (0..index).rev() {
            let item = &self.todo_list.items[i];
            if item.indent_level < target_indent {
                if item.collapsed {
                    return true;
                }
                break;
            }
        }
        false
    }

    pub fn selected_item(&self) -> Option<&TodoItem> {
        self.todo_list.items.get(self.cursor_position)
    }

    pub fn selected_item_mut(&mut self) -> Option<&mut TodoItem> {
        self.todo_list.items.get_mut(self.cursor_position)
    }

    pub fn clamp_cursor(&mut self) {
        if !self.todo_list.items.is_empty() {
            self.cursor_position = self.cursor_position.min(self.todo_list.items.len() - 1);
        } else {
            self.cursor_position = 0;
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    pub fn start_or_extend_selection(&mut self) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
    }

    pub fn get_selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            let start = anchor.min(self.cursor_position);
            let end = anchor.max(self.cursor_position);
            (start, end)
        })
    }

    pub fn is_selected(&self, index: usize) -> bool {
        if let Some((start, end)) = self.get_selection_range() {
            index >= start && index <= end
        } else {
            false
        }
    }

    pub fn find_parent_index(&self, index: usize) -> Option<usize> {
        if index >= self.todo_list.items.len() {
            return None;
        }
        let target_indent = self.todo_list.items[index].indent_level;
        if target_indent == 0 {
            return None;
        }
        (0..index)
            .rev()
            .find(|&i| self.todo_list.items[i].indent_level < target_indent)
    }

    pub fn move_to_parent(&mut self) {
        if let Some(parent_idx) = self.find_parent_index(self.cursor_position) {
            self.cursor_position = parent_idx;
        }
    }

    /// Reload the todo list from the database.
    /// Used when external changes are detected (e.g., from API server).
    pub fn reload_from_database(&mut self) -> Result<()> {
        let date = self.todo_list.date;
        let new_list = load_todo_list(date)?;
        self.todo_list = new_list;
        self.clamp_cursor();
        self.unsaved_changes = false;
        Ok(())
    }
}
