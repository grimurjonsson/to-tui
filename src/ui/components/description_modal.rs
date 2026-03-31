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
use unicode_width::UnicodeWidthChar;

/// Truncate a string to at most `max_chars` characters, safe for multi-byte UTF-8.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    if truncated.len() < s.len() {
        format!("{}...", s.chars().take(max_chars.saturating_sub(3)).collect::<String>())
    } else {
        truncated
    }
}

/// A visual line produced by wrapping a buffer line at the modal width.
struct VisualLine {
    buf_row: usize,
    byte_start: usize,
    byte_end: usize,
}

/// Wrap a single line into byte-range segments that each fit within `max_width` display columns.
fn wrap_line(line: &str, max_width: usize) -> Vec<(usize, usize)> {
    if max_width == 0 || line.is_empty() {
        return vec![(0, line.len())];
    }

    let mut ranges = Vec::new();
    let mut start = 0;
    let mut current_width = 0;

    for (idx, ch) in line.char_indices() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + ch_width > max_width && start != idx {
            ranges.push((start, idx));
            start = idx;
            current_width = ch_width;
        } else {
            current_width += ch_width;
        }
    }

    ranges.push((start, line.len()));
    ranges
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

    let max_width = inner_area.width as usize;
    let visible_lines = inner_area.height as usize;

    // Build visual lines by wrapping each buffer line at the modal width
    let mut visual_lines: Vec<VisualLine> = Vec::new();
    let mut cursor_visual_row = 0;

    for (buf_row, line_text) in state.desc_buffer.iter().enumerate() {
        let ranges = wrap_line(line_text, max_width);
        for &(byte_start, byte_end) in &ranges {
            if buf_row == state.desc_cursor_row {
                let cursor_col = state.desc_cursor_col.min(line_text.len());
                let is_last = byte_end == line_text.len();
                if cursor_col >= byte_start
                    && (cursor_col < byte_end || (is_last && cursor_col == byte_end))
                {
                    cursor_visual_row = visual_lines.len();
                }
            }
            visual_lines.push(VisualLine {
                buf_row,
                byte_start,
                byte_end,
            });
        }
    }

    // Adjust scroll offset to keep cursor visible (in visual-line coordinates)
    if cursor_visual_row < state.desc_scroll_offset {
        state.desc_scroll_offset = cursor_visual_row;
    } else if cursor_visual_row >= state.desc_scroll_offset + visible_lines {
        state.desc_scroll_offset = cursor_visual_row - visible_lines + 1;
    }

    // Build styled lines to render with block cursor
    let cursor_style = Style::default()
        .bg(Color::Yellow)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let block_cursor_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(state.theme.foreground);
    let active_line_style = Style::default().fg(state.theme.foreground);

    let lines: Vec<Line> = visual_lines
        .iter()
        .enumerate()
        .skip(state.desc_scroll_offset)
        .take(visible_lines)
        .map(|(vis_row, vline)| {
            let sub_text = &state.desc_buffer[vline.buf_row][vline.byte_start..vline.byte_end];

            if vis_row == cursor_visual_row {
                let cursor_col =
                    state.desc_cursor_col.min(state.desc_buffer[vline.buf_row].len());
                let rel_cursor = cursor_col.saturating_sub(vline.byte_start);
                let before_cursor = &sub_text[..rel_cursor.min(sub_text.len())];
                let after_cursor = &sub_text[rel_cursor.min(sub_text.len())..];

                let mut spans: Vec<Span<'static>> =
                    vec![Span::styled(before_cursor.to_string(), active_line_style)];

                if after_cursor.is_empty() {
                    spans.push(Span::styled("\u{2588}", block_cursor_style));
                } else {
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
                Line::from(Span::styled(sub_text.to_string(), text_style))
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner_area);
}
