use crate::app::AppState;
use crate::utils::unicode::{after_first_char, first_char_as_str};
use super::centered_rect;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Truncate a string to at most `max_chars` characters, safe for multi-byte UTF-8.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    if truncated.len() < s.len() {
        format!("{}...", s.chars().take(max_chars.saturating_sub(3)).collect::<String>())
    } else {
        truncated
    }
}

pub fn render_description_modal(f: &mut Frame, state: &mut AppState) {
    let area = f.area();

    // Calculate modal size: 60% width, 40% height, minimum 40x10
    let percent_x = 60u16;
    let percent_y = 40u16;
    let modal_area = centered_rect(percent_x, percent_y, area);

    // Enforce minimum size
    let modal_area = if modal_area.width < 40 || modal_area.height < 10 {
        let w = modal_area.width.max(40).min(area.width);
        let h = modal_area.height.max(10).min(area.height);
        let x = (area.width.saturating_sub(w)) / 2;
        let y = (area.height.saturating_sub(h)) / 2;
        Rect::new(x, y, w, h)
    } else {
        modal_area
    };

    // Clear background
    f.render_widget(Clear, modal_area);

    // Build title from selected item content (UTF-8 safe truncation)
    let title = if let Some(item) = state.selected_item() {
        let max_title_chars = (modal_area.width as usize).saturating_sub(18);
        let content = truncate_chars(&item.content, max_title_chars);
        format!(" Description: {} ", content)
    } else {
        " Description ".to_string()
    };

    let block = Block::default()
        .title(title)
        .title_bottom(Line::from(" Esc: save | Ctrl+C: cancel "))
        .borders(Borders::ALL)
        .style(Style::default().bg(state.theme.background));

    let inner_area = block.inner(modal_area);
    f.render_widget(block, modal_area);

    // Calculate visible lines based on inner area height
    let visible_lines = inner_area.height as usize;

    // Adjust scroll offset to keep cursor visible
    if state.desc_cursor_row < state.desc_scroll_offset {
        state.desc_scroll_offset = state.desc_cursor_row;
    } else if state.desc_cursor_row >= state.desc_scroll_offset + visible_lines {
        state.desc_scroll_offset = state.desc_cursor_row - visible_lines + 1;
    }

    // Build lines to render with block cursor (matching todo_list.rs pattern)
    let cursor_style = Style::default()
        .bg(Color::Yellow)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let block_cursor_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(state.theme.foreground);
    let active_line_style = Style::default().fg(state.theme.foreground);

    let lines: Vec<Line> = state
        .desc_buffer
        .iter()
        .enumerate()
        .skip(state.desc_scroll_offset)
        .take(visible_lines)
        .map(|(row_idx, line_text)| {
            if row_idx == state.desc_cursor_row {
                // Render cursor line with block cursor
                let before_cursor = &line_text[..state.desc_cursor_col.min(line_text.len())];
                let after_cursor = &line_text[state.desc_cursor_col.min(line_text.len())..];

                let mut spans: Vec<Span<'static>> = vec![
                    Span::styled(before_cursor.to_string(), active_line_style),
                ];

                if after_cursor.is_empty() {
                    // Cursor at end of line: show block character
                    spans.push(Span::styled("\u{2588}", block_cursor_style));
                } else {
                    // Cursor on a character: highlight it
                    spans.push(Span::styled(
                        first_char_as_str(after_cursor).to_string(),
                        cursor_style,
                    ));
                    spans.push(Span::styled(
                        after_first_char(after_cursor).to_string(),
                        active_line_style,
                    ));
                }

                Line::from(spans)
            } else {
                Line::from(Span::styled(line_text.clone(), text_style))
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner_area);
}
