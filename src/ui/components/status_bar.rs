use crate::app::AppState;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let mode_text = format!("{}", state.mode);
    let save_indicator = if state.unsaved_changes {
        " [unsaved]"
    } else {
        ""
    };
    let help_text = "Press ? for help, q to quit";
    let version_text = format!("v{VERSION}");

    let left_content = format!(
        " {} | {} items{}",
        mode_text,
        state.todo_list.items.len(),
        save_indicator
    );

    let padding = area
        .width
        .saturating_sub(left_content.len() as u16 + help_text.len() as u16 + version_text.len() as u16 + 3);

    let status_line = format!(
        "{} {} {:>padding$} {}",
        left_content,
        help_text,
        "",
        version_text,
        padding = padding as usize
    );

    let status = Paragraph::new(Line::from(vec![Span::styled(
        status_line,
        Style::default()
            .fg(state.theme.status_bar_fg)
            .bg(state.theme.status_bar_bg),
    )]));

    f.render_widget(status, area);
}
