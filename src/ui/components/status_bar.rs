use crate::app::mode::Mode;
use crate::app::AppState;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    if state.mode == Mode::ConfirmDelete {
        render_confirm_delete(f, state, area);
        return;
    }

    if let Some((message, time)) = &state.status_message
        && time.elapsed().as_secs() <= 3 {
            render_status_message(f, message, area);
            return;
        }

    let mode_text = format!("{}", state.mode);
    let readonly_indicator = if state.is_readonly() {
        " [READONLY]"
    } else {
        ""
    };
    let save_indicator = if state.unsaved_changes {
        " [unsaved]"
    } else {
        ""
    };

    let date_str = state.viewing_date.format("%Y-%m-%d").to_string();
    let date_label = if state.viewing_date == state.today {
        format!("{date_str} (today)")
    } else {
        format!("{date_str} (archived)")
    };

    let nav_hint = if state.is_readonly() {
        "< prev  > next  T today"
    } else {
        "? help  q quit"
    };
    let version_text = format!("v{VERSION}");

    let left_content = format!(
        " {} | {} | {} items{}{}",
        mode_text,
        date_label,
        state.todo_list.items.len(),
        readonly_indicator,
        save_indicator
    );

    let padding = area.width.saturating_sub(
        left_content.len() as u16 + nav_hint.len() as u16 + version_text.len() as u16 + 3,
    );

    let base_style = Style::default()
        .fg(state.theme.status_bar_fg)
        .bg(state.theme.status_bar_bg);

    let readonly_style = if state.is_readonly() {
        base_style.add_modifier(Modifier::BOLD)
    } else {
        base_style
    };

    let status_line = format!(
        "{} {} {:>padding$} {}",
        left_content,
        nav_hint,
        "",
        version_text,
        padding = padding as usize
    );

    let status = Paragraph::new(Line::from(vec![Span::styled(status_line, readonly_style)]));

    f.render_widget(status, area);
}

fn render_confirm_delete(f: &mut Frame, state: &AppState, area: Rect) {
    let subtask_count = state.pending_delete_subtask_count.unwrap_or(0);
    let prompt = format!(
        " Delete task and its {} subtask{}? (Y/n) ",
        subtask_count,
        if subtask_count == 1 { "" } else { "s" }
    );

    let style = Style::default()
        .fg(ratatui::style::Color::Black)
        .bg(ratatui::style::Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let padding = area.width.saturating_sub(prompt.len() as u16);
    let status_line = format!("{}{:padding$}", prompt, "", padding = padding as usize);

    let status = Paragraph::new(Line::from(vec![Span::styled(status_line, style)]));
    f.render_widget(status, area);
}

fn render_status_message(f: &mut Frame, message: &str, area: Rect) {
    let display_message = format!(" {message} ");

    let style = Style::default()
        .fg(ratatui::style::Color::Black)
        .bg(ratatui::style::Color::Green)
        .add_modifier(Modifier::BOLD);

    let padding = area.width.saturating_sub(display_message.len() as u16);
    let status_line = format!(
        "{}{:padding$}",
        display_message,
        "",
        padding = padding as usize
    );

    let status = Paragraph::new(Line::from(vec![Span::styled(status_line, style)]));
    f.render_widget(status, area);
}
