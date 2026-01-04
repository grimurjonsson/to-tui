use super::mode::Mode;
use super::state::AppState;
use crate::keybindings::{Action, KeyBinding, KeyLookupResult};
use crate::storage::save_todo_list;
use crate::utils::unicode::{next_char_boundary, prev_char_boundary};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_key_event(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match state.mode {
        Mode::Navigate => handle_navigate_mode(key, state)?,
        Mode::Visual => handle_visual_mode(key, state)?,
        Mode::Edit => handle_edit_mode(key, state)?,
    }
    Ok(())
}

fn handle_navigate_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    let pending = if let (Some(pending_key), Some(pending_time)) =
        (state.pending_key.take(), state.pending_key_time.take())
    {
        let elapsed = pending_time.elapsed().as_millis() as u64;
        if elapsed < state.timeoutlen {
            Some(pending_key)
        } else {
            None
        }
    } else {
        None
    };

    match state.keybindings.lookup_navigate(&key, pending) {
        KeyLookupResult::Pending => {
            state.pending_key = Some(KeyBinding::from_event(&key));
            state.pending_key_time = Some(std::time::Instant::now());
            return Ok(());
        }
        KeyLookupResult::Action(action) => {
            execute_navigate_action(action, state)?;
        }
        KeyLookupResult::None => {}
    }

    if state.unsaved_changes {
        save_todo_list(&state.todo_list)?;
        state.unsaved_changes = false;
        state.last_save_time = Some(std::time::Instant::now());
    }

    Ok(())
}

fn execute_navigate_action(action: Action, state: &mut AppState) -> Result<()> {
    match action {
        Action::MoveUp => {
            state.clear_selection();
            state.move_cursor_up();
        }
        Action::MoveDown => {
            state.clear_selection();
            state.move_cursor_down();
        }
        Action::ToggleVisual => {
            state.start_or_extend_selection();
            state.mode = Mode::Visual;
        }
        Action::ExitVisual => {}
        Action::ToggleState => {
            if state.selected_item().is_some() {
                state.save_undo();
                if let Some(item) = state.selected_item_mut() {
                    item.toggle_state();
                    state.unsaved_changes = true;
                }
            }
        }
        Action::CycleState => {
            if state.selected_item().is_some() {
                state.save_undo();
                if let Some(item) = state.selected_item_mut() {
                    item.cycle_state();
                    state.unsaved_changes = true;
                }
            }
        }
        Action::Delete => {
            if !state.todo_list.items.is_empty() {
                state.save_undo();
                delete_current_item(state)?;
                save_todo_list(&state.todo_list)?;
                state.unsaved_changes = false;
                state.last_save_time = Some(std::time::Instant::now());
            }
        }
        Action::NewItem => {
            new_item_below(state);
        }
        Action::NewItemSameLevel => {
            new_item_at_same_level(state);
        }
        Action::EnterEditMode => {
            enter_edit_mode(state);
        }
        Action::Indent => {
            if let Some((start, end)) = state.get_selection_range() {
                state.save_undo();
                for idx in start..=end {
                    let _ = state.todo_list.indent_item(idx);
                }
                state.unsaved_changes = true;
                state.clear_selection();
            } else {
                state.save_undo();
                if state.todo_list.indent_item(state.cursor_position).is_ok() {
                    state.unsaved_changes = true;
                }
            }
        }
        Action::Outdent => {
            if let Some((start, end)) = state.get_selection_range() {
                state.save_undo();
                for idx in start..=end {
                    let _ = state.todo_list.outdent_item(idx);
                }
                state.unsaved_changes = true;
                state.clear_selection();
            } else {
                state.save_undo();
                if state.todo_list.outdent_item(state.cursor_position).is_ok() {
                    state.unsaved_changes = true;
                }
            }
        }
        Action::IndentWithChildren => {
            state.save_undo();
            if state
                .todo_list
                .indent_item_with_children(state.cursor_position)
                .is_ok()
            {
                state.unsaved_changes = true;
            }
        }
        Action::OutdentWithChildren => {
            state.save_undo();
            if state
                .todo_list
                .outdent_item_with_children(state.cursor_position)
                .is_ok()
            {
                state.unsaved_changes = true;
            }
        }
        Action::MoveItemUp => {
            state.save_undo();
            if let Ok(displacement) = state
                .todo_list
                .move_item_with_children_up(state.cursor_position)
            {
                state.cursor_position = state.cursor_position.saturating_sub(displacement);
                state.unsaved_changes = true;
            }
        }
        Action::MoveItemDown => {
            state.save_undo();
            if let Ok(displacement) = state
                .todo_list
                .move_item_with_children_down(state.cursor_position)
            {
                state.cursor_position = (state.cursor_position + displacement)
                    .min(state.todo_list.items.len().saturating_sub(1));
                state.unsaved_changes = true;
            }
        }
        Action::ToggleCollapse => {
            if state.todo_list.has_children(state.cursor_position) {
                state.save_undo();
                if let Some(item) = state.todo_list.items.get_mut(state.cursor_position) {
                    item.collapsed = !item.collapsed;
                    state.unsaved_changes = true;
                }
            }
        }
        Action::Expand => {
            let should_expand = state.todo_list.has_children(state.cursor_position)
                && state
                    .todo_list
                    .items
                    .get(state.cursor_position)
                    .map(|item| item.collapsed)
                    .unwrap_or(false);

            if should_expand {
                state.save_undo();
                if let Some(item) = state.todo_list.items.get_mut(state.cursor_position) {
                    item.collapsed = false;
                    state.unsaved_changes = true;
                }
            }
        }
        Action::CollapseOrParent => {
            let has_children = state.todo_list.has_children(state.cursor_position);
            let is_collapsed = state
                .todo_list
                .items
                .get(state.cursor_position)
                .map(|item| item.collapsed)
                .unwrap_or(false);

            if has_children && !is_collapsed {
                state.save_undo();
                if let Some(item) = state.todo_list.items.get_mut(state.cursor_position) {
                    item.collapsed = true;
                    state.unsaved_changes = true;
                }
            } else {
                state.move_to_parent();
            }
        }
        Action::Undo => {
            if state.undo() {
                save_todo_list(&state.todo_list)?;
                state.last_save_time = Some(std::time::Instant::now());
            }
        }
        Action::ToggleHelp => {
            state.show_help = !state.show_help;
        }
        Action::CloseHelp => {
            if state.show_help {
                state.show_help = false;
            }
        }
        Action::Quit => {
            if state.show_help {
                state.show_help = false;
            } else {
                state.should_quit = true;
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_visual_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    if let Some(action) = state.keybindings.get_visual_action(&key) {
        execute_visual_action(action, state)?;
    }

    if state.unsaved_changes {
        save_todo_list(&state.todo_list)?;
        state.unsaved_changes = false;
        state.last_save_time = Some(std::time::Instant::now());
    }

    Ok(())
}

fn execute_visual_action(action: Action, state: &mut AppState) -> Result<()> {
    match action {
        Action::MoveUp => {
            state.move_cursor_up();
        }
        Action::MoveDown => {
            state.move_cursor_down();
        }
        Action::ToggleVisual | Action::ExitVisual | Action::CloseHelp => {
            state.clear_selection();
            state.mode = Mode::Navigate;
        }
        Action::Quit => {
            state.clear_selection();
            state.mode = Mode::Navigate;
        }
        Action::Undo => {
            if state.undo() {
                save_todo_list(&state.todo_list)?;
                state.last_save_time = Some(std::time::Instant::now());
            }
        }
        Action::Indent => {
            if let Some((start, end)) = state.get_selection_range() {
                let can_indent = if start == 0 {
                    false
                } else {
                    let prev_indent = state.todo_list.items[start - 1].indent_level;
                    let first_indent = state.todo_list.items[start].indent_level;
                    first_indent <= prev_indent
                };

                if can_indent {
                    state.save_undo();
                    for idx in start..=end {
                        state.todo_list.items[idx].indent_level += 1;
                    }
                    state.todo_list.recalculate_parent_ids();
                    state.unsaved_changes = true;
                }
            }
        }
        Action::Outdent => {
            if let Some((start, end)) = state.get_selection_range() {
                let can_outdent = state.todo_list.items[start].indent_level > 0;

                if can_outdent {
                    state.save_undo();
                    for idx in start..=end {
                        if state.todo_list.items[idx].indent_level > 0 {
                            state.todo_list.items[idx].indent_level -= 1;
                        }
                    }
                    state.todo_list.recalculate_parent_ids();
                    state.unsaved_changes = true;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_edit_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    if let Some(action) = state.keybindings.get_edit_action(&key) {
        match action {
            Action::EditCancel => {
                save_edit_buffer(state)?;
                state.mode = Mode::Navigate;
            }
            Action::EditConfirm => {
                save_edit_buffer(state)?;
                new_item_at_same_level(state);
            }
            Action::EditBackspace => {
                if state.edit_cursor_pos > 0 {
                    let prev_boundary =
                        prev_char_boundary(&state.edit_buffer, state.edit_cursor_pos);
                    state
                        .edit_buffer
                        .drain(prev_boundary..state.edit_cursor_pos);
                    state.edit_cursor_pos = prev_boundary;
                }
            }
            Action::EditLeft => {
                if state.edit_cursor_pos > 0 {
                    state.edit_cursor_pos =
                        prev_char_boundary(&state.edit_buffer, state.edit_cursor_pos);
                }
            }
            Action::EditRight => {
                if state.edit_cursor_pos < state.edit_buffer.len() {
                    state.edit_cursor_pos =
                        next_char_boundary(&state.edit_buffer, state.edit_cursor_pos);
                }
            }
            Action::EditHome => {
                state.edit_cursor_pos = 0;
            }
            Action::EditEnd => {
                state.edit_cursor_pos = state.edit_buffer.len();
            }
            Action::EditIndent => {
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
            Action::EditOutdent => {
                if state.is_creating_new_item {
                    state.pending_indent_level = state.pending_indent_level.saturating_sub(1);
                } else if state.todo_list.outdent_item(state.cursor_position).is_ok() {
                    state.unsaved_changes = true;
                }
            }
            _ => {}
        }
    } else if let KeyCode::Char(c) = key.code {
        state.edit_buffer.insert(state.edit_cursor_pos, c);
        state.edit_cursor_pos += c.len_utf8();
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
            state
                .todo_list
                .add_item_with_indent(state.edit_buffer.clone(), state.pending_indent_level);
            state.cursor_position = 0;
        } else {
            let insert_position = state.cursor_position + 1;
            state.todo_list.insert_item(
                insert_position,
                state.edit_buffer.clone(),
                state.pending_indent_level,
            )?;
            state.cursor_position = insert_position;
        }
        state.is_creating_new_item = false;
    } else if state.cursor_position < state.todo_list.items.len() {
        state.todo_list.items[state.cursor_position].content = state.edit_buffer.clone();
    } else {
        state
            .todo_list
            .add_item_with_indent(state.edit_buffer.clone(), 0);
        state.cursor_position = state.todo_list.items.len() - 1;
    }

    state.edit_buffer.clear();
    state.edit_cursor_pos = 0;
    state.unsaved_changes = true;

    Ok(())
}
