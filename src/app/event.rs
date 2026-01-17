use super::mode::Mode;
use super::state::{AppState, PluginSubState};
use crate::clipboard::copy_to_clipboard;
use crate::keybindings::{Action, KeyBinding, KeyLookupResult};
use crate::plugin::PluginRegistry;
use crate::storage::{execute_rollover, find_rollover_candidates, save_todo_list, soft_delete_todos};
use crate::utils::unicode::{
    next_char_boundary, next_word_boundary, prev_char_boundary, prev_word_boundary,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use std::sync::mpsc;
use std::thread;

pub fn handle_key_event(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match state.mode {
        Mode::Navigate => handle_navigate_mode(key, state)?,
        Mode::Visual => handle_visual_mode(key, state)?,
        Mode::Edit => handle_edit_mode(key, state)?,
        Mode::ConfirmDelete => handle_confirm_delete_mode(key, state)?,
        Mode::Plugin => handle_plugin_mode(key, state)?,
        Mode::Rollover => handle_rollover_mode(key, state)?,
    }
    Ok(())
}

pub fn handle_mouse_event(mouse: MouseEvent, state: &mut AppState) -> Result<()> {
    if state.mode != Mode::Navigate || state.is_readonly() {
        return Ok(());
    }

    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
        let clicked_row = mouse.row as usize;
        let clicked_col = mouse.column as usize;

        if let Some((item_idx, click_zone)) = map_click_to_item(state, clicked_row, clicked_col) {
            match click_zone {
                ClickZone::FoldIcon => {
                    let has_children = state.todo_list.has_children(item_idx);
                    let has_description = state
                        .todo_list
                        .items
                        .get(item_idx)
                        .map(|i| i.description.is_some())
                        .unwrap_or(false);

                    if has_children || has_description {
                        state.save_undo();
                        if let Some(item) = state.todo_list.items.get_mut(item_idx) {
                            item.collapsed = !item.collapsed;
                            state.unsaved_changes = true;
                        }
                    }
                    state.cursor_position = item_idx;
                }
                ClickZone::Checkbox => {
                    state.save_undo();
                    if let Some(item) = state.todo_list.items.get_mut(item_idx) {
                        item.toggle_state();
                        state.unsaved_changes = true;
                    }
                    state.cursor_position = item_idx;
                }
                ClickZone::Content => {
                    state.cursor_position = item_idx;
                }
            }
        }
    }

    if state.unsaved_changes {
        save_todo_list(&state.todo_list)?;
        state.unsaved_changes = false;
        state.last_save_time = Some(std::time::Instant::now());
    }

    Ok(())
}

enum ClickZone {
    FoldIcon,
    Checkbox,
    Content,
}

fn map_click_to_item(
    state: &AppState,
    clicked_row: usize,
    clicked_col: usize,
) -> Option<(usize, ClickZone)> {
    let list_start_row = 1;

    if clicked_row < list_start_row {
        return None;
    }

    let visual_row = clicked_row - list_start_row;
    let mut current_visual_row = 0;

    let hidden_indices = state.todo_list.build_hidden_indices();

    for (idx, item) in state.todo_list.items.iter().enumerate() {
        if hidden_indices.contains(&idx) {
            continue;
        }

        let item_height = calculate_item_visual_height(state, idx, item);

        if visual_row >= current_visual_row && visual_row < current_visual_row + item_height {
            let indent_width = item.indent_level * 2;
            let fold_icon_end = indent_width + 2;
            let checkbox_end = fold_icon_end + 4;

            let zone = if clicked_col < fold_icon_end {
                ClickZone::FoldIcon
            } else if clicked_col < checkbox_end {
                ClickZone::Checkbox
            } else {
                ClickZone::Content
            };

            return Some((idx, zone));
        }

        current_visual_row += item_height;
    }

    None
}

fn calculate_item_visual_height(
    state: &AppState,
    idx: usize,
    item: &crate::todo::TodoItem,
) -> usize {
    let mut height = 1;

    let has_description = item.description.is_some();
    let is_collapsed = item.collapsed;

    if has_description && !is_collapsed
        && let Some(ref desc) = item.description {
            let line_count = desc.lines().count().max(1);
            height += line_count + 2;
        }

    let has_children = state.todo_list.has_children(idx);
    if is_collapsed && has_children {
        return 1;
    }

    height
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
    let dominated_by_readonly = matches!(
        action,
        Action::ToggleState
            | Action::CycleState
            | Action::Delete
            | Action::NewItem
            | Action::NewItemSameLevel
            | Action::InsertItemAbove
            | Action::EnterEditMode
            | Action::Indent
            | Action::Outdent
            | Action::IndentWithChildren
            | Action::OutdentWithChildren
            | Action::MoveItemUp
            | Action::MoveItemDown
            | Action::ToggleCollapse
            | Action::Undo
    );

    if state.is_readonly() && dominated_by_readonly {
        return Ok(());
    }

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
            state.toggle_current_item_state();
        }
        Action::CycleState => {
            state.cycle_current_item_state();
        }
        Action::Delete => {
            if !state.todo_list.items.is_empty() {
                let has_children = state.todo_list.has_children(state.cursor_position);
                if has_children {
                    let (_, end) = state
                        .todo_list
                        .get_item_range(state.cursor_position)
                        .unwrap_or((state.cursor_position, state.cursor_position + 1));
                    let subtask_count = end - state.cursor_position - 1;
                    state.pending_delete_subtask_count = Some(subtask_count);
                    state.mode = Mode::ConfirmDelete;
                } else {
                    state.save_undo();
                    delete_current_item(state)?;
                    save_todo_list(&state.todo_list)?;
                    state.unsaved_changes = false;
                    state.last_save_time = Some(std::time::Instant::now());
                }
            }
        }
        Action::NewItem => {
            new_item_below(state);
        }
        Action::NewItemSameLevel => {
            new_item_at_same_level(state);
        }
        Action::InsertItemAbove => {
            insert_item_above(state);
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
            state.toggle_current_item_collapse();
        }
        Action::Expand => {
            state.expand_current_item();
        }
        Action::CollapseOrParent => {
            state.collapse_or_move_to_parent();
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
        Action::PrevDay => {
            state.navigate_prev_day()?;
        }
        Action::NextDay => {
            state.navigate_next_day()?;
        }
        Action::GoToToday => {
            state.navigate_to_today()?;
        }
        Action::OpenPluginMenu => {
            state.open_plugin_menu();
        }
        Action::OpenRolloverModal => {
            // Check if we have pending rollover data, or try to find new candidates
            if state.has_pending_rollover() {
                state.mode = Mode::Rollover;
            } else if let Ok(Some((source_date, items))) = find_rollover_candidates() {
                state.open_rollover_modal(source_date, items);
            } else {
                state.set_status_message("No incomplete items to rollover".to_string());
            }
        }
        Action::Yank => {
            if let Some(item) = state.selected_item() {
                let text = item.content.clone();
                match copy_to_clipboard(&text) {
                    Ok(()) => {
                        // Truncate display text if too long
                        let display_text = if text.len() > 40 {
                            format!("{}...", &text[..37])
                        } else {
                            text.clone()
                        };
                        state.set_status_message(format!("Copied: {}", display_text));
                    }
                    Err(e) => {
                        state.set_status_message(format!("Clipboard error: {}", e));
                    }
                }
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

fn handle_confirm_delete_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            state.save_undo();
            delete_current_item(state)?;
            save_todo_list(&state.todo_list)?;
            state.unsaved_changes = false;
            state.last_save_time = Some(std::time::Instant::now());
            state.pending_delete_subtask_count = None;
            state.mode = Mode::Navigate;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            state.pending_delete_subtask_count = None;
            state.mode = Mode::Navigate;
        }
        _ => {}
    }
    Ok(())
}

fn handle_rollover_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            // Execute rollover
            if let Some(pending) = state.pending_rollover.take() {
                let new_list = execute_rollover(pending.source_date, pending.items)?;
                state.todo_list = new_list;
                state.cursor_position = 0;
                state.set_status_message("Rolled over incomplete items".to_string());
            }
            state.mode = Mode::Navigate;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Esc => {
            // Close modal but keep pending_rollover for later
            state.close_rollover_modal();
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
            Action::EditWordLeft => {
                state.edit_cursor_pos =
                    prev_word_boundary(&state.edit_buffer, state.edit_cursor_pos);
            }
            Action::EditWordRight => {
                state.edit_cursor_pos =
                    next_word_boundary(&state.edit_buffer, state.edit_cursor_pos);
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
                } else {
                    state.save_undo();
                    if state.todo_list.indent_item(state.cursor_position).is_ok() {
                        state.unsaved_changes = true;
                    }
                }
            }
            Action::EditOutdent => {
                if state.is_creating_new_item {
                    state.pending_indent_level = state.pending_indent_level.saturating_sub(1);
                } else {
                    state.save_undo();
                    if state.todo_list.outdent_item(state.cursor_position).is_ok() {
                        state.unsaved_changes = true;
                    }
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
    state.insert_above = false;
    state.pending_indent_level = state
        .selected_item()
        .map(|item| item.indent_level)
        .unwrap_or(0);
}

fn new_item_at_same_level(state: &mut AppState) {
    new_item_below(state);
}

fn insert_item_above(state: &mut AppState) {
    state.edit_buffer.clear();
    state.edit_cursor_pos = 0;
    state.mode = Mode::Edit;
    state.is_creating_new_item = true;
    state.insert_above = true;
    state.pending_indent_level = state
        .selected_item()
        .map(|item| item.indent_level)
        .unwrap_or(0);
}

fn delete_current_item(state: &mut AppState) -> Result<()> {
    if state.todo_list.items.is_empty() {
        return Ok(());
    }

    let date = state.todo_list.date;
    let (start, end) = state
        .todo_list
        .get_item_range(state.cursor_position)
        .unwrap_or((state.cursor_position, state.cursor_position + 1));

    let ids: Vec<_> = state.todo_list.items[start..end]
        .iter()
        .map(|item| item.id)
        .collect();

    soft_delete_todos(&ids, date)?;
    state.todo_list.remove_item_range(start, end)?;
    state.clamp_cursor();
    Ok(())
}

fn save_edit_buffer(state: &mut AppState) -> Result<()> {
    if state.edit_buffer.trim().is_empty() {
        state.edit_buffer.clear();
        state.edit_cursor_pos = 0;
        state.is_creating_new_item = false;
        state.insert_above = false;
        return Ok(());
    }

    state.save_undo();

    if state.is_creating_new_item {
        if state.todo_list.items.is_empty() {
            state
                .todo_list
                .add_item_with_indent(state.edit_buffer.clone(), state.pending_indent_level);
            state.cursor_position = 0;
        } else {
            let insert_position = if state.insert_above {
                state.cursor_position
            } else {
                let current_item = &state.todo_list.items[state.cursor_position];
                let has_hidden_children =
                    current_item.collapsed && state.todo_list.has_children(state.cursor_position);

                if has_hidden_children {
                    let (_, after_all_children) = state
                        .todo_list
                        .get_item_range(state.cursor_position)
                        .unwrap_or((state.cursor_position, state.cursor_position + 1));
                    after_all_children
                } else {
                    state.cursor_position + 1
                }
            };
            state.todo_list.insert_item(
                insert_position,
                state.edit_buffer.clone(),
                state.pending_indent_level,
            )?;
            if state.insert_above {
                state.cursor_position += 1;
            } else {
                state.cursor_position = insert_position;
            }
        }
        state.is_creating_new_item = false;
        state.insert_above = false;
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

fn handle_plugin_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    let plugin_state = match state.plugin_state.take() {
        Some(ps) => ps,
        None => {
            state.close_plugin_menu();
            return Ok(());
        }
    };

    match plugin_state {
        PluginSubState::Selecting {
            plugins,
            selected_index,
        } => handle_plugin_selecting(key, state, plugins, selected_index),
        PluginSubState::InputPrompt {
            plugin_name,
            input_buffer,
            cursor_pos,
        } => handle_plugin_input(key, state, plugin_name, input_buffer, cursor_pos),
        PluginSubState::Executing { plugin_name } => {
            state.plugin_state = Some(PluginSubState::Executing { plugin_name });
            Ok(())
        }
        PluginSubState::Error { message } => handle_plugin_error(key, state, message),
        PluginSubState::Preview { items } => handle_plugin_preview(key, state, items),
    }
}

fn handle_plugin_selecting(
    key: KeyEvent,
    state: &mut AppState,
    plugins: Vec<crate::plugin::GeneratorInfo>,
    mut selected_index: usize,
) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.close_plugin_menu();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            selected_index = selected_index.saturating_sub(1);
            state.plugin_state = Some(PluginSubState::Selecting {
                plugins,
                selected_index,
            });
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if selected_index < plugins.len().saturating_sub(1) {
                selected_index += 1;
            }
            state.plugin_state = Some(PluginSubState::Selecting {
                plugins,
                selected_index,
            });
        }
        KeyCode::Enter => {
            if let Some(plugin) = plugins.get(selected_index) {
                if plugin.available {
                    state.plugin_state = Some(PluginSubState::InputPrompt {
                        plugin_name: plugin.name.clone(),
                        input_buffer: String::new(),
                        cursor_pos: 0,
                    });
                } else {
                    let reason = plugin
                        .unavailable_reason
                        .clone()
                        .unwrap_or_else(|| "Unknown reason".to_string());
                    state.plugin_state = Some(PluginSubState::Error {
                        message: format!("Plugin '{}' is not available: {}", plugin.name, reason),
                    });
                }
            }
        }
        _ => {
            state.plugin_state = Some(PluginSubState::Selecting {
                plugins,
                selected_index,
            });
        }
    }
    Ok(())
}

fn handle_plugin_input(
    key: KeyEvent,
    state: &mut AppState,
    plugin_name: String,
    mut input_buffer: String,
    mut cursor_pos: usize,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            let plugins = state.plugin_registry.list();
            state.plugin_state = Some(PluginSubState::Selecting {
                plugins,
                selected_index: 0,
            });
            return Ok(());
        }
        KeyCode::Enter if !input_buffer.trim().is_empty() => {
            state.plugin_state = Some(PluginSubState::Executing {
                plugin_name: plugin_name.clone(),
            });

            let (tx, rx) = mpsc::channel();
            state.plugin_result_rx = Some(rx);

            let input = input_buffer.clone();
            let name = plugin_name.clone();

            thread::spawn(move || {
                let registry = PluginRegistry::new();
                let result = match registry.get(&name) {
                    Some(generator) => generator
                        .generate(&input)
                        .map_err(|e| format!("Plugin error: {e}")),
                    None => Err(format!("Plugin '{name}' not found")),
                };
                let _ = tx.send(result);
            });
            return Ok(());
        }
        KeyCode::Backspace if cursor_pos > 0 => {
            let prev = prev_char_boundary(&input_buffer, cursor_pos);
            input_buffer.drain(prev..cursor_pos);
            cursor_pos = prev;
        }
        KeyCode::Left if cursor_pos > 0 => {
            cursor_pos = prev_char_boundary(&input_buffer, cursor_pos);
        }
        KeyCode::Right if cursor_pos < input_buffer.len() => {
            cursor_pos = next_char_boundary(&input_buffer, cursor_pos);
        }
        KeyCode::Home => cursor_pos = 0,
        KeyCode::End => cursor_pos = input_buffer.len(),
        KeyCode::Char(c) => {
            input_buffer.insert(cursor_pos, c);
            cursor_pos += c.len_utf8();
        }
        _ => {}
    }

    // Restore InputPrompt state with potentially modified input_buffer and cursor_pos
    state.plugin_state = Some(PluginSubState::InputPrompt {
        plugin_name,
        input_buffer,
        cursor_pos,
    });
    Ok(())
}

fn handle_plugin_error(key: KeyEvent, state: &mut AppState, message: String) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
            state.close_plugin_menu();
        }
        _ => {
            state.plugin_state = Some(PluginSubState::Error { message });
        }
    }
    Ok(())
}

fn handle_plugin_preview(
    key: KeyEvent,
    state: &mut AppState,
    items: Vec<crate::todo::TodoItem>,
) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            let count = items.len();
            state.save_undo();
            for item in items {
                state.todo_list.items.push(item);
            }
            state.unsaved_changes = true;
            save_todo_list(&state.todo_list)?;
            state.unsaved_changes = false;
            state.last_save_time = Some(std::time::Instant::now());
            state.set_status_message(format!("Added {count} item(s) from plugin"));
            state.close_plugin_menu();
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            state.close_plugin_menu();
        }
        _ => {
            state.plugin_state = Some(PluginSubState::Preview { items });
        }
    }
    Ok(())
}
