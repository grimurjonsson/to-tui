pub mod status_bar;
pub mod todo_list;

use crate::app::mode::Mode;
use crate::app::state::PluginSubState;
use crate::app::AppState;
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

    if let Some(ref plugin_state) = state.plugin_state {
        render_plugin_overlay(f, state, plugin_state);
    }

    if state.mode == Mode::Rollover {
        render_rollover_overlay(f, state);
    }

    if state.mode == Mode::UpgradePrompt {
        render_upgrade_overlay(f, state);
    }
}

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
        Span::styled("Cycle state: [ ]→[x]→[*]→[?]→[!]", desc_style),
    ]));
    lines.push(Line::from(""));

    // Item Management section
    lines.push(Line::from(Span::styled("  ── Item Management ──", section_style)));
    lines.push(Line::from(vec![
        Span::styled("    n / o           ", key_style),
        Span::styled("New item below", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    O               ", key_style),
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
        Span::styled("Tab/Shift+Tab", key_style),
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
        Span::styled("    p               ", key_style),
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
        Span::styled("    Alt+← / Alt+→   ", key_style),
        Span::styled("Move by word", desc_style),
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

    let mut lines: Vec<Line> = vec![];
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "  Download complete!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "  The application will restart to complete the update.",
            Style::default().fg(state.theme.foreground),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "  Any unsaved changes will be lost.",
            Style::default().fg(Color::Yellow),
        ),
    ]));
    lines.push(Line::from(""));

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
