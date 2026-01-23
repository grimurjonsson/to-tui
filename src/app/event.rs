use super::mode::Mode;
use super::state::{AppState, MoveToProjectSubState, PluginSubState, ProjectSubState};
use crate::clipboard::copy_to_clipboard;
use crate::config::Config;
use crate::keybindings::{Action, KeyBinding, KeyLookupResult};
use crate::plugin::PluginRegistry;
use crate::project::{Project, ProjectRegistry, DEFAULT_PROJECT_NAME};
use crate::storage::file::save_todo_list_for_project;
use crate::storage::{execute_rollover_for_project, find_rollover_candidates_for_project, soft_delete_todos_for_project};
use crate::utils::paths::{get_dailies_dir_for_project, get_project_dir};
use crate::utils::cursor::{set_mouse_cursor_default, set_mouse_cursor_pointer};
use crate::utils::unicode::{
    next_char_boundary, next_word_boundary, prev_char_boundary, prev_word_boundary,
};
use crate::utils::upgrade::{check_write_permission, prepare_binary, replace_and_restart, UpgradeSubState};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use std::fs;
use std::sync::mpsc;
use std::thread;

/// Total number of lines in the help content (must match render_help_overlay)
const HELP_TOTAL_LINES: u16 = 57;
const GITHUB_URL: &str = "https://github.com/grimurjonsson/to-tui";

pub fn handle_key_event(key: KeyEvent, state: &mut AppState) -> Result<()> {
    // Handle help overlay scrolling when help is visible
    if state.show_help {
        // Calculate max scroll based on terminal height
        // Help popup is 80% of terminal height, minus 2 for borders
        let popup_height = (state.terminal_height * 80) / 100;
        let inner_height = popup_height.saturating_sub(2);
        let max_scroll = HELP_TOTAL_LINES.saturating_sub(inner_height);

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                state.help_scroll = state.help_scroll.saturating_sub(1);
                return Ok(());
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if state.help_scroll < max_scroll {
                    state.help_scroll = state.help_scroll.saturating_add(1);
                }
                return Ok(());
            }
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                state.show_help = false;
                state.help_scroll = 0;
                return Ok(());
            }
            _ => return Ok(()),
        }
    }

    match state.mode {
        Mode::Navigate => handle_navigate_mode(key, state)?,
        Mode::Visual => handle_visual_mode(key, state)?,
        Mode::Edit => handle_edit_mode(key, state)?,
        Mode::ConfirmDelete => handle_confirm_delete_mode(key, state)?,
        Mode::Plugin => handle_plugin_mode(key, state)?,
        Mode::Rollover => handle_rollover_mode(key, state)?,
        Mode::UpgradePrompt => handle_upgrade_prompt_mode(key, state)?,
        Mode::ProjectSelect => handle_project_select_mode(key, state)?,
        Mode::MoveToProject => handle_move_to_project_mode(key, state)?,
    }
    Ok(())
}

pub fn handle_mouse_event(mouse: MouseEvent, state: &mut AppState) -> Result<()> {
    // Handle scroll events in help overlay
    if state.show_help {
        let popup_height = (state.terminal_height * 80) / 100;
        let inner_height = popup_height.saturating_sub(2);
        let max_scroll = HELP_TOTAL_LINES.saturating_sub(inner_height);

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                state.help_scroll = state.help_scroll.saturating_sub(3);
            }
            MouseEventKind::ScrollDown => {
                state.help_scroll = state.help_scroll.saturating_add(3).min(max_scroll);
            }
            _ => {}
        }
        return Ok(());
    }

    // Handle mouse move events for cursor hover effects
    if let MouseEventKind::Moved = mouse.kind {
        let is_over_link = is_mouse_over_status_bar_link(state, mouse.row as usize, mouse.column as usize);

        if is_over_link && !state.cursor_is_pointer {
            set_mouse_cursor_pointer();
            state.cursor_is_pointer = true;
        } else if !is_over_link && state.cursor_is_pointer {
            set_mouse_cursor_default();
            state.cursor_is_pointer = false;
        }
        return Ok(());
    }

    if state.mode != Mode::Navigate {
        return Ok(());
    }

    // Handle scroll events (allowed even in readonly mode for viewing archived dates)
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            for _ in 0..3 {
                state.move_cursor_up();
            }
            return Ok(());
        }
        MouseEventKind::ScrollDown => {
            for _ in 0..3 {
                state.move_cursor_down();
            }
            return Ok(());
        }
        _ => {}
    }

    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
        let clicked_row = mouse.row as usize;
        let clicked_col = mouse.column as usize;

        // Check if click is on status bar (bottom row)
        if clicked_row == state.terminal_height.saturating_sub(1) as usize {
            // Check if GitHub link is clicked
            let github_link = "[github repo] ";
            let version_text = if let Some(ref new_version) = state.new_version_available {
                let current_version = env!("CARGO_PKG_VERSION");
                format!("v{} → v{}", current_version, new_version)
            } else {
                format!("v{}", env!("CARGO_PKG_VERSION"))
            };

            // GitHub link is just before version text at the right end
            let version_start = state.terminal_width.saturating_sub(version_text.len() as u16) as usize;
            let github_start = version_start.saturating_sub(github_link.len());
            let github_end = version_start - 1; // -1 to exclude the trailing space

            if clicked_col >= github_start && clicked_col < github_end {
                // Clicked on GitHub link - open browser
                let _ = open::that(GITHUB_URL);
                return Ok(());
            }

            // Check if version upgrade notification is visible and clicked
            if state.new_version_available.is_some() && clicked_col >= version_start {
                // Clicked on version text - open upgrade modal
                state.open_upgrade_modal();
                return Ok(());
            }
        }
    }

    // Block other mouse interactions in readonly mode
    if state.is_readonly() {
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
            // Sync list_state to update visual highlight
            state.sync_list_state();
        }
    }

    if state.unsaved_changes {
        save_todo_list_for_project(&state.todo_list, &state.current_project.name)?;
        state.unsaved_changes = false;
        state.last_save_time = Some(std::time::Instant::now());
    }

    Ok(())
}

/// Check if the mouse is over a clickable link in the status bar.
/// Returns true if over the GitHub link or the upgrade version text (when available).
fn is_mouse_over_status_bar_link(state: &AppState, row: usize, col: usize) -> bool {
    // Check if on status bar (bottom row)
    if row != state.terminal_height.saturating_sub(1) as usize {
        return false;
    }

    let github_link = "[github repo]";
    let github_link_with_space = "[github repo] ";
    let version_text = if let Some(ref new_version) = state.new_version_available {
        let current_version = env!("CARGO_PKG_VERSION");
        format!("v{} → v{}", current_version, new_version)
    } else {
        format!("v{}", env!("CARGO_PKG_VERSION"))
    };

    // Layout: ... {github_link} {version_text} {trailing_space}
    // version_text ends at terminal_width - 1 (trailing space)
    let version_end = state.terminal_width.saturating_sub(1) as usize;
    let version_start = version_end.saturating_sub(version_text.len());
    let github_start = version_start.saturating_sub(github_link_with_space.len());
    let github_end = github_start + github_link.len(); // exclude the trailing space

    // Check if over GitHub link
    if col >= github_start && col < github_end {
        return true;
    }

    // Check if over version text (only clickable when upgrade is available)
    if state.new_version_available.is_some() && col >= version_start && col < version_end {
        return true;
    }

    false
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

    // Calculate scroll offset based on current selection, not list_state.offset()
    // which may be stale (only updated during render)
    let scroll_offset = calculate_expected_offset(state);

    let visual_row = clicked_row - list_start_row;
    let mut current_visual_row = 0;
    let mut list_item_count = 0;

    let hidden_indices = state.todo_list.build_hidden_indices();

    for (idx, item) in state.todo_list.items.iter().enumerate() {
        if hidden_indices.contains(&idx) {
            continue;
        }

        // Skip ListItems that are scrolled past (above viewport)
        // Each todo is 1 ListItem, plus 1 more if it has an expanded description
        if list_item_count < scroll_offset {
            list_item_count += 1;
            // Account for expanded description as separate ListItem
            if !item.collapsed && item.description.is_some() {
                list_item_count += 1;
            }
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
        list_item_count += 1;

        // Account for expanded description box as separate ListItem with its own visual height
        if !item.collapsed && item.description.is_some() {
            let desc_height = calculate_description_visual_height(state, item);
            // Click on description box area - treat as clicking the parent item
            if visual_row >= current_visual_row && visual_row < current_visual_row + desc_height {
                return Some((idx, ClickZone::Content));
            }
            current_visual_row += desc_height;
            list_item_count += 1;
        }
    }

    None
}

/// Calculate the expected scroll offset based on current selection.
/// This mirrors what ratatui would calculate during render.
fn calculate_expected_offset(state: &AppState) -> usize {
    let selected = match state.list_state.selected() {
        Some(s) => s,
        None => return 0,
    };

    // Viewport height = terminal height - borders (2) - status bar (1)
    let viewport_height = state.terminal_height.saturating_sub(3).max(1) as usize;

    // Current offset from list_state (may be stale but gives us a starting point)
    let current_offset = state.list_state.offset();

    // If selected item is above current viewport, offset should be at selected
    if selected < current_offset {
        return selected;
    }

    // If selected item is below current viewport, scroll to show it
    if selected >= current_offset + viewport_height {
        return selected.saturating_sub(viewport_height - 1);
    }

    // Selected item is within viewport, keep current offset
    current_offset
}

fn calculate_item_visual_height(
    state: &AppState,
    idx: usize,
    item: &crate::todo::TodoItem,
) -> usize {
    // Calculate available width for content (terminal - borders)
    let available_width = state.terminal_width.saturating_sub(2) as usize;

    // Calculate prefix width: indent + fold_icon + checkbox
    let indent_width = item.indent_level * 2;
    let fold_icon_width = 2; // "▶ " or "▼ " or "  "
    let checkbox_width = 4; // "[x] "
    let prefix_width = indent_width + fold_icon_width + checkbox_width;

    // Content area width
    let content_max_width = available_width.saturating_sub(prefix_width);

    // Calculate wrapped height for the todo content
    let due_date_len = item
        .due_date
        .map(|_| 13) // " [YYYY-MM-DD]"
        .unwrap_or(0);

    let has_children = state.todo_list.has_children(idx);
    let collapse_indicator_len = if item.collapsed && has_children {
        8 // " (X/Y)" rough estimate
    } else {
        0
    };

    let content_len = item.content.len() + due_date_len + collapse_indicator_len;
    let wrapped_lines = if content_max_width > 0 {
        (content_len + content_max_width - 1) / content_max_width
    } else {
        1
    };
    let height = wrapped_lines.max(1);

    // Note: Description boxes are handled separately via calculate_description_visual_height
    height
}

/// Calculate the visual height of an expanded description box.
/// Description boxes have: top border (1) + content lines + bottom border (1)
fn calculate_description_visual_height(
    state: &AppState,
    item: &crate::todo::TodoItem,
) -> usize {
    if let Some(ref desc) = item.description {
        // Calculate box width similar to rendering
        let available_width = state.terminal_width.saturating_sub(2) as usize;
        let box_indent_width = item.indent_level * 2 + 4; // base indent + "    "
        let inner_width = available_width.saturating_sub(box_indent_width + 4); // 4 for borders and padding

        // Count wrapped lines
        let mut line_count = 0;
        for paragraph in desc.split('\n') {
            if paragraph.is_empty() {
                line_count += 1;
            } else {
                let para_len = paragraph.len();
                let wrapped = if inner_width > 0 {
                    (para_len + inner_width - 1) / inner_width
                } else {
                    1
                };
                line_count += wrapped.max(1);
            }
        }

        // top border + content lines + bottom border
        line_count + 2
    } else {
        0
    }
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
        save_todo_list_for_project(&state.todo_list, &state.current_project.name)?;
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
            | Action::CyclePriority
            | Action::SortByPriority
            | Action::MoveToProject
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
        Action::CyclePriority => {
            state.cycle_priority();
        }
        Action::SortByPriority => {
            state.sort_by_priority();
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
                    save_todo_list_for_project(&state.todo_list, &state.current_project.name)?;
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
                save_todo_list_for_project(&state.todo_list, &state.current_project.name)?;
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
            } else if let Ok(Some((source_date, items))) = find_rollover_candidates_for_project(&state.current_project.name) {
                state.open_rollover_modal(source_date, items);
            } else {
                state.set_status_message("No incomplete items to rollover".to_string());
            }
        }
        Action::OpenProjectModal => {
            state.open_project_modal();
        }
        Action::MoveToProject => {
            state.open_move_to_project_modal();
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
        save_todo_list_for_project(&state.todo_list, &state.current_project.name)?;
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
                save_todo_list_for_project(&state.todo_list, &state.current_project.name)?;
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
            save_todo_list_for_project(&state.todo_list, &state.current_project.name)?;
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
                let new_list = execute_rollover_for_project(&state.current_project.name, pending.source_date, pending.items)?;
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

fn handle_upgrade_prompt_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    let sub_state = state.upgrade_sub_state.clone();

    match sub_state {
        Some(UpgradeSubState::Prompt) | None => {
            // Initial prompt: Y (download), N (dismiss), S (skip)
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    // Check write permission before downloading
                    if let Err(e) = check_write_permission() {
                        state.upgrade_sub_state = Some(UpgradeSubState::Error {
                            message: e.to_string(),
                        });
                        return Ok(());
                    }
                    state.start_download();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    // Dismiss for this session
                    state.dismiss_upgrade_session();
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    // Skip this version permanently
                    if let Some(version) = state.new_version_available.clone() {
                        state.skip_version_permanently(version)?;
                        state.set_status_message("Skipped version updates".to_string());
                    }
                }
                _ => {}
            }
        }
        Some(UpgradeSubState::Downloading { .. }) => {
            // During download, only allow cancel with Esc
            if key.code == KeyCode::Esc {
                state.download_progress_rx = None;
                state.upgrade_sub_state = None;
                state.show_upgrade_prompt = false;
                state.mode = Mode::Navigate;
            }
            // Otherwise ignore - download continues
        }
        Some(UpgradeSubState::Error { .. }) => {
            match key.code {
                KeyCode::Char('r') | KeyCode::Char('R') | KeyCode::Enter => {
                    // Check write permission before retrying
                    if let Err(e) = check_write_permission() {
                        state.upgrade_sub_state = Some(UpgradeSubState::Error {
                            message: e.to_string(),
                        });
                        return Ok(());
                    }
                    // Retry download
                    state.start_download();
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    // Dismiss error
                    state.upgrade_sub_state = None;
                    state.show_upgrade_prompt = false;
                    state.mode = Mode::Navigate;
                }
                _ => {}
            }
        }
        Some(UpgradeSubState::RestartPrompt { downloaded_path }) => {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    // Check write permission first (should already be checked, but verify)
                    if let Err(e) = check_write_permission() {
                        state.upgrade_sub_state = Some(UpgradeSubState::Error {
                            message: e.to_string(),
                        });
                        return Ok(());
                    }

                    // Prepare binary (set permissions)
                    match prepare_binary(&downloaded_path) {
                        Ok(binary_path) => {
                            // Attempt replacement and restart
                            // Note: replace_and_restart will not return on success (exec replaces process)
                            if let Err(e) = replace_and_restart(&binary_path) {
                                state.upgrade_sub_state = Some(UpgradeSubState::Error {
                                    message: format!("Upgrade failed: {}", e),
                                });
                            }
                            // Clean up downloaded binary on failure (success doesn't return)
                            let _ = std::fs::remove_file(&downloaded_path);
                        }
                        Err(e) => {
                            state.upgrade_sub_state = Some(UpgradeSubState::Error {
                                message: format!("Preparation failed: {}", e),
                            });
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    // Clean up downloaded file
                    let _ = std::fs::remove_file(&downloaded_path);
                    state.upgrade_sub_state = None;
                    state.show_upgrade_prompt = false;
                    state.mode = Mode::Navigate;
                }
                _ => {}
            }
        }
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
    state.sync_list_state_for_new_item();
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
    state.sync_list_state_for_new_item();
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

    soft_delete_todos_for_project(&ids, date, &state.current_project.name)?;
    state.todo_list.remove_item_range(start, end)?;
    state.clamp_cursor();
    Ok(())
}

fn save_edit_buffer(state: &mut AppState) -> Result<()> {
    if state.edit_buffer.trim().is_empty() {
        let was_creating = state.is_creating_new_item;
        state.edit_buffer.clear();
        state.edit_cursor_pos = 0;
        state.is_creating_new_item = false;
        state.insert_above = false;
        if was_creating {
            // Reset visual highlight since phantom row is gone
            state.sync_list_state();
        }
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
            save_todo_list_for_project(&state.todo_list, &state.current_project.name)?;
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

fn handle_project_select_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    let project_state = match state.project_state.take() {
        Some(ps) => ps,
        None => {
            state.close_project_modal();
            return Ok(());
        }
    };

    match project_state {
        ProjectSubState::Selecting {
            projects,
            selected_index,
        } => handle_project_selecting(key, state, projects, selected_index),
        ProjectSubState::CreateInput {
            input_buffer,
            cursor_pos,
        } => handle_project_create_input(key, state, input_buffer, cursor_pos),
        ProjectSubState::RenameInput {
            project_name,
            input_buffer,
            cursor_pos,
        } => handle_project_rename_input(key, state, project_name, input_buffer, cursor_pos),
        ProjectSubState::ConfirmDelete { project_name } => {
            handle_project_confirm_delete(key, state, project_name)
        }
    }
}

fn handle_project_selecting(
    key: KeyEvent,
    state: &mut AppState,
    projects: Vec<Project>,
    mut selected_index: usize,
) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.close_project_modal();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            selected_index = selected_index.saturating_sub(1);
            state.project_state = Some(ProjectSubState::Selecting {
                projects,
                selected_index,
            });
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if selected_index < projects.len().saturating_sub(1) {
                selected_index += 1;
            }
            state.project_state = Some(ProjectSubState::Selecting {
                projects,
                selected_index,
            });
        }
        KeyCode::Enter => {
            if let Some(project) = projects.get(selected_index) {
                if project.name != state.current_project.name {
                    let project = project.clone();

                    // Save last_used_project to config
                    if let Ok(mut config) = Config::load() {
                        config.last_used_project = Some(project.name.clone());
                        let _ = config.save();
                    }

                    state.switch_project(project)?;
                    state.set_status_message("Switched project".to_string());
                }
                state.close_project_modal();
            }
        }
        KeyCode::Char('n') => {
            // Start creating a new project
            state.project_state = Some(ProjectSubState::CreateInput {
                input_buffer: String::new(),
                cursor_pos: 0,
            });
        }
        KeyCode::Char('r') => {
            // Start renaming the selected project
            if let Some(project) = projects.get(selected_index) {
                if project.name == DEFAULT_PROJECT_NAME {
                    state.set_status_message("Cannot rename the default project".to_string());
                    state.project_state = Some(ProjectSubState::Selecting {
                        projects,
                        selected_index,
                    });
                } else {
                    state.project_state = Some(ProjectSubState::RenameInput {
                        project_name: project.name.clone(),
                        input_buffer: project.name.clone(),
                        cursor_pos: project.name.len(),
                    });
                }
            }
        }
        KeyCode::Char('d') => {
            // Start deleting the selected project
            if let Some(project) = projects.get(selected_index) {
                if project.name == DEFAULT_PROJECT_NAME {
                    state.set_status_message("Cannot delete the default project".to_string());
                    state.project_state = Some(ProjectSubState::Selecting {
                        projects,
                        selected_index,
                    });
                } else if project.name == state.current_project.name {
                    state
                        .set_status_message("Cannot delete the currently active project".to_string());
                    state.project_state = Some(ProjectSubState::Selecting {
                        projects,
                        selected_index,
                    });
                } else {
                    state.project_state = Some(ProjectSubState::ConfirmDelete {
                        project_name: project.name.clone(),
                    });
                }
            }
        }
        _ => {
            state.project_state = Some(ProjectSubState::Selecting {
                projects,
                selected_index,
            });
        }
    }
    Ok(())
}

fn handle_project_create_input(
    key: KeyEvent,
    state: &mut AppState,
    mut input_buffer: String,
    mut cursor_pos: usize,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            // Go back to project list
            state.open_project_modal();
        }
        KeyCode::Enter if !input_buffer.trim().is_empty() => {
            let name = input_buffer.trim().to_string();

            // Create the project
            let mut registry = ProjectRegistry::load()?;
            match registry.create(&name) {
                Ok(project) => {
                    let project = project.clone();
                    // Create the project directory
                    let dailies_dir = get_dailies_dir_for_project(&project.name)?;
                    fs::create_dir_all(&dailies_dir)?;

                    state.set_status_message(format!("Created project '{}'", project.name));

                    // Switch to the new project
                    if let Ok(mut config) = Config::load() {
                        config.last_used_project = Some(project.name.clone());
                        let _ = config.save();
                    }
                    state.switch_project(project)?;
                    state.close_project_modal();
                }
                Err(e) => {
                    state.set_status_message(format!("Error: {}", e));
                    state.open_project_modal();
                }
            }
        }
        KeyCode::Backspace if cursor_pos > 0 => {
            let prev = prev_char_boundary(&input_buffer, cursor_pos);
            input_buffer.drain(prev..cursor_pos);
            cursor_pos = prev;
            state.project_state = Some(ProjectSubState::CreateInput {
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::Left if cursor_pos > 0 => {
            cursor_pos = prev_char_boundary(&input_buffer, cursor_pos);
            state.project_state = Some(ProjectSubState::CreateInput {
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::Right if cursor_pos < input_buffer.len() => {
            cursor_pos = next_char_boundary(&input_buffer, cursor_pos);
            state.project_state = Some(ProjectSubState::CreateInput {
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::Home => {
            cursor_pos = 0;
            state.project_state = Some(ProjectSubState::CreateInput {
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::End => {
            cursor_pos = input_buffer.len();
            state.project_state = Some(ProjectSubState::CreateInput {
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::Char(c) => {
            input_buffer.insert(cursor_pos, c);
            cursor_pos += c.len_utf8();
            state.project_state = Some(ProjectSubState::CreateInput {
                input_buffer,
                cursor_pos,
            });
        }
        _ => {
            state.project_state = Some(ProjectSubState::CreateInput {
                input_buffer,
                cursor_pos,
            });
        }
    }
    Ok(())
}

fn handle_project_rename_input(
    key: KeyEvent,
    state: &mut AppState,
    project_name: String,
    mut input_buffer: String,
    mut cursor_pos: usize,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            // Go back to project list
            state.open_project_modal();
        }
        KeyCode::Enter if !input_buffer.trim().is_empty() => {
            let new_name = input_buffer.trim().to_string();

            if new_name == project_name {
                // No change
                state.open_project_modal();
                return Ok(());
            }

            // Rename the project
            let mut registry = ProjectRegistry::load()?;
            match registry.rename(&project_name, &new_name) {
                Ok(()) => {
                    // Rename the project directory
                    let old_dir = get_project_dir(&project_name)?;
                    let new_dir = get_project_dir(&new_name)?;
                    if old_dir.exists() {
                        fs::rename(&old_dir, &new_dir)?;
                    }

                    state.set_status_message(format!(
                        "Renamed '{}' to '{}'",
                        project_name, new_name
                    ));
                    state.open_project_modal();
                }
                Err(e) => {
                    state.set_status_message(format!("Error: {}", e));
                    state.open_project_modal();
                }
            }
        }
        KeyCode::Backspace if cursor_pos > 0 => {
            let prev = prev_char_boundary(&input_buffer, cursor_pos);
            input_buffer.drain(prev..cursor_pos);
            cursor_pos = prev;
            state.project_state = Some(ProjectSubState::RenameInput {
                project_name,
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::Left if cursor_pos > 0 => {
            cursor_pos = prev_char_boundary(&input_buffer, cursor_pos);
            state.project_state = Some(ProjectSubState::RenameInput {
                project_name,
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::Right if cursor_pos < input_buffer.len() => {
            cursor_pos = next_char_boundary(&input_buffer, cursor_pos);
            state.project_state = Some(ProjectSubState::RenameInput {
                project_name,
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::Home => {
            cursor_pos = 0;
            state.project_state = Some(ProjectSubState::RenameInput {
                project_name,
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::End => {
            cursor_pos = input_buffer.len();
            state.project_state = Some(ProjectSubState::RenameInput {
                project_name,
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::Char(c) => {
            input_buffer.insert(cursor_pos, c);
            cursor_pos += c.len_utf8();
            state.project_state = Some(ProjectSubState::RenameInput {
                project_name,
                input_buffer,
                cursor_pos,
            });
        }
        _ => {
            state.project_state = Some(ProjectSubState::RenameInput {
                project_name,
                input_buffer,
                cursor_pos,
            });
        }
    }
    Ok(())
}

fn handle_project_confirm_delete(
    key: KeyEvent,
    state: &mut AppState,
    project_name: String,
) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            // Delete the project
            let mut registry = ProjectRegistry::load()?;
            match registry.delete(&project_name) {
                Ok(()) => {
                    // Delete the project directory
                    let project_dir = get_project_dir(&project_name)?;
                    if project_dir.exists() {
                        fs::remove_dir_all(&project_dir)?;
                    }

                    // TODO: Also delete todos from database for this project

                    state.set_status_message(format!("Deleted project '{}'", project_name));
                    state.open_project_modal();
                }
                Err(e) => {
                    state.set_status_message(format!("Error: {}", e));
                    state.open_project_modal();
                }
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            // Cancel - go back to project list
            state.open_project_modal();
        }
        _ => {
            state.project_state = Some(ProjectSubState::ConfirmDelete { project_name });
        }
    }
    Ok(())
}

fn handle_move_to_project_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    let move_state = match state.move_to_project_state.take() {
        Some(ms) => ms,
        None => {
            state.close_move_to_project_modal();
            return Ok(());
        }
    };

    match move_state {
        MoveToProjectSubState::Selecting {
            projects,
            mut selected_index,
            item_index,
        } => {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    state.close_move_to_project_modal();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    selected_index = selected_index.saturating_sub(1);
                    state.move_to_project_state = Some(MoveToProjectSubState::Selecting {
                        projects,
                        selected_index,
                        item_index,
                    });
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if selected_index < projects.len().saturating_sub(1) {
                        selected_index += 1;
                    }
                    state.move_to_project_state = Some(MoveToProjectSubState::Selecting {
                        projects,
                        selected_index,
                        item_index,
                    });
                }
                KeyCode::Enter => {
                    if let Some(dest_project) = projects.get(selected_index) {
                        let dest_project = dest_project.clone();
                        // Re-set state temporarily so execute_move_to_project can read item_index
                        state.move_to_project_state = Some(MoveToProjectSubState::Selecting {
                            projects: projects.clone(),
                            selected_index,
                            item_index,
                        });

                        match state.execute_move_to_project(&dest_project) {
                            Ok(count) => {
                                state.set_status_message(format!(
                                    "Moved {} item(s) to '{}'",
                                    count,
                                    dest_project.name
                                ));
                                // Save source list
                                save_todo_list_for_project(&state.todo_list, &state.current_project.name)?;
                                state.unsaved_changes = false;
                                state.last_save_time = Some(std::time::Instant::now());
                            }
                            Err(e) => {
                                state.set_status_message(format!("Move failed: {}", e));
                            }
                        }
                        state.close_move_to_project_modal();
                    }
                }
                _ => {
                    state.move_to_project_state = Some(MoveToProjectSubState::Selecting {
                        projects,
                        selected_index,
                        item_index,
                    });
                }
            }
        }
    }
    Ok(())
}
