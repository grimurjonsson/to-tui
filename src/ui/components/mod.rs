pub mod plugin_modal;
pub mod status_bar;
pub mod todo_list;

use crate::app::mode::Mode;
use crate::app::state::{MoveToProjectSubState, PluginSubState, ProjectSubState};
use crate::app::AppState;
use crate::project::DEFAULT_PROJECT_NAME;
use crate::utils::upgrade::{format_bytes, UpgradeSubState};
use chrono::{Local, NaiveDate};

use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

pub fn render(f: &mut Frame, state: &mut AppState) {
    // Update terminal dimensions for click and scroll calculations
    state.terminal_width = f.area().width;
    state.terminal_height = f.area().height;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // Todo list
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    // Render todo list
    todo_list::render(f, state, chunks[0]);

    // Render status bar
    status_bar::render(f, state, chunks[1]);

    if state.show_help {
        render_help_overlay(f, state);
    }

    // Render new plugins modal if active, otherwise fall back to old plugin overlay
    if state.plugins_modal_state.is_some() {
        plugin_modal::render_plugins_modal(f, state);
    } else if let Some(ref plugin_state) = state.plugin_state {
        render_plugin_overlay(f, state, plugin_state);
    }

    if state.mode == Mode::Rollover {
        render_rollover_overlay(f, state);
    }

    // Render plugin error popup overlay
    if state.show_plugin_error_popup {
        render_plugin_error_popup(f, state);
    }

    if state.mode == Mode::UpgradePrompt {
        render_upgrade_overlay(f, state);
    }

    if state.mode == Mode::ProjectSelect
        && let Some(ref project_state) = state.project_state
    {
        render_project_overlay(f, state, project_state);
    }

    if state.mode == Mode::MoveToProject {
        render_move_to_project_modal(f, state);
    }
}

#[allow(clippy::vec_init_then_push)]
fn render_help_overlay(f: &mut Frame, state: &AppState) {
    let key_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(state.theme.foreground);
    let section_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(Color::DarkGray);

    let mut lines: Vec<Line> = vec![];

    // Title
    lines.push(Line::from(vec![
        Span::styled("  TO-TUI Help", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(""));

    // Navigation section
    lines.push(Line::from(Span::styled("  ── Navigation ──", section_style)));
    lines.push(Line::from(vec![
        Span::styled("    j / ↓           ", key_style),
        Span::styled("Move cursor down", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    k / ↑           ", key_style),
        Span::styled("Move cursor up", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    h / ←           ", key_style),
        Span::styled("Collapse item or go to parent", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    l / →           ", key_style),
        Span::styled("Expand collapsed item", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    c               ", key_style),
        Span::styled("Toggle collapse/expand", desc_style),
    ]));
    lines.push(Line::from(""));

    // Item State section
    lines.push(Line::from(Span::styled("  ── Item State ──", section_style)));
    lines.push(Line::from(vec![
        Span::styled("    x               ", key_style),
        Span::styled("Toggle done/undone", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Space           ", key_style),
        Span::styled("Cycle: [ ]→[x]→[*]→[?]→[!]→[-]", desc_style),
    ]));
    lines.push(Line::from(""));

    // Item Management section
    lines.push(Line::from(Span::styled("  ── Item Management ──", section_style)));
    lines.push(Line::from(vec![
        Span::styled("    n / o           ", key_style),
        Span::styled("New item below", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    O / Shift+Enter ", key_style),
        Span::styled("New item above", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Enter           ", key_style),
        Span::styled("New item at same indent level", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    i               ", key_style),
        Span::styled("Edit current item", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    dd              ", key_style),
        Span::styled("Delete item (with children)", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    y               ", key_style),
        Span::styled("Yank (copy) item to clipboard", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    u               ", key_style),
        Span::styled("Undo last action", desc_style),
    ]));
    lines.push(Line::from(""));

    // Indentation section
    lines.push(Line::from(Span::styled("  ── Indentation ──", section_style)));
    lines.push(Line::from(vec![
        Span::styled("    Tab             ", key_style),
        Span::styled("Indent item", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Shift+Tab       ", key_style),
        Span::styled("Outdent item", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Alt+Shift+→     ", key_style),
        Span::styled("Indent with children", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Alt+Shift+←     ", key_style),
        Span::styled("Outdent with children", desc_style),
    ]));
    lines.push(Line::from(""));

    // Move Items section
    lines.push(Line::from(Span::styled("  ── Move Items ──", section_style)));
    lines.push(Line::from(vec![
        Span::styled("    Alt+Shift+↑     ", key_style),
        Span::styled("Move item up (with children)", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Alt+Shift+↓     ", key_style),
        Span::styled("Move item down (with children)", desc_style),
    ]));
    lines.push(Line::from(""));

    // Priority section
    lines.push(Line::from(Span::styled("  ── Priority ──", section_style)));
    lines.push(Line::from(vec![
        Span::styled("    p               ", key_style),
        Span::styled("Cycle priority: none→low→medium→high", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    s               ", key_style),
        Span::styled("Sort items by priority", desc_style),
    ]));
    lines.push(Line::from(""));

    // Visual Mode section
    lines.push(Line::from(Span::styled("  ── Visual Mode ──", section_style)));
    lines.push(Line::from(vec![
        Span::styled("    v               ", key_style),
        Span::styled("Enter visual mode (select multiple)", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Esc / v / q     ", key_style),
        Span::styled("Exit visual mode", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled("In visual: ", dim_style),
        Span::styled("j/k", key_style),
        Span::styled(" extend selection, ", dim_style),
        Span::styled("Tab/S-Tab", key_style),
        Span::styled(" indent/outdent", dim_style),
    ]));
    lines.push(Line::from(""));

    // Day Navigation section
    lines.push(Line::from(Span::styled("  ── Day Navigation ──", section_style)));
    lines.push(Line::from(vec![
        Span::styled("    <               ", key_style),
        Span::styled("Previous day (archived, readonly)", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    >               ", key_style),
        Span::styled("Next day", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    T               ", key_style),
        Span::styled("Go to today", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    R               ", key_style),
        Span::styled("Open rollover modal", desc_style),
    ]));
    lines.push(Line::from(""));

    // Other section
    lines.push(Line::from(Span::styled("  ── Other ──", section_style)));
    lines.push(Line::from(vec![
        Span::styled("    Ctrl+p          ", key_style),
        Span::styled("Open project switcher", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    P               ", key_style),
        Span::styled("Open plugins menu", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    ?               ", key_style),
        Span::styled("Toggle this help", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    q               ", key_style),
        Span::styled("Quit", desc_style),
    ]));
    lines.push(Line::from(""));

    // Edit Mode section
    lines.push(Line::from(Span::styled("  ── Edit Mode ──", section_style)));
    lines.push(Line::from(vec![
        Span::styled("    Esc             ", key_style),
        Span::styled("Save and exit edit mode", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Enter           ", key_style),
        Span::styled("Save and create new item below", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    ← / →           ", key_style),
        Span::styled("Move cursor", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Alt+b / Alt+←   ", key_style),
        Span::styled("Move word left", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Alt+f / Alt+→   ", key_style),
        Span::styled("Move word right", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Home / Ctrl+a   ", key_style),
        Span::styled("Go to start of line", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    End / Ctrl+e    ", key_style),
        Span::styled("Go to end of line", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Tab / Shift+Tab ", key_style),
        Span::styled("Indent/outdent while editing", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Backspace       ", key_style),
        Span::styled("Delete character", desc_style),
    ]));

    // Plugin Actions section (only if any enabled plugins have actions)
    let actions_by_plugin = state.plugin_action_registry.actions_by_plugin();
    if !actions_by_plugin.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ── Plugin Actions ──",
            section_style,
        )));

        // Sort plugin names for consistent display
        let mut plugin_names: Vec<_> = actions_by_plugin.keys().collect();
        plugin_names.sort();

        for plugin_name in plugin_names {
            let actions = &actions_by_plugin[plugin_name];

            lines.push(Line::from(vec![Span::styled(
                format!("  [{}]", plugin_name),
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )]));

            for action in actions {
                // Format the KeyBinding directly using its Display impl
                let key_text = action
                    .keybinding
                    .as_ref()
                    .map(|kb| format!("{:<16}", kb))
                    .unwrap_or_else(|| "(no binding)    ".to_string());

                lines.push(Line::from(vec![
                    Span::styled(format!("    {}  ", key_text), key_style),
                    Span::styled(&action.description, desc_style),
                ]));
            }
        }
    }

    lines.push(Line::from(""));

    // Footer hint
    lines.push(Line::from(vec![
        Span::styled("  ↑/↓ or j/k to scroll • Esc or ? to close", dim_style),
    ]));

    let total_lines = lines.len() as u16;

    // Center the help popup
    let area = centered_rect(65, 80, f.area());
    let inner_height = area.height.saturating_sub(2); // Account for borders

    // Clamp scroll to valid range
    let max_scroll = total_lines.saturating_sub(inner_height);
    let scroll_offset = state.help_scroll.min(max_scroll) as usize;

    // Create list items from visible lines only
    let visible_items: Vec<ListItem> = lines
        .into_iter()
        .skip(scroll_offset)
        .take(inner_height as usize)
        .map(ListItem::new)
        .collect();

    let list_widget = List::new(visible_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .title_bottom(Line::from(" ↑↓ scroll ").centered())
                .style(Style::default().bg(state.theme.background)),
        );

    f.render_widget(Clear, area);
    f.render_widget(list_widget, area);

    // Render scrollbar if content exceeds viewport
    if total_lines > inner_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        // For scrollbar: content_length is the scrollable range (max_scroll + 1),
        // and position is where we are in that range
        let max_scroll = total_lines.saturating_sub(inner_height) as usize;
        let mut scrollbar_state = ScrollbarState::new(max_scroll + 1)
            .position(scroll_offset);

        f.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_plugin_overlay(f: &mut Frame, state: &AppState, plugin_state: &PluginSubState) {
    match plugin_state {
        PluginSubState::Selecting {
            plugins,
            selected_index,
        } => render_plugin_selecting(f, state, plugins, *selected_index),
        PluginSubState::InputPrompt {
            plugin_name,
            input_buffer,
            cursor_pos,
        } => render_plugin_input(f, state, plugin_name, input_buffer, *cursor_pos),
        PluginSubState::Executing { plugin_name } => render_plugin_executing(f, state, plugin_name),
        PluginSubState::Error { message } => render_plugin_error(f, state, message),
        PluginSubState::Preview { items } => render_plugin_preview(f, state, items),
    }
}

fn render_plugin_selecting(
    f: &mut Frame,
    state: &AppState,
    plugins: &[crate::plugin::GeneratorInfo],
    selected_index: usize,
) {
    let area = centered_rect(50, 40, f.area());

    let items: Vec<ListItem> = plugins
        .iter()
        .enumerate()
        .map(|(i, plugin)| {
            let status = if plugin.available {
                Span::styled("[OK]", Style::default().fg(ratatui::style::Color::Green))
            } else {
                Span::styled("[N/A]", Style::default().fg(ratatui::style::Color::Red))
            };

            let name_style = if i == selected_index {
                Style::default()
                    .fg(ratatui::style::Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else if plugin.available {
                Style::default().fg(state.theme.foreground)
            } else {
                Style::default().fg(ratatui::style::Color::DarkGray)
            };

            let line = Line::from(vec![
                Span::styled(format!(" {} ", plugin.name), name_style),
                status,
                Span::raw(" "),
                Span::styled(
                    &plugin.description,
                    Style::default().fg(ratatui::style::Color::Gray),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Plugins (Enter to select, Esc to cancel) ")
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(list, area);
}

fn render_plugin_input(
    f: &mut Frame,
    state: &AppState,
    plugin_name: &str,
    input_buffer: &str,
    cursor_pos: usize,
) {
    let area = centered_rect(60, 20, f.area());

    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {plugin_name} - Enter input (Esc to go back) "))
        .style(Style::default().bg(state.theme.background));

    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let before_cursor = &input_buffer[..cursor_pos];
    let after_cursor = &input_buffer[cursor_pos..];

    let cursor_char = if after_cursor.is_empty() {
        "█"
    } else {
        &after_cursor[..after_cursor
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0)]
    };

    let after_cursor_rest = if after_cursor.is_empty() {
        ""
    } else {
        &after_cursor[after_cursor
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0)..]
    };

    let input_line = Line::from(vec![
        Span::raw(before_cursor),
        Span::styled(
            cursor_char,
            Style::default()
                .bg(ratatui::style::Color::Yellow)
                .fg(ratatui::style::Color::Black),
        ),
        Span::raw(after_cursor_rest),
    ]);

    let input_paragraph = Paragraph::new(input_line);
    f.render_widget(input_paragraph, inner_area);
}

fn render_plugin_executing(f: &mut Frame, state: &AppState, plugin_name: &str) {
    let area = centered_rect(40, 15, f.area());

    let text = format!("Running {plugin_name}...\n\nPlease wait.");

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Executing Plugin ")
        .style(Style::default().bg(state.theme.background));

    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(state.theme.foreground))
        .wrap(Wrap { trim: true });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn render_plugin_error(f: &mut Frame, state: &AppState, message: &str) {
    let area = centered_rect(60, 30, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Error (Press Esc to dismiss) ")
        .style(
            Style::default()
                .bg(state.theme.background)
                .fg(ratatui::style::Color::Red),
        );

    let paragraph = Paragraph::new(message)
        .block(block)
        .style(Style::default().fg(ratatui::style::Color::Red))
        .wrap(Wrap { trim: true });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn render_plugin_preview(f: &mut Frame, state: &AppState, items: &[crate::todo::TodoItem]) {
    let area = centered_rect(70, 60, f.area());

    let list_items: Vec<ListItem> = items
        .iter()
        .map(|item| {
            let indent = "  ".repeat(item.indent_level);
            let line = format!("{}[ ] {}", indent, item.content);
            ListItem::new(Line::from(Span::styled(
                line,
                Style::default().fg(state.theme.foreground),
            )))
        })
        .collect();

    let title = format!(" Generated {} item(s) - Add to list? (Y/n) ", items.len());

    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(list, area);
}

fn format_date_description(source_date: NaiveDate) -> String {
    let today = Local::now().date_naive();
    let days_ago = (today - source_date).num_days();

    if days_ago == 1 {
        "yesterday".to_string()
    } else {
        format!(
            "{} ({} days ago)",
            source_date.format("%B %d, %Y"),
            days_ago
        )
    }
}

fn render_rollover_overlay(f: &mut Frame, state: &AppState) {
    let Some(ref pending) = state.pending_rollover else {
        return;
    };

    let area = centered_rect(60, 50, f.area());

    let date_desc = format_date_description(pending.source_date);
    let title = format!(
        " Rollover {} incomplete item(s) from {} ",
        pending.items.len(),
        date_desc
    );

    // Build list items from pending rollover
    let list_items: Vec<ListItem> = pending
        .items
        .iter()
        .map(|item| {
            let indent = "  ".repeat(item.indent_level);
            let state_char = item.state.to_char();
            let line = format!("{}[{}] {}", indent, state_char, item.content);
            ListItem::new(Line::from(Span::styled(
                line,
                Style::default().fg(state.theme.foreground),
            )))
        })
        .collect();

    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(list, area);

    // Render footer with options
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("[Y]", Style::default().fg(ratatui::style::Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("es - Rollover now    "),
        Span::styled("[L]", Style::default().fg(ratatui::style::Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("ater - Dismiss (press R anytime to reopen)"),
    ]));

    f.render_widget(footer, footer_area);
}

fn render_upgrade_overlay(f: &mut Frame, state: &AppState) {
    if state.new_version_available.is_none() {
        return;
    }

    let sub_state = state.upgrade_sub_state.as_ref();

    match sub_state {
        Some(UpgradeSubState::Downloading { progress, bytes_downloaded, total_bytes }) => {
            render_upgrade_downloading(f, state, *progress, *bytes_downloaded, *total_bytes);
        }
        Some(UpgradeSubState::Error { message }) => {
            render_upgrade_error(f, state, message);
        }
        Some(UpgradeSubState::RestartPrompt { downloaded_path: _ }) => {
            render_upgrade_restart_prompt(f, state);
        }
        Some(UpgradeSubState::Prompt) | None => {
            render_upgrade_prompt(f, state);
        }
    }
}

fn render_upgrade_prompt(f: &mut Frame, state: &AppState) {
    let area = centered_rect(60, 40, f.area());

    let current_version = env!("CARGO_PKG_VERSION");
    let new_version = state.new_version_available.as_ref().unwrap();

    let title = " New Version Available ";

    // Build content lines
    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("  Current: "),
        Span::styled(
            format!("v{}", current_version),
            Style::default().fg(Color::Yellow),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  New:     "),
        Span::styled(
            format!("v{}", new_version),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "  Release page:",
            Style::default().fg(state.theme.foreground),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  https://github.com/grimurjonsson/to-tui/releases",
            Style::default().fg(Color::Cyan),
        ),
    ]));
    lines.push(Line::from(""));

    let content = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(content, area);

    // Render footer with options
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Y]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("es - Download & install  "),
        Span::styled(
            "[N]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("o - Later  "),
        Span::styled(
            "[S]",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("kip - Don't remind"),
    ]));

    f.render_widget(footer, footer_area);
}

fn render_upgrade_downloading(f: &mut Frame, state: &AppState, progress: f64, bytes_downloaded: u64, total_bytes: Option<u64>) {
    let area = centered_rect(50, 25, f.area());

    let current_version = env!("CARGO_PKG_VERSION");
    let new_version = state.new_version_available.as_ref().unwrap();

    // Build content lines
    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("  Upgrading: "),
        Span::styled(
            format!("v{}", current_version),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(" -> "),
        Span::styled(
            format!("v{}", new_version),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    let content = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Downloading Update ")
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(content, area);

    // Render progress bar
    let gauge_area = Rect {
        x: area.x + 2,
        y: area.y + 4,
        width: area.width - 4,
        height: 1,
    };

    let progress_label = format!(
        "{} / {}",
        format_bytes(bytes_downloaded),
        total_bytes.map(format_bytes).unwrap_or_else(|| "???".to_string())
    );

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent((progress * 100.0) as u16)
        .label(progress_label);

    f.render_widget(gauge, gauge_area);

    // Render footer
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Esc]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Cancel"),
    ]));

    f.render_widget(footer, footer_area);
}

fn render_upgrade_error(f: &mut Frame, state: &AppState, message: &str) {
    let area = centered_rect(60, 30, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Download Error ")
        .style(
            Style::default()
                .bg(state.theme.background)
                .fg(Color::Red),
        );

    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {}", message),
            Style::default().fg(Color::Red),
        ),
    ]));
    lines.push(Line::from(""));

    let content = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(Clear, area);
    f.render_widget(content, area);

    // Render footer
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[R]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("etry  "),
        Span::styled(
            "[Esc]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Dismiss"),
    ]));

    f.render_widget(footer, footer_area);
}

fn render_upgrade_restart_prompt(f: &mut Frame, state: &AppState) {
    let area = centered_rect(55, 35, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Update Ready ")
        .style(Style::default().bg(state.theme.background));

    let lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Download complete!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  The application will restart to complete the update.",
            Style::default().fg(state.theme.foreground),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Any unsaved changes will be lost.",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(""),
    ];

    let content = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(content, area);

    // Render footer
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Y]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("es - Restart now  "),
        Span::styled(
            "[N]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("o - Later"),
    ]));

    f.render_widget(footer, footer_area);
}

fn render_project_overlay(f: &mut Frame, state: &AppState, project_state: &ProjectSubState) {
    match project_state {
        ProjectSubState::Selecting {
            projects,
            selected_index,
        } => render_project_selecting(f, state, projects, *selected_index),
        ProjectSubState::CreateInput {
            input_buffer,
            cursor_pos,
        } => render_project_create_input(f, state, input_buffer, *cursor_pos),
        ProjectSubState::RenameInput {
            project_name,
            input_buffer,
            cursor_pos,
        } => render_project_rename_input(f, state, project_name, input_buffer, *cursor_pos),
        ProjectSubState::ConfirmDelete { project_name } => {
            render_project_confirm_delete(f, state, project_name)
        }
    }
}

fn render_project_selecting(
    f: &mut Frame,
    state: &AppState,
    projects: &[crate::project::Project],
    selected_index: usize,
) {
    let area = centered_rect(50, 50, f.area());

    let items: Vec<ListItem> = projects
        .iter()
        .enumerate()
        .map(|(i, project)| {
            let is_current = project.name == state.current_project.name;
            let is_default = project.name == DEFAULT_PROJECT_NAME;

            let marker = if is_current { "● " } else { "  " };

            let name_style = if i == selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else if is_current {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(state.theme.foreground)
            };

            let suffix = if is_default { " (default)" } else { "" };

            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Green)),
                Span::styled(format!("{}{}", project.name, suffix), name_style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Projects (Enter to switch, Esc to cancel) ")
                .style(Style::default().bg(state.theme.background)),
        )
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(list, area);

    // Render footer with options
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[n]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("ew  "),
        Span::styled(
            "[r]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("ename  "),
        Span::styled(
            "[d]",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("elete"),
    ]));

    f.render_widget(footer, footer_area);
}

fn render_project_create_input(
    f: &mut Frame,
    state: &AppState,
    input_buffer: &str,
    cursor_pos: usize,
) {
    let area = centered_rect(50, 20, f.area());

    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 2,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(4),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Create New Project (Esc to cancel) ")
        .style(Style::default().bg(state.theme.background));

    f.render_widget(Clear, area);
    f.render_widget(block, area);

    // Render prompt
    let prompt_area = Rect {
        x: inner_area.x,
        y: inner_area.y,
        width: inner_area.width,
        height: 1,
    };
    let prompt = Paragraph::new("Project name:")
        .style(Style::default().fg(state.theme.foreground));
    f.render_widget(prompt, prompt_area);

    // Render input with cursor
    let input_area = Rect {
        x: inner_area.x,
        y: inner_area.y + 1,
        width: inner_area.width,
        height: 1,
    };

    let before_cursor = &input_buffer[..cursor_pos];
    let after_cursor = &input_buffer[cursor_pos..];

    let cursor_char = if after_cursor.is_empty() {
        "█"
    } else {
        &after_cursor[..after_cursor
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0)]
    };

    let after_cursor_rest = if after_cursor.is_empty() {
        ""
    } else {
        &after_cursor[after_cursor
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0)..]
    };

    let input_line = Line::from(vec![
        Span::raw(before_cursor),
        Span::styled(
            cursor_char,
            Style::default().bg(Color::Yellow).fg(Color::Black),
        ),
        Span::raw(after_cursor_rest),
    ]);

    let input_paragraph = Paragraph::new(input_line);
    f.render_widget(input_paragraph, input_area);
}

fn render_project_rename_input(
    f: &mut Frame,
    state: &AppState,
    project_name: &str,
    input_buffer: &str,
    cursor_pos: usize,
) {
    let area = centered_rect(50, 25, f.area());

    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 2,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(4),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Rename '{}' (Esc to cancel) ", project_name))
        .style(Style::default().bg(state.theme.background));

    f.render_widget(Clear, area);
    f.render_widget(block, area);

    // Render prompt
    let prompt_area = Rect {
        x: inner_area.x,
        y: inner_area.y,
        width: inner_area.width,
        height: 1,
    };
    let prompt =
        Paragraph::new("New name:").style(Style::default().fg(state.theme.foreground));
    f.render_widget(prompt, prompt_area);

    // Render input with cursor
    let input_area = Rect {
        x: inner_area.x,
        y: inner_area.y + 1,
        width: inner_area.width,
        height: 1,
    };

    let before_cursor = &input_buffer[..cursor_pos];
    let after_cursor = &input_buffer[cursor_pos..];

    let cursor_char = if after_cursor.is_empty() {
        "█"
    } else {
        &after_cursor[..after_cursor
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0)]
    };

    let after_cursor_rest = if after_cursor.is_empty() {
        ""
    } else {
        &after_cursor[after_cursor
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0)..]
    };

    let input_line = Line::from(vec![
        Span::raw(before_cursor),
        Span::styled(
            cursor_char,
            Style::default().bg(Color::Yellow).fg(Color::Black),
        ),
        Span::raw(after_cursor_rest),
    ]);

    let input_paragraph = Paragraph::new(input_line);
    f.render_widget(input_paragraph, input_area);
}

fn render_project_confirm_delete(f: &mut Frame, state: &AppState, project_name: &str) {
    let area = centered_rect(50, 25, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm Delete ")
        .style(
            Style::default()
                .bg(state.theme.background)
                .fg(Color::Red),
        );

    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        format!("  Delete project '{}'?", project_name),
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "  This will delete all todos in this project.",
        Style::default().fg(state.theme.foreground),
    )]));
    lines.push(Line::from(vec![Span::styled(
        "  This action cannot be undone!",
        Style::default().fg(Color::Yellow),
    )]));
    lines.push(Line::from(""));

    let content = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(content, area);

    // Render footer with options
    let footer_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "[Y]",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("es - Delete permanently  "),
        Span::styled(
            "[N]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("o - Cancel"),
    ]));

    f.render_widget(footer, footer_area);
}

pub fn render_move_to_project_modal(frame: &mut Frame, state: &AppState) {
    let move_state = match &state.move_to_project_state {
        Some(s) => s,
        None => return,
    };

    let MoveToProjectSubState::Selecting {
        projects,
        selected_index,
        item_index,
    } = move_state;

    // Get the item being moved for display
    let item_name = state
        .todo_list
        .items
        .get(*item_index)
        .map(|i| i.content.as_str())
        .unwrap_or("(unknown)");

    // Build title with truncated item name
    let max_title_len = 40;
    let truncated_name = if item_name.len() > max_title_len {
        format!("{}...", &item_name[..max_title_len.saturating_sub(3)])
    } else {
        item_name.to_string()
    };
    let title = format!(" Move '{}' to (j/k to navigate, Enter to select) ", truncated_name);

    let area = centered_rect(60, 50, frame.area());

    // Clear background
    frame.render_widget(Clear, area);

    // Render project list
    let items: Vec<ListItem> = projects
        .iter()
        .enumerate()
        .map(|(i, project)| {
            let name_style = if i == *selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(state.theme.foreground)
            };
            ListItem::new(Line::from(Span::styled(&project.name, name_style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().bg(state.theme.background)),
        );

    frame.render_widget(list, area);
}

/// Render the plugin error popup overlay.
/// Shows loading errors with plugin names and messages, plus a hint to run `totui plugin status`.
pub fn render_plugin_error_popup(f: &mut Frame, state: &AppState) {
    if !state.show_plugin_error_popup || state.pending_plugin_errors.is_empty() {
        return;
    }

    let errors = &state.pending_plugin_errors;
    let area = f.area();

    // Center popup, 80% width to accommodate longer error messages, height based on error count
    let popup_width = (area.width * 80) / 100;
    let content_lines = errors.len() * 3 + 6; // ~3 lines per error (wrapped) + header/footer
    let popup_height = (content_lines as u16 + 4).min((area.height * 70) / 100);

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height - popup_height) / 2),
            Constraint::Length(popup_height),
            Constraint::Min(0),
        ])
        .split(area);

    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((area.width - popup_width) / 2),
            Constraint::Length(popup_width),
            Constraint::Min(0),
        ])
        .split(popup_layout[1])[1];

    // Clear background
    f.render_widget(Clear, popup_area);

    // Build error text
    let mut lines = vec![
        Line::from(Span::styled(
            format!("{} plugin(s) failed to load:", errors.len()),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for error in errors {
        lines.push(Line::from(vec![
            Span::styled("  - ", Style::default().fg(Color::Red)),
            Span::styled(
                &error.plugin_name,
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(&error.message, Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Run `totui plugin status` for details",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press any key to dismiss",
        Style::default().fg(Color::Yellow),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Plugin Loading Errors ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        )
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(state.theme.background));

    f.render_widget(paragraph, popup_area);
}

