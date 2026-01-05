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

    if state.show_help {
        render_help_overlay(f, state);
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
