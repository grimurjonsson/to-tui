pub mod status_bar;
pub mod todo_list;

use crate::app::AppState;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
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

    // Render help overlay if active
    if state.show_help {
        render_help_overlay(f, state);
    }

    // Render delete confirmation overlay
    if state.confirm_delete {
        render_delete_confirm(f, state);
    }
}

fn render_delete_confirm(f: &mut Frame, state: &AppState) {
    let item_text = state
        .selected_item()
        .map(|item| item.content.as_str())
        .unwrap_or("");
    
    let truncated = if item_text.len() > 40 {
        format!("{}...", &item_text[..37])
    } else {
        item_text.to_string()
    };

    let area = centered_rect(50, 20, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Delete Item? ")
        .style(Style::default().bg(state.theme.background));

    let text = format!(
        "\n  \"{}\"\n\n  Press Enter or Y to delete\n  Press N or Esc to cancel",
        truncated
    );

    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(state.theme.foreground));

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
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
      i or Enter            Edit current item
      Shift+Enter           New item at same level
      n                     New item below
      d                     Delete item (confirms)
      u                     Undo
      ?                     Toggle help
      Esc                   Close help
      q                     Quit (or close help)

    Edit Mode:
      Esc                   Cancel edit
      Enter                 Save and exit to Navigate
      Shift+Enter           Save and create new item
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
