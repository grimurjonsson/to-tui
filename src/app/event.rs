use super::mode::Mode;
use super::state::{
    AppState, MoveToProjectSubState, PluginSubState, PluginsModalState, PluginsTab,
    ProjectSubState,
};
use crate::clipboard::{copy_to_clipboard, CopyResult};
use crate::config::Config;
use crate::keybindings::{Action, KeyBinding, KeyLookupResult};
use crate::plugin::{
    marketplace::PluginEntry, CommandExecutor, GeneratorInfo, PluginAction, PluginErrorKind,
    PluginHostApiImpl, PluginLoadError,
};
use crate::project::{Project, ProjectRegistry, DEFAULT_PROJECT_NAME};
use crate::storage::file::save_todo_list_for_project;
use crate::storage::{execute_rollover_for_project, find_rollover_candidates_for_project, soft_delete_todos_for_project};
use crate::utils::paths::{get_dailies_dir_for_project, get_project_dir};
use crate::utils::cursor::{set_mouse_cursor_default, set_mouse_cursor_pointer};
use crate::utils::unicode::{
    next_char_boundary, next_word_boundary, prev_char_boundary, prev_word_boundary,
};
use crate::utils::upgrade::{check_write_permission, prepare_binary, replace_and_restart, UpgradeSubState};
use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::RBox;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use std::collections::HashSet;
use std::fs;
use totui_plugin_interface::{
    call_plugin_execute_with_host, FfiConfigType, FfiConfigValue, FfiEvent, FfiEventSource,
    FfiFieldChange, HostApi_TO,
};

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
        content_len.div_ceil(content_max_width)
    } else {
        1
    };

    // Note: Description boxes are handled separately via calculate_description_visual_height
    wrapped_lines.max(1)
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
                    para_len.div_ceil(inner_width)
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
        KeyLookupResult::None => {
            // Check plugin actions when host keybinding returns None
            let binding = KeyBinding::from_event(&key);
            if let Some(plugin_action) = state.plugin_action_registry.lookup(&binding) {
                execute_plugin_action(plugin_action.clone(), state)?;
            }
        }
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
                state.sync_list_state();
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
                state.sync_list_state();
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
            state.open_plugins_modal();
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
                // Truncate display text if too long
                let display_text = if text.len() > 40 {
                    format!("{}...", &text[..37])
                } else {
                    text.clone()
                };
                match copy_to_clipboard(&text) {
                    Ok(CopyResult::SystemClipboard) => {
                        state.set_status_message(format!("Copied: {}", display_text));
                    }
                    Ok(CopyResult::InternalBuffer { file_path }) => {
                        // Headless fallback - saved to internal buffer and file
                        let msg = match file_path {
                            Some(path) => format!(
                                "Copied to buffer (no clipboard): {} | Saved to {}",
                                display_text,
                                path.display()
                            ),
                            None => format!("Copied to buffer (no clipboard): {}", display_text),
                        };
                        state.set_status_message(msg);
                    }
                    Err(e) => {
                        state.set_status_message(format!("Copy failed: {}", e));
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

    // Fire OnDelete event BEFORE deletion (to capture item data)
    if let Some(item) = state.todo_list.items.get(state.cursor_position) {
        let ffi_item: totui_plugin_interface::FfiTodoItem = item.into();
        let event = FfiEvent::OnDelete { todo: ffi_item };
        state.fire_event(event);
    }

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

    // Track whether this is a new item or content edit
    let was_creating = state.is_creating_new_item;
    let mut new_item_index: Option<usize> = None;

    if state.is_creating_new_item {
        if state.todo_list.items.is_empty() {
            state
                .todo_list
                .add_item_with_indent(state.edit_buffer.clone(), state.pending_indent_level);
            state.cursor_position = 0;
            new_item_index = Some(0);
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
                new_item_index = Some(state.cursor_position - 1);
            } else {
                state.cursor_position = insert_position;
                new_item_index = Some(insert_position);
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
        new_item_index = Some(state.cursor_position);
    }

    // Fire appropriate event based on whether this was a new item or edit
    if let Some(idx) = new_item_index {
        // New item created - fire OnAdd
        if let Some(item) = state.todo_list.items.get(idx) {
            let ffi_item: totui_plugin_interface::FfiTodoItem = item.into();
            let event = FfiEvent::OnAdd {
                todo: ffi_item,
                source: FfiEventSource::Manual,
            };
            state.fire_event(event);
        }
    } else if was_creating {
        // was_creating but no new_item_index means edge case - skip
    } else if state.cursor_position < state.todo_list.items.len() {
        // Content was edited - fire OnModify
        if let Some(item) = state.todo_list.items.get(state.cursor_position) {
            let ffi_item: totui_plugin_interface::FfiTodoItem = item.into();
            let event = FfiEvent::OnModify {
                todo: ffi_item,
                field_changed: FfiFieldChange::Content,
            };
            state.fire_event(event);
        }
    }

    state.edit_buffer.clear();
    state.edit_cursor_pos = 0;
    state.unsaved_changes = true;

    Ok(())
}

fn handle_plugin_mode(key: KeyEvent, state: &mut AppState) -> Result<()> {
    // First check for new plugins modal state (tabbed UI)
    if let Some(modal_state) = state.plugins_modal_state.take() {
        return handle_plugins_modal(key, state, modal_state);
    }

    // Fall back to old plugin_state for backward compatibility
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

/// Handle events for the new tabbed plugins modal
fn handle_plugins_modal(
    key: KeyEvent,
    state: &mut AppState,
    modal_state: PluginsModalState,
) -> Result<()> {
    match modal_state {
        PluginsModalState::Tabs {
            active_tab,
            installed_index,
            marketplace_index,
            marketplace_plugins,
            marketplace_loading,
            marketplace_error,
            marketplace_name,
        } => handle_plugins_tabs(
            key,
            state,
            active_tab,
            installed_index,
            marketplace_index,
            marketplace_plugins,
            marketplace_loading,
            marketplace_error,
            marketplace_name,
        ),
        PluginsModalState::Details {
            plugin,
            marketplace_plugins,
            marketplace_index,
        } => handle_plugins_modal_details(key, state, plugin, marketplace_plugins, marketplace_index),
        PluginsModalState::Input {
            plugin_name,
            input_buffer,
            cursor_pos,
        } => handle_plugins_modal_input(key, state, plugin_name, input_buffer, cursor_pos),
        PluginsModalState::SelectInput {
            plugin_name,
            field_name,
            options,
            selected_index,
        } => handle_plugins_modal_select_input(
            key,
            state,
            plugin_name,
            field_name,
            options,
            selected_index,
        ),
        PluginsModalState::Executing { plugin_name } => {
            // While executing, ignore keypresses - state is managed by async result
            state.plugins_modal_state = Some(PluginsModalState::Executing { plugin_name });
            Ok(())
        }
        PluginsModalState::Preview { items } => handle_plugins_modal_preview(key, state, items),
        PluginsModalState::Error { message } => handle_plugins_modal_error(key, state, message),
    }
}

/// Handle key events in the Tabs view of plugins modal
#[allow(clippy::too_many_arguments)]
fn handle_plugins_tabs(
    key: KeyEvent,
    state: &mut AppState,
    mut active_tab: PluginsTab,
    mut installed_index: usize,
    mut marketplace_index: usize,
    marketplace_plugins: Option<Vec<PluginEntry>>,
    marketplace_loading: bool,
    marketplace_error: Option<String>,
    marketplace_name: String,
) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.close_plugins_modal();
        }
        KeyCode::Tab | KeyCode::BackTab => {
            // Switch tabs
            active_tab = match active_tab {
                PluginsTab::Installed => PluginsTab::Marketplace,
                PluginsTab::Marketplace => PluginsTab::Installed,
            };

            // Start marketplace fetch when switching to Marketplace tab if not loaded
            if active_tab == PluginsTab::Marketplace
                && marketplace_plugins.is_none()
                && !marketplace_loading
            {
                state.plugins_modal_state = Some(PluginsModalState::Tabs {
                    active_tab,
                    installed_index,
                    marketplace_index,
                    marketplace_plugins,
                    marketplace_loading: true,
                    marketplace_error: None,
                    marketplace_name: marketplace_name.clone(),
                });
                state.start_marketplace_fetch();
                return Ok(());
            }

            state.plugins_modal_state = Some(PluginsModalState::Tabs {
                active_tab,
                installed_index,
                marketplace_index,
                marketplace_plugins,
                marketplace_loading,
                marketplace_error,
                marketplace_name: marketplace_name.clone(),
            });
        }
        KeyCode::Up | KeyCode::Char('k') => {
            match active_tab {
                PluginsTab::Installed => {
                    installed_index = installed_index.saturating_sub(1);
                }
                PluginsTab::Marketplace => {
                    marketplace_index = marketplace_index.saturating_sub(1);
                }
            }
            state.plugins_modal_state = Some(PluginsModalState::Tabs {
                active_tab,
                installed_index,
                marketplace_index,
                marketplace_plugins,
                marketplace_loading,
                marketplace_error,
                marketplace_name: marketplace_name.clone(),
            });
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match active_tab {
                PluginsTab::Installed => {
                    let max = state.plugin_loader.loaded_plugins().count().saturating_sub(1);
                    if installed_index < max {
                        installed_index += 1;
                    }
                }
                PluginsTab::Marketplace => {
                    let max = marketplace_plugins
                        .as_ref()
                        .map(|p| p.len().saturating_sub(1))
                        .unwrap_or(0);
                    if marketplace_index < max {
                        marketplace_index += 1;
                    }
                }
            }
            state.plugins_modal_state = Some(PluginsModalState::Tabs {
                active_tab,
                installed_index,
                marketplace_index,
                marketplace_plugins,
                marketplace_loading,
                marketplace_error,
                marketplace_name: marketplace_name.clone(),
            });
        }
        KeyCode::Enter => {
            match active_tab {
                PluginsTab::Installed => {
                    // Get selected plugin from loader (sorted by name for stable ordering)
                    let mut plugins: Vec<_> = state.plugin_loader.loaded_plugins().collect();
                    plugins.sort_by(|a, b| a.name.cmp(&b.name));

                    if let Some(plugin) = plugins.get(installed_index) {
                        if !plugin.session_disabled {
                            // Check if plugin has a Select field in its config schema
                            let schema = plugin.plugin.config_schema();
                            let first_select = schema
                                .fields
                                .iter()
                                .find(|f| f.field_type == FfiConfigType::Select);

                            if let Some(select_field) = first_select {
                                // Show SelectInput modal with parsed options
                                let options = parse_select_options(&select_field.options);
                                state.plugins_modal_state = Some(PluginsModalState::SelectInput {
                                    plugin_name: plugin.name.clone(),
                                    field_name: select_field.name.to_string(),
                                    options,
                                    selected_index: 0,
                                });
                            } else {
                                // No Select field, use regular text Input
                                state.plugins_modal_state = Some(PluginsModalState::Input {
                                    plugin_name: plugin.name.clone(),
                                    input_buffer: String::new(),
                                    cursor_pos: 0,
                                });
                            }
                        } else {
                            state.plugins_modal_state = Some(PluginsModalState::Error {
                                message: format!(
                                    "Plugin '{}' is disabled for this session",
                                    plugin.name
                                ),
                            });
                        }
                    } else {
                        // No plugins available
                        state.plugins_modal_state = Some(PluginsModalState::Tabs {
                            active_tab,
                            installed_index,
                            marketplace_index,
                            marketplace_plugins,
                            marketplace_loading,
                            marketplace_error,
                            marketplace_name: marketplace_name.clone(),
                        });
                    }
                }
                PluginsTab::Marketplace => {
                    // Open details view for selected marketplace plugin
                    if let Some(plugins) = marketplace_plugins {
                        if let Some(plugin) = plugins.get(marketplace_index).cloned() {
                            state.plugins_modal_state = Some(PluginsModalState::Details {
                                plugin,
                                marketplace_plugins: plugins,
                                marketplace_index,
                            });
                        } else {
                            state.plugins_modal_state = Some(PluginsModalState::Tabs {
                                active_tab,
                                installed_index,
                                marketplace_index,
                                marketplace_plugins: Some(plugins),
                                marketplace_loading,
                                marketplace_error,
                                marketplace_name: marketplace_name.clone(),
                            });
                        }
                    } else {
                        state.plugins_modal_state = Some(PluginsModalState::Tabs {
                            active_tab,
                            installed_index,
                            marketplace_index,
                            marketplace_plugins: None,
                            marketplace_loading,
                            marketplace_error,
                            marketplace_name: marketplace_name.clone(),
                        });
                    }
                }
            }
        }
        _ => {
            state.plugins_modal_state = Some(PluginsModalState::Tabs {
                active_tab,
                installed_index,
                marketplace_index,
                marketplace_plugins,
                marketplace_loading,
                marketplace_error,
                marketplace_name,
            });
        }
    }
    Ok(())
}

/// Handle input in the plugins modal
fn handle_plugins_modal_input(
    key: KeyEvent,
    state: &mut AppState,
    plugin_name: String,
    mut input_buffer: String,
    mut cursor_pos: usize,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            // Go back to Tabs view with Installed tab
            use crate::config::Config;
            use crate::plugin::marketplace::DEFAULT_MARKETPLACE;
            let marketplace_name = Config::load()
                .map(|c| c.marketplaces.default)
                .unwrap_or_else(|_| DEFAULT_MARKETPLACE.to_string());
            state.plugins_modal_state = Some(PluginsModalState::Tabs {
                active_tab: PluginsTab::Installed,
                installed_index: 0,
                marketplace_index: 0,
                marketplace_plugins: None,
                marketplace_loading: false,
                marketplace_error: None,
                marketplace_name,
            });
        }
        KeyCode::Enter if !input_buffer.trim().is_empty() => {
            // Execute plugin
            state.plugins_modal_state = Some(PluginsModalState::Executing {
                plugin_name: plugin_name.clone(),
            });

            // Call plugin generate synchronously
            let result = state
                .plugin_loader
                .call_generate(&plugin_name, &input_buffer)
                .map_err(|e| e.message);

            match result {
                Ok(items) => {
                    if items.is_empty() {
                        state.plugins_modal_state = Some(PluginsModalState::Error {
                            message: "Plugin generated no items".to_string(),
                        });
                    } else {
                        state.plugins_modal_state = Some(PluginsModalState::Preview { items });
                    }
                }
                Err(message) => {
                    state.plugins_modal_state = Some(PluginsModalState::Error { message });
                }
            }
        }
        KeyCode::Backspace if cursor_pos > 0 => {
            let prev = prev_char_boundary(&input_buffer, cursor_pos);
            input_buffer.drain(prev..cursor_pos);
            cursor_pos = prev;
            state.plugins_modal_state = Some(PluginsModalState::Input {
                plugin_name,
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::Left if cursor_pos > 0 => {
            cursor_pos = prev_char_boundary(&input_buffer, cursor_pos);
            state.plugins_modal_state = Some(PluginsModalState::Input {
                plugin_name,
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::Right if cursor_pos < input_buffer.len() => {
            cursor_pos = next_char_boundary(&input_buffer, cursor_pos);
            state.plugins_modal_state = Some(PluginsModalState::Input {
                plugin_name,
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::Home => {
            cursor_pos = 0;
            state.plugins_modal_state = Some(PluginsModalState::Input {
                plugin_name,
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::End => {
            cursor_pos = input_buffer.len();
            state.plugins_modal_state = Some(PluginsModalState::Input {
                plugin_name,
                input_buffer,
                cursor_pos,
            });
        }
        KeyCode::Char(c) => {
            input_buffer.insert(cursor_pos, c);
            cursor_pos += c.len_utf8();
            state.plugins_modal_state = Some(PluginsModalState::Input {
                plugin_name,
                input_buffer,
                cursor_pos,
            });
        }
        _ => {
            state.plugins_modal_state = Some(PluginsModalState::Input {
                plugin_name,
                input_buffer,
                cursor_pos,
            });
        }
    }
    Ok(())
}

/// Parse Select field options from "display|value" format.
/// If no pipe separator, uses the same value for both display and value.
fn parse_select_options(options: &abi_stable::std_types::RVec<abi_stable::std_types::RString>) -> Vec<(String, String)> {
    options
        .iter()
        .map(|opt| {
            let s = opt.as_str();
            if let Some(idx) = s.find('|') {
                (s[..idx].to_string(), s[idx + 1..].to_string())
            } else {
                (s.to_string(), s.to_string())
            }
        })
        .collect()
}

/// Handle select input in the plugins modal (dropdown for Select type config fields)
fn handle_plugins_modal_select_input(
    key: KeyEvent,
    state: &mut AppState,
    plugin_name: String,
    field_name: String,
    options: Vec<(String, String)>,
    selected_index: usize,
) -> Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            let new_index = selected_index.saturating_sub(1);
            state.plugins_modal_state = Some(PluginsModalState::SelectInput {
                plugin_name,
                field_name,
                options,
                selected_index: new_index,
            });
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let new_index = (selected_index + 1).min(options.len().saturating_sub(1));
            state.plugins_modal_state = Some(PluginsModalState::SelectInput {
                plugin_name,
                field_name,
                options,
                selected_index: new_index,
            });
        }
        KeyCode::Enter => {
            if let Some((_, value)) = options.get(selected_index) {
                // Create config with selected value
                let mut config = std::collections::HashMap::new();
                config.insert(
                    abi_stable::std_types::RString::from(field_name.as_str()),
                    FfiConfigValue::String(abi_stable::std_types::RString::from(value.as_str())),
                );

                // Call on_config_loaded with the selection
                if let Some(plugin) = state.plugin_loader.get(&plugin_name) {
                    // Convert HashMap to RHashMap
                    let r_config: abi_stable::std_types::RHashMap<
                        abi_stable::std_types::RString,
                        FfiConfigValue,
                    > = config.into_iter().collect();

                    tracing::info!(
                        plugin = %plugin_name,
                        field = %field_name,
                        value = %value,
                        "SelectInput: calling on_config_loaded"
                    );

                    // Call on_config_loaded to set the selected value
                    plugin.plugin.on_config_loaded(r_config);
                }

                // Fire OnLoad event to trigger plugin to process its sync events
                // This allows plugins like claude-tasks to immediately show their tasks
                tracing::info!(plugin = %plugin_name, "SelectInput: firing OnLoad event");
                state.fire_on_load_event();

                // Close modal and return to Navigate mode
                state.plugins_modal_state = None;
                state.mode = Mode::Navigate;
            }
        }
        KeyCode::Esc => {
            // Return to Tabs view with Installed tab
            use crate::plugin::marketplace::DEFAULT_MARKETPLACE;
            let marketplace_name = Config::load()
                .map(|c| c.marketplaces.default)
                .unwrap_or_else(|_| DEFAULT_MARKETPLACE.to_string());
            state.plugins_modal_state = Some(PluginsModalState::Tabs {
                active_tab: PluginsTab::Installed,
                installed_index: 0,
                marketplace_index: 0,
                marketplace_plugins: None,
                marketplace_loading: false,
                marketplace_error: None,
                marketplace_name,
            });
        }
        _ => {
            // Keep current state for unhandled keys
            state.plugins_modal_state = Some(PluginsModalState::SelectInput {
                plugin_name,
                field_name,
                options,
                selected_index,
            });
        }
    }
    Ok(())
}

/// Handle details view in plugins modal
fn handle_plugins_modal_details(
    key: KeyEvent,
    state: &mut AppState,
    plugin: PluginEntry,
    marketplace_plugins: Vec<PluginEntry>,
    marketplace_index: usize,
) -> Result<()> {
    use crate::config::Config;
    use crate::plugin::marketplace::DEFAULT_MARKETPLACE;
    let marketplace_name = Config::load()
        .map(|c| c.marketplaces.default)
        .unwrap_or_else(|_| DEFAULT_MARKETPLACE.to_string());

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
            // Go back to Marketplace tab
            state.plugins_modal_state = Some(PluginsModalState::Tabs {
                active_tab: PluginsTab::Marketplace,
                installed_index: 0,
                marketplace_index,
                marketplace_plugins: Some(marketplace_plugins),
                marketplace_loading: false,
                marketplace_error: None,
                marketplace_name,
            });
        }
        KeyCode::Char('i') | KeyCode::Enter => {
            // Install the plugin
            let plugin_name = plugin.name.clone();
            let plugin_version = plugin.version.clone();

            // Check if already installed
            let is_installed = state
                .plugin_loader
                .loaded_plugins()
                .any(|p| p.name.eq_ignore_ascii_case(&plugin_name));

            if is_installed {
                state.set_status_message(format!("{} is already installed", plugin_name));
                state.plugins_modal_state = Some(PluginsModalState::Details {
                    plugin,
                    marketplace_plugins,
                    marketplace_index,
                });
                return Ok(());
            }

            // Run plugin install synchronously
            use crate::plugin::installer::{PluginInstaller, PluginSource};
            use crate::plugin::marketplace::DEFAULT_MARKETPLACE;

            // Build source for marketplace install
            let parts: Vec<&str> = DEFAULT_MARKETPLACE.split('/').collect();
            if parts.len() != 2 {
                state.plugins_modal_state = Some(PluginsModalState::Error {
                    message: "Invalid marketplace configuration".to_string(),
                });
                return Ok(());
            }

            let source = PluginSource {
                owner: Some(parts[0].to_string()),
                repo: Some(parts[1].to_string()),
                plugin_name: plugin_name.clone(),
                version: Some(plugin_version),
                local_path: None,
            };

            // Install from remote
            match PluginInstaller::install_from_remote(&source, false) {
                Ok(result) => {
                    state.set_status_message(format!(
                        "Installed {} v{} - restart to load",
                        result.plugin_name, result.version
                    ));
                    // Go back to marketplace tab
                    state.plugins_modal_state = Some(PluginsModalState::Tabs {
                        active_tab: PluginsTab::Marketplace,
                        installed_index: 0,
                        marketplace_index,
                        marketplace_plugins: Some(marketplace_plugins),
                        marketplace_loading: false,
                        marketplace_error: None,
                        marketplace_name,
                    });
                }
                Err(e) => {
                    state.plugins_modal_state = Some(PluginsModalState::Error {
                        message: format!("Install failed: {}", e),
                    });
                }
            }
        }
        _ => {
            state.plugins_modal_state = Some(PluginsModalState::Details {
                plugin,
                marketplace_plugins,
                marketplace_index,
            });
        }
    }
    Ok(())
}

/// Handle preview in plugins modal
fn handle_plugins_modal_preview(
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
            state.set_status_message(format!("Added {} item(s) from plugin", count));
            state.close_plugins_modal();
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            state.close_plugins_modal();
        }
        _ => {
            state.plugins_modal_state = Some(PluginsModalState::Preview { items });
        }
    }
    Ok(())
}

/// Handle error in plugins modal
fn handle_plugins_modal_error(key: KeyEvent, state: &mut AppState, message: String) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
            state.close_plugins_modal();
        }
        _ => {
            state.plugins_modal_state = Some(PluginsModalState::Error { message });
        }
    }
    Ok(())
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
            // Build GeneratorInfo list from loaded plugins
            let plugins: Vec<GeneratorInfo> = state
                .plugin_loader
                .loaded_plugins()
                .map(|p| GeneratorInfo {
                    name: p.name.clone(),
                    description: p.description.clone(),
                    available: !p.session_disabled,
                    unavailable_reason: if p.session_disabled {
                        Some("Disabled after error".to_string())
                    } else {
                        None
                    },
                })
                .collect();
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

            // Call plugin generate synchronously via plugin loader
            // (External plugins are already loaded, no need to spawn thread)
            let result = state
                .plugin_loader
                .call_generate(&plugin_name, &input_buffer)
                .map_err(|e| e.message);

            match result {
                Ok(items) => {
                    state.plugin_state = Some(PluginSubState::Preview { items });
                }
                Err(message) => {
                    state.plugin_state = Some(PluginSubState::Error { message });
                }
            }
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

/// Execute a plugin action triggered by keybinding.
///
/// This function:
/// 1. Shows a status message while running
/// 2. Finds the loaded plugin
/// 3. Calls execute_with_host with the action name as input
/// 4. Processes returned commands
/// 5. Shows completion message or error popup
fn execute_plugin_action(action: PluginAction, state: &mut AppState) -> Result<()> {
    // Show status message while running
    state.set_status_message(format!("Running {}...", action.action_name));

    // Find the loaded plugin
    let loaded_plugin = state
        .plugin_loader
        .loaded_plugins()
        .find(|p| p.name == action.plugin_name);

    let loaded_plugin = match loaded_plugin {
        Some(p) => p,
        None => {
            state.pending_plugin_errors.push(PluginLoadError {
                plugin_name: action.plugin_name.clone(),
                error_kind: PluginErrorKind::Other("Plugin not loaded".to_string()),
                message: format!("Plugin '{}' is not loaded", action.plugin_name),
            });
            state.show_plugin_error_popup = true;
            return Ok(());
        }
    };

    // Build enabled projects set (for now, just current project)
    let mut enabled_projects = HashSet::new();
    enabled_projects.insert(state.current_project.name.clone());

    // Create HostApi implementation
    let host_api = PluginHostApiImpl::new(
        &state.todo_list,
        &state.current_project,
        enabled_projects,
        action.plugin_name.clone(),
    );

    // Convert to FFI-safe trait object
    let host_to: HostApi_TO<'_, RBox<()>> = HostApi_TO::from_value(host_api, TD_Opaque);

    // Execute plugin action (blocking)
    // The plugin's execute_with_host receives action name as input string
    let result = call_plugin_execute_with_host(
        &loaded_plugin.plugin,
        action.action_name.as_str().into(),
        host_to,
    );

    match result.into_result() {
        Ok(commands) => {
            if !commands.is_empty() {
                state.save_undo();
                let mut executor = CommandExecutor::new(action.plugin_name.clone());
                let commands_vec: Vec<_> = commands.into_iter().collect();
                if let Err(e) = executor.execute_batch(commands_vec, &mut state.todo_list) {
                    state.set_status_message(format!("Error: {}", e));
                } else {
                    state.unsaved_changes = true;
                    state.set_status_message(format!("{} complete", action.action_name));
                }
            } else {
                state.set_status_message(format!("{} complete", action.action_name));
            }
        }
        Err(e) => {
            // Show error in popup
            state.pending_plugin_errors.push(PluginLoadError {
                plugin_name: action.plugin_name.clone(),
                error_kind: PluginErrorKind::Other(e.to_string()),
                message: e.to_string(),
            });
            state.show_plugin_error_popup = true;
        }
    }

    Ok(())
}
