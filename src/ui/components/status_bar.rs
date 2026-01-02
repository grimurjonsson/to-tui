use crate::app::AppState;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let mode_text = format!("{}", state.mode);
    let save_indicator = if state.unsaved_changes { " [unsaved]" } else { "" };
    let help_text = " Press ? for help, q to quit";

    let status_line = format!(
        " {} | {} items{} {}",
        mode_text,
        state.todo_list.items.len(),
        save_indicator,
        help_text
    );

    let status = Paragraph::new(Line::from(vec![Span::styled(
        status_line,
        Style::default()
            .fg(state.theme.status_bar_fg)
            .bg(state.theme.status_bar_bg),
    )]));

    f.render_widget(status, area);
}
