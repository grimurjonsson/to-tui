use super::state::AppState;
use super::mode::Mode;
use crate::storage::save_todo_list;
use crate::utils::unicode::{next_char_boundary, prev_char_boundary};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle_key_event(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match state.mode {
        Mode::Navigate => handle_navigate_mode(key, state)?,
        Mode::Edit => handle_edit_mode(key, state)?,
    }
    Ok(())
}

fn handle_navigate_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    if state.confirm_delete {
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                state.confirm_delete = false;
                state.save_undo();
                delete_current_item(state)?;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                state.confirm_delete = false;
            }
            _ => {}
        }
        return Ok(());
    }

    match (key.code, key.modifiers) {
        (KeyCode::Up, mods) if mods.intersects(KeyModifiers::SHIFT) &&
                                mods.intersects(KeyModifiers::ALT) => {
            if let Ok(displacement) = state.todo_list.move_item_with_children_up(state.cursor_position) {
                state.cursor_position = state.cursor_position.saturating_sub(displacement);
                state.unsaved_changes = true;
            }
        }
        (KeyCode::Down, mods) if mods.intersects(KeyModifiers::SHIFT) &&
                                  mods.intersects(KeyModifiers::ALT) => {
            if let Ok(displacement) = state.todo_list.move_item_with_children_down(state.cursor_position) {
                state.cursor_position = (state.cursor_position + displacement)
                    .min(state.todo_list.items.len().saturating_sub(1));
                state.unsaved_changes = true;
            }
        }

        // Navigation (plain arrows)
        (KeyCode::Up, KeyModifiers::NONE) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
            state.move_cursor_up();
        }
        (KeyCode::Down, KeyModifiers::NONE) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
            state.move_cursor_down();
        }

        // Toggle state (checked/unchecked)
        (KeyCode::Char('x'), KeyModifiers::NONE) => {
            if state.selected_item().is_some() {
                state.save_undo();
                if let Some(item) = state.selected_item_mut() {
                    item.toggle_state();
                    state.unsaved_changes = true;
                }
            }
        }

        // Cycle through all 4 states
        (KeyCode::Char(' '), _) => {
            if state.selected_item().is_some() {
                state.save_undo();
                if let Some(item) = state.selected_item_mut() {
                    item.cycle_state();
                    state.unsaved_changes = true;
                }
            }
        }

        // Indent/outdent WITH children using Alt/Option+Shift+Left/Right
        (KeyCode::Right, mods) if mods.intersects(KeyModifiers::SHIFT) &&
                                   mods.intersects(KeyModifiers::ALT) => {
            if let Err(_) = state.todo_list.indent_item_with_children(state.cursor_position) {
                // Silently ignore errors
            } else {
                state.unsaved_changes = true;
            }
        }
        (KeyCode::Left, mods) if mods.intersects(KeyModifiers::SHIFT) &&
                                  mods.intersects(KeyModifiers::ALT) => {
            if let Err(_) = state.todo_list.outdent_item_with_children(state.cursor_position) {
                // Silently ignore errors
            } else {
                state.unsaved_changes = true;
            }
        }

        // Indent/outdent single item (WITHOUT children) with Tab/Shift+Tab
        (KeyCode::Tab, KeyModifiers::NONE) => {
            if let Err(_) = state.todo_list.indent_item(state.cursor_position) {
                // Silently ignore errors
            } else {
                state.unsaved_changes = true;
            }
        }
        (KeyCode::BackTab, _) => {
            // BackTab is sent when Shift+Tab is pressed
            if let Err(_) = state.todo_list.outdent_item(state.cursor_position) {
                // Silently ignore errors
            } else {
                state.unsaved_changes = true;
            }
        }

        // Enter edit mode
        (KeyCode::Char('i'), KeyModifiers::NONE) | (KeyCode::Enter, KeyModifiers::NONE) => {
            enter_edit_mode(state);
        }

        // New item
        (KeyCode::Char('n'), KeyModifiers::NONE) => {
            new_item_below(state);
        }

        // Delete item
        (KeyCode::Char('d'), KeyModifiers::NONE) => {
            if !state.todo_list.items.is_empty() {
                state.confirm_delete = true;
            }
        }

        // Undo
        (KeyCode::Char('u'), KeyModifiers::NONE) => {
            if state.undo() {
                save_todo_list(&state.todo_list)?;
                state.last_save_time = Some(std::time::Instant::now());
            }
        }

        // Help toggle
        (KeyCode::Char('?'), KeyModifiers::NONE) => {
            state.show_help = !state.show_help;
        }

        (KeyCode::Esc, _) => {
            if state.show_help {
                state.show_help = false;
            }
        }

        // Quit - but if help is showing, just close help
        (KeyCode::Char('q'), KeyModifiers::NONE) => {
            if state.show_help {
                state.show_help = false;
            } else {
                state.should_quit = true;
            }
        }

        // Shift+Enter in Navigate mode: create new item at same level
        (KeyCode::Enter, mods) if mods.contains(KeyModifiers::SHIFT) => {
            new_item_at_same_level(state);
        }

        _ => {}
    }

    // Auto-save on changes
    if state.unsaved_changes {
        save_todo_list(&state.todo_list)?;
        state.unsaved_changes = false;
        state.last_save_time = Some(std::time::Instant::now());
    }

    Ok(())
}

fn handle_edit_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => {
            // Cancel edit
            state.mode = Mode::Navigate;
            state.edit_buffer.clear();
            state.edit_cursor_pos = 0;
            state.is_creating_new_item = false;
        }
        (KeyCode::Enter, mods) if mods.contains(KeyModifiers::SHIFT) => {
            // Shift+Enter: Save and create new item at same level
            save_edit_buffer(state)?;
            new_item_at_same_level(state);
        }
        (KeyCode::Enter, _) => {
            // Regular Enter: Save edit and return to Navigate
            save_edit_buffer(state)?;
            state.mode = Mode::Navigate;
        }
        (KeyCode::Backspace, _) => {
            if state.edit_cursor_pos > 0 {
                let prev_boundary = prev_char_boundary(&state.edit_buffer, state.edit_cursor_pos);
                state.edit_buffer.drain(prev_boundary..state.edit_cursor_pos);
                state.edit_cursor_pos = prev_boundary;
            }
        }
        (KeyCode::Left, _) => {
            if state.edit_cursor_pos > 0 {
                state.edit_cursor_pos = prev_char_boundary(&state.edit_buffer, state.edit_cursor_pos);
            }
        }
        (KeyCode::Right, _) => {
            if state.edit_cursor_pos < state.edit_buffer.len() {
                state.edit_cursor_pos = next_char_boundary(&state.edit_buffer, state.edit_cursor_pos);
            }
        }
        (KeyCode::Home, _) => {
            state.edit_cursor_pos = 0;
        }
        (KeyCode::End, _) => {
            state.edit_cursor_pos = state.edit_buffer.len();
        }
        (KeyCode::Tab, _) | (KeyCode::Char('\t'), _) => {
            if state.is_creating_new_item {
                let max_indent = state
                    .selected_item()
                    .map(|item| item.indent_level + 1)
                    .unwrap_or(0);
                if state.pending_indent_level < max_indent {
                    state.pending_indent_level += 1;
                }
            } else if state.todo_list.indent_item(state.cursor_position).is_ok() {
                state.unsaved_changes = true;
            }
        }
        (KeyCode::BackTab, _) => {
            if state.is_creating_new_item {
                state.pending_indent_level = state.pending_indent_level.saturating_sub(1);
            } else if state.todo_list.outdent_item(state.cursor_position).is_ok() {
                state.unsaved_changes = true;
            }
        }
        (KeyCode::Char(c), _) => {
            state.edit_buffer.insert(state.edit_cursor_pos, c);
            state.edit_cursor_pos += c.len_utf8();
        }
        _ => {}
    }
    Ok(())
}

fn enter_edit_mode(state: &mut AppState) {
    if let Some(item) = state.selected_item() {
        state.edit_buffer = item.content.clone();
        state.edit_cursor_pos = state.edit_buffer.len();
        state.mode = Mode::Edit;
        state.is_creating_new_item = false;
    }
}

fn new_item_below(state: &mut AppState) {
    state.edit_buffer.clear();
    state.edit_cursor_pos = 0;
    state.mode = Mode::Edit;
    state.is_creating_new_item = true;
    state.pending_indent_level = state
        .selected_item()
        .map(|item| item.indent_level)
        .unwrap_or(0);
}

fn new_item_at_same_level(state: &mut AppState) {
    state.edit_buffer.clear();
    state.edit_cursor_pos = 0;
    state.mode = Mode::Edit;
    state.is_creating_new_item = true;
    state.pending_indent_level = state
        .selected_item()
        .map(|item| item.indent_level)
        .unwrap_or(0);
}

fn delete_current_item(state: &mut AppState) -> Result<()> {
    if state.todo_list.items.is_empty() {
        return Ok(());
    }

    state.todo_list.delete_item(state.cursor_position)?;
    state.clamp_cursor();
    state.unsaved_changes = true;
    Ok(())
}

fn save_edit_buffer(state: &mut AppState) -> Result<()> {
    if state.edit_buffer.trim().is_empty() {
        state.edit_buffer.clear();
        state.edit_cursor_pos = 0;
        state.is_creating_new_item = false;
        return Ok(());
    }

    if state.is_creating_new_item {
        if state.todo_list.items.is_empty() {
            state.todo_list.add_item_with_indent(state.edit_buffer.clone(), state.pending_indent_level);
            state.cursor_position = 0;
        } else {
            let insert_position = state.cursor_position + 1;
            state.todo_list.insert_item(insert_position, state.edit_buffer.clone(), state.pending_indent_level)?;
            state.cursor_position = insert_position;
        }
        state.is_creating_new_item = false;
    } else {
        // Editing existing item
        if state.cursor_position < state.todo_list.items.len() {
            state.todo_list.items[state.cursor_position].content = state.edit_buffer.clone();
        } else {
            // Fallback: adding new item at end
            state.todo_list.add_item_with_indent(state.edit_buffer.clone(), 0);
            state.cursor_position = state.todo_list.items.len() - 1;
        }
    }

    state.edit_buffer.clear();
    state.edit_cursor_pos = 0;
    state.unsaved_changes = true;

    Ok(())
}
