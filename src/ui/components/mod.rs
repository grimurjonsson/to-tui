pub mod status_bar;
pub mod todo_list;

use crate::app::mode::Mode;
use crate::app::state::PluginSubState;
use crate::app::AppState;
use chrono::{Local, NaiveDate};

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub fn render(f: &mut Frame, state: &AppState) {
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
}

fn render_help_overlay(f: &mut Frame, state: &AppState) {
    let help_text = r#"
    TODO-CLI Help

    Navigate Mode:
      ↑/↓ or j/k            Move cursor
      x                     Toggle checked/unchecked
      Space                 Cycle state ([ ] → [x] → [?] → [!])
      Alt/Option+Shift+↑/↓  Move item with children
      Alt/Option+Shift+←/→  Indent/outdent with children
      Tab / Shift+Tab       Indent/outdent single item
      i                     Enter Insert mode
      Enter / n             New item
      dd                    Delete item
      c                     Collapse/expand children
      p                     Open plugins menu
      u                     Undo
      ?                     Toggle help
      Esc                   Close help
      q                     Quit (or close help)

    Day Navigation:
      <                     Previous day (archived, readonly)
      >                     Next day
      T                     Go to today

    Insert Mode:
      Esc                   Save and exit to Navigate
      Enter                 Save and create new item
      Tab / Shift+Tab       Indent/outdent item
      ←/→                   Move cursor
      Home/End              Jump to start/end
      Backspace             Delete character
    "#;

    // Center the help popup
    let area = centered_rect(60, 60, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .style(Style::default().bg(state.theme.background));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .style(Style::default().fg(state.theme.foreground))
        .wrap(Wrap { trim: true });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
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
